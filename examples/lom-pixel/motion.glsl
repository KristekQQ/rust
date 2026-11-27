// Shadertoy Buffer A: iChannel0 = Music, iChannel1 = Buffer A (its own previous frame).
// Use the same Music input on Image. Image iChannel1 = Buffer A.
#ifndef LOM_SPEED_AMOUNT
#define LOM_SPEED_AMOUNT 0.35
#endif

// Byte-packed floats work in ordinary RGBA8 as well as Shadertoy float buffers.
vec4 packMotion(float value) {
    uint bits=floatBitsToUint(value);
    return vec4((uvec4(bits)>>uvec4(24,16,8,0))&255u)/255.;
}
float previousMotion(int index) {
    uvec4 b=uvec4(texelFetch(iChannel1,ivec2(index,0),0)*255.+.5);
    return uintBitsToFloat((b.r<<24)|(b.g<<16)|(b.b<<8)|b.a);
}
float musicBin(float bin) {
    return texture(iChannel0,vec2((bin+.5)/512.,.25)).r;
}
void mainImage(out vec4 color,in vec2 coord) {
    ivec2 cell=ivec2(coord);
    if(cell.y!=0 || cell.x>2) { color=vec4(0.); return; }
    if(iFrame==0) { color=packMotion(cell.x==1 ? 1. : 0.); return; }

    float dt=clamp(iTimeDelta,0.,.12);
    float travel=previousMotion(0), speed=previousMotion(1), slow=previousMotion(2);
    float kick=.62*musicBin(2.)+.38*musicBin(4.);
    float body=.45*musicBin(8.)+.32*musicBin(16.)+.23*musicBin(36.);
    float energy=.70*kick*kick+.30*body*body;
    float punch=clamp((energy-slow)*2.8,0.,1.);
    float amount=clamp(LOM_SPEED_AMOUNT,0.,1.);
    float target=mix(1.,.42+1.55*energy+.95*punch,amount);
    // Integrate the smoothed speed. Both attack and release move forward.
    float rate=target>speed ? 12. : 4.5;
    float decay=exp(-rate*dt);
    travel+=amount==0. ? dt : target*dt+(speed-target)*(1.-decay)/rate;
    speed=amount==0. ? 1. : mix(target,speed,decay);
    slow=mix(energy,slow,exp(-2.4*dt));
    color=packMotion(cell.x==0 ? travel : (cell.x==1 ? speed : slow));
}
