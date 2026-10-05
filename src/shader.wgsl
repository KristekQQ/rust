struct Light {
    position: vec3<f32>,
    _pad_p: f32,
    color: vec3<f32>,
    _pad_c: f32,
};

const LIGHT_COUNT: u32 = 4u;

struct SceneUniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
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

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.pos = scene.mvp * vec4<f32>(input.position, 1.0);
    out.color = input.color;
    out.world_pos = (scene.model * vec4<f32>(input.position, 1.0)).xyz;
    // Transform the normal by the model matrix without applying translation
    // (w = 0). This keeps lighting separate from camera rotation.
    out.world_normal = normalize((scene.model * vec4<f32>(input.normal, 0.0)).xyz);
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
        let l_dir = normalize(light.position - input.world_pos);
        let diff = max(dot(normal, l_dir), 0.0);
        let spec = pow(max(dot(normal, normalize(l_dir + view_dir)), 0.0), 32.0);
        result += (diff * input.color + spec) * light.color;
    }

    return vec4<f32>(result, 1.0);
}

@fragment
fn fs_color(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
