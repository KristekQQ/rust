struct Fade { value: vec4<f32> };
@group(0) @binding(0) var<uniform> fade: Fade;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) @interpolate(flat) id: u32 };
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) id: u32) -> Out {
    let corners = array<vec2<f32>, 6>(vec2(-1.,-1.), vec2(1.,-1.), vec2(-1.,1.), vec2(-1.,1.), vec2(1.,-1.), vec2(1.,1.));
    let centers = array<vec2<f32>, 4>(vec2(-.45,.38), vec2(.55,.28), vec2(-.48,-.46), vec2(.48,-.55));
    let sizes = array<vec2<f32>, 4>(vec2(.35,.28), vec2(.20,.25), vec2(.23,.15), vec2(.16,.12));
    var o: Out; o.uv = corners[v]; o.id = id; o.pos = vec4(centers[id] + o.uv * (sizes[id] * 1.9), 0., 1.); return o;
}
@fragment fn fs(o: Out) -> @location(0) vec4<f32> {
    let p = o.uv;
    let a = atan2(p.y, p.x);
    let boundary = .66 + .12 * sin(a * 3. + f32(o.id) * 1.7) + .07 * sin(a * 5. - f32(o.id));
    let body = 1. - smoothstep(boundary - .33, boundary + .15, length(p));
    // A short tapered run on the main smear, tapering away from its body.
    let taper = max(0., 1. - abs(p.y + .73) / .27);
    let drip = (1. - smoothstep(.025, max(.001, .12 * taper) + .025, abs(p.x + .19))) * taper;
    let opacity = select(.60, .81, o.id == 0u);
    return vec4(.98, .99, 1., max(body, select(0., drip * .7, o.id == 0u)) * opacity * fade.value.x);
}
