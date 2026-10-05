@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;
struct Input {
    @location(0) position:vec3<f32>,
    @location(3) model0:vec4<f32>, @location(4) model1:vec4<f32>,
    @location(5) model2:vec4<f32>, @location(6) model3:vec4<f32>,
};
@vertex fn vs_main(input:Input)->@builtin(position) vec4<f32> {
    let model=mat4x4<f32>(input.model0,input.model1,input.model2,input.model3);
    return view_projection*model*vec4(input.position,1.0);
}
