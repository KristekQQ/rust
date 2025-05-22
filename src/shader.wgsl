struct Uniforms {
    mvp : mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms : Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.pos = uniforms.mvp * vec4<f32>(input.position, 1.0);
    out.normal = (uniforms.model * vec4<f32>(input.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let light = normalize(uniforms.light_dir.xyz);
    let n = normalize(input.normal);
    let diffuse = max(dot(n, light), 0.0);
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let reflect_dir = reflect(-light, n);
    let spec = pow(max(dot(view_dir, reflect_dir), 0.0), 16.0);
    let ambient = 0.3;
    let color = vec3<f32>(0.4, 0.6, 0.8) * (ambient + diffuse) + vec3<f32>(1.0) * spec;
    return vec4<f32>(color, 1.0);
}
