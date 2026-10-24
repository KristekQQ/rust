// ACES RRT/ODT fit by Stephen Hill, matrix/exposure convention used by Three.js
// r170 (MIT): src/renderers/shaders/ShaderChunk/tonemapping_pars_fragment.glsl.js.
// This is an opt-in per-material display transform, before output sRGB encoding.
fn appearance_color(input:vec3<f32>, view_depth:f32)->vec3<f32> {
    if(scene.appearance_sky.w<0.5 || scene.appearance_sun.w<=0.0) {return input;}
    let aces_in=mat3x3<f32>(vec3(0.59719,0.07600,0.02840),vec3(0.35458,0.90834,0.13383),vec3(0.04823,0.01566,0.83777));
    let aces_out=mat3x3<f32>(vec3(1.60475,-0.10208,-0.00327),vec3(-0.53108,1.10813,-0.07276),vec3(-0.07367,-0.00605,1.07602));
    let v=aces_in*(max(input,vec3(0.0))*(scene.appearance_sun.w/0.6));
    let fitted=(v*(v+vec3(0.0245786))-vec3(0.000090537))/(v*(0.983729*v+vec3(0.4329510))+vec3(0.238081));
    let mapped=clamp(aces_out*fitted,vec3(0.0),vec3(1.0));
    let fog=smoothstep(scene.appearance_fog.w,scene.appearance_params.x,view_depth);
    return mix(mapped,scene.appearance_fog.rgb,fog);
}
fn appearance_ambient(normal:vec3<f32>)->vec3<f32> {
    return mix(scene.appearance_ground.rgb,scene.appearance_sky.rgb,normal.y*0.5+0.5)/3.14159265;
}
