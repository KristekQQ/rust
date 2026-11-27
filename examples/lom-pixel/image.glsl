// LOM / PIXEL — Image: iChannel0 = Music, optional iChannel1 = Buffer A (motion.glsl).
float flightTime;
#ifdef VYBOJ_VORTEX_ONLY
float lomWallDepth;
#endif
float flightClock() {
    if (textureSize(iChannel1,0).x>=3) {
        vec4 packedSpeed=texelFetch(iChannel1,ivec2(1,0),0);
        // The high byte of every valid speed (0.42–2.92) is 62, 63 or 64.
        float tag=round(packedSpeed.r*255.);
        if (tag>=62. && tag<=64.) {
            uvec4 b=uvec4(texelFetch(iChannel1,ivec2(0,0),0)*255.+.5);
            return uintBitsToFloat((b.r<<24)|(b.g<<16)|(b.b<<8)|b.a);
        }
    }
    return iTime;
}
float lomHash(vec2 p) {
    vec3 q = fract(p.xyx * vec3(.1031,.1030,.0973));
    q += dot(q, q.yzx+33.33);
    return fract((q.x+q.y)*q.z);
}
vec2 spine(float z) {
    return vec2(3.8*sin(.16*z+.17*flightTime)+1.2*sin(.33*z-.14*flightTime),
                2.9*cos(.19*z+.11*flightTime)+1.0*sin(.36*z+.18*flightTime));
}
// The first component is clearance inside the passage. Piecewise planar
// ridges meet at a pointed interior peak on each triangular tile.
vec3 crystal(vec3 p,vec4 bands) {
    vec2 xy=p.xy-spine(p.z);
    float angle=atan(xy.y,xy.x+.000001);
    float helical=angle/6.2831853+.021*p.z+(.040+.030*bands.y)*sin(.43*p.z-.51*flightTime+.24*bands.z);
    // A few whole triangles become large crowns. Divide the other triangles
    // into four smaller ones; every shared boundary stays flush with the wall.
    vec2 grid=vec2(helical*12.,p.z*.47);
    vec2 id=floor(grid), f=fract(grid);
    float upper=step(1.,f.x+f.y);
    vec2 tile=vec2(mod(id.x,12.),mod(id.y,251.)+upper*.37);
    float family=lomHash(tile);
    vec2 local=mix(f,1.-f,upper);
    bool large=family>.74;
    float seed=family;
    if(!large) {
        vec2 sub=local*2.;
        vec2 halfCell=floor(sub);
        vec2 sf=fract(sub);
        float flipped=step(1.,sf.x+sf.y);
        local=mix(sf,1.-sf,flipped);
        seed=lomHash(tile+vec2(17.31,8.17)+halfCell*2.43+flipped*.81);
    }
    vec3 bary=vec3(local,1.-local.x-local.y);
    vec3 peak=vec3(.16+.20*seed,.44,.40-.20*seed);
    float blade=max(0.,min(bary.x/peak.x,min(bary.y/peak.y,bary.z/peak.z)));
    float bay=.5+.5*cos(.26*p.z);
    float bank=.5+.5*cos(angle-.38*p.z+.65*sin(flightTime));
    float radius=2.28+1.05*bay+.42*cos(3.*angle+.37*p.z)+.19*sin(5.*angle-.43*p.z);
    // A slowly turning oval squeeze with one shallow fold on its shoulder.
    float turn=.31*p.z+(.40+.25*bands.y)*sin(.18*p.z-.36*flightTime)+.13*bands.z;
    float fold=angle-turn;
    radius+=(.19+.09*bands.x)*cos(2.*fold)+.10*sin(fold+.34*p.z);
    // Each tile keeps its own register and gain as the camera flies past.
    float tuning=fract(seed*37.73)*4.;
    int band=int(tuning);
    float drive=mix(bands[band],bands[(band+1)%4],.22*fract(tuning));
    // Height ranges stay distinct even when the entire spectrum is loud.
    float size=seed*seed;
    float resting=large ? .33+.15*fract(9.7*seed) : .035+.17*size;
    float stroke=large ? .34+.13*fract(13.4*seed) : .065+.20*size;
    float height=(radius-.66)*(resting+stroke*drive)*(.88+.12*bank);
    radius-=blade*height;
    return vec3(radius-length(xy),seed,blade);
}
// Read the music once per pixel, outside the ray march.
float fftBin(float bin) {
    return texture(iChannel0,vec2((bin+.5)/512.,.25)).r;
}
void mainImage(out vec4 fragColor,in vec2 fragCoord) {
    flightTime=flightClock();
    vec2 uv=(2.*fragCoord-iResolution.xy)/iResolution.y;
    float bass=fftBin(2.);
    float high=fftBin(95.);
    vec4 energy=vec4(.55*bass+.45*fftBin(4.),
                     .48*fftBin(7.)+.32*fftBin(12.)+.20*fftBin(18.),
                     .40*fftBin(27.)+.35*fftBin(43.)+.25*fftBin(61.),
                     .35*high+.35*fftBin(136.)+.30*fftBin(204.));
    vec4 bands=smoothstep(vec4(.09,.08,.06,.035),vec4(.96,.92,.85,.72),energy);
    float z=flightTime*12.3;
    vec3 eye=vec3(spine(z),z);
    vec3 front=normalize(vec3(spine(z+3.)-eye.xy,3.));
    vec3 right=normalize(cross(vec3(0.,1.,0.),front)), up=cross(front,right);
    float roll=(.14+.11*bands.y)*sin(z*.18);
    vec2 lens=mat2(cos(roll),sin(roll),-sin(roll),cos(roll))*uv;
    vec3 ray=normalize(front*(1.32-.10*bass)+right*lens.x+up*lens.y);
    float t=.07,prev=t;
    vec3 hit=vec3(0.);
    for(int i=0;i<104;i++) {
        hit=crystal(eye+t*ray,bands);
        if(hit.x<.004||t>36.)break;
        prev=t;
        t+=clamp(hit.x*.49,.014,1.1);
    }
    if(hit.x<0.) {
        for(int i=0;i<5;i++) {
            float mid=.5*(prev+t);
            if(crystal(eye+mid*ray,bands).x>0.)prev=mid;else t=mid;
        }
    }
#ifdef VYBOJ_VORTEX_ONLY
    lomWallDepth=t;
#endif
    vec3 p=eye+t*ray;
    hit=crystal(p,bands);
    vec2 e=vec2(.002,0.);
    vec3 n=normalize(vec3(crystal(p+e.xyy,bands).x-crystal(p-e.xyy,bands).x,
                          crystal(p+e.yxy,bands).x-crystal(p-e.yxy,bands).x,
                          crystal(p+e.yyx,bands).x-crystal(p-e.yyx,bands).x)+.000001);
    vec3 refl=reflect(ray,n);
    float warm=smoothstep(-.7,.7,dot(n.xy,vec2(cos(z*.19),sin(z*.19))));
    vec3 glow=mix(vec3(1.18,.18,1.36),vec3(1.75,.44,.075),warm);
    float edge=pow(1.-clamp(dot(n,-ray),0.,1.),3.);
    float spec=pow(max(0.,dot(refl,normalize(front+.48*up-.26*right))),13.);
    vec3 col=mix(vec3(.011,.008,.016),vec3(.19,.17,.20),hit.y*hit.y)*(.28+.7*abs(n.y));
    col+=glow*(.24*edge+.90*spec)*(.55+.65*high);
    float haze=smoothstep(5.,23.,t);
    col+=glow*haze*(.58+.60*bass)+vec3(1.8,1.02,1.3)*pow(haze,3.)*(.65+bass);
    fragColor=vec4(pow(1.-exp(-1.6*col),vec3(.85)),1.);
}
