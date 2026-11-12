// Experimental glyph boundary-distance halo. The real mesh core is unchanged.
struct View { vp: mat4x4<f32>, camera: vec4<f32>, params: vec4<f32> };
struct Glyph { bounds: vec4<f32>, depth_pad: vec4<f32>, front: vec4<f32>, back: vec4<f32> };
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var field: texture_2d<f32>;
@group(1) @binding(1) var<uniform> glyph: Glyph;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) radius: f32, @location(2) @interpolate(flat) color: vec3<f32> };
@vertex fn vs_main(@builtin(vertex_index) vertex: u32,
    @location(3) m0: vec4<f32>, @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>, @location(6) m3: vec4<f32>,
    @location(10) fade: vec4<f32>, @location(12) emission: vec4<f32>) -> Out {
    let model = mat4x4<f32>(m0,m1,m2,m3);
    let front = dot(m2.xyz, view.camera.xyz-m3.xyz) >= 0.0;
    let z = select(glyph.depth_pad.x, glyph.depth_pad.y, front);
    var uv = vec2<f32>(0.0,0.0);
    switch vertex { case 1u, 4u: { uv=vec2<f32>(1.0,0.0); }
        case 2u, 3u: { uv=vec2<f32>(0.0,1.0); } case 5u: { uv=vec2<f32>(1.0,1.0); } default: {} }
    let p = mix(glyph.bounds.xy, glyph.bounds.zw, uv);
    let color = select(glyph.back.xyz,glyph.front.xyz,front);
    let peak = max(max(color.x,color.y),color.z);
    var out: Out;
    out.position=view.vp*model*vec4<f32>(p,z,1.0);
    out.uv=uv;
    out.radius=view.params.y*1.6/length(m0.xyz);
    out.color=max(color,vec3<f32>(0.0))/max(peak,1.0)
        *sqrt(min(emission.x,8.0))*view.params.x*0.65*1.5*fade.w;
    if emission.x<=0.0 || peak<emission.y { out.position=vec4<f32>(0.0,0.0,-1.0,1.0); }
    return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
    let size=vec2<i32>(textureDimensions(field));
    let p=input.uv*vec2<f32>(size)-vec2<f32>(0.5);
    let base=vec2<i32>(floor(p)); let f=fract(p);
    let hi=size-vec2<i32>(1);
    let a=textureLoad(field,clamp(base,vec2<i32>(0),hi),0).r;
    let b=textureLoad(field,clamp(base+vec2<i32>(1,0),vec2<i32>(0),hi),0).r;
    let c=textureLoad(field,clamp(base+vec2<i32>(0,1),vec2<i32>(0),hi),0).r;
    let e=textureLoad(field,clamp(base+vec2<i32>(1,1),vec2<i32>(0),hi),0).r;
    let d=mix(mix(a,b,f.x),mix(c,e,f.x),f.y)/input.radius;
    if d>=1.0 { discard; }
    let falloff=(0.65*exp(-40.0*d*d)+0.35*exp(-4.0*d*d))*(1.0-smoothstep(0.75,1.0,d));
    return vec4<f32>(input.color*falloff,0.0);
}
