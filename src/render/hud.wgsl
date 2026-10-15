struct Output { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs(@builtin(vertex_index) vertex: u32,
    @location(0) rect: vec4<f32>, @location(1) color: vec4<f32>) -> Output {
    let corners = array<vec2<f32>, 6>(vec2(0.,0.), vec2(1.,0.), vec2(0.,1.),
        vec2(0.,1.), vec2(1.,0.), vec2(1.,1.));
    var out: Output;
    out.position = vec4(rect.xy + corners[vertex] * rect.zw, 0., 1.);
    out.color = color;
    return out;
}
@fragment fn fs(input: Output) -> @location(0) vec4<f32> { return input.color; }
