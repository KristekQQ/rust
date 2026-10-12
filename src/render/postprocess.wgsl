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
};
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var scene_depth: texture_2d<f32>;
@group(0) @binding(3) var<uniform> uniforms: Uniforms;
@group(0) @binding(4) var depth_sampler: sampler;
@group(0) @binding(6) var reflection_mask: texture_2d<f32>;
@group(1) @binding(0) var volume_static_shadow: texture_depth_2d;
@group(1) @binding(1) var volume_dynamic_shadow: texture_depth_2d;
@group(1) @binding(2) var volume_shadow_sampler: sampler_comparison;
struct Output { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Output {
    let positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let p = positions[index];
    var output: Output;
    output.position = vec4(p, 0.0, 1.0);
    output.uv = vec2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return output;
}
fn depth_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(uniforms.params.zw);
    let pixel = clamp(vec2<i32>(uv * uniforms.params.zw), vec2<i32>(0), size - vec2<i32>(1));
    // RGB base-255 depth packing avoids depth-sampler LOD limitations in GLSL ES300.
    // An auxiliary attachment cleared to white represents untouched far depth.
    let packed = textureSampleLevel(scene_depth, depth_sampler, (vec2<f32>(pixel) + vec2(0.5)) / uniforms.params.zw, 0.0).rgb;
    return clamp(dot(packed, vec3(1.0, 1.0 / 255.0, 1.0 / 65025.0)), 0.0, 1.0);
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
fn reflected_color(uv: vec2<f32>, source: vec4<f32>) -> vec3<f32> {
    let depth = depth_at(uv);
    if depth >= 0.999999 || source.a <= 0.001 { return source.rgb; }
    let position = world_at(uv, depth);
    // Choose the nearer depth neighbour on each axis to reduce silhouette normals.
    let pixel = 1.0 / uniforms.params.zw;
    let left = world_at(uv - vec2(pixel.x, 0.0), depth_at(uv - vec2(pixel.x, 0.0)));
    let right = world_at(uv + vec2(pixel.x, 0.0), depth_at(uv + vec2(pixel.x, 0.0)));
    let up = world_at(uv - vec2(0.0, pixel.y), depth_at(uv - vec2(0.0, pixel.y)));
    let down = world_at(uv + vec2(0.0, pixel.y), depth_at(uv + vec2(0.0, pixel.y)));
    let dx = select(position - left, right - position, distance(right, position) < distance(left, position));
    let dy = select(position - up, down - position, distance(down, position) < distance(up, position));
    let raw_normal = cross(dx, dy);
    if dot(raw_normal, raw_normal) < 0.000000000001 { return source.rgb; }
    let incident = normalize(position - uniforms.camera.xyz);
    var normal = normalize(raw_normal);
    if dot(normal, incident) > 0.0 { normal = -normal; }
    let direction = reflect(incident, normal);
    let camera_distance = distance(position, uniforms.camera.xyz);
    let step_length = clamp(camera_distance * 0.025, 0.04, 0.5);
    let thickness = clamp(camera_distance * 0.012, 0.06, 0.4);
    let origin = position + normal * max(0.025, thickness * 0.5);
    var previous_t = 0.0;
    var previous_delta = -thickness;
    for (var step = 1u; step <= 64u; step += 1u) {
        let t = f32(step) * step_length;
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
                let range_fade = 1.0 - smoothstep(40.0, 64.0, f32(step));
                let fresnel = 0.2 + 0.8 * pow(1.0 - max(dot(-incident, normal), 0.0), 5.0);
                let weight = clamp(source.a * reflection_mask_at(position) * uniforms.params.x * edge_fade * range_fade * fresnel, 0.0, 1.0);
                return mix(source.rgb, scene_at(hit_uv).rgb, weight);
            }
        }
        previous_t = t;
        previous_delta = delta;
    }
    return source.rgb;
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
@fragment fn fs_ssr(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    var surface = source;
    let depth = depth_at(input.uv);
    if uniforms.camera.w > 0.5 && source.a > 0.001 && depth < 0.999999 {
        let wetness = reflection_mask_at(world_at(input.uv, depth));
        surface = vec4(source.rgb * mix(1.0, 0.58, wetness), source.a);
    }
    return vec4(reflected_color(input.uv, surface), select(source.a, 1.0, uniforms.params.y > 0.5));
}
@fragment fn fs_god_rays(input: Output) -> @location(0) vec4<f32> {
    let source = scene_at(input.uv);
    return vec4(god_rays(input.uv, source.rgb), select(source.a, 1.0, uniforms.params.y > 0.5));
}
