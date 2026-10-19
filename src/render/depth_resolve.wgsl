@group(0) @binding(0) var scene_depth: texture_depth_2d;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let positions = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return vec4f(positions[index], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @builtin(frag_depth) f32 {
    return textureLoad(scene_depth, vec2i(position.xy), 0);
}
