// WebGPU reads the exact hardware depth without RGB packing or normalized UVs.
@group(0) @binding(2) var scene_depth: texture_depth_2d;
fn depth_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(scene_depth, pixel, 0);
}
