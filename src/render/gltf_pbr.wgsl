struct Light {
    position: vec3<f32>,
    _pad_p: f32,
    color: vec3<f32>,
    _pad_c: f32,
};

const LIGHT_COUNT: u32 = 4u;

struct SceneUniforms {
    view_projection: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    lights: array<Light, LIGHT_COUNT>,
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;

struct EffectsUniforms {
    shadow_matrices:array<mat4x4<f32>,4>, reflection_matrix:mat4x4<f32>,
    clip_plane:vec4<f32>, params:vec4<f32>,
};
@group(1) @binding(0) var<uniform> effects:EffectsUniforms;
@group(1) @binding(1) var static_shadow:texture_depth_2d;
@group(1) @binding(2) var dynamic_shadow:texture_depth_2d;
@group(1) @binding(3) var shadow_sampler:sampler_comparison;
@group(1) @binding(4) var reflection_texture:texture_2d<f32>;
@group(1) @binding(5) var reflection_sampler:sampler;
@group(1) @binding(6) var static_shadow_1:texture_depth_2d;
@group(1) @binding(7) var dynamic_shadow_1:texture_depth_2d;
@group(1) @binding(8) var static_shadow_2:texture_depth_2d;
@group(1) @binding(9) var dynamic_shadow_2:texture_depth_2d;
@group(1) @binding(10) var static_shadow_3:texture_depth_2d;
@group(1) @binding(11) var dynamic_shadow_3:texture_depth_2d;
fn compare_shadow(index:u32,uv:vec2<f32>,depth:f32)->f32 {
    switch index {
        case 0u: { return min(textureSampleCompareLevel(static_shadow,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow,shadow_sampler,uv,depth)); }
        case 1u: { return min(textureSampleCompareLevel(static_shadow_1,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow_1,shadow_sampler,uv,depth)); }
        case 2u: { return min(textureSampleCompareLevel(static_shadow_2,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow_2,shadow_sampler,uv,depth)); }
        case 3u: { return min(textureSampleCompareLevel(static_shadow_3,shadow_sampler,uv,depth),textureSampleCompareLevel(dynamic_shadow_3,shadow_sampler,uv,depth)); }
        default: { return 1.0; }
    }
}
fn shadow_visibility(index:u32,world:vec3<f32>,normal:vec3<f32>,light_direction:vec3<f32>)->f32 {
    let clip=effects.shadow_matrices[index]*vec4(world,1.0);
    let ndc=clip.xyz/clip.w;
    if (clip.w<=0.0 || ndc.z<=0.0 || ndc.z>=1.0 || any(abs(ndc.xy)>vec2(1.0))) {return 1.0;}
    let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);
    let bias=effects.params.w*(1.0+2.0*(1.0-max(dot(normal,light_direction),0.0)));
    let depth=ndc.z-bias;
    let texel=1.0/vec2<f32>(textureDimensions(static_shadow));
    var visibility=0.0;
    // Combine static and dynamic visibility per texel, then apply 3x3 PCF.
    for(var y=-1;y<=1;y++) {for(var x=-1;x<=1;x++) {
        let sample_uv=uv+vec2<f32>(f32(x),f32(y))*texel;
        visibility+=compare_shadow(index,sample_uv,depth);
    }}
    return visibility/9.0;
}


struct Material { base:vec4<f32>, emissive:vec4<f32>, factors:vec4<f32>, uv:vec4<u32>, extra:vec4<f32>, options:vec4<f32> };
@group(3) @binding(0) var<uniform> material:Material;
@group(2) @binding(0) var base_map:texture_2d<f32>;
@group(2) @binding(1) var base_sampler:sampler;
@group(2) @binding(2) var mr_map:texture_2d<f32>;
@group(2) @binding(3) var mr_sampler:sampler;
@group(2) @binding(4) var normal_map:texture_2d<f32>;
@group(2) @binding(5) var normal_sampler:sampler;
@group(2) @binding(6) var ao_map:texture_2d<f32>;
@group(2) @binding(7) var ao_sampler:sampler;
@group(2) @binding(8) var emissive_map:texture_2d<f32>;
@group(2) @binding(9) var emissive_sampler:sampler;
struct Input { @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) tangent:vec4<f32>, @location(3) uv0:vec2<f32>, @location(4) uv1:vec2<f32>, @location(5) uv2:vec2<f32>, @location(6) uv3:vec2<f32>, @location(7) uv4:vec2<f32>, @location(8) color:vec4<f32> };
struct Output { @builtin(position) pos:vec4<f32>, @location(0) world:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) tangent:vec4<f32>, @location(3) uv0:vec2<f32>, @location(4) uv1:vec2<f32>, @location(5) uv2:vec2<f32>, @location(6) uv3:vec2<f32>, @location(7) uv4:vec2<f32>, @location(8) color:vec4<f32> };
@vertex fn vs_main(v:Input)->Output { var o:Output; o.pos=scene.view_projection*vec4(v.position,1.0); o.world=v.position; o.normal=v.normal; o.tangent=v.tangent; o.uv0=v.uv0; o.uv1=v.uv1; o.uv2=v.uv2; o.uv3=v.uv3; o.uv4=v.uv4; o.color=v.color; return o; }
fn uv(v:Output,index:u32)->vec2<f32> { return select(v.uv0,v.uv1,index==1u); }
fn linear_color(c:vec3<f32>)->vec3<f32> { return select(c/12.92,pow((c+vec3(0.055))/1.055,vec3(2.4)),c>vec3(0.04045)); }
fn base_color(v:Output)->vec4<f32> { let t=textureSample(base_map,base_sampler,v.uv0); return t*material.base*v.color; }
fn alpha_test(alpha:f32) { if(material.extra.y==1.0 && alpha<material.extra.x) { discard; } }
fn unit(v:vec3<f32>)->vec3<f32> { return v*inverseSqrt(max(dot(v,v),0.000000000001)); }
fn surface(v:Output,front:bool)->vec4<f32> {
    let p1=dpdx(v.world); let p2=dpdy(v.world); let u1=dpdx(v.uv2); let u2=dpdy(v.uv2);
    let base=base_color(v); let emission=textureSample(emissive_map,emissive_sampler,v.uv4).rgb*material.emissive.rgb;
    alpha_test(base.a);
    if(effects.params.z>0.5 && dot(effects.clip_plane,vec4(v.world,1.0)) < -0.001) { discard; }
    if(material.extra.w>0.5) { return vec4(base.rgb+emission,base.a); }
    var n=unit(v.normal); let view=unit(scene.camera_pos-v.world);
    if(material.options.w>0.5) {
        let geometric=cross(p1,p2);
        if(dot(geometric,geometric)>0.0000000001) {
            n=unit(geometric);
            if(dot(n,view)<0.0) { n=-n; }
        }
    }
    // Screen-space handedness differs between LH main and reflected passes.
    if(material.extra.z>0.5 && dot(n,view)<0.0) { n=-n; }
    let mapped=textureSample(normal_map,normal_sampler,v.uv2).xyz*2.0-vec3(1.0);
    let scaled=vec3(mapped.xy*material.factors.z,mapped.z);
    var t=unit(v.tangent.xyz-n*dot(n,v.tangent.xyz));
    var b=cross(n,t)*v.tangent.w;
    if(dot(v.tangent.xyz,v.tangent.xyz)<0.01) {
        let det=u1.x*u2.y-u1.y*u2.x;
        if(abs(det)>0.0000001) { t=unit((p1*u2.y-p2*u1.y)/det); b=unit((-p1*u2.x+p2*u1.x)/det); }
        else { t=unit(cross(select(vec3(0.0,1.0,0.0),vec3(1.0,0.0,0.0),abs(n.y)>0.9),n)); b=cross(n,t); }
    }
    n=unit(t*scaled.x+b*scaled.y+n*scaled.z);
    let mr=textureSample(mr_map,mr_sampler,v.uv1);
    let metallic=clamp(material.factors.x*mr.b,0.0,1.0); let rough=max(0.045,material.factors.y*mr.g);
    let ao=mix(1.0,textureSample(ao_map,ao_sampler,v.uv3).r,material.factors.w);
    let f0=mix(vec3(0.04),base.rgb,metallic); let nv=max(dot(n,view),0.0001); let a=rough*rough; let a2=a*a;
    var color=base.rgb*(1.0-metallic)*0.1*ao;
    for(var i=0u;i<LIGHT_COUNT;i++) {
        let delta=scene.lights[i].position-v.world; let l=unit(delta); let h=unit(l+view); let nl=max(dot(n,l),0.0); let nh=max(dot(n,h),0.0); let vh=max(dot(view,h),0.0);
        let d=a2/(3.14159265*pow(nh*nh*(a2-1.0)+1.0,2.0));
        let gv=2.0*nv/(nv+sqrt(a2+(1.0-a2)*nv*nv)); let gl=2.0*nl/max(nl+sqrt(a2+(1.0-a2)*nl*nl),0.0001);
        let f=f0+(vec3(1.0)-f0)*pow(1.0-vh,5.0); let spec=d*gv*gl*f/max(4.0*nv*nl,0.0001);
        var visibility=1.0; if(i<u32(effects.params.x) && material.options.y>0.5) { visibility=shadow_visibility(i,v.world,n,l); }
        color+=((vec3(1.0)-f)*(1.0-metallic)*base.rgb/3.14159265+spec)*scene.lights[i].color*nl*visibility;
    }
    color+=emission;
    if(effects.params.y>0.5 && material.options.z>0.5 && material.options.x>0.0) {
        let r=effects.reflection_matrix*vec4(v.world,1.0); let u=r.xy/r.w*vec2(0.5,-0.5)+vec2(0.5);
        if(r.w>0.0 && all(u>=vec2(0.0)) && all(u<=vec2(1.0))) { color=mix(color,textureSampleLevel(reflection_texture,reflection_sampler,u,0.0).rgb,material.options.x); }
    }
    return vec4(color,base.a);
}
@fragment fn fs_main(v:Output,@builtin(front_facing) front:bool)->@location(0) vec4<f32> { let c=surface(v,front); return vec4(c.rgb,select(1.0,c.a,material.extra.y==2.0)); }
struct PostSurface { @location(0) color:vec4<f32>, @location(1) depth:vec4<f32> };
@fragment fn fs_postprocess(v:Output,@builtin(front_facing) front:bool)->PostSurface { var o:PostSurface; let c=surface(v,front); o.color=vec4(c.rgb,material.options.x); var e=fract(min(v.pos.z,0.9999999)*vec3(1.0,255.0,65025.0)); e-=e.yzz*vec3(1.0/255.0,1.0/255.0,0.0); o.depth=vec4(e,1.0); return o; }
@fragment fn fs_shadow(v:Output) { alpha_test(base_color(v).a); if(material.extra.y==2.0 && base_color(v).a<0.5) { discard; } }
