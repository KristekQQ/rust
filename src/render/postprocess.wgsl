struct Uniforms {
    inverse_view_projection: mat4x4<f32>,
    view_projection: mat4x4<f32>,
    camera: vec4<f32>,
    light_position: vec4<f32>,
    light_color: vec4<f32>,
    params: vec4<f32>,
    mask_params: vec4<f32>,
    shadow_view_projection: mat4x4<f32>,
    // shadow-enabled, extinction density, max ray range, scattering exposure
    volume_params: vec4<f32>,
    environment_center: vec4<f32>,
    environment_half_extents: vec4<f32>,
    sky_cube_params: vec4<f32>,
    sky_cube_center: vec4<f32>,
    sky_cube_half_extents: vec4<f32>,
    reflection_trace: vec4<f32>,
    receiver_bounds_min: vec4<f32>,
    receiver_bounds_max: vec4<f32>,
};
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(3) var<uniform> uniforms: Uniforms;
@group(0) @binding(4) var depth_sampler: sampler;
@group(0) @binding(6) var reflection_mask: texture_2d<f32>;
@group(0) @binding(7) var environment_texture: texture_cube<f32>;
@group(0) @binding(8) var sky_cube_color: texture_cube<f32>;
@group(0) @binding(9) var sky_cube_emission: texture_cube<f32>;
@group(1) @binding(0) var volume_static_shadow: texture_depth_2d;
@group(1) @binding(1) var volume_dynamic_shadow: texture_depth_2d;
@group(1) @binding(2) var volume_shadow_sampler: sampler_comparison;
@group(2) @binding(0) var ssr_trace: texture_2d<f32>;
struct Output { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Output {
    let positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let p = positions[index];
    var output: Output;
    output.position = vec4(p, 0.0, 1.0);
    output.uv = vec2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return output;
}
fn world_at(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let p = uniforms.inverse_view_projection * vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    return p.xyz / p.w;
}
fn inside(uv: vec2<f32>) -> bool { return all(uv > vec2(0.0)) && all(uv < vec2(1.0)); }
fn scene_at(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(scene_texture, scene_sampler, uv, 0.0); }
fn reflection_mask_at(position: vec3<f32>) -> f32 {
    if uniforms.camera.w < 0.5 { return 1.0; }
    let mask_uv = (position.xz - uniforms.mask_params.xy) / uniforms.mask_params.zw + vec2(0.5);
    if !all(mask_uv >= vec2(0.0)) || !all(mask_uv <= vec2(1.0)) { return 0.0; }
    return textureSampleLevel(reflection_mask, scene_sampler, mask_uv, 0.0).r;
}
// Cubemap faces use WebGPU's +X,-X,+Y,-Y,+Z,-Z layer order.
fn sky_cube_at(direction: vec3<f32>) -> vec3<f32> {
    return textureSampleLevel(sky_cube_color, scene_sampler, direction, 0.0).rgb
        + textureSampleLevel(sky_cube_emission, scene_sampler, direction, 0.0).rgb * uniforms.sky_cube_params.y;
}
// Capture-relative exit point makes a finite cubemap respond to camera translation.
fn box_projected_direction(position: vec3<f32>, direction: vec3<f32>, center: vec3<f32>, half_extents: vec3<f32>) -> vec3<f32> {
    let local = position - center;
    var sample_direction = direction;
    if all(abs(local) <= half_extents) {
        // Intersect the forward ray with the volume's exit plane. Protect parallel
        // components before division, including exact zero on a box boundary.
        let safe_direction = select(vec3(-1.0), vec3(1.0), direction >= vec3(0.0)) * max(abs(direction), vec3(0.00001));
        let exit_planes = select(-half_extents, half_extents, direction >= vec3(0.0));
        let distances = select(vec3(1e20), (exit_planes - local) / safe_direction, abs(direction) > vec3(0.00001));
        let exit_distance = min(min(distances.x, distances.y), distances.z);
        let projected = local + direction * max(exit_distance, 0.0);
        if dot(projected, projected) > 0.000001 { sample_direction = projected; }
    }
    return sample_direction;
}
fn sample_world_cube(position: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    var sample_direction = direction;
    if uniforms.sky_cube_center.w > 0.5 {
        sample_direction = box_projected_direction(position, direction, uniforms.sky_cube_center.xyz, uniforms.sky_cube_half_extents.xyz);
    }
    return sky_cube_at(sample_direction);
}
@fragment fn fs_environment(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    var color = source.rgb;
    if depth_at(input.uv) >= 0.999999 {
        color = sample_world_cube(uniforms.camera.xyz, world_at(input.uv, 0.999999) - uniforms.camera.xyz);
    }
    return vec4(color, select(source.a, 1.0, uniforms.params.y > 0.5));
}
fn environment_at(position: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    if uniforms.sky_cube_params.x > 0.5 { return sample_world_cube(position, direction); }
    let sample_direction = box_projected_direction(position, direction, uniforms.environment_center.xyz, uniforms.environment_half_extents.xyz);
    return textureSampleLevel(environment_texture, scene_sampler, sample_direction, 0.0).rgb;
}
fn inside_receiver(position: vec3<f32>) -> bool {
    return uniforms.receiver_bounds_min.w < 0.5 || (all(position >= uniforms.receiver_bounds_min.xyz) && all(position <= uniforms.receiver_bounds_max.xyz));
}
fn reflection_sample(uv: vec2<f32>, source: vec4<f32>) -> vec4<f32> {
    if source.a <= 0.001 { return vec4(0.0); }
    let depth = depth_at(uv);
    if depth >= 0.999999 { return vec4(0.0); }
    let position = world_at(uv, depth);
    if !inside_receiver(position) { return vec4(0.0); }
    // Choose the nearer depth neighbour on each axis to reduce silhouette normals.
    let pixel = 1.0 / uniforms.params.zw;
    let left = world_at(uv - vec2(pixel.x, 0.0), depth_at(uv - vec2(pixel.x, 0.0)));
    let right = world_at(uv + vec2(pixel.x, 0.0), depth_at(uv + vec2(pixel.x, 0.0)));
    let up = world_at(uv - vec2(0.0, pixel.y), depth_at(uv - vec2(0.0, pixel.y)));
    let down = world_at(uv + vec2(0.0, pixel.y), depth_at(uv + vec2(0.0, pixel.y)));
    let dx = select(position - left, right - position, distance(right, position) < distance(left, position));
    let dy = select(position - up, down - position, distance(down, position) < distance(up, position));
    let raw_normal = cross(dx, dy);
    if dot(raw_normal, raw_normal) < 0.000000000001 { return vec4(0.0); }
    let incident = normalize(position - uniforms.camera.xyz);
    var normal = normalize(raw_normal);
    if dot(normal, incident) > 0.0 { normal = -normal; }
    let direction = reflect(incident, normal);
    let fresnel = 0.2 + 0.8 * pow(1.0 - max(dot(-incident, normal), 0.0), 5.0);
    let weight = clamp(source.a * reflection_mask_at(position) * uniforms.params.x * fresnel, 0.0, 1.0);
    var fallback = vec3(0.0);
    var fallback_weight = 0.0;
    if uniforms.environment_center.w > 0.5 || uniforms.sky_cube_params.x > 0.5 {
        fallback = environment_at(position, direction);
        fallback_weight = weight;
    }
    let camera_distance = distance(position, uniforms.camera.xyz);
    let step_length = clamp(camera_distance * 0.025, 0.04, 0.5);
    let thickness = clamp(camera_distance * 0.012, 0.06, 0.4);
    let origin = position + normal * max(0.025, thickness * 0.5);
    var previous_t = 0.0;
    var previous_delta = -thickness;
    for (var step = 1u; step <= 64u; step += 1u) {
        if step > u32(uniforms.reflection_trace.x) { break; }
        // Spend XR's smaller budget near the receiver, growing the stride
        // toward the same 64-step far distance. Desktop spacing is unchanged.
        let near_step = f32(step) - 1.0;
        let trace_step = f32(step) + near_step * near_step * uniforms.reflection_trace.z;
        let t = trace_step * step_length;
        let ray = origin + direction * t;
        let clip = uniforms.view_projection * vec4(ray, 1.0);
        if clip.w <= 0.0 { break; }
        let ndc = clip.xyz / clip.w;
        let sample_uv = vec2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if !inside(sample_uv) || ndc.z < 0.0 || ndc.z > 1.0 { break; }
        let sampled_depth = depth_at(sample_uv);
        if sampled_depth >= 0.999999 { previous_t = t; previous_delta = -thickness; continue; }
        let surface = world_at(sample_uv, sampled_depth);
        // Signed separation along the camera ray detects front-to-back crossings.
        let eye_direction = normalize(ray - uniforms.camera.xyz);
        let delta = dot(ray - surface, eye_direction);
        if step > 2u && previous_delta < 0.0 && delta >= 0.0 {
            var low = previous_t;
            var high = t;
            var hit_uv = sample_uv;
            var hit_delta = delta;
            // Refine the crossing; reject depth discontinuities thicker than geometry budget.
            for (var refine = 0u; refine < 5u; refine += 1u) {
                if refine >= u32(uniforms.reflection_trace.y) { break; }
                let mid = (low + high) * 0.5;
                let mid_ray = origin + direction * mid;
                let mid_clip = uniforms.view_projection * vec4(mid_ray, 1.0);
                let mid_ndc = mid_clip.xyz / mid_clip.w;
                let mid_uv = vec2(mid_ndc.x * 0.5 + 0.5, 0.5 - mid_ndc.y * 0.5);
                let mid_surface = world_at(mid_uv, depth_at(mid_uv));
                let mid_delta = dot(mid_ray - mid_surface, normalize(mid_ray - uniforms.camera.xyz));
                if mid_delta >= 0.0 { high = mid; hit_uv = mid_uv; hit_delta = mid_delta; } else { low = mid; }
            }
            let separation = length((hit_uv - uv) * uniforms.params.zw);
            if hit_delta < thickness && separation > 3.0 {
                let border = min(min(hit_uv.x, 1.0 - hit_uv.x), min(hit_uv.y, 1.0 - hit_uv.y));
                let edge_fade = smoothstep(0.0, 0.08, border);
                let range_fade = 1.0 - smoothstep(40.0, 64.0, trace_step);
                // Confidence fades into the environment instead of losing the reflection.
                let confidence = edge_fade * range_fade;
                let combined_weight = mix(fallback_weight, weight, confidence);
                let radiance = fallback * fallback_weight * (1.0 - confidence) + scene_at(hit_uv).rgb * weight * confidence;
                return vec4(radiance / max(combined_weight, 0.000001), combined_weight);
            }
        }
        previous_t = t;
        previous_delta = delta;
    }
    return vec4(fallback, fallback_weight);
}
// Single-scattering eye-ray integration with shadow-map visibility, inspired by
// Toth/Umenhoffer (Eurographics 2009). No radial screen emitter or screen-light gating.
fn volume_visibility(position: vec3<f32>) -> f32 {
    let clip = uniforms.shadow_view_projection * vec4(position, 1.0);
    if clip.w <= 0.0 { return 0.0; }
    let ndc = clip.xyz / clip.w;
    let uv = vec2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    // Existing point-light shadow maps cover a perspective cone, not all 360 degrees.
    if !inside(uv) || ndc.z <= 0.0 || ndc.z >= 1.0 { return 0.0; }
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let cone_fade = smoothstep(0.0, 0.04, edge);
    // With shadows disabled, retain explicitly unshadowed scattering in this cone.
    if uniforms.volume_params.x < 0.5 { return cone_fade; }
    // A small depth bias avoids self-shadowing without leaking light through stones.
    let reference_depth = ndc.z - 0.00008;
    let static_visibility = textureSampleCompareLevel(volume_static_shadow, volume_shadow_sampler, uv, reference_depth);
    let dynamic_visibility = textureSampleCompareLevel(volume_dynamic_shadow, volume_shadow_sampler, uv, reference_depth);
    return min(static_visibility, dynamic_visibility) * cone_fade;
}
fn god_rays(uv: vec2<f32>, source: vec3<f32>) -> vec3<f32> {
    if max(max(uniforms.light_color.r, uniforms.light_color.g), uniforms.light_color.b) <= 0.0 { return source; }
    let depth = depth_at(uv);
    let endpoint = world_at(uv, min(depth, 0.999999));
    let camera_to_surface = endpoint - uniforms.camera.xyz;
    let surface_distance = length(camera_to_surface);
    if surface_distance < 0.0001 { return source; }
    let direction = camera_to_surface / surface_distance;
    var ray_length = min(surface_distance, uniforms.volume_params.z);
    if depth >= 0.999999 { ray_length = uniforms.volume_params.z; }
    let step_length = ray_length / 32.0;
    let density = uniforms.volume_params.y * uniforms.params.x;
    let step_transmission = exp(-density * step_length);
    // Deterministic per-pixel jitter removes aligned bands without temporal flicker.
    let pixel = floor(uv * uniforms.params.zw);
    let jitter = fract(52.9829189 * fract(dot(pixel, vec2(0.06711056, 0.00583715))));
    var transmission = 1.0;
    var radiance = vec3(0.0);
    let g = 0.45;
    for (var step = 0u; step < 32u; step += 1u) {
        let t = (f32(step) + jitter) * step_length;
        let position = uniforms.camera.xyz + direction * t;
        let to_light = uniforms.light_position.xyz - position;
        let distance_squared = max(dot(to_light, to_light), 0.01);
        let light_direction = to_light * inverseSqrt(distance_squared);
        let cosine = clamp(dot(direction, light_direction), -1.0, 1.0);
        // Henyey-Greenstein phase with moderate forward scattering.
        let phase = (1.0 - g * g) / (12.5663706 * pow(max(1.0 + g * g - 2.0 * g * cosine, 0.001), 1.5));
        let attenuation = 5.0 / (1.0 + 0.2 * distance_squared);
        let visibility = volume_visibility(position);
        radiance += max(uniforms.light_color.rgb, vec3(0.0)) * (transmission * (1.0 - step_transmission) * phase * attenuation * visibility);
        transmission *= step_transmission;
    }
    // Keep original surface lighting; the integrated medium adds bounded light.
    // This additive approximation omits surface fog extinction and multiple scattering.
    let scattering = radiance * uniforms.volume_params.w;
    return source + scattering / (vec3(1.0) + scattering);
}
// Intermediate alpha remains material metadata; final output alpha is opaque. Keep SSR before color-only effects in EFFECTS by default.
fn wet_surface(uv: vec2<f32>, source: vec4<f32>) -> vec3<f32> {
    if uniforms.camera.w < 0.5 || source.a <= 0.001 { return source.rgb; }
    let depth = depth_at(uv);
    if uniforms.camera.w > 0.5 && source.a > 0.001 && depth < 0.999999 {
        return source.rgb * mix(1.0, 0.58, reflection_mask_at(world_at(uv, depth)));
    }
    return source.rgb;
}
@fragment fn fs_ssr(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    let reflected = reflection_sample(input.uv, source);
    return vec4(mix(wet_surface(input.uv, source), reflected.rgb, reflected.a), select(source.a, 1.0, uniforms.params.y > 0.5));
}
@fragment fn fs_ssr_trace(input: Output) -> @location(0) vec4<f32> {
    return reflection_sample(input.uv, scene_at(input.uv));
}
@fragment fn fs_ssr_composite(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    if source.a <= 0.001 { return vec4(source.rgb, select(source.a, 1.0, uniforms.params.y > 0.5)); }
    let surface = wet_surface(input.uv, source);
    let depth = depth_at(input.uv);
    var weighted = vec3(0.0);
    var confidence = 0.0;
    var total = 0.0;
    if source.a > 0.001 && depth < 0.999999 {
        let position = world_at(input.uv, depth);
        if !inside_receiver(position) { return vec4(surface, select(source.a, 1.0, uniforms.params.y > 0.5)); }
        let size = vec2<f32>(textureDimensions(ssr_trace));
        let grid = input.uv * size - vec2(0.5);
        let base = floor(grid);
        let fraction = fract(grid);
        // Reconstruct each trace receiver from ORIGINAL depth. Reject edges,
        // material boundaries and dry/grout pixels before interpolating radiance.
        let footprint = max(distance(world_at(input.uv + vec2(1.0 / uniforms.params.z, 0.0), depth), position), distance(world_at(input.uv + vec2(0.0, 1.0 / uniforms.params.w), depth), position));
        let eye_direction = normalize(position - uniforms.camera.xyz);
        for (var y = 0; y < 2; y += 1) {
            for (var x = 0; x < 2; x += 1) {
                let tap_pixel = clamp(base + vec2(f32(x), f32(y)), vec2(0.0), size - vec2(1.0));
                let tap_uv = (tap_pixel + vec2(0.5)) / size;
                let tap_depth = depth_at(tap_uv);
                let tap_source = scene_at(tap_uv);
                let tap_position = world_at(tap_uv, tap_depth);
                if !inside_receiver(tap_position) { continue; }
                let separation = abs(dot(tap_position - position, eye_direction));
                if tap_depth >= 0.999999 || tap_source.a <= 0.001 || abs(tap_source.a - source.a) > 0.08 || separation > max(0.025, footprint * 3.0) { continue; }
                let tap_mask = reflection_mask_at(tap_position);
                let mask = reflection_mask_at(position);
                if tap_mask <= 0.001 || mask <= 0.001 || abs(tap_mask - mask) > 0.2 { continue; }
                let bilinear = select(1.0 - fraction.x, fraction.x, x == 1) * select(1.0 - fraction.y, fraction.y, y == 1);
                let sample = textureSampleLevel(ssr_trace, depth_sampler, tap_uv, 0.0);
                weighted += sample.rgb * sample.a * bilinear;
                confidence += sample.a * bilinear;
                total += bilinear;
            }
        }
    }
    let weight = confidence / max(total, 0.000001);
    let color = mix(surface, weighted / max(confidence, 0.000001), weight);
    return vec4(color, select(source.a, 1.0, uniforms.params.y > 0.5));
}
@fragment fn fs_god_rays(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    return vec4(god_rays(input.uv, source.rgb), select(source.a, 1.0, uniforms.params.y > 0.5));
}
fn bloom_bright(uv: vec2<f32>) -> vec3<f32> {
    let color = scene_at(uv).rgb;
    let luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
    return color * smoothstep(0.65, 0.95, luminance);
}
// Small LDR highlight blur, deliberately bounded; this is not an HDR bloom pyramid.
@fragment fn fs_bloom(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    let pixel = 1.0 / uniforms.params.zw;
    var glow = bloom_bright(input.uv) * 0.2;
    for (var axis = 0u; axis < 4u; axis += 1u) {
        let offsets = array<vec2<f32>, 4>(vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0));
        glow += bloom_bright(input.uv + offsets[axis] * pixel * 2.0) * 0.1;
        glow += bloom_bright(input.uv + offsets[axis] * pixel * 4.0) * 0.05;
        let diagonal = vec2(offsets[axis].x - offsets[axis].y, offsets[axis].x + offsets[axis].y);
        glow += bloom_bright(input.uv + diagonal * pixel * 2.0) * 0.05;
    }
    return vec4(source.rgb + glow * (uniforms.params.x * 0.22), select(source.a, 1.0, uniforms.params.y > 0.5));
}
