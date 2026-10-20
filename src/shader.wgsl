struct Light {
    position: vec3<f32>,
    _pad_p: f32,
    color: vec3<f32>,
    _pad_c: f32,
};

const LIGHT_COUNT: u32 = 4u;

struct SceneUniforms {
    view_projection: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    lights: array<Light, LIGHT_COUNT>,
    guide_plane: vec4<f32>,
    guide_origin: vec4<f32>,
    guide_axis: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;
@group(0) @binding(1) var variant_colors: texture_2d<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) @interpolate(flat) material: vec4<f32>,
    @location(4) @interpolate(flat) emission: f32,
    @location(5) @interpolate(flat) color_cutout: f32,
    @location(6) @interpolate(flat) transparency: vec2<f32>,
    @location(7) @interpolate(flat) opacity_controls: vec2<f32>,
    @location(8) @interpolate(flat) facet_key: vec4<f32>,
    @location(9) numeric_axis: vec3<f32>,
};

struct InstanceInput {
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(7) normal0: vec4<f32>,
    @location(8) normal1: vec4<f32>,
    @location(9) normal2: vec4<f32>,
    @location(10) normal3: vec4<f32>,
    @location(11) material: vec4<f32>,
    @location(12) emission: vec4<f32>,
    @location(13) surface_params: vec4<f32>,
};
@vertex
fn vs_main(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let model = mat4x4<f32>(instance.model0, instance.model1, instance.model2, instance.model3);
    let normal_matrix = mat4x4<f32>(instance.normal0, instance.normal1, instance.normal2, instance.normal3);
    out.pos = scene.view_projection * model * vec4<f32>(input.position, 1.0);
    var color = input.color;
    if (instance.surface_params.w > 0.0 && input.color.r < 0.0) {
        let offset = u32(instance.surface_params.z) * u32(instance.surface_params.w) + u32(-input.color.r - 1.0);
        let width = textureDimensions(variant_colors).x;
        color = textureLoad(variant_colors, vec2<i32>(i32(offset % width), i32(offset / width)), 0).rgb;
    }
    out.numeric_axis = vec3<f32>(0.0);
    if (instance.material.w < -1.5) {
        out.numeric_axis = (model*vec4<f32>(input.color*2.0-vec3<f32>(1.0),0.0)).xyz;
        // Numeric RGB carries an internal axis, never its visible palette.
        color = vec3<f32>(0.015,0.32,1.0);
    }
    out.color = color;
    out.material = instance.material;
    out.emission = instance.emission.x;
    out.color_cutout = instance.emission.y;
    out.transparency = instance.emission.zw;
    out.opacity_controls = instance.surface_params.xy;
    out.facet_key = vec4(input.normal, max(max(color.r, color.g), color.b));
    out.world_pos = (model * vec4<f32>(input.position, 1.0)).xyz;
    // Rust supplies the inverse transpose. Normalize after interpolation so
    // non-uniformly scaled smooth surfaces retain the correct normal field.
    out.world_normal = (normal_matrix * vec4<f32>(input.normal, 0.0)).xyz;
    return out;
}

@vertex
fn vs_lines(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.pos = scene.view_projection * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    out.numeric_axis = vec3<f32>(0.0);
    out.material = vec4(0.0);
    out.emission = 0.0;
    out.color_cutout = 0.0;
    out.transparency = vec2(1.0, 8.0);
    out.opacity_controls = vec2(0.0, -1.0);
    out.facet_key = vec4(input.normal, max(max(input.color.r, input.color.g), input.color.b));
    out.world_pos = input.position;
    out.world_normal = input.normal;
    return out;
}

struct EffectsUniforms {
    shadow_matrices:array<mat4x4<f32>,4>, reflection_matrix:mat4x4<f32>,
    clip_plane:vec4<f32>, params:vec4<f32>,
};
@group(1) @binding(0) var<uniform> effects:EffectsUniforms;
@group(1) @binding(1) var static_shadow:texture_depth_2d;
@group(1) @binding(2) var dynamic_shadow:texture_depth_2d;
@group(1) @binding(3) var shadow_sampler:sampler_comparison;
@group(1) @binding(4) var reflection_texture:texture_2d<f32>;
@group(1) @binding(5) var reflection_sampler:sampler;
@group(1) @binding(6) var static_shadow_1:texture_depth_2d;
@group(1) @binding(7) var dynamic_shadow_1:texture_depth_2d;
@group(1) @binding(8) var static_shadow_2:texture_depth_2d;
@group(1) @binding(9) var dynamic_shadow_2:texture_depth_2d;
@group(1) @binding(10) var static_shadow_3:texture_depth_2d;
@group(1) @binding(11) var dynamic_shadow_3:texture_depth_2d;
fn compare_shadow(index:u32,uv:vec2<f32>,depth:f32)->f32 {
    switch index {
        case 0u: { return min(textureSampleCompareLevel(static_shadow,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow,shadow_sampler,uv,depth)); }
        case 1u: { return min(textureSampleCompareLevel(static_shadow_1,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow_1,shadow_sampler,uv,depth)); }
        case 2u: { return min(textureSampleCompareLevel(static_shadow_2,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow_2,shadow_sampler,uv,depth)); }
        case 3u: { return min(textureSampleCompareLevel(static_shadow_3,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow_3,shadow_sampler,uv,depth)); }
        default: { return 1.0; }
    }
}
fn shadow_visibility(index:u32,world:vec3<f32>,normal:vec3<f32>,light_direction:vec3<f32>)->f32 {
    let clip=effects.shadow_matrices[index]*vec4(world,1.0);
    let ndc=clip.xyz/clip.w;
    if (clip.w<=0.0 || ndc.z<=0.0 || ndc.z>=1.0 || any(abs(ndc.xy)>vec2(1.0))) {return 1.0;}
    let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);
    let bias=effects.params.w*(1.0+2.0*(1.0-max(dot(normal,light_direction),0.0)));
    let depth=ndc.z-bias;
    let texel=1.0/vec2<f32>(textureDimensions(static_shadow));
    var visibility=0.0;
    // Combine static and dynamic visibility per texel, then apply 3x3 PCF.
    for(var y=-1;y<=1;y++) {for(var x=-1;x<=1;x++) {
        let sample_uv=uv+vec2<f32>(f32(x),f32(y))*texel;
        visibility+=compare_shadow(index,sample_uv,depth);
    }}
    return visibility/9.0;
}

fn surface_translucent(input: VertexOutput) -> bool {
    return input.transparency.x < 1.0 && max(max(input.color.r, input.color.g), input.color.b) < input.transparency.y;
}
fn opaque_surface(input: VertexOutput) -> vec4<f32> {
    if (surface_translucent(input)) { discard; }
    return shade_surface(input);
}
fn shade_surface(input: VertexOutput) -> vec4<f32> {
    if (max(max(input.color.r, input.color.g), input.color.b) < input.color_cutout) { discard; }
    if (effects.params.z>0.5 && dot(effects.clip_plane,vec4(input.world_pos,1.0)) < -0.001) { discard; }
    // Numeric tubes radiate continuously across their smooth surface. Build the
    // hot core per fragment from view angle, not baked facet colors or lights.
    if (input.material.w < -1.5) {
        let normal = normalize(input.world_normal);
        let view = normalize(scene.camera_pos-input.world_pos);
        let axis = normalize(input.numeric_axis);
        let across = view-axis*dot(view,axis);
        // A neon tube emits along its whole axis. Project the view into its
        // cross section so oblique/horizontal strokes cannot lose their core.
        let fallback_basis = select(vec3<f32>(0.0,0.0,1.0),vec3<f32>(0.0,1.0,0.0),abs(axis.z)>0.9);
        var cross_view = normalize(cross(axis,fallback_basis));
        if dot(across,across)>0.000001 { cross_view = normalize(across); }
        let facing = clamp(dot(normal,cross_view),0.0,1.0);
        let drive = clamp(log2(1.0+max(input.emission,0.0))/3.0,0.0,1.25);
        let tube = vec3<f32>(0.0135,0.165,0.975)*(0.12+0.88*sqrt(facing));
        let core = vec3<f32>(0.86,0.96,1.0)*4.5*pow(facing,32.0);
        let radiance = (tube+core)*drive;
        // Exponential exposure rolls HDR energy smoothly into display range;
        // broad blue emission stays saturated while only the narrow core pales.
        return vec4<f32>(vec3<f32>(1.0)-exp(-radiance),0.0);
    }
    // World-space text keeps its palette and normal depth/clip behavior.
    // Alpha is postprocessing metadata; zero excludes surface lighting effects.
    if (input.material.w < -0.5) { return vec4<f32>(input.color, 0.0); }
    let normal = normalize(input.world_normal);
    let view_dir = normalize(scene.camera_pos - input.world_pos);
    var result = input.color * 0.1; // ambient

    for (var i: u32 = 0u; i < LIGHT_COUNT; i = i + 1u) {
        let light = scene.lights[i];
        if (all(light.color == vec3<f32>(0.0))) {
            continue;
        }
        let light_delta = light.position - input.world_pos;
        let l_dir = light_delta / max(length(light_delta), 0.00001);
        let diff = max(dot(normal, l_dir), 0.0);
        // Blinn-Phong: an unlit/back-facing surface cannot reflect this light.
        var spec = 0.0;
        if (diff > 0.0 && dot(normal, view_dir) > 0.0) {
            let half_vector = l_dir + view_dir;
            let halfway = half_vector / max(length(half_vector), 0.00001);
            spec = pow(max(dot(normal, halfway), 0.0), 32.0);
        }
        var visibility=1.0;
        if(i<u32(effects.params.x) && input.material.y>0.5) {
            visibility=shadow_visibility(i,input.world_pos,normal,l_dir);
        }
        result += (diff * input.color + spec) * light.color * visibility;
    }

    result += input.color * input.emission;

    if(effects.params.y>0.5 && input.material.z>0.5 && input.material.x>0.0) {
        let reflected=effects.reflection_matrix*vec4(input.world_pos,1.0);
        let uv=reflected.xy/reflected.w*vec2(0.5,-0.5)+vec2(0.5);
        if(reflected.w>0.0 && all(uv>=vec2(0.0)) && all(uv<=vec2(1.0))) {
            result=mix(result,textureSampleLevel(reflection_texture,reflection_sampler,uv,0.0).rgb,input.material.x);
        }
    }
    // Intersect the same finite blade sweep with actual visible cuttable surfaces.
    // Regular depth testing keeps hidden fruit hidden; no duplicate contour mesh.
    let along = dot(input.world_pos - scene.guide_origin.xyz, scene.guide_axis.xyz);
    let distance = abs(dot(vec4(input.world_pos, 1.0), scene.guide_plane));
    if (input.material.w > 0.5 && scene.guide_axis.w > 0.0
        && abs(along) <= scene.guide_axis.w
        && input.world_pos.y >= scene.guide_origin.w
        && input.world_pos.y <= scene.guide_origin.y) {
        let band = 1.0 - smoothstep(0.022, 0.042, distance);
        result = mix(result, vec3<f32>(0.08, 1.0, 0.025), band);
    }
    return vec4<f32>(result, input.material.x);
}

@fragment
fn fs_color(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}

// Alpha is material metadata only in the offscreen postprocessing path.
@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(opaque_surface(input).rgb, 1.0);
}
@fragment fn fs_material_mask(input: VertexOutput) -> @location(0) vec4<f32> {
    return opaque_surface(input);
}
struct PostSurface {
    @location(0) color: vec4<f32>,
    @location(1) depth: vec4<f32>,
};
@fragment fn fs_postprocess(input: VertexOutput) -> PostSurface {
    var output: PostSurface;
    output.color = opaque_surface(input);
    var encoded = fract(min(input.pos.z, 0.9999999) * vec3(1.0, 255.0, 65025.0));
    encoded -= encoded.yzz * vec3(1.0 / 255.0, 1.0 / 255.0, 0.0);
    output.depth = vec4(encoded, 1.0);
    return output;
}

// Straight alpha blends linear lighting into the final target (sRGB encoding is hardware-owned).
@fragment fn fs_translucent(input: VertexOutput, @builtin(front_facing) _front: bool) -> @location(0) vec4<f32> {
    if (!surface_translucent(input) || (input.transparency.x <= 0.0 && input.opacity_controls.x <= 0.0)) { discard; }
    var surface = input;
    // LH projection reverses winding; light the inside of the rear surface too.
    surface.world_normal = select(-input.world_normal, input.world_normal, dot(input.world_normal, scene.camera_pos - input.world_pos) >= 0.0);
    // Local face orientation distinguishes equal colors. Baked facet brightness
    // supplies the authored random seed; both triangles of a quad share it.
    // Quantization removes tiny normal differences from coplanar triangulation.
    let authored = round(input.facet_key * vec4(4096.0, 4096.0, 4096.0, 65536.0)) / vec4(4096.0, 4096.0, 4096.0, 65536.0);
    let seed = fract(authored.xyz * vec3(0.1031, 0.1030, 0.0973) + vec3(authored.w * 17.37) + vec3(0.37, 0.61, 0.83));
    let mixed = seed + dot(seed, seed.yzx + vec3(33.33));
    let facet_hash = fract((mixed.x + mixed.y) * mixed.z);
    var weight = facet_hash;
    if (input.opacity_controls.y == 0.0) {
        weight = 0.0;
    } else if (input.opacity_controls.y == 1.0) {
        weight = 1.0;
    } else if (input.opacity_controls.y > 0.0) {
        let threshold = 1.0 - input.opacity_controls.y;
        weight = smoothstep(threshold - 0.06, threshold + 0.06, facet_hash);
    }
    let alpha = clamp(input.transparency.x + (1.0 - input.transparency.x) * input.opacity_controls.x * weight, 0.0, 1.0);
    return vec4(shade_surface(surface).rgb, alpha);
}
