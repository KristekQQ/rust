struct GlowUniforms {
    view_projection: mat4x4<f32>,
    params: vec4<f32>,
    font_profile: vec4<f32>,
};
@group(0) @binding(0) var<uniform> glow: GlowUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) endpoints: vec4<f32>,
    @location(1) @interpolate(flat) color_radius: vec4<f32>,
    @location(2) coord: vec2<f32>,
    @location(3) @interpolate(flat) depths: vec2<f32>,
};

@vertex fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) edge_a: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) edge_b: vec3<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
    @location(11) material: vec4<f32>,
    @location(12) emission: vec4<f32>,
    @location(13) surface_params: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    // A clipped degenerate primitive safely suppresses dark/behind-eye edges.
    out.position = vec4<f32>(0.0, 0.0, -1.0, 1.0);
    out.endpoints = vec4<f32>(0.0);
    out.color_radius = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    out.coord = vec2<f32>(0.0);
    out.depths = vec2<f32>(0.0);
    let model = mat4x4<f32>(model0, model1, model2, model3);
    let a = glow.view_projection * model * vec4<f32>(edge_a, 1.0);
    let b = glow.view_projection * model * vec4<f32>(edge_b, 1.0);
    let peak = max(max(color.r, color.g), color.b);
    if a.w <= 0.01 || b.w <= 0.01 || emission.x <= 0.0 || peak < emission.y { return out; }
    let viewport = glow.params.xy;
    let screen_scale = viewport * vec2<f32>(0.5, -0.5);
    let pa = (a.xy / a.w) * screen_scale + viewport * 0.5;
    let pb = (b.xy / b.w) * screen_scale + viewport * 0.5;
    let delta = pb - pa;
    let length_px = length(delta);
    if length_px < 0.001 { return out; }
    let tangent = delta / length_px;
    let side = vec2<f32>(-tangent.y, tangent.x);
    // Projection scale is constant across all edges in this view; prepared on CPU.
    // World-space text gets a stronger halo; ordinary scene geometry keeps its profile.
    let numeric = material.w < -1.5;
    let text_radius = select(1.0, 1.6, material.w < -0.5);
    let text_energy = select(select(1.0,1.5,material.w < -0.5),0.9,numeric);
    var projected_radius = glow.params.w*text_radius;
    if numeric {
        let base_world_radius = glow.font_profile.x;
        let glyph_world_radius = length(model0.xyz)*glow.font_profile.y;
        let projected_scale = glow.params.w/max(base_world_radius,0.000001);
        projected_radius = (base_world_radius+glyph_world_radius)*projected_scale;
    }
    let radius = clamp(projected_radius/min(a.w,b.w),0.75,128.0);
    var corner = vec2<f32>(-1.0, -1.0);
    switch vertex_index % 4u {
        case 1u: { corner = vec2<f32>(1.0, -1.0); }
        case 2u: { corner = vec2<f32>(1.0, 1.0); }
        case 3u: { corner = vec2<f32>(-1.0, 1.0); }
        default: {}
    }
    let t = (corner.x + 1.0) * 0.5;
    let pixel = mix(pa, pb, t) + tangent * corner.x * radius + side * corner.y * radius;
    // Interpolate viewport-local pixels instead of reading absolute fragment position.
    out.coord = pixel;
    out.depths = vec2<f32>(a.z/a.w,b.z/b.w);
    // NDC depth varies linearly in screen space, including clamped capsule endcaps.
    let depth_t = clamp(dot(pixel - pa, tangent) / length_px, 0.0, 1.0);
    out.position = vec4<f32>((pixel - viewport * 0.5) / screen_scale, mix(a.z / a.w, b.z / b.w, depth_t), 1.0);
    out.endpoints = vec4<f32>(pa, pb);
    // Normalize only HDR colors: repeated bright triangle halos should keep their hue.
    let hue = max(color, vec3<f32>(0.0)) / max(peak, 1.0);
    // A bright tube profile must be visible along the whole edge, even at low
    // scene emission; max-compositing prevents this energy accumulating at joins.
    let energy = sqrt(min(emission.x, 8.0)) * glow.params.z * 0.65 * text_energy;
    out.color_radius = vec4<f32>(hue * energy, radius);
    return out;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};
@fragment fn fs_main(input: VertexOutput) -> FragmentOutput {
    let a = input.endpoints.xy;
    let delta = input.endpoints.zw - a;
    let t = clamp(dot(input.coord - a, delta) / max(dot(delta, delta), 0.000001), 0.0, 1.0);
    let d = length(input.coord - (a + delta * t)) / input.color_radius.w;
    if d >= 1.0 { discard; }
    // Narrow luminous tube + a broad colored skirt. Their union stays equally
    // bright at a line midpoint and at a vertex shared by several strokes.
    let falloff = (0.65 * exp(-40.0*d*d) + 0.35 * exp(-4.0*d*d))
        * (1.0-smoothstep(0.75,1.0,d));
    var output: FragmentOutput;
    output.color = vec4<f32>(input.color_radius.rgb * falloff, 0.0);
    // Use the original segment length, not the extended quad, for depth.
    output.depth = clamp(mix(input.depths.x,input.depths.y,t),0.0,1.0);
    return output;
}
