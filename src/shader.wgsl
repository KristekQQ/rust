struct Light {
    position: vec3<f32>,
    _pad_p: f32,
    color: vec3<f32>,
    _pad_c: f32,
};

struct SceneUniforms {
    vp: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    lights: array<Light, 2>,
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(7) inst_color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let model = mat4x4<f32>(
        input.model0,
        input.model1,
        input.model2,
        input.model3,
    );
    let world_pos = model * vec4<f32>(input.position, 1.0);
    out.pos = scene.vp * world_pos;
    out.color = input.inst_color * input.color;
    out.world_pos = world_pos.xyz;
    // Transform the normal by the model matrix without applying translation
    out.world_normal = normalize((model * vec4<f32>(input.normal, 0.0)).xyz);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.world_normal);
    let view_dir = normalize(scene.camera_pos - input.world_pos);
    var result = input.color * 0.1; // ambient

    // light 0
    let l0_dir = normalize(scene.lights[0].position - input.world_pos);
    let diff0 = max(dot(normal, l0_dir), 0.0);
    let spec0 = pow(max(dot(normal, normalize(l0_dir + view_dir)), 0.0), 32.0);
    result += (diff0 * input.color + spec0) * scene.lights[0].color;

    // light 1
    let l1_dir = normalize(scene.lights[1].position - input.world_pos);
    let diff1 = max(dot(normal, l1_dir), 0.0);
    let spec1 = pow(max(dot(normal, normalize(l1_dir + view_dir)), 0.0), 32.0);
    result += (diff1 * input.color + spec1) * scene.lights[1].color;

    return vec4<f32>(result, 1.0);
}

@fragment
fn fs_color(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
