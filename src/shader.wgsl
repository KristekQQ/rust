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
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;

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
};
@vertex
fn vs_main(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let model = mat4x4<f32>(instance.model0, instance.model1, instance.model2, instance.model3);
    let normal_matrix = mat4x4<f32>(instance.normal0, instance.normal1, instance.normal2, instance.normal3);
    out.pos = scene.view_projection * model * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    out.material = instance.material;
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
    out.material = vec4(0.0);
    out.world_pos = input.position;
    out.world_normal = input.normal;
    return out;
}

struct EffectsUniforms {
    shadow_matrix:mat4x4<f32>, reflection_matrix:mat4x4<f32>,
    clip_plane:vec4<f32>, params:vec4<f32>,
};
@group(1) @binding(0) var<uniform> effects:EffectsUniforms;
@group(1) @binding(1) var static_shadow:texture_depth_2d;
@group(1) @binding(2) var dynamic_shadow:texture_depth_2d;
@group(1) @binding(3) var shadow_sampler:sampler_comparison;
@group(1) @binding(4) var reflection_texture:texture_2d<f32>;
@group(1) @binding(5) var reflection_sampler:sampler;
fn shadow_visibility(world:vec3<f32>,normal:vec3<f32>,light_direction:vec3<f32>)->f32 {
    let clip=effects.shadow_matrix*vec4(world,1.0);
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
        visibility+=min(textureSampleCompareLevel(static_shadow,shadow_sampler,sample_uv,depth),
            textureSampleCompareLevel(dynamic_shadow,shadow_sampler,sample_uv,depth));
    }}
    return visibility/9.0;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if (effects.params.z>0.5 && dot(effects.clip_plane,vec4(input.world_pos,1.0)) < -0.001) { discard; }
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
        if(i==0u && effects.params.x>0.5 && input.material.y>0.5) {
            visibility=shadow_visibility(input.world_pos,normal,l_dir);
        }
        result += (diff * input.color + spec) * light.color * visibility;
    }

    if(effects.params.y>0.5 && input.material.x>0.0) {
        let reflected=effects.reflection_matrix*vec4(input.world_pos,1.0);
        let uv=reflected.xy/reflected.w*vec2(0.5,-0.5)+vec2(0.5);
        if(reflected.w>0.0 && all(uv>=vec2(0.0)) && all(uv<=vec2(1.0))) {
            result=mix(result,textureSampleLevel(reflection_texture,reflection_sampler,uv,0.0).rgb,input.material.x);
        }
    }
    return vec4<f32>(result, 1.0);
}

@fragment
fn fs_color(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
