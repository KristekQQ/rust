// scene_color is an UNORM view of our sRGB render target: already encoded bytes.
// Do not sample the sRGB view or apply another transfer function here.
@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var scene_depth: texture_depth_2d;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let positions = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return vec4f(positions[index], 0.0, 1.0);
}

@fragment
fn fs_color(@builtin(position) position: vec4f) -> @location(0) vec4f {
    return vec4f(textureLoad(scene_color, vec2i(position.xy), 0).rgb, 1.0);
}

struct DepthOutput {
    @location(0) color: vec4f,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_depth(@builtin(position) position: vec4f) -> DepthOutput {
    var output: DepthOutput;
    output.color = vec4f(textureLoad(scene_color, vec2i(position.xy), 0).rgb, 1.0);
    output.depth = textureLoad(scene_depth, vec2i(position.xy), 0);
    return output;
}
