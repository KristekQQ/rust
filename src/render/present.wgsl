@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
struct Output { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Output {
    let positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let p = positions[index];
    var output: Output;
    output.position = vec4(p, 0.0, 1.0);
    output.uv = vec2((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return output;
}
@fragment fn fs_main(input: Output) -> @location(0) vec4<f32> {
    return textureSample(scene_texture, scene_sampler, input.uv);
}
