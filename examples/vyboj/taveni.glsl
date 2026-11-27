// TAVENÍ — Shadertoy Image. iChannel0: Music, drag to orbit.
const float TAU=6.2831853;
const vec2 IMPACT=vec2(.18,-.08);
const float CYCLE=20.57; // Twelve bars at the bundled track's 140 BPM.
const float IMPACT_TIME=3.17;
float sceneTime, crownRise, crownDrain, lift, drawThrough, split, returnFlow;
float ringRadius, ringTube, ridgeHeight, ringTilt, squeeze;
mat2 ringFrame, ringHeading;
float heat, bass, treble, phase, release, fall;
mat2 turn(float a){float c=cos(a),s=sin(a);return mat2(c,s,-s,c);}
float fftAt(float bin){return texture(iChannel0,vec2((bin+.5)/512.,.25)).r;}
float joinSurfaces(float a,float b,float k){
    float h=max(k-abs(a-b),0.)/k;
    return min(a,b)-.25*k*h*h;
}
float shard(vec3 p,vec3 size,float soften){
    vec3 q=p/size;
    float cut=(dot(abs(q),vec3(1.))-1.)*.57735;
    return mix(cut,length(q)-.89,soften)*min(size.x,min(size.y,size.z));
}
float crystal(vec3 p){
    vec3 q=p-vec3(0.,2.48,0.);
    q.xz=turn(.24*iTime+.28*q.y*heat)*q.xz;
    q.x+=heat*.15*sin(3.*q.y-iTime*2.);
    vec3 core=q;
    core.xy=turn(-.25+.36*heat)*core.xy;
    float d=shard(core,vec3(1.02,1.45,.95),heat);
    for(int i=0;i<3;i++){
        float k=float(i);
        float angle=TAU*k/3.+.31;
        vec3 lobe=q;
        lobe.xz=turn(angle)*lobe.xz;
        lobe-=vec3(1.08+.16*sin(1.3*iTime+k)-.24*heat,
                   .28+.43*sin(.97*iTime+2.1*k),0.);
        lobe.xy=turn(.33+.33*sin(k*2.+iTime*.8))*lobe.xy;
        float piece=shard(lobe,vec3(.55+.08*k,1.06-.08*k,.57),heat);
        d=joinSurfaces(d,piece,.09+.62*heat);
    }
    // A pendant grows out of the underside, thins, then releases its weight.
    vec3 stem=p-vec3(IMPACT.x,1.64-.20*release,IMPACT.y);
    float neck=shard(stem,vec3(.10+.27*release,.26+.69*release,.11+.25*release),1.);
    d=joinSurfaces(d,neck+.40*(1.-release),.22);
    vec3 drop=p-vec3(IMPACT.x,mix(1.15,-.37,fall*fall),IMPACT.y);
    float bulb=shard(drop,vec3(.32,.40-.10*fall,.32),1.);
    float visible=smoothstep(.26,.50,phase)*(1.-smoothstep(.73,.83,phase));
    return joinSurfaces(d,bulb+.49*(1.-visible),.18*release+.028);
}

// A scalloped sheet rises from the first impact. Its thick upper lip is
// also the centreline of the ring, so detachment follows the same geometry.
float sheet(vec3 p,float radial,float scallop){
    float top=ridgeHeight*scallop;
    float y=clamp(p.y,0.,top);
    float u=y/max(.015,top);
    float radius=ringRadius-.42*(1.-u*u);
    float wall=abs(radial-radius)-mix(.105,.047,u);
    return max(wall*.56,max(-p.y,p.y-top))+.27*crownDrain;
}
vec3 ringPoint(vec3 p){
    p-=vec3(IMPACT.x,ridgeHeight+2.05*lift,IMPACT.y);
    p.xz=ringHeading*p.xz;
    p.yz=ringFrame*p.yz;
    return p;
}
float field(vec3 p){
    if(sceneTime<IMPACT_TIME || sceneTime>19.7) return crystal(p);
    vec3 q=ringPoint(p);
    float angle=atan(q.z,q.x+.00001);
    float radius=length(q.xz);
    float petal=.5+.5*cos(7.*angle);
    float scallop=.71+.29*petal*petal;
    float arch=(scallop-1.)*ridgeHeight*(1.-lift);
    vec2 cord=vec2(radius-ringRadius,q.y-arch);
    // The metal ring unlays into two strands around one closed path.
    float twist=3.*angle-1.65*iTime;
    vec2 braid=vec2(cos(twist),sin(twist))*split*(.24+.09*bass);
    float d=min(length(cord-braid),length(cord+braid))-ringTube;
    if(crownDrain<1. && ridgeHeight>0.) d=joinSurfaces(d,sheet(p,radius,scallop),.09*(1.-crownDrain)+.006);
    if(drawThrough<1. || returnFlow>0.){
        // Draw the core into a filament along the ring normal, through its
        // centre. On release it rounds out on the opposite side.
        vec3 core=p;
        if(squeeze>.001){
            vec3 along=vec3(0.,cos(ringTilt),sin(ringTilt));
            along.xz=transpose(ringHeading)*along.xz;
            vec3 pivot=vec3(0.,2.48,0.);
            vec3 relative=p-pivot-along*(.54*sin(TAU*drawThrough)*squeeze);
            vec3 axial=along*dot(relative,along);
            core=pivot+(relative-axial)*(1.+2.7*squeeze)+axial/(1.+.52*squeeze);
        }
        float remnant=crystal(core)/(1.+2.7*squeeze)+.64*drawThrough*(1.-returnFlow);
        d=joinSurfaces(d,remnant,.10);
    }
    return d;
}

// Restrict all volume work, including the floor reflection, to the sculpture.
float traceForm(vec3 ro,vec3 rd,float limit,int steps){
    vec3 o=ro-vec3(0.,2.05,0.);
    float b=dot(o,rd), disc=b*b-dot(o,o)+10.24;
    if(disc<=0.) return -1.;
    float root=sqrt(disc), t=max(.01,-b-root), end=min(limit,-b+root);
    for(int i=0;i<95;i++){
        if(t>end || i>=steps) break;
        float d=field(ro+rd*t);
        if(d<.005+.00055*t) return t;
        t+=max(.004,.87*d);
    }
    return -1.;
}
vec3 formNormal(vec3 p){
    vec2 e=vec2(.006,-.006);
    return normalize(e.xyy*field(p+e.xyy)+e.yyx*field(p+e.yyx)
                    +e.yxy*field(p+e.yxy)+e.xxx*field(p+e.xxx));
}
// Softbox and narrow neon strips reflected in the flowing metal.
vec3 studio(vec3 r){
    float az=atan(r.z,r.x);
    float ribbon=r.y-.21-.26*sin(2.*az+.28*iTime);
    float white=exp(-14.*abs(ribbon))*(.62+.38*cos(az-.8));
    float rail=exp(-38.*abs(r.y+.17+.18*cos(3.*az-.25*iTime)));
    float soft=pow(max(0.,dot(r,normalize(vec3(-.59,.74,.32)))),8.);
    return vec3(.012,.024,.060)+vec3(.08,.19,.62)*(.5+.5*r.y)
          +vec3(3.5,3.85,4.2)*white+vec3(.09,.87,3.6)*rail+vec3(1.4,1.85,2.8)*soft;
}
vec3 metal(vec3 p,vec3 rd){
    vec3 n=formNormal(p);
    float rim=pow(1.-max(dot(n,-rd),0.),3.);
    float flow=sin(5.*p.y+.75*sin(4.*p.x+iTime)-2.7*iTime);
    float vein=exp(-10.*abs(flow))*heat*(.40+.6*treble);
    return studio(reflect(rd,n))*(.81+.35*rim)+vec3(.021,.13,.42)*(.7+.6*n.y)
          +vec3(.025,.32,.92)*(rim+.34*vein);
}
vec2 wavePacket(float r,float age,float speed){
    if(age<0.) return vec2(0.);
    float x=r-speed*age;
    return vec2(sin(11.*x),cos(11.*x))*exp(-2.05*x*x-.43*age)*smoothstep(0.,.13,age);
}
vec2 ripples(float r){
    return wavePacket(r,sceneTime-IMPACT_TIME,2.55)
          +.65*wavePacket(r,sceneTime-5.45,2.95)
          +.83*wavePacket(r,sceneTime-16.4,2.2);
}
void mainImage(out vec4 fragColor,in vec2 fragCoord){
    bass=.62*fftAt(2.)+.38*fftAt(4.);
    treble=.53*fftAt(74.)+.47*fftAt(121.);
    sceneTime=mod(iTime,CYCLE);
    phase=min(sceneTime/4.5,1.);
    crownRise=smoothstep(IMPACT_TIME,4.4,sceneTime);
    crownDrain=smoothstep(4.85,6.,sceneTime);
    lift=smoothstep(4.8,8.1,sceneTime)*(1.-smoothstep(14.8,17.8,sceneTime));
    drawThrough=smoothstep(6.8,10.8,sceneTime);
    split=smoothstep(8.5,11.4,sceneTime)*(1.-smoothstep(15.,17.45,sceneTime));
    returnFlow=smoothstep(16.35,19.15,sceneTime);
    squeeze=drawThrough*(1.-returnFlow);
    float sink=smoothstep(16.15,19.,sceneTime);
    ringRadius=(.32+1.06*crownRise+.25*lift)*(1.-sink);
    ridgeHeight=mix(-.26,.96,crownRise)-.93*crownDrain-.28*sink;
    ringTube=(.15+.075*lift-.07*split)*(1.-sink)+.008;
    ringTilt=lift*(1.30+.19*sin(.82*iTime));
    ringFrame=turn(-ringTilt);
    ringHeading=turn(lift*(.42*iTime+.32*sin(iTime*.53)));
    heat=smoothstep(.06,.5,phase)*(1.-smoothstep(.74,.99,phase));
    heat=max(heat,smoothstep(5.,8.,sceneTime)*(1.-returnFlow));
    heat=clamp(heat*(.88+.22*bass),0.,1.);
    release=smoothstep(.17,.53,phase)*(1.-smoothstep(.56,.69,phase));
    fall=clamp((phase-.54)/.19,0.,1.);
    float t=iTime, drive=bass;
    vec2 steer=vec2(0.);
#ifdef VYBOJ_CAMERA_INPUT
    t=uOrbit.z; drive=uOrbit.w; steer=uOrbit.xy;
#else
    if(abs(iMouse.z)+abs(iMouse.w)>0.)
        steer=(iMouse.xy-abs(iMouse.zw))/iResolution.xy*vec2(-TAU,1.7);
#endif
    float yaw=.45*t+.14*sin(.6*t)+steer.x;
    float rise=clamp(.40+.30*sin(.48*t-.82)+steer.y,0.,1.);
    float radius=(6.6+.32*cos(.38*t)-.52*drive)*max(1.,.90*iResolution.y/iResolution.x);
    float pitch=mix(asin((.55-1.92)/radius),1.37,rise);
    vec3 target=vec3(0.,1.92,0.);
    vec3 ro=target+radius*vec3(cos(pitch)*sin(yaw),sin(pitch),cos(pitch)*cos(yaw));
#ifdef VESPER_MELT_EXIT
    vec3 entry=vec3(0.,2.48,0.);
    target=mix(target,entry,uTravel);
    ro=mix(ro,entry,.995*uTravel);
#endif
    vec3 f=normalize(target-ro), right=normalize(cross(f,vec3(0,1,0)));
    vec2 uv=(2.*fragCoord-iResolution.xy)/iResolution.y;
    vec3 rd=normalize(2.03*f+right*uv.x+cross(right,f)*uv.y);
#ifdef VESPER_MELT_EXIT
    // Project the opening onto the crystal as the camera approaches it.
    // Its view fills the viewport continuously before the next native Image takes over.
    float cover=0.;
    vec3 through=vec3(0.);
    if(uTravel>0.){
        vec3 front=entry-ro;
        float depth=max(.025,dot(front,f));
        vec2 center=2.03*vec2(dot(front,right),dot(front,cross(right,f)))/depth;
        vec2 offset=uv-center;
        float aperture=.94*uTravel/depth;
        cover=1.-smoothstep(aperture-.016,aperture+.016,length(offset));
        vec2 nextUV=(fragCoord-.5*center*iResolution.y)/iResolution.xy;
        through=texture(nextView,clamp(nextUV,0.,1.)).rgb;
        if(cover>=1.){fragColor=vec4(through,1.);return;}
    }
#endif
    vec3 sky=vec3(.009,.014,.035)+vec3(.032,.058,.12)*exp(-7.*abs(rd.y));
    vec3 col=sky;
    float floorT=rd.y<-.002 ? -ro.y/rd.y : 1.e4;
    float hit=traceForm(ro,rd,floorT,95);
    if(hit>0.) col=metal(ro+rd*hit,rd);
    else if(floorT<100.){
        vec3 p=ro+floorT*rd;
        vec2 q=p.xz-IMPACT;
        float r=length(q);
        vec2 wave=ripples(r);
        vec2 slope=q/max(.035,r)*wave.y*.33+.014*vec2(sin(p.x*2.+iTime),cos(p.z*3.1-iTime));
        vec3 bounce=reflect(rd,normalize(vec3(-slope.x,1.,-slope.y)));
        vec3 floorCol=vec3(.017,.05,.12)+.17*studio(bounce);
        float reflection= r<7. && bounce.y>0. ? traceForm(p+.012*bounce,bounce,13.,65) : -1.;
        if(reflection>0.) floorCol=mix(floorCol,metal(p+(.012+reflection)*bounce,bounce),.42);
        floorCol+=vec3(.16,.59,1.5)*max(wave.x,0.)*(.64+.40*bass);
        floorCol+=vec3(.16,.40,.80)*exp(-.69*r*r);
        col=mix(sky,floorCol,exp(-.0018*floorT*floorT));
    }
    col*=1.-.13*smoothstep(.45,1.65,length(uv));
    fragColor=vec4(pow(1.-exp(-1.39*max(col,0.)),vec3(.84)),1.);
#ifdef VESPER_MELT_EXIT
    fragColor.rgb=mix(fragColor.rgb,through,cover);
#endif
}
