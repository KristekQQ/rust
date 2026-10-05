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
};
@vertex
fn vs_main(input: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let model = mat4x4<f32>(instance.model0, instance.model1, instance.model2, instance.model3);
    let normal_matrix = mat4x4<f32>(instance.normal0, instance.normal1, instance.normal2, instance.normal3);
    out.pos = scene.view_projection * model * vec4<f32>(input.position, 1.0);
    out.color = input.color;
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
    out.world_pos = input.position;
    out.world_normal = input.normal;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
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
        result += (diff * input.color + spec) * light.color;
    }

    return vec4<f32>(result, 1.0);
}

@fragment
fn fs_color(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
