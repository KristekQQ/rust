// Portable WebGL depth, paired with the scene MRT attachment.
@group(0) @binding(2) var scene_depth: texture_2d<f32>;
fn depth_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(uniforms.params.zw);
    let pixel = clamp(vec2<i32>(uv * uniforms.params.zw), vec2<i32>(0), size - vec2<i32>(1));
    // RGB base-255 depth packing avoids depth-sampler LOD limitations in GLSL ES300.
    // An auxiliary attachment cleared to white represents untouched far depth.
    let packed = textureSampleLevel(scene_depth, depth_sampler, (vec2<f32>(pixel) + vec2(0.5)) / uniforms.params.zw, 0.0).rgb;
    return clamp(dot(packed, vec3(1.0, 1.0 / 255.0, 1.0 / 65025.0)), 0.0, 1.0);
}
