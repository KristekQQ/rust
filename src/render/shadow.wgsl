@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;
@group(0) @binding(1) var variant_colors: texture_2d<f32>;
struct Input {
    @location(0) position:vec3<f32>,
    @location(1) color:vec3<f32>,
    @location(12) emission:vec4<f32>,
    @location(13) surface_params:vec4<f32>,
    @location(3) model0:vec4<f32>, @location(4) model1:vec4<f32>,
    @location(5) model2:vec4<f32>, @location(6) model3:vec4<f32>,
};
struct Output {
    @builtin(position) position:vec4<f32>,
    @location(0) color:vec3<f32>,
    @location(1) @interpolate(flat) color_cutout:f32,
    @location(2) @interpolate(flat) transparency:vec2<f32>,
};
@vertex fn vs_main(input:Input)->Output {
    let model=mat4x4<f32>(input.model0,input.model1,input.model2,input.model3);
    var output:Output;
    output.position=view_projection*model*vec4(input.position,1.0);
    output.color=input.color;
    if (input.surface_params.w > 0.0 && input.color.r < 0.0) {
        let offset = u32(input.surface_params.z) * u32(input.surface_params.w) + u32(-input.color.r - 1.0);
        let width = textureDimensions(variant_colors).x;
        output.color = textureLoad(variant_colors, vec2<i32>(i32(offset % width), i32(offset / width)), 0).rgb;
    }
    output.color_cutout=input.emission.y;
    output.transparency=input.emission.zw;
    return output;
}
@fragment fn fs_main(input:Output) {
    if (input.transparency.x < 1.0 && max(max(input.color.r,input.color.g),input.color.b)<input.transparency.y) { discard; }
    if (max(max(input.color.r,input.color.g),input.color.b)<input.color_cutout) { discard; }
}
