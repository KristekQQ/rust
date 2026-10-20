struct SceneUniforms { view_projection: mat4x4<f32> };
@group(0) @binding(0) var<uniform> scene: SceneUniforms;
@group(1) @binding(0) var ring_texture: texture_2d<f32>;
@group(1) @binding(1) var ring_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) fade: f32,
    @location(2) local_z: f32,
    @location(3) @interpolate(flat) top_surface: f32,
};
@vertex fn vs_main(
    @location(0) position: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(12) emission: vec4<f32>,
    @location(13) surface_params: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let world = mat4x4<f32>(model0, model1, model2, model3) * vec4<f32>(position, 1.0);
    out.position = scene.view_projection * world;
    out.local_z = position.z;
    out.top_surface = normal.z;
    out.uv = position.xy / 2.16 + vec2<f32>(0.5);
    out.fade = clamp(emission.x / 2.8, 0.0, 1.0);
    return out;
}
fn sampled(input: VertexOutput) -> vec4<f32> {
    // Use one cap of the closed pooled mesh as a double-sided planar decal.
    // Discard the other cap and sidewalls so alpha is applied exactly once.
    if input.local_z < 0.006 || input.top_surface < 0.9 { discard; }
    let texel = textureSample(ring_texture, ring_sampler, input.uv);
    let alpha = texel.a * input.fade;
    if alpha < 0.003 { discard; }
    return vec4<f32>(texel.rgb, alpha);
}
@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> { return sampled(input); }
struct MaskOutput { @location(0) color: vec4<f32>, @location(1) mask: vec4<f32> };
@fragment fn fs_mask(input: VertexOutput) -> MaskOutput {
    var out: MaskOutput;
    out.color = sampled(input);
    out.mask = vec4<f32>(0.0);
    return out;
}
