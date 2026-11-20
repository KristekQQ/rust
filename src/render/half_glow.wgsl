struct Params { viewport: vec4<u32>, };
@group(0) @binding(0) var scene_depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var halo: texture_2d<f32>;
@group(0) @binding(3) var low_depth: texture_depth_2d;

@vertex fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));
    return vec4<f32>(p[i],0.,1.);
}
@fragment fn fs_reduce(@builtin(position) p: vec4<f32>) -> @builtin(frag_depth) f32 {
    let local = vec2<u32>(p.xy)*2u;
    let end = params.viewport.xy+params.viewport.zw-vec2<u32>(1u);
    let a = params.viewport.xy+local;
    // Minimum scene depth conservatively rejects hidden halos, including thin
    // foreground silhouettes. Odd dimensions duplicate the last source texel.
    let d0 = textureLoad(scene_depth,vec2<i32>(min(a,end)),0);
    let d1 = textureLoad(scene_depth,vec2<i32>(min(a+vec2<u32>(1u,0u),end)),0);
    let d2 = textureLoad(scene_depth,vec2<i32>(min(a+vec2<u32>(0u,1u),end)),0);
    let d3 = textureLoad(scene_depth,vec2<i32>(min(a+vec2<u32>(1u),end)),0);
    return min(min(d0,d1),min(d2,d3));
}
@fragment fn fs_composite(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(p.xy);
    let local = (p.xy-vec2<f32>(params.viewport.xy))*0.5-vec2<f32>(0.5);
    let base = vec2<i32>(floor(local));
    let f = fract(local);
    let end = vec2<i32>(textureDimensions(halo))-vec2<i32>(1);
    let q00 = clamp(base, vec2<i32>(0), end);
    let q10 = clamp(base + vec2<i32>(1, 0), vec2<i32>(0), end);
    let q01 = clamp(base + vec2<i32>(0, 1), vec2<i32>(0), end);
    let q11 = clamp(base + vec2<i32>(1, 1), vec2<i32>(0), end);
    let h00 = textureLoad(halo, q00, 0).rgb;
    let h10 = textureLoad(halo, q10, 0).rgb;
    let h01 = textureLoad(halo, q01, 0).rgb;
    let h11 = textureLoad(halo, q11, 0).rgb;
    // With no glow in this footprint every depth-tested weight contributes zero.
    if all(h00 == vec3<f32>(0.)) && all(h10 == vec3<f32>(0.)) &&
       all(h01 == vec3<f32>(0.)) && all(h11 == vec3<f32>(0.)) {
        return vec4<f32>(0.);
    }
    let depth = textureLoad(scene_depth, pixel, 0) + 0.000001;
    var color = vec3<f32>(0.);
    if textureLoad(low_depth, q00, 0) <= depth { color += h00 * ((1. - f.x) * (1. - f.y)); }
    if textureLoad(low_depth, q10, 0) <= depth { color += h10 * (f.x * (1. - f.y)); }
    if textureLoad(low_depth, q01, 0) <= depth { color += h01 * ((1. - f.x) * f.y); }
    if textureLoad(low_depth, q11, 0) <= depth { color += h11 * (f.x * f.y); }
    return vec4<f32>(color,0.);
}
