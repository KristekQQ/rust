// GLES eye storage is sRGB: sampling decodes it. Encode once for the RGBA8
// XR framebuffer, like the pinned HAL's canvas sRGB presentation shader.
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Output { @builtin(position) position:vec4f, @location(0) uv:vec2f };
@vertex fn vs_main(@builtin(vertex_index) i:u32)->Output {
    let positions=array<vec2f,3>(vec2f(-1.,-1.),vec2f(3.,-1.),vec2f(-1.,3.));
    let p=positions[i];var o:Output;o.position=vec4f(p,0.,1.);
    o.uv=vec2f((p.x+1.)*.5,(1.+p.y)*.5);return o;
}
@fragment fn fs_main(i:Output)->@location(0) vec4f {
    let linear=textureSample(image,image_sampler,i.uv).rgb;
    let selector=ceil(linear-vec3f(.0031308));
    let encoded=mix(12.92*linear,1.055*pow(linear,vec3f(.41666))-.055,selector);
    return vec4f(encoded,1.);
}
