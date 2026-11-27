// VÝBOJ / INVERSION — Shadertoy Image. Optional iChannel0: Music.
// iChannel1: lastura.png (data texture, nearest, VFlip off).
// Source: SharedTestModels/seashell.ply, CC0.
// World units: y=0 is the floor. The funnel has its widest rim at the floor.
const float H = 5.7;
// World-space ceiling and shaft, shared by the vortex tip and reflections.
const float LIP_Y=5.32, APERTURE=.33, BORE=.86, SHAFT_TOP=10.4;
const vec2 MOUTH=vec2(.83,-.36);

const float TAU = 6.2831853;
const vec3 STEEL = vec3(.42,.70,.83);
const vec3 COPPER = vec3(1.16,.34,.059);
vec3 arcLamp;
float arcPower;
#ifdef VYBOJ_VORTEX_ONLY
float rimAge, rimFlash, rope, liftFront;
#ifdef VYBOJ_CAMERA_INPUT
uniform highp uint uEncounterSeed;
#endif
float cueState(int cell) {
    uvec4 b=uvec4(texelFetch(iChannel1,ivec2(cell,0),0)*255.+.5);
    return uintBitsToFloat((b.r<<24)|(b.g<<16)|(b.b<<8)|b.a);
}
bool crystalEncounter(float tripIndex) {
    uint x=uint(tripIndex);
#ifdef VYBOJ_CAMERA_INPUT
    x+=uEncounterSeed;
#endif
    // An invertible integer mix gives either branch half the possible seeds.
    x^=x>>16u; x*=0x7feb352du;
    x^=x>>15u; x*=0x846ca68bu;
    x^=x>>16u;
    return (x&1u)==0u;
}
#endif
const float BREAK_TIME=13.714286; // Eight bars of the bundled 140 BPM track.
float breakAge;
vec3 music; // kick, body, air
float exposurePx;
vec2 ruptureAxis;
float ruptureLoad, skyStrikeAge, skyStrikeSeed;

float rimPull(vec2 dir) {
    float alignment=max(0.,dot(dir,ruptureAxis));
    return ruptureLoad*alignment*alignment*alignment;
}
float openingRadius(vec2 p) {
    return APERTURE+.29*rimPull(p*inversesqrt(max(dot(p,p),.00001)));
}

float hash(float x) { return fract(sin(mod(x,8192.)*12.9898)*4375.5453); }
float fftAt(float bin) { return texture(iChannel0,vec2((bin+.5)/512.,.25)).r; }
vec2 axisAtTime(float h,float t,float body) {
    vec2 bend=vec2(sin(5.3*h-.72*t),cos(4.6*h+.61*t));
    vec2 freeCenter=h*(vec2(.83,-.36)+(.34+.47*h+.23*body)*bend)
        + .11*h*sin(12.*h-1.2*t)*vec2(1.,-.5);
#ifdef VYBOJ_VORTEX_ONLY
    // A weakening funnel wanders furthest at its free end.
    freeCenter+=rope*(1.-h)*vec2(.75*sin(8.*h-1.8*t),.56*cos(10.*h-2.1*t));
#endif
    return mix(freeCenter,MOUTH+.16*ruptureLoad*ruptureAxis,smoothstep(.72,LIP_Y/H,h));
}
vec2 axisAt(float h) { return axisAtTime(h,iTime,music.y); }
float rimAtTime(float h,float t,float bass,float body) {
    float s=max(0.,1.-h);
    float radius=.014+s*(.23+1.94*s+.34*exp(-19.*h))*(1.+.12*bass)
        + .15*s*sin(12.*h-2.3*t+.4*body);
#ifdef VYBOJ_VORTEX_ONLY
    radius*=mix(1.,.20+.36*h,rope);
#endif
    return radius;
}

float rim(float h) { return rimAtTime(h,iTime,music.x,music.y); }

// Continuous 3D value noise. The four adjacent lattice edges are evaluated
// together; the same field is sampled for density and for light extinction.
vec4 lattice(vec4 n) {
    n=fract(n*.1031);
    n*=n+33.33;
    return fract(n*(n+n));
}
float noise3(vec3 p) {
    vec3 cell=floor(p), f=fract(p);
    f=f*f*(3.-2.*f);
    float id=dot(cell,vec3(1.,37.,109.));
    vec4 corners=id+vec4(0.,37.,109.,146.);
    vec4 x=mix(lattice(corners),lattice(corners+1.),f.x);
    return mix(mix(x.x,x.y,f.y),mix(x.z,x.w,f.y),f.z);
}
mat2 rotate2(float a) { float c=cos(a),s=sin(a); return mat2(c,s,-s,c); }

// Return signed depth in the cloud, the moisture/ash variation and local heat.
// The outer boundary shares the rolling field inside the body, so clumps
// change the actual silhouette and can completely hide the opposite side.
vec3 cloud(vec3 p,bool detail) {
    float h=clamp(p.y/H,0.,1.);
    vec2 q=p.xz-axisAt(h);
    float rad=length(q), radius=rim(h);
    float size=.14+.27*radius;
    float sweep=4.4*h+3.3*h*h-iTime*1.24;
    vec2 spun=rotate2(sweep)*q;
#ifdef VESPER_SOLO_STORM
    const float climb=1.05;
#else
    const float climb=.91;
#endif
    vec3 v=vec3(spun.x/size,1.8*p.y-climb*iTime,spun.y/size);
    // Domain warping rolls the larger eddies over one another.
    v+=.29*sin(v.yzx*1.4+.63*sin(v.zxy*1.17)+vec3(0.,2.5,4.1));
    float big=noise3(v*.92+vec3(9.,24.,7.));
    vec3 fine=vec3(v.z+.48*v.y,v.x-.36*v.z,v.y+.29*v.x);
    float mid=noise3(2.12*fine+vec3(19.,6.,27.));
    float grain=detail ? noise3(v*4.62+vec3(7.,13.,5.)) : .5;
    float clouds=.54*big+.30*mid+.16*grain;
    float angle=atan(spun.y,spun.x+.00001);
    float lobes=.11*cos(2.*angle+.72*sin(1.8*p.y))+.065*sin(3.*angle+3.*h);
    float depth=radius*(.98+lobes+1.30*(clouds-.5))-rad;
#ifdef VYBOJ_VORTEX_ONLY
    if(rope>0.){
        // Spiral crests outlive the air between them. The ragged lower ends
        // lift at different heights instead of retreating as a flat slice.
        float thread=.5+.5*cos(2.*angle-13.*h+3.4*big+1.6*mid+1.85*iTime);
        depth-=rope*radius*(1.-thread)*(.77+.30*mid);
        float tail=liftFront-rope*(.21*thread+.10*(big-.5));
        depth=min(depth,(h-tail)*H*.65);
    }
#endif
    depth=min(depth,(H-p.y)*.82);
    float pocket=.5+.5*cos(2.*angle-3.9*h+2.8*big+2.*mid+.33*iTime);
    float heat=.47*pocket+.35*(1.-mid)+.18*grain;
    return vec3(depth,clouds,heat);
}

// Paired ion pockets ride in the rotating cloud. Their light is integrated
// through the normal smoke density, including on reflected floor rays.
vec4 plasma(vec2 q,float h,float radius,vec3 material) {
    float radial=length(q)/max(radius,.035);
    if(radial<=.46 || radial>=1.72) return vec4(0.);
    float angle=atan(q.y,q.x+.00001);
    vec2 uv=vec2(3.*(angle/TAU)+1.85*h-.21*iTime,
                 3.3*h-.19*iTime+.09*sin(2.*angle+2.8*h));
    vec2 cell=floor(uv), local=fract(uv)-.5;
    float seed=hash(dot(vec2(mod(cell.x,3.),cell.y),vec2(17.,67.)));
    local=rotate2((seed-.5)*1.76)*local;
    local.y+=(material.y-.5)*.17;
    vec2 a=vec2(-.20,-.065), b=vec2(.18,.075);
    vec2 width=vec2(1.,1.12);
    vec2 da=(local-a)*width, db=(local-b)*width;
    float life=.22+.78*smoothstep(-.16,.84,sin(1.32*iTime+seed*TAU));
    float skin=smoothstep(.46,.82,radial)*(1.-smoothstep(1.12,1.72,radial));
    float blue=exp(-dot(da,da)*54.)*(.70+.90*music.x);
    float pink=exp(-dot(db,db)*62.)*(.63+1.08*music.z);
    float u=clamp(dot(local-a,b-a)/dot(b-a,b-a),0.,1.);
    vec2 path=mix(a,b,u);
    path.y+=sin(3.14159*u)*(.037+.048*sin(8.*u+iTime*2.2+TAU*seed));
    float bridge=exp(-dot(local-path,local-path)*560.)*sin(3.14159*u)
                 *smoothstep(.25,.82,music.y+.24*life);
    float weight=skin*life;
    vec3 color=blue*vec3(.018,.22,2.8)+pink*vec3(2.55,.018,.58)+bridge*vec3(.96,.21,1.65);
    return vec4(color*weight,clamp(max(blue,pink)*weight,0.,1.));
}

vec4 storm(vec3 ro,vec3 rd,float limit,bool mirror,vec2 marks,out vec2 visibility) {
    visibility=vec2(1.);
    float dd=dot(rd.xz,rd.xz), od=dot(ro.xz,rd.xz);
    float det=od*od-dd*(dot(ro.xz,ro.xz)-18.5);
    if(det<=0. || dd<1.e-6) return vec4(0.);
    float root=sqrt(det);
    float t=max(.001,(-od-root)/dd), end=min(limit,(-od+root)/dd);
    float dy=abs(rd.y)<1.e-6 ? (rd.y<0.?-1.e-6:1.e-6) : rd.y;
    vec2 caps=(vec2(.008,H)-ro.y)/dy;
    t=max(t,min(caps.x,caps.y)); end=min(end,max(caps.x,caps.y));
    if(end<=t) return vec4(0.);
    float hMin=clamp(min(ro.y+t*rd.y,ro.y+end*rd.y)/H,0.,1.);
    float hMax=clamp(max(ro.y+t*rd.y,ro.y+end*rd.y)/H,0.,1.);
    float s=1.-hMin;
    float widest=.014+s*((.23+1.94*s+.34*exp(-19.*hMin))*(1.+.12*music.x)+.15);
    float axisRange=hMax*(.91+1.415*(.34+.47*hMax+.23*music.y)+.124);
    // The upper axis blends toward the mouth, whose radius is below .91.
    if(hMax>.72) axisRange=max(axisRange,.91+.16*ruptureLoad);
#ifdef VYBOJ_VORTEX_ONLY
    axisRange+=.94*rope*(1.-hMin);
#endif
    float closest=clamp(-od/max(dd,1.e-6),t,end);
    if(length(ro.xz+closest*rd.xz)>axisRange+1.74*widest+.10) return vec4(0.);
    vec3 color=vec3(0.); float clear=1.;
    vec3 sun=vec3(-.68,.58,.45);
    for(int i=0;i<94;i++) {
        if(t>=end || clear<(mirror?.035:.012)) break;
        vec3 p=ro+rd*t;
        float h=clamp(p.y/H,0.,1.), radius=rim(h);
        vec2 center=axisAt(h);
        float outer=length(p.xz-center)-(1.74*radius+.038);
        if(outer>.06) { t+=clamp(outer*.55,.055,.68); continue; }
        float stride=(mirror?.145:.060)*(.57+.43*(1.-h));
        stride=min(stride,end-t);
        p+=.5*stride*rd;
        vec3 sampleCloud=cloud(p,!mirror);
        float depth=sampleCloud.x;
        float edge=.016+.042*radius;
        float density=smoothstep(-edge,edge*2.8,depth);
        if(breakAge>0.) {
            // Loss of suction travels from the broken tip down to the skirt.
            float arrival=.26+1.85*(1.-h)+.46*sampleCloud.y;
            density*=1.-smoothstep(arrival,arrival+.85,breakAge);
        }
        if(density>.001) {
            float lightStep=.14+.12*radius;
            float blocker=cloud(p+sun*lightStep,false).x;
            float gradient=clamp(.36+.78*(depth-blocker)/lightStep,.02,1.);
            float sunlight=gradient*exp(-5.2*max(blocker,0.));
            float furnace=smoothstep(.45,.79,sampleCloud.z);
            vec3 ash=mix(vec3(.011,.021,.028),vec3(.085,.104,.114),sampleCloud.y);
            vec3 lit=ash*(.52+.94*sunlight);
            // The existing light-depth probe picks out exposed folds while
            // neighbouring layers cast shadows into the creases.
            float crest=smoothstep(.29,.96,sunlight)*(.34+.66*sampleCloud.y);
            lit+=vec3(.59,.63,.67)*crest*(1.-.64*furnace);
            vec3 fire=mix(vec3(1.51,.12,.009),vec3(2.98,.82,.065),furnace*furnace);
            lit+=fire*furnace*furnace*(.71+.85*music.x);
            lit+=vec3(.050,.085,.095)*(1.-gradient)*sampleCloud.y;
            vec4 ions=plasma(p.xz-center,h,radius,sampleCloud);
            lit=lit*(1.-.74*ions.a)+ions.rgb;
            // The white pool below the cloud lights the underside of low eddies.
            float underlight=exp(-2.05*p.y)*(.47+.57*music.x);
            lit+=vec3(1.00,.97,.94)*underlight*(.42+.58*(1.-sunlight));
            vec3 toArc=p-arcLamp;
            float flash=.83*arcPower/(1.+3.6*dot(toArc,toArc));
            lit+=vec3(.86,1.02,1.09)*flash*(.36+.64*gradient);
            float alpha=1.-exp(-density*stride*(6.8+1.4*sampleCloud.y));
            color+=clear*alpha*lit;
            visibility*=mix(vec2(1.),vec2(1.-alpha),step(vec2(t+.5*stride),marks));
            clear*=1.-alpha;
        }
        t+=stride;
        // The field is smooth outside the envelope; taking longer empty steps
        // reserves the finer samples for the changing silhouette.
        if(depth < -2.*edge) t+=min(.28,(-depth-edge)*.36);
    }
    return vec4(color,1.-clear);
}

// A weak leader descends first; the bright return stroke and its afterstrokes
// reuse the same world-space channel for the lifetime of this event.
const int STRIKES=4;
struct Discharge {
    vec3 top,foot;
    float seed,level,leader,branches;
};
Discharge strikes[STRIKES];
void stormLamp() {
    arcPower=0.; arcLamp=vec3(0.);
#ifdef VYBOJ_VORTEX_ONLY
    // Reserve one channel every five ticks for the ceiling discharge.
    // The early leader, return strokes and wide camera share this clock.
    float rimTick=5.*floor((iTime*.92)/5.);
    rimAge=iTime-(rimTick+.18*hash(rimTick+28.))/.92;
    float stroke=max(0.,rimAge-.47);
    rimFlash=step(.47,rimAge)*exp(-13.*stroke)
            +step(.64,rimAge)*.64*exp(-17.*max(0.,rimAge-.64));
#endif
    for(int i=0;i<STRIKES;i++) {
        float id=float(i), rate=.92+.08*id, clock=iTime*rate+id*9.31;
        float tick=floor(clock), seed=tick+id*193.;
        float delay=.18*hash(seed+28.);
        float start=(tick-id*9.31+delay)/rate;
        float age=iTime-start;
        float lead=clamp(age/.096,0.,1.);
        float after=age-.096;
        float again=.066+.076*hash(seed+38.);
        float first=after>=0. ? exp(-after*25.) : 0.;
        float second=after>=again ? .78*exp(-34.*(after-again)) : 0.;
        float last=after>=again+.105 ? .36*exp(-38.*(after-again-.105)) : 0.;
        float charge=.52+1.75*music.z+.56*music.x;
        float level=charge*(first*1.65+second+last+.15*lead*(1.-smoothstep(.09,.14,age)));
#ifdef VYBOJ_VORTEX_ONLY
        bool ceilingStrike=i==0 && tick==rimTick;
        if(ceilingStrike){
            lead=smoothstep(.02,.38,age);
            level=charge*(1.8*rimFlash+.20*lead*(1.-smoothstep(.46,.72,age)));
            first=rimFlash;
        }
#endif
        strikes[i].level=level;
        if(level<.018 && level<=arcPower) continue;
        float a=TAU*hash(seed+.42), h=.26+.60*hash(seed+1.7);
        vec2 root=axisAtTime(h,start,.5)+rimAtTime(h,start,.5,.5)*vec2(cos(a),sin(a));
#ifdef VYBOJ_VORTEX_ONLY
        if(!ceilingStrike){
            level*=1.-smoothstep(h-.08,h+.13,liftFront);
            strikes[i].level=level;
            if(level<.018 && level<=arcPower) continue;
        }
#endif
        float b=a+.50*(hash(seed+3.)-.5), reach=2.25+1.65*hash(seed+7.7);
        Discharge bolt;
        bolt.top=vec3(root.x,h*H,root.y);
#ifdef VYBOJ_VORTEX_ONLY
        if(ceilingStrike){
            vec2 contact=MOUTH+ruptureAxis*(APERTURE+.29*ruptureLoad);
            bolt.top=vec3(contact.x,LIP_Y-.15-.23*ruptureLoad,contact.y);
            reach=4.05;
        }
#endif
        bolt.foot=vec3(cos(b)*reach,.012,sin(b)*reach);
        bolt.seed=seed; bolt.level=level; bolt.leader=lead;
        bolt.branches=.24+.76*max(first,1.-lead);
        strikes[i]=bolt;
        if(level>arcPower){arcLamp=bolt.top;arcPower=level;}
    }
}

// Closest approach of the viewing ray to a finite 3D segment.
vec2 wire(vec3 ro,vec3 ray,vec3 a,vec3 b) {
    vec3 e=b-a, p=a-ro, ep=e-ray*dot(e,ray), pp=p-ray*dot(p,ray);
    float s=clamp(-dot(pp,ep)/max(dot(ep,ep),1.e-7),0.,1.);
    return vec2(length(pp+s*ep),dot(p+s*e,ray));
}
vec2 knot(float key) {
    vec2 n=fract(vec2(key,key+41.71)*vec2(.1031,.11369));
    n*=n+33.33;
    return 2.*fract(n*(n+n)) - 1.;
}
vec2 channelBend(float u,float segments,float key) {
    float x=u*segments, index=floor(x);
    return mix(knot(key+index),knot(key+index+1.),fract(x));
}
vec3 channel(vec3 a,vec3 b,float u,float seed,float bend) {
    vec3 along=normalize(b-a);
    vec3 left=normalize(cross(along,vec3(.93,.32,.15))), side=cross(along,left);
    vec2 wobble=.59*channelBend(u,4.,seed)+.29*channelBend(u,11.,seed+43.)
               +.12*channelBend(u,29.,seed+97.);
    wobble*=bend*(4.*u*(1.-u));
    return mix(a,b,u)+left*wobble.x+side*wobble.y;
}
vec3 bolts(vec3 ro,vec3 rd,float limit,bool floorVisible,out float strikeDepth) {
    vec3 glow=vec3(0.); float brightest=0.; strikeDepth=0.;
    vec3 flip=vec3(1.,-1.,1.);
    for(int arc=0;arc<STRIKES;arc++) {
        Discharge bolt=strikes[arc];
        if(bolt.level<.018) continue;
        vec3 tint=vec3(.07,.44,1.65);
        if(floorVisible && bolt.leader>=1.) {
            float d=length((ro+limit*rd).xz-bolt.foot.xz);
            glow+=bolt.level*tint*(.87*exp(-7.*d)+.11*exp(-1.8*d));
        }
        float bend=.35+.15*length(bolt.top-bolt.foot);
        float bound=wire(ro,rd,bolt.top,bolt.foot).x;
        if(floorVisible) bound=min(bound,wire(ro,rd,bolt.top*flip,bolt.foot*flip).x);
        if(bound>bend+1.45) continue;
        float best=1.e3, depth=10., branchD=1.e3, branchDepth=10., reflected=1.e3;
        // Bound five consecutive segments before spending work on fine kinks.
        // The low-frequency bend is piecewise linear; its slope and the
        // residual two octaves bound the deviation from the centre chord.
        for(int block=0;block<6;block++) {
            float u0=float(block)/6., u1=min(float(block+1)/6.,bolt.leader);
            if(u0>=u1) break;
            float mid=.5*(u0+u1);
            vec3 dir=normalize(bolt.foot-bolt.top);
            vec3 left=normalize(cross(dir,vec3(.93,.32,.15))), side=cross(dir,left);
            vec2 coarse=.59*channelBend(mid,4.,bolt.seed)*bend*(4.*mid*(1.-mid));
            vec3 shift=left*coarse.x+side*coarse.y;
            vec3 a=mix(bolt.top,bolt.foot,u0)+shift, b=mix(bolt.top,bolt.foot,u1)+shift;
            // Direct bloom beyond .78 units is below one display code value.
            float margin=bend*(.59*1.415*12.*(u1-u0)*.5+.41*1.415);
            bool direct=wire(ro,rd,a,b).x<margin+.78;
            bool mirror=floorVisible && wire(ro,rd,a*flip,b*flip).x<margin+.29;
            if(!direct&&!mirror) continue;
            vec3 prev=channel(bolt.top,bolt.foot,u0,bolt.seed,bend);
            for(int j=1;j<=5;j++) {
                float u=min(float(block*5+j)/30.,bolt.leader);
                vec3 node=channel(bolt.top,bolt.foot,u,bolt.seed,bend);
                if(direct) {
                    vec2 d=wire(ro,rd,prev,node);
                    if(d.y>0.&&d.y<limit&&d.x<best){best=d.x;depth=d.y;}
                }
                if(mirror) {
                    vec2 m=wire(ro,rd,prev*flip,node*flip);
                    if(m.y>=limit) reflected=min(reflected,m.x);
                }
                prev=node;
                if(u>=bolt.leader) break;
            }
        }
        for(int j=0;j<4;j++) {
            float key=bolt.seed+float(j)*19.53;
            float at=.12+(float(j)+.65*hash(key+2.))*.16;
            if(bolt.leader<=at) continue;
            vec3 a=channel(bolt.top,bolt.foot,at,bolt.seed,bend);
            float yaw=TAU*hash(key+4.), size=(.33+.84*hash(key+5.))*(1.-.55*at);
            vec3 end=a+size*vec3(cos(yaw),-.44-.62*hash(key+7.),sin(yaw));
            end.y=max(.035,end.y);
            if(wire(ro,rd,a,end).x>.60*size+.20) continue;
            vec3 last=a;
            float grown=clamp((bolt.leader-at)*3.8,0.,1.);
            for(int k=1;k<=7;k++) {
                float s=min(float(k)/7.,grown);
                vec3 p=channel(a,end,s,key+107.,size*.42);
                vec2 d=wire(ro,rd,last,p);
                float taper=d.x/(1.-.69*s);
                if(d.y>0.&&d.y<limit&&taper<branchD){branchD=taper;branchDepth=d.y;}
                last=p;
                if(s>=grown) break;
            }
        }
        float px=max(.003,depth*exposurePx);
        float white=1.-smoothstep(.002,.008+px,best);
        float fork=(1.-smoothstep(.001,.004+max(.003,branchDepth*exposurePx),branchD))*bolt.branches;
        vec3 direct=bolt.level*(vec3(.90,.96,1.)*(2.8*white+.72*fork)
            +tint*(.95*exp(-43.*best)+.19*exp(-9.*best)+.27*exp(-72.*branchD)*bolt.branches));
        float power=max(direct.r,max(direct.g,direct.b));
        if(power>brightest){brightest=power;strikeDepth=fork>white ? branchDepth : depth;}
        glow+=direct+bolt.level*tint*.26*exp(-32.*reflected);
    }
    return glow;
}

// Short-lived charged fragments travel on expanding, rising orbits.
void sparkOrbit(float id,float seed,float age,float h,out vec3 head,out vec2 tangent){
    float a=TAU*hash(id+1.)+iTime*(1.5+.80*seed)+3.*age;
    float radius=rim(h)+(.23+1.10*hash(id+13.))*(.5+.80*age);
    vec2 outward=vec2(cos(a),sin(a));
    tangent=vec2(-outward.y,outward.x);
    vec2 xz=axisAt(h)+radius*outward;
    head=vec3(xz.x,h*H,xz.y);
}
#ifdef VESPER_WRITE_SPRAY
vec4 sparkTexel(vec2 pixel){
    float id=floor(pixel.x),seed=hash(id+8.4);
    float age=fract(seed+iTime*(.17+.13*seed)),h=.06+.77*age;
    vec3 head;vec2 tangent;
    sparkOrbit(id,seed,age,h,head,tangent);
    return pixel.y<1.?vec4(head,seed):vec4(tangent,age,0.);
}
#endif
// The head is a ray/sphere hit, the tail lies along the orbital tangent.
vec3 spray(vec3 ro,vec3 rd,float limit,out float sparkDepth) {
    vec3 glow=vec3(0.); float brightest=0.; sparkDepth=0.;
    // Reject spark heights before computing their orbit and tail. The margins
    // include the widest glow (.183) and the largest tail drop (.133).
    float endY=ro.y+limit*rd.y;
    float low=min(ro.y,endY)-.184, high=max(ro.y,endY)+.317;
    if(high<.06*H || low>.83*H) return glow;
#ifdef VESPER_SOLO_STORM
    // Enclose all visible heads and tails before testing 40 individual orbits.
    // Restrict the ray to their height slab first, including the glow margins.
    float nearT=0.,farT=limit;
    if(abs(rd.y)>.00001){
        vec2 slab=(vec2(.06*H-.317,.83*H+.184)-ro.y)/rd.y;
        nearT=max(nearT,min(slab.x,slab.y));farT=min(farT,max(slab.x,slab.y));
    }
    if(nearT>farT)return glow;
    float lo=clamp(low/H,.06,.83),hi=clamp(high/H,.06,.83);
    float s=1.-lo;
    float maxRim=.014+s*((.23+1.94*s+.34*exp(-19.*lo))*(1.+.12*music.x)+.15);
    float axisBound=max(hi*(.91+1.415*(.34+.47*hi+.23*music.y)+.124)
                      +.94*rope*s,.91+.16*ruptureLoad);
    float orbitBound=1.33*(.5+.8*(hi-.06)/.77);
    float bound=axisBound+maxRim+orbitBound+.60;
    float closeT=clamp(-dot(ro.xz,rd.xz)/max(dot(rd.xz,rd.xz),1.e-9),nearT,farT);
    vec2 closest=ro.xz+closeT*rd.xz;
    if(dot(closest,closest)>bound*bound)return glow;
#endif
    for(int j=0;j<40;j++) {
        vec3 head;vec2 tangent;
#ifdef VESPER_CACHED_SPRAY
        vec4 orbit=texelFetch(iChannel1,ivec2(j,0),0);
        float seed=orbit.w;
        if(orbit.y<low || orbit.y>high)continue;
        head=orbit.xyz;
#else
        float id=float(j), seed=hash(id+8.4);
        float age=fract(seed+iTime*(.17+.13*seed));
        float h=.06+.77*age;
        if(h*H<low || h*H>high) continue;
        sparkOrbit(id,seed,age,h,head,tangent);
#endif
        vec3 fromEye=head-ro;
        float frontT=dot(fromEye,rd);
        vec3 perpendicular=fromEye-frontT*rd;
        // The full tail and its glow fit within this sphere.
        float reach=1.054*(.08+.23*seed)*(.65+.65*music.y)+.151+.032*seed*seed;
        if(dot(perpendicular,perpendicular)>reach*reach) continue;
#ifdef VESPER_CACHED_SPRAY
        vec4 trail=texelFetch(iChannel1,ivec2(j,1),0);
        tangent=trail.xy;
        float age=trail.z;
#endif
        vec3 tail=head-vec3(tangent.x,.33,tangent.y)*(.08+.23*seed)*(.65+.65*music.y);
        vec2 dst=wire(ro,rd,tail,head);
        float size=.011+.032*seed*seed, px=dst.y*exposurePx;
        if(dst.y<=0.||dst.y>=limit||dst.x>.14+size) continue;
        float d=length(perpendicular);
        float sphere=sqrt(max(0.,size*size-d*d));
        float face=(1.-smoothstep(size*.78,size+px,d))*(.40+.60*sphere/size);
        float tailLight=exp(-dst.x/max(.006,size*.52));
        vec3 tint=mix(vec3(1.38,.25,.026),vec3(1.24,.78,.31),seed);
        vec3 light=(tint*(1.6*face+.71*tailLight+.095*exp(-24.*dst.x))+vec3(.88)*face)
                   *smoothstep(0.,.10,age)*(1.-smoothstep(.72,1.,age))*(.42+1.26*music.z);
        float power=dot(light,vec3(.33));
        if(power>brightest){brightest=power;sparkDepth=dst.y;}
        glow+=light;
    }
    return glow;
}

float rough(vec2 p) {
    return sin(p.x*5.1+p.y*2.3+sin(p.y*7.4))*.48
         +sin(p.y*12.7-p.x*1.4+sin(p.x*9.))*.30;
}
#ifdef VESPER_SOLO_STORM
// Smooth value noise and its two spatial derivatives, from the same four corners.
vec3 seaPatch(vec2 p) {
    vec2 cell=floor(p), f=fract(p), w=f*f*(3.-2.*f);
    float id=dot(cell,vec2(1.,37.));
    vec4 c=lattice(id+vec4(0.,1.,37.,38.));
    vec2 rows=mix(c.xz,c.yw,w.x);
    vec2 gradient=6.*f*(1.-f)*vec2(mix(c.y-c.x,c.w-c.z,w.y),rows.y-rows.x);
    return vec3(mix(rows.x,rows.y,w.y)-.5,gradient);
}
// x is elevation, yz is slope and w is the height of the short breaking crests.
vec4 oceanSurface(vec2 p,float footprint) {
    vec3 group=seaPatch(.32*p+vec2(-.12,.047)*iTime+vec2(7.1,3.2));
    vec3 chop=seaPatch(.83*p+vec2(-.22,-.11)*iTime+vec2(23.6,41.));
    // The swell and crossing chop travel at different speeds. Wave packets
    // bend over the slower field and weaken where their wavelengths meet.
    const vec4 kx=vec4(.86,.47,2.46,4.72), kz=vec4(.38,-1.45,.94,-2.93);
    const vec4 drift=vec4(1.32,1.67,2.19,3.12);
    const vec4 warp=vec4(2.7,-3.4,5.1,7.2), fineWarp=vec4(.16,.36,1.5,2.7);
    vec4 phase=p.x*kx+p.y*kz-iTime*drift+warp*group.x+fineWarp*chop.x;
    vec4 wave=sin(phase);
    vec4 band=1.-smoothstep(vec4(.6),vec4(2.3),footprint*vec4(1.,1.6,2.8,5.8));
    vec4 amplitude=vec4(.13,.062,.035,.014)*band;
    float swell=.85+.50*group.x;
    vec4 profile=wave+.21*(wave*wave-.5);
    vec4 derivative=amplitude*cos(phase)*(1.+.42*wave);
    float elevation=dot(amplitude,profile);
    vec2 slope=vec2(dot(derivative,kx),dot(derivative,kz))
        +dot(derivative,warp)*.32*group.yz+dot(derivative,fineWarp)*.83*chop.yz;
    // Capillary ripples gather on exposed faces, with shorter crests that
    // disappear into the pixel footprint before they can flicker at distance.
    vec2 wind=vec2(.91,.41),crosswind=vec2(-wind.y,wind.x);
    vec2 rippleUV=vec2(dot(p,wind)*3.7,dot(p,crosswind)*1.6);
    vec3 ripple=seaPatch(rippleUV+vec2(-.86,.19)*iTime+vec2(8.2*group.x,2.1*chop.x));
    float rippleHeight=.017*(.62+.65*chop.x)*(1.-smoothstep(.06,.29,footprint));
    vec2 rippleSlope=rippleHeight*(3.7*wind*ripple.y+1.6*crosswind*ripple.z);
    return vec4(swell*elevation+rippleHeight*ripple.x,swell*slope+.16*elevation*group.yz+rippleSlope,
                dot(band.zw*wave.zw,vec2(.64,.36)));
}
#endif
vec3 floorLight(vec3 p,vec3 ro,vec3 rd,float transmission) {
    float dist=length(p-ro);
    float damp=1./(1.+.017*dist*dist);
#ifdef VESPER_SOLO_STORM
    float footprint=dist*exposurePx/max(.06,-rd.y);
    vec4 sea=oceanSurface(p.xz,footprint);
    vec3 normal=normalize(vec3(-sea.y,1.,-sea.z));
    // Intersect the local sloping wave face. Fade the displacement toward the
    // horizon where many crests project into one pixel.
    float incidence=dot(rd,vec3(-sea.y,1.,-sea.z));
    p+=rd*clamp(sea.x/min(-.14,incidence),-.55,.55)*(1.-smoothstep(.12,.7,footprint));
    float dust=clamp(.45+1.6*sea.x+.9*sea.y,0.,1.);
    // Trace the floor illumination along a slice of the moving funnel.
    // The signed cloud depth places the softer falloff outside the skirt.
    const float sliceY=.07;
    float h=sliceY/H;
    float radius=rim(h);
    vec2 local=p.xz-axisAt(h);
    float radial=length(local);
    vec3 skyRay=reflect(rd,normal);
    float fresnel=.035+.70*pow(1.-clamp(-dot(rd,normal),0.,1.),4.);
    float horizon=exp(-4.5*abs(skyRay.y));
    vec3 ground=mix(vec3(.009,.024,.033)*(.82+.38*dust),
                    vec3(.030,.052,.073)+horizon*vec3(.13,.18,.22),fresnel);
    float foam=smoothstep(.10,.19,sea.x)*smoothstep(.38,.87,sea.w);
    ground+=foam*vec3(.085,.12,.14)*damp;
    // A broken reflection of the bright sky around the mouth. Only wave
    // faces pointing towards both the viewer and that patch catch its light.
    vec3 toEye=vec3(MOUTH.x,LIP_Y,MOUTH.y)-p;
    float glint=max(0.,dot(normal,normalize(normalize(toEye)-rd)));
    float stormLight=(.65+.23*music.y+.75*rimFlash)/(1.+.014*dot(toEye,toEye));
    ground+=vec3(.27,.47,.65)*stormLight*(.43*pow(glint,26.)+.70*pow(glint,117.));
    float contact=1.-smoothstep(.025,.30,liftFront);
    if(contact>0. && radial>.22*radius && radial<1.9*radius+2.9){
        vec2 outward=local/radial;
        vec2 wind=vec2(cos(.19*iTime+.7),sin(.19*iTime+.7));
        vec2 crosswind=vec2(-wind.y,wind.x);
        // Follow a fold of the skirt outwards, gently stretching its last
        // sample instead of repeating the entire cloud across the floor.
        float drift=max(0.,radial-1.28*radius);
        vec2 source=p.xz-.82*drift*outward;
        vec3 fold=cloud(vec3(source.x,sliceY,source.y),false);
        float outside=.82*drift-fold.x;
        float width=.045+.032*radius;
        float core=1.-smoothstep(.15*width,1.18*width,abs(outside));
        float shade=.60+.72*fold.y+.30*dust;

        // Grazing light catches patches of drifting mist on the lee side.
        // Use continuous ground coordinates so neighbouring wisps can merge.
        float along=dot(local,wind), across=dot(local,crosswind);
        float mist=noise3(vec3(.58*along-.20*iTime,
                               1.18*across+.42*sin(.9*along-.27*iTime),.12*iTime));
        float mistCover=smoothstep(.24,.81,mist+.30*(fold.y-.5));
        float lee=smoothstep(-.65,.92,dot(wind,outward));
        float spread=.36+mistCover*(.56+1.62*lee)*(.87+.26*music.x);
        float distanceFromEdge=max(outside,0.)/spread+max(-outside,0.)/(2.0*width);
        float haze=exp(-2.25*distanceFromEdge*distanceFromEdge)
                  *(1.-smoothstep(1.2,1.95,distanceFromEdge));
        float edgeLight=core*(.83+.64*music.x)*shade;
        float spill=haze*(.15+.68*mistCover)*(.70+.30*lee);
        float cutoff=1.-smoothstep(1.9*radius+2.2,1.9*radius+2.9,radial);
        ground+=contact*cutoff*(vec3(.76,.96,1.18)*edgeLight+vec3(.38,.66,1.05)*spill);
    }
#else
    float dust=.5+.5*rough(p.xz*.85);
    float r=length(p.xz), a=atan(p.z,p.x);
    float waves=sin(13.*r-5.*iTime+.7*sin(5.*a));
    float ring=pow(.5+.5*waves,9.)*exp(-1.25*abs(r-2.4));
    float cracks=abs(sin(a*11.+r*1.1+.75*sin(r*4.+a*7.)));
    float vein=exp(-32.*cracks)*exp(-1.65*abs(r-2.65));
    vec3 tint=mix(STEEL,COPPER,.5+.5*sin(a+.8*iTime));
    float contact=exp(-1.2*max(r-1.6,0.));
    vec3 ground=vec3(.022,.033,.037)*(.60+.50*dust)*(1.-.65*contact);
    // A broad white pool extends beyond the skirt. Give it a soft falloff
    // across the slab so the floor stays readable from the low camera angle.
    float poolRadius=2.28+.25*music.x;
    float contactLight=1.;
#ifdef VYBOJ_VORTEX_ONLY
    poolRadius*=1.-.66*rope;
    contactLight=1.-.83*smoothstep(.02,.55,liftFront);
#endif
    float outside=max(0.,r-poolRadius);
    float pool=exp(-.72*outside*outside);
    float spill=exp(-.12*r*r);
    vec3 white=vec3(1.,.985,.97);
    ground+=white*((2.28+.85*music.x)*pool+.34*spill)*(.91+.10*dust)*contactLight;
    ground+=tint*exp(-.58*abs(r-1.95))*.15;
    float wake=exp(-7.*abs(r-(2.1+.83*music.x+.16*sin(a*3.+iTime))));
    ground+=tint*wake*(.14+.46*music.x)*(.68+.32*dust);
    ground+=tint*(ring*(.075+.19*music.x)+vein*(.14+.40*music.z))*damp;
#endif
#ifndef VESPER_SOLO_STORM
    // The other scenes use their original, gently rippled floor.
    vec2 slope=.013*vec2(rough(p.xz),rough(p.zx+4.));
    vec3 normal=normalize(vec3(slope.x,1.,slope.y));
#endif
    vec3 bounce=reflect(rd,normal);
    // Skip a second volume march behind an already opaque foreground.
    if(bounce.y>.003 && transmission>.018) {
        vec2 unused; vec4 reflection=storm(p+.003*normal,bounce,25.,true,vec2(0.),unused);
        ground+=reflection.rgb*(.31+.24*pow(1.-max(-rd.y,0.),3.))*damp;
    }
    return ground;
}

#ifndef VYBOJ_VORTEX_ONLY
// Ray/triangle intersection for the three visible flanks of a shaft tooth.
void toothFace(vec3 ray,vec3 a,vec3 b,vec3 c,float material,
               inout float nearest,inout vec3 normal,inout float toothMaterial) {
    vec3 e=b-a, f=c-a, n=cross(e,f);
    float divisor=dot(n,ray);
    if(abs(divisor)<1.e-7) return;
    float t=dot(a,n)/divisor;
    if(t<.001 || t>=nearest) return;
    vec3 side=cross(a,ray);
    vec2 bary=vec2(dot(side,f),-dot(side,e))/divisor;
    if(min(bary.x,bary.y)<-.00001 || bary.x+bary.y>1.00001) return;
    nearest=t; normal=divisor>0. ? -n : n; toothMaterial=material;
}

void shaftTeeth(vec3 ro,vec3 rd,inout float limit,inout vec3 color) {
    if(abs(rd.y)<.0001) return;
    vec3 n=vec3(0.);
    float material=-1.;
    // Visit rings from front to back. Never test teeth beyond the wall or
    // in bands the ray does not enter. There is no marching inside the bore.
    for(int i=0;i<6;i++) {
        int row=rd.y>0. ? i : 5-i;
        float layer=float(row), y=LIP_Y+.35+.78*layer;
        vec2 slab=(vec2(y-.29,y+.29)-ro.y)/rd.y;
        float start=max(.001,min(slab.x,slab.y)), end=min(limit,max(slab.x,slab.y));
        if(start>=limit) break;
        if(end<=start) continue;
        vec2 a=ro.xz+start*rd.xz-MOUTH, b=ro.xz+end*rd.xz-MOUTH;
        if(max(dot(a,a),dot(b,b))<.18) continue;
        vec2 radial=vec2(cos(layer*1.17),sin(layer*1.17));
        const mat2 spin=mat2(.70710678,.70710678,-.70710678,.70710678);
        for(int j=0;j<8;j++) {
            vec2 tangent=vec2(-radial.y,radial.x);
            float lateralA=dot(a,tangent), lateralB=dot(b,tangent);
            if(max(dot(a,radial),dot(b,radial))>.40 &&
               min(lateralA,lateralB)<.34 && max(lateralA,lateralB)>-.34) {
                float id=layer*8.+float(j), seed=hash(id+9.4);
                vec2 root=MOUTH+radial*(BORE+.035)-ro.xz;
                float spread=.18+.11*seed;
                vec3 base=vec3(root.x,y-ro.y,root.y);
                vec3 wide=vec3(tangent.x,0.,tangent.y)*spread;
                vec3 p0=base-wide+vec3(0.,.17,0.);
                vec3 p1=base+wide+vec3(0.,.23,0.);
                vec3 p2=base-vec3(0.,.13,0.);
                vec2 xy=root-(.22+.19*seed)*radial+(.14*seed-.035)*tangent;
                vec3 point=vec3(xy.x,y-ro.y-.235,xy.y);
                toothFace(rd,p0,p1,point,seed,limit,n,material);
                toothFace(rd,p1,p2,point,seed,limit,n,material);
                toothFace(rd,p2,p0,point,seed,limit,n,material);
            }
            radial=spin*radial;
        }
    }
    if(material>=0.) {
        n=normalize(n);
        vec3 key=normalize(vec3(-.55,-.66,.52));
        float diffuse=.47+.53*dot(n,key);
        float rimLight=pow(1.-max(0.,dot(n,-rd)),2.);
        float glint=pow(max(dot(reflect(rd,n),key),0.),12.);
        vec3 alloy=mix(vec3(.19,.31,.40),vec3(.51,.23,.115),material);
        vec3 edge=mix(vec3(.15,.70,.94),vec3(1.18,.30,.10),material);
        color=alloy*(.32+.84*diffuse)+edge*(.17+.60*music.y)*rimLight
             +vec3(1.,.84,.66)*glint*(.85+.42*music.z);
    }
}

#endif

// Rotating patches and short, branching creepage discharges on the metal lip.
// Run only on visible ceiling hits near the opening.
vec3 chargedLip(vec2 p,float r,float footprint) {
    float a=atan(p.y,p.x+.00001), d=r-openingRadius(p);
    float spin=a-.96*iTime;
    float pulse=.5+.5*cos(3.*spin+.46*sin(4.*a+iTime));
    float braid=.105+.040*sin(5.*a+2.7*iTime)+.015*sin(15.*a-3.6*iTime);
    float width=.012+min(.016,footprint*.45);
    float mainArc=exp(-abs(d-braid)/width)*smoothstep(.22,.91,pulse);
    float forkLine=braid+.094*sin(4.*spin+.68);
    float fork=exp(-abs(d-forkLine)/width)*pow(pulse,5.);
    float halo=exp(-6.*max(d,0.));
    vec3 tint=mix(vec3(.046,.38,1.35),vec3(.62,.89,1.60),
                  .5+.5*sin(2.*a+1.6*iTime));
    float heat=(.52+.80*music.z)*(.45+.55*pulse);
    return tint*heat*(.25*halo+1.14*mainArc+.56*fork)
         +vec3(.68,.83,.99)*exp(-52.*abs(d))*(.43+.74*music.x);
}

// Slowly spreading ionized cloud on the underside of the vault. Its outer
// wisps recede into the lip during a strike, then refill the space around it.
vec3 vaultCorona(vec2 p,float r){
    float discharge=smoothstep(-.31,.10,skyStrikeAge)*(1.-smoothstep(.38,2.25,skyStrikeAge));
    float breath=.5+.5*sin(.88*iTime+.26*sin(.35*iTime));
    float reach=mix(3.15+.48*breath,1.52,discharge);
    float u=r/reach;
    if(u>=1.12)return vec3(0.);
    float angle=atan(p.y,p.x+.00001);
    float turn=angle+2.65*u-.34*iTime;
    float swirl=sin(3.*turn+.61*sin(2.*angle+5.1*u+.18*iTime));
    float edge=.72+.13*swirl+.07*sin(5.*turn-3.*u);
    float veil=(1.-smoothstep(edge-.24,edge+.16,u))*smoothstep(.23,.87,r);
    float fold=abs(u-edge+.19+.11*sin(2.*turn+.43*iTime));
    float ribbon=exp(-10.5*fold)*(.40+.60*smoothstep(-.86,.83,swirl));
    return vec3(.19,.62,1.22)*(veil*(.25+.12*swirl)+.49*ribbon)
          *(.78+.22*breath+.16*music.y)*(1.-smoothstep(.94,1.12,u));
}

// The ceiling splits into hinged plates. Intersect the sagging undersides so
// their silhouette, shadows in the gaps and overlap survive a grazing view.
float rimStress(float phase) {
    return smoothstep(.08,.73,phase)*(1.-smoothstep(.80,1.,phase));
}

void brokenRim(vec3 ro,vec3 rd,inout float limit,inout vec3 color) {
    if(rd.y<=.0001) return;
    vec2 slab=(vec2(LIP_Y-.84,LIP_Y)-ro.y)/rd.y;
    float first=max(.001,slab.x), last=min(limit,slab.y);
    if(last<=first) return;
    vec2 origin=ro.xz-MOUTH;
    float closest=clamp(-dot(origin,rd.xz)/max(dot(rd.xz,rd.xz),.00001),first,last);
    vec2 miss=origin+closest*rd.xz;
    if(dot(miss,miss)>1.32*1.32) return;

    vec2 axis=vec2(1.,0.);
    const mat2 stepAngle=mat2(.76604444,.64278761,-.64278761,.76604444);
    for(int i=0;i<9;i++,axis=stepAngle*axis){
        float seed=hash(float(i)+23.);
        if(breakAge>=.80*seed) continue;
        float phase=fract(iTime*.21+.22*seed+.10*axis.x);
        float stress=max(rimStress(phase),smoothstep(-1.5,0.,breakAge));
        float hinge=.83+.23*seed;
        float pull=rimPull(axis);
        float slope=.032+stress*(.19+.21*seed+.11*music.x)+.88*pull;
        float divisor=rd.y-slope*dot(rd.xz,axis);
        if(abs(divisor)<.0001) continue;
        float t=(LIP_Y-ro.y+slope*(dot(origin,axis)-hinge))/divisor;
        if(t<first || t>last || t>=limit) continue;
        vec2 p=origin+t*rd.xz;
        float x=dot(p,axis), y=dot(p,vec2(-axis.y,axis.x));
        float side=.364*x-abs(y+.028*sin(15.*x+seed*8.)*(1.1-x));
        float end=APERTURE+.29*pull-.035*seed-.050*stress+.10*y+.016*sin(y*24.+seed);
        if(x<end || x>hinge || side<.004+.018*stress+.037*pull) continue;
        float seam=side-.004-.018*stress-.037*pull;
        float edge=exp(-max(seam,0.)/(.014+min(.017,t*exposurePx)));
        float face=(.5+.5*dot(axis,vec2(-.8,.6)))*(.16+.58*slope);
        float innerEdge=exp(-18.*(x-end));
        float light=.052+face+.12*seed+innerEdge*(.50+.66*stress)
                    +edge*(.38+stress*(1.65+.85*music.z))*(1.-smoothstep(.65,hinge,x));
        // The outer end is still embedded in the slab. Blend its finish into
        // the unbroken ceiling and expose the fresh silver along the inner lip.
        float loose=1.-smoothstep(end+.04,hinge,x);
        float grain=.5+.5*sin(26.*x+seed*18.+2.5*sin(13.*y));
        float ceilingShade=.035+.040*exp(-.71*(hinge-APERTURE));
        color=vec3(mix(ceilingShade,light*(.86+.14*grain),loose));
        // A jagged return stroke forks along the moving face from its exposed tip.
        if(pull>.025){
            float run=x-end;
            float zig=.035*sin(31.*run+seed*9.)+.016*sin(79.*run+seed);
            float trunk=abs(y-zig);
            float fork=abs(y-zig*.62-.48*(run-.19));
            float line=min(trunk,mix(trunk,fork,smoothstep(.17,.23,run)));
            float width=clamp(t*exposurePx,.009,.032);
            float spark=exp(-line/width)+.24*exp(-line/.073);
            color+=vec3(.57,1.46,2.85)*spark*pull*4.*(1.-smoothstep(.48,.82,run));
        }
        limit=t;
    }
}

// Angular splinters break free just under the plates and spiral into the tip.
// Each group lies at its own moving height, so the fragments cross in depth.
vec3 rimSplinters(vec3 ro,vec3 rd,float limit) {
    if(rd.y<.035) return vec3(0.);
    float light=0.;
    for(int i=0;i<3;i++){
        float k=float(i), phase=fract(iTime*.21+.12*k+.25);
        float slip=smoothstep(.52,.98,phase);
        float y=LIP_Y-.04-.40*sin(3.14159*slip)+.045*slip;
        float t=(y-ro.y)/rd.y;
        if(t<=0. || t>=limit || slip<=0. || slip>=1.) continue;
        vec2 p=ro.xz+t*rd.xz-MOUTH;
        float radius=mix(.77+.09*k,.08,slip);
        if(abs(length(p)-radius)>.12) continue;
        float angle=2.36*k+2.8*slip+.4;
        vec2 dir=vec2(cos(angle),sin(angle));
        vec2 q=vec2(dot(p,dir)-radius,dot(p,vec2(-dir.y,dir.x)));
        float edge=max(abs(q.y)*.86+q.x*.35-.034,abs(q.x)-.073);
        float aa=clamp(t*exposurePx,.006,.033);
        light+=(1.-smoothstep(-aa,aa,edge))*sin(3.14159*slip)
              *(.60+.70*music.z+.43*dir.x);
    }
    return vec3(light);
}

// Discharges graze the vault. Evaluate the channel at the ray's world-space
// ceiling hit so even its distant forks keep their perspective as the eye orbits.
float skyKink(float x,float seed){
    float coarse=channelBend(x,.50,seed).x;
    float medium=channelBend(x,1.9,seed+19.).y;
    float fine=channelBend(x,5.4,seed+64.).x;
    return min(1.,x*1.4)*(.76*coarse+.32*medium+.095*fine);
}

float skyLightning(vec2 p,float r,float footprint){
    if(skyStrikeAge<-.16 || skyStrikeAge>2.0 || r>28.)return 0.;
    float leader=clamp((skyStrikeAge+.16)*1.65,0.,1.);
    float light=0.;
    vec2 axis=ruptureAxis;
    const mat2 spread=mat2(-.5,.8660254,-.8660254,-.5);
    float aa=clamp(footprint*.30,.008,.065);
    for(int arm=0;arm<3;arm++,axis=spread*axis){
        float seed=skyStrikeSeed+float(arm)*73.;
        float reach=17.+9.*hash(seed+1.8);
        float x=dot(p,axis);
        if(x<.25 || x>reach*leader)continue;
        float y=dot(p,vec2(-axis.y,axis.x));
        float forkAt=reach*(.20+.19*hash(seed+16.));
        float branch=x-forkAt;
        if(abs(y)>3.4+.80*max(0.,branch))continue;
        float d=abs(y-skyKink(x,seed));
        float section=x/reach;
        float trunk=(1.-smoothstep(.76,1.,section))*(.92-.44*section);
        float core=exp(-d/(.022+aa))*trunk;
        float halo=exp(-d/.19)*trunk;
        if(branch>0. && branch<reach*.40){
            float fork=skyKink(forkAt,seed)+(.48+.20*hash(seed+2.))*branch
                       *sign(hash(seed+27.)-.5)+skyKink(branch,seed+31.)*.76;
            float gap=abs(y-fork);
            float fade=.66*smoothstep(0.,.55,branch)*(1.-smoothstep(.25,.40,branch/reach));
            core+=fade*exp(-gap/(.015+aa));
            halo+=fade*exp(-gap/.20);
        }
        float twigAt=reach*(.52+.10*hash(seed+5.));
        float twig=x-twigAt;
        if(twig>0. && twig<.22*reach){
            float side=hash(seed+27.)>.5 ? -1. : 1.;
            float path=skyKink(twigAt,seed)+.57*side*twig+.51*skyKink(twig,seed+57.);
            float w=.49*smoothstep(0.,.32,twig)*(1.-smoothstep(.09,.22,twig/reach));
            core+=w*exp(-abs(y-path)/(.012+aa));
        }
        // A dim advancing leader, then return strokes along the same fault.
        float local=skyStrikeAge-.42*section;
        float first=smoothstep(-.05,.025,local)*exp(-5.*max(0.,local));
        float echo=smoothstep(.26,.31,local)*exp(-3.6*max(0.,local-.31));
        float energy=(.20+first*2.8+echo*(.9+1.25*music.z))
                    *(1.-smoothstep(.80,1.9,local))*(.64+.84*music.x);
        light+=(core+.28*halo)*energy*smoothstep(.24,.56,x)
              *smoothstep(0.,.65,reach*leader-x);
    }
    return light;
}

#ifdef VESPER_SOLO_STORM
uniform vec4 swarmBirths,swarmPower;
uniform float uDoorTravel;
uniform highp sampler2D uDoorView;

// Light peeling from a crest and spiralling inwards. Its wake samples the
// same path a little earlier, so it curls behind the head on a tight turn.
float intakeStreamer(float radius,float angle,float age,float seed,float strength,float aa){
    float life=1.02+.39*seed;
    float u=age/life;
    if(u<=0. || u>=1.)return 0.;
    float fade=smoothstep(0.,.14,u)*(1.-smoothstep(.68,1.,u));
    float origin=1.1+2.5*seed;
    float head=mix(origin,.23,u);
    float turn=seed*TAU+1.5*log(origin/head);
    float lag=mod(turn-angle+3.14159,TAU)-3.14159;
    if(lag<-.28 || lag>1.38)return 0.;
    float trail=clamp(lag,0.,1.38);
    float path=head*exp(trail/1.5);
    if(abs(radius-path)>.55)return 0.;
    float width=.033+.035*strength+.45*aa;
    float crossTrack=(radius-path)/width;
    float ahead=min(lag,0.)*head/(width*1.8);
    float taper=exp(-2.6*trail-ahead*ahead);
    float core=exp(-1.6*crossTrack*crossTrack);
    float mist=exp(-.20*crossTrack*crossTrack);
    return fade*taper*(.8+2.2*strength)*(core+.37*mist);
}

// One finite bank of vapour, in a curved coordinate system around the eye.
// Growth changes its footprint. During breakup the erosion front cuts holes
// in the density until even the densest lobe is gone.
void intakeCloud(inout vec3 sky,vec2 p,float life,float seed,float size){
    if(life<=0. || life>=1.)return;
    float growth=smoothstep(0.,.32,life);
    float erosion=smoothstep(.61,1.,life);
    float start=1.65+1.95*fract(seed*11.7);
    float orbit=mix(start,.22+.13*seed,life);
    float r=length(p);
    float breadth=(.31+.42*size)*growth+.015;
    if(abs(r-orbit)>1.45*breadth || r<.2)return;
    float yaw=TAU*seed+1.32*log(start/r)+.53*life;
    float offset=mod(atan(p.y,p.x)-yaw+3.14159,TAU)-3.14159;
    vec2 q=vec2(offset/min(2.35,(.78+size)*breadth*1.9/orbit),(r-orbit)/breadth);
    float envelope=1.-dot(q,q);
    if(envelope<-.9)return;
    vec3 uv=vec3(q*vec2(2.65,1.8),seed*39.+life*.74);
    uv.xy+=.32*vec2(sin(q.y*2.7+5.*life),sin(q.x*3.8-2.*life));
    float billow=noise3(uv);
    float detail=noise3(uv*2.43+vec3(2.,9.,life*.8));
    float curls=noise3(uv*vec3(2.6,4.8,2.)+2.7*vec3(detail,billow,-life*.3));
    float vapour=1.32*(billow-.5)+.85*(detail-.5)+.55*(curls-.5);
    float density=.81*envelope+vapour-.13-(1.85+.5*(1.-detail))*erosion-.55*(1.-growth);
    float alpha=growth*smoothstep(-.065,.55,density);
    if(alpha<=0.)return;
    float edge=smoothstep(.02,.22,density)*(1.-smoothstep(.22,.75,density));
    float relief=billow-noise3(uv+vec3(.23,-.32,.12));
    vec3 body=vec3(.11,.28,.55)*(.27+.83*detail+.42*curls+1.45*max(0.,relief));
    vec3 lit=body+vec3(.32,.62,1.00)*(edge*(.32+.85*billow)+1.35*max(0.,relief));
    // Buried sheet lightning travels along the same bearing as the sky strike.
    // Thin folds transmit the light, while dense lobes screen it from view.
    if(skyStrikeAge>-.39 && skyStrikeAge<1.06){
        vec2 side=vec2(-ruptureAxis.y,ruptureAxis.x);
        vec2 along=vec2(dot(p,ruptureAxis),dot(p,side));
        float delay=.085*clamp(along.x+.48*along.y,0.,3.);
        float age=skyStrikeAge-delay;
        float leader=.46*smoothstep(-.38,-.15,age)*(1.-smoothstep(-.13,.03,age));
        float stroke=smoothstep(-.08,.035,age)*(1.-smoothstep(.055,.31,age));
        float echo=.68*smoothstep(.22,.31,age)*(1.-smoothstep(.32,.72,age));
        vec2 fromChannel=(along-vec2(1.05+.85*smoothstep(-.24,.38,age),
                                      .48*(billow-.5)))*vec2(.63,1.08);
        vec2 fromFork=(along-vec2(-.76,1.02))*vec2(.99,.78);
        float source=exp(-dot(fromChannel,fromChannel))*(leader+1.65*stroke)
                    +exp(-dot(fromFork,fromFork))*echo;
        float transmission=1.-smoothstep(.20,1.32,density);
        lit+=vec3(.58,1.06,1.85)*source*(.65+.70*music.x+.35*music.z)
            *(.18+1.32*transmission)*(.38+.78*curls+.48*edge);
    }
    sky=mix(sky,lit,alpha*.89);
}

vec3 drawIntake(vec3 ro,vec3 rd,float roofDistance,vec3 vault){
    vec2 face=ro.xz+rd.xz*roofDistance-MOUTH;
    float radius=length(face);
    float hole=.185+.024*rimPull(face/max(.001,radius));
    if(radius<hole){
        // Far wall of a dark throat. The near lip occludes this glint as the eye orbits.
        vec2 back=face+.28*rd.xz/max(.12,rd.y);
        float inner=smoothstep(.69,1.02,length(back)/hole);
        return vec3(.0008,.0015,.004)+inner*.012*STEEL;
    }
    float angle=atan(face.y,face.x);
    float lamp=.5+.5*cos(angle-.72+.22*sin(iTime*.58));
    float crest=pow(lamp,5.)*(.58+.54*music.z);
    float gap=radius-hole;
    float aa=max(.009,exposurePx*roofDistance/max(.18,rd.y));
    float lip=exp(-gap/(.015+.65*aa));
    vec3 color=vault+vec3(.47,.86,1.38)*(lip*(.18+4.5*crest)+crest*.38*exp(-gap/.13));
    // A folded collar of plasma. Its illuminated crests peel away from
    // the lip, then curl back into the same small opening as the funnel.
    if(radius<2.35){
        float pull=log(1.+gap*4.);
        float turn=angle+1.76*pull-.36*iTime;
        float ripple=sin(3.*turn+.78*sin(2.*turn+.4*iTime));
        float sweep=.5+.5*sin(turn-.42*iTime+.46*sin(3.*turn));
        float crestRadius=.23+.16*ripple+(.40+.12*music.x)*sweep*sweep;
        float ridge=gap-crestRadius;
        float foam=noise3(vec3(3.2*face+.8*vec2(cos(turn),sin(turn)),.42*iTime));
        float width=.052+.080*foam;
        float curtain=exp(-abs(ridge)/(.14+width));
        float seam=exp(-abs(ridge+.044*sin(7.*turn+4.*foam))/(width*.44+aa));
        vec2 travel=pull-1.63+3.45*tunnelHits.xz;
        float arrival=dot(exp(-travel*travel/.11),tunnelHits.yw);
        float scallop=.35+.65*foam;
        color+=vec3(.33,.68,1.08)*smoothstep(0.,.085,gap)*(1.-smoothstep(1.5,2.1,gap))
            *(curtain*(.28+.65*foam)+seam*scallop*(.90+1.35*sweep+2.3*arrival));
    }
    // Independent, staggered lifetimes leave real gaps between banks.
    // Layers are at distinct heights for parallax during the camera orbit.
    for(int i=0;i<4;i++){
        float layer=float(i);
        float cycle=iTime/(4.15+.52*layer)+layer*.357;
        float seed=hash(67.*layer+floor(cycle)+13.);
        float life=fract(cycle)/(.77+.17*seed);
        float belowLip=mix(.28+.075*layer,.026+.012*layer,smoothstep(0.,.90,life));
        float distanceToLayer=roofDistance-belowLip/rd.y;
        if(distanceToLayer>0.)
            intakeCloud(color,ro.xz+rd.xz*distanceToLayer-MOUTH,life,seed,.69+.56*seed);
    }
    if(radius<4.6){
        float swarm=0.;
        for(int i=0;i<4;i++){
            float age=iTime-swarmBirths[i];
            if(age<0. || age>1.42)continue;
            float seed=hash(floor(swarmBirths[i]*48.+.5)+17.);
            float power=swarmPower[i];
            // The attack condenses a separate, faster cloud. It can finish
            // dissipating before its entry in the four-hit history expires.
            float life=age/(1.08+.32*seed);
            float belowLip=mix(.25,.025,smoothstep(0.,.90,life));
            float t=roofDistance-belowLip/rd.y;
            if(t>0.)intakeCloud(color,ro.xz+rd.xz*t-MOUTH,life,seed,.45+.53*power);
            swarm+=intakeStreamer(radius,angle,age,seed,power,aa);
            if(power>.68){
                float second=fract(seed+.18+.23*seed);
                swarm+=intakeStreamer(radius,angle,(age-.09)*1.12,second,.72*power,aa);
            }
        }
        color+=swarm*vec3(.55,.98,1.58);
    }
    return color;
}
#endif

// Intersect the underside, inside of the bore, and its distant end in 3D.
float ceiling(vec3 ro,vec3 rd,inout vec3 background) {
    float nearHit=1.e4;
    vec2 q=ro.xz-MOUTH;
    if(ro.y<LIP_Y && rd.y<=.0001) return nearHit;
    if(ro.y<LIP_Y && rd.y>.0001){
        float t=(LIP_Y-ro.y)/rd.y;
        vec2 p=q+t*rd.xz; float r=length(p);
#ifdef VESPER_SOLO_STORM
        vec3 vault=vec3(.024,.034,.039)+skyLightning(p,r,t*exposurePx/max(.16,rd.y))*vec3(.56,1.38,2.40);
        if(r<6.4)vault=drawIntake(ro,rd,t,vault);
        background=mix(background,vault,exp(-.00055*t*t));
        return t;
#else
        float tear=smoothstep(0.,1.55,breakAge);
        if(tear>0. && r<APERTURE+tear*(1.30+.13*sin(7.*atan(p.y,p.x+.00001)))) return nearHit;
        float opening=r<.63 ? openingRadius(p) : APERTURE;
        if(r>=opening){
            float lip=exp(-15.*(r-opening));
            float fissure=0.;
            if(r<1.64){
                float a=atan(p.y,p.x);
                float crack=abs(fract(a*(9./TAU)+.07*sin(17.*r+3.*sin(a)))-.5)*r*(TAU/9.);
                float stress=rimStress(fract(iTime*.21+.11*sin(3.*a)));
                float reach=.74+stress*(.51+.22*sin(3.*a));
                fissure=exp(-crack/(.010+.010*stress))*(1.-smoothstep(reach,reach+.11,r))
                       *(.16+1.38*stress+.38*stress*music.z);
            }
            vec3 steel=vec3(.024,.034,.039)+(.065+.09*arcPower)*exp(-.71*(r-APERTURE))*STEEL;
            steel+=skyLightning(p,r,t*exposurePx/max(.16,rd.y))*vec3(.56,1.38,2.40);
#ifdef VYBOJ_VORTEX_ONLY
            steel+=rimFlash*(.46+music.z)*exp(-1.25*r)*vec3(.18,.44,.86);
#endif
            steel+=vec3(.53,.65,.75)*lip+vec3(fissure);
            if(r<4.1) steel+=vaultCorona(p,r);
            if(r<1.55) steel+=chargedLip(p,r,t*exposurePx/max(.12,rd.y));
            background=mix(background,steel,exp(-.00055*t*t)*(1.-smoothstep(1.5,3.,breakAge)));
            return t;
        }
#endif
    }
#ifndef VYBOJ_VORTEX_ONLY
    float a=dot(rd.xz,rd.xz), b=dot(q,rd.xz), c=dot(q,q)-BORE*BORE;
    // Short inner collar; the larger toothed bore opens out behind it.
    float innerDisc=b*b-a*(dot(q,q)-APERTURE*APERTURE);
    if(a>1.e-7 && innerDisc>=0.){
        float t=(-b+sqrt(innerDisc))/a;
        float y=ro.y+t*rd.y-LIP_Y;
        if(t>.001 && y>=0. && y<.19){
            vec2 xy=q+t*rd.xz;
            background=vec3(.055,.079,.105)+chargedLip(xy,APERTURE+y*.49,t*exposurePx)
                       *(.38+.62*exp(-11.*y));
            nearHit=t;
        }
    }
    float disc=b*b-a*c;
    if(a>1.e-7 && disc>=0.){
        float t=(-b+sqrt(disc))/a;
        float y=ro.y+t*rd.y-LIP_Y;
        if(t>.001 && t<nearHit && y>=0. && y<SHAFT_TOP-LIP_Y){
            vec2 n=normalize(q+t*rd.xz);
            float mouthLight=exp(-1.55*y);
            float seam=pow(.5+.5*cos(8.*atan(n.y,n.x)+1.65*y),6.);
            background=vec3(.024,.044,.058)+(.16+.12*n.x)*mouthLight*STEEL
                +vec3(.065,.12,.15)*seam*(.68+.32*music.y);
            nearHit=t;
        }
    }
    if(rd.y>.0001){
        float t=(SHAFT_TOP-ro.y)/rd.y;
        if(t>.001 && t<nearHit && length(q+t*rd.xz)<BORE){
            float port=length(q+t*rd.xz)/BORE;
            background=mix(vec3(.015,.023,.031),vec3(.28,.43,.51),pow(port,5.));
            nearHit=t;
        }
    }
    shaftTeeth(ro,rd,nearHit,background);
#endif
    return nearHit;
}

#ifdef VYBOJ_VORTEX_ONLY
float ritualTime, bow, takeSword, grace, kingPresence, recoil;
vec3 heart;

vec3 bodyPoint(vec3 p) {
    p.xy=rotate2(.09*bow+.13*recoil)*p.xy;
    p.yz=rotate2(.47*bow)*p.yz;
    return vec3(-.16,1.89-.26*bow+.055*sin(iTime*1.4)+.21*recoil,-.10)+p;
}
void setRitual(float age) {
    ritualTime=age;
    takeSword=smoothstep(1.38,2.66,age);
    bow=smoothstep(2.52,3.52,age)*(1.-smoothstep(5.50,6.66,age));
    grace=smoothstep(5.08,5.55,age);
    float impact=max(0.,age-5.08);
    recoil=sin(5.7*impact)*exp(-1.55*impact);
    heart=bodyPoint(vec3(.04,.18,.54));
    kingPresence=smoothstep(.55,3.4,breakAge);
}

// A hexagonal crystal with two pointed ends and a slightly flared prism.
// Entry and exit facets give each pixel an optical thickness and a second highlight.
void ritualGem(vec3 ro,vec3 rd,vec3 base,vec3 tip,float radius,float aspect,
               float twist,vec3 pigment,bool blade,inout float nearest,inout vec3 color) {
    vec3 along=tip-base;
    float len=length(along);
    vec3 mid=.5*(tip+base)-ro;
    if(dot(mid,rd)<0. || length(cross(mid,rd))>.50*len+radius) return;
    vec3 y=along/len,x=normalize(cross(y,abs(y.z)<.94 ? vec3(0,0,1) : vec3(0,1,0)));
    vec3 z=cross(x,y);
    float c=cos(twist), s=sin(twist);
    vec3 u=c*x+s*z, v=(-s*x+c*z)/aspect;
    float low=blade?.16:.27, high=blade?.39:.66;
    float h0=low*len, h1=high*len, r0=.73*radius;
    vec3 o=ro-base;
    float entry=0.,exitDepth=1e4;
    vec3 front=vec3(0.),back=vec3(0.);
    for(int side=0;side<6;side++){
        float a=TAU*float(side)/6.;
        vec3 lateral=cos(a)*u+sin(a)*v;
        for(int cut=0;cut<3;cut++){
            float slope=cut==0 ? r0/h0 : cut==1 ? (radius-r0)/(h1-h0) : -radius/(len-h1);
            float intercept=cut==0 ? 0. : cut==1 ? r0-slope*h0 : -slope*len;
            vec3 n=lateral-slope*y;
            float gap=intercept-dot(n,o), speed=dot(n,rd);
            if(abs(speed)<.00001){if(gap<0.)return;}
            else {
                float t=gap/speed;
                if(speed<0. && t>entry){entry=t;front=n;}
                if(speed>0. && t<exitDepth){exitDepth=t;back=n;}
            }
        }
        if(entry>=min(nearest,exitDepth)) return;
    }
    if(entry<=0.) return;
    nearest=entry;
    vec3 n=normalize(front), farN=normalize(back), hit=o+entry*rd;
    float h=dot(hit,y);
    vec2 xy=vec2(dot(hit,u),dot(hit,v));
    float angle=atan(xy.y,xy.x+.00001);
    float sideEdge=length(xy)*abs(sin(mod(angle,TAU/6.)-TAU/12.));
    float seam=min(sideEdge,min(abs(h-h0),abs(h-h1)));
    float bevel=1.-smoothstep(.002,.009+entry*exposurePx,seam);
    vec3 reflected=reflect(rd,n);
    float sky=.5+.5*reflected.y;
    float window=exp(-7.*abs(dot(reflected,normalize(vec3(.80,.23,.56)))-.32));
    vec3 through=refract(rd,n, .72);
    float inner=pow(max(0.,dot(reflect(through,farN),normalize(vec3(-.52,.78,-.35)))),5.);
    float path=exitDepth-entry, clear=exp(-1.5*path/max(radius,.12));
    vec3 stone=pigment*(.24+.87*sky*sky+1.65*inner)+mix(pigment,vec3(.89,.96,1.),.48)*(.12+.79*window);
    // An internal cleavage plane and its reflection sit at different depths.
    vec3 inside=hit+rd*path*.43;
    float cleave=abs(dot(inside,u)*.61+dot(inside,y)*.31-dot(inside,v)*.64-.30*len);
    stone+=pigment*exp(-20.*cleave/max(.3,len))*(.17+.56*inner);
    stone+=mix(pigment,vec3(.75,.91,1.),.64)*(.25*clear+.74*bevel);
    float d=length(base+hit-heart);
    float wave=exp(-8.*abs(d-max(0.,ritualTime-5.08)*2.25))*grace;
    vec3 gleam=blade ? vec3(.12,.52,1.40) : vec3(1.44,.62,.18);
    color=stone+(1.9*wave+.78*grace*exp(-2.7*d*d))*gleam;
}

#ifdef VESPER_KING_ASCENT
float pyramidHeight(){
    return 2.4*log(1.+.10*max(0.,uAscent.y-1.8))+.036*max(0.,uAscent.y-1.8);
}

void crystalPyramid(vec3 ro,vec3 rd,inout float depth,inout vec3 color){
    float g=uAscent.y;
    if(g<=0.)return;
    float h=pyramidHeight();
    // Two crystallization fronts crawl inwards from the sword points.
    if(h<5.){
        for(int i=0;i<38;i++){
            float k=floor(float(i)*.5),side=i%2==0?1.:-1.;
            float seed=hash(k+side*17.),along=k/18.;
            vec3 root=vec3(side*1.55*(1.-along),-.03,.76*(1.-along));
            root.xz+=(.07+.65*along)*vec2(sin(k*2.4),cos(k*2.4));
            float growth=smoothstep(.08+2.1*along,.54+2.1*along,g);
            if(growth<.01)continue;
            vec3 axis=vec3(-side*(.10+.20*along),.23+.66*seed+.4*along,.20*sin(k));
            ritualGem(ro,rd,root,root+growth*axis,(.09+.13*seed)*growth,.85,k,
                      vec3(.08,.43,.83),false,depth,color);
        }
    }
    if(h<.025)return;
    // The planes only bound the search. A pixel gets its colour and depth
    // from an individual prism, so gaps reveal the king and the layers behind.
    float enter=0.,leave=depth;
    for(int i=0;i<5;i++){
        vec3 n=i==0?vec3(1.,.93,0.):i==1?vec3(-1.,.93,0.):
               i==2?vec3(0.,.93,1.):i==3?vec3(0.,.93,-1.):vec3(0.,-1.,0.);
        float gap=(i==4?.03:.93*h+1.65)-dot(n,ro),v=dot(n,rd);
        if(abs(v)<.00001){if(gap<0.)return;continue;}
        float t=gap/v;
        if(v<0.)enter=max(enter,t);else leave=min(leave,t);
    }
    if(enter>=leave)return;
    // Stagger the columns. Test the current tier and the roots below it:
    // their tips overlap the next tier, so layers interlock as they grow.
    vec3 tile=vec3(.49,.38,.49);
    vec3 o=vec3(ro.x+.17*ro.y,ro.y,ro.z-.11*ro.y)/tile;
    vec3 d=vec3(rd.x+.17*rd.y,rd.y,rd.z-.11*rd.y)/tile;
    vec3 cell=floor(o+(enter+.0001)*d);
    vec3 stepDir=sign(d),stride=1./max(abs(d),vec3(.000001));
    vec3 bound=(cell+step(vec3(0.),d)-o)*stepDir*stride;
    for(int i=0;i<120;i++){
        for(int below=0;below<2;below++){
            vec3 c=cell-vec3(0.,float(below),0.);
            if(c.y<0.)continue;
            float key=dot(c,vec3(71.,353.,173.));
            float seed=hash(key+4.),turn=hash(key+27.)*TAU;
            vec3 base=(c+vec3(.5+.17*cos(turn),.04,.5+.17*sin(turn)))*tile;
            base.x-=.17*base.y;base.z+=.11*base.y;
            float ring=max(abs(base.x),abs(base.z));
            // Independent birth times leave broken terraces at the growing edge.
            float birth=base.y+ring/.93+.84*seed+
                        .38*sin(2.1*base.x+base.z)+.26*sin(2.5*base.z-base.y);
            float phase=smoothstep(birth,birth+.53+.38*seed,h);
            if(phase<.012)continue;
            vec3 axis=vec3(.057*cos(turn),.48+.24*hash(key+13.),.065*sin(turn));
            float oldDepth=depth;
            vec3 tint=mix(vec3(.019,.16,.48),vec3(.10,.55,.74),seed);
            ritualGem(ro,rd,base,base+axis*phase,(.12+.049*hash(key+9.))*sqrt(phase),
                      .83,turn,tint,false,depth,color);
            if(depth<oldDepth){
                float spark=(1.-smoothstep(.65,1.75,h-birth))*(.5+music.x);
                color+=vec3(.07,.54,.95)*spark*phase;
            }
        }
        float next=min(bound.x,min(bound.y,bound.z));
        if(next>=min(depth,leave))break;
        vec3 advance=step(bound,vec3(next));
        cell+=advance*stepDir;bound+=advance*stride;
    }
}
#endif

void crystalSovereign(vec3 ro,vec3 rd,inout float nearest,inout vec3 color) {
    if(kingPresence<=.001) return;
    const vec3 anchor=vec3(0.,1.92,0.);
    ro=anchor+(ro-anchor)/kingPresence;
    if(length(cross(anchor-ro,rd))>2.65) return;
    float limit=nearest/kingPresence, depth=limit;
    vec3 sapphire=vec3(.055,.29,.75);
    ritualGem(ro,rd,bodyPoint(vec3(-.10,-1.52,.01)),bodyPoint(vec3(.08,1.64,-.05)),
              .86,.77,.36+.13*sin(.54*iTime),sapphire,false,depth,color);

#ifdef VESPER_KING_ASCENT
    // Two veins climb the king himself while loose shards build up around him.
    float crust=pyramidHeight();
    if(crust>0.){
        for(int i=0;i<54;i++){
            float k=float(i),tier=floor(k/6.),seed=hash(k+52.);
            float u=(tier+.22+.55*seed)/9.;
            float angle=mod(k,6.)*(TAU/6.)+.39*tier;
            float onset=.16+3.7*u+.39*seed;
            float grown=smoothstep(onset,onset+.43,crust);
            if(grown<=.01)continue;
            float girth=.86*(u<.27?.73*u/.27:u<.66?mix(.73,1.,(u-.27)/.39):(1.-u)/.34);
            vec3 outwards=vec3(cos(angle),0.,sin(angle));
            vec3 pin=bodyPoint(mix(vec3(-.10,-1.52,.01),vec3(.08,1.64,-.05),u)
                               +girth*outwards*vec3(.94,1.,.78));
            vec3 spike=.24*outwards+vec3(0.,.23+.18*seed,0.);
            ritualGem(ro,rd,pin,pin+grown*spike,.14*sqrt(grown),.80,angle,
                      mix(vec3(.05,.30,.64),vec3(.24,.68,.85),seed),false,depth,color);
        }
    }
#endif
    float lag=max(0.,ritualTime-5.48);
    float swing=sin(5.1*lag)*exp(-1.65*lag);
    vec3 grip=mix(vec3(1.36,1.24,.22),vec3(1.30,1.57,.62),takeSword);
    grip+=vec3(.20*sin(3.14159*takeSword),.15*sin(.96*ritualTime)-.15*bow+.19*swing,.12*bow);
    vec3 direction=normalize(vec3(.48-.12*takeSword+.15*bow+.22*swing,-1.,.14+.12*takeSword));
#ifdef VESPER_KING_ASCENT
    float raise=smoothstep(1.5,3.45,uAscent.x);
    float strike=smoothstep(4.60,5.,uAscent.x);
    grip=mix(grip,vec3(.58,3.72,.60),raise);
    grip=mix(grip,vec3(1.27,1.10,.62),strike);
    float tilt=mix(2.82,.24,strike)*raise;
    direction=normalize(mix(direction,vec3(sin(tilt),-cos(tilt),.12),raise));
#endif
    for(int hand=0;hand<2;hand++){
        float side=hand==0 ? 1. : -1.;
        vec3 mirror=vec3(side,1.,1.);
        vec3 base=mirror*(grip-.13*direction),tip=mirror*(grip+1.18*direction);
        float width=.17;

        ritualGem(ro,rd,base,tip,width,.52,side*.42,vec3(.028,.26,.96),true,depth,color);
    }
    if(depth<limit) nearest=depth*kingPresence;
}

#endif

// Clip a ray against the eight planes of a double-ended crystal. The same
// faces give us front/back thickness, a facet normal and the bevel highlight.
void crystalPieces(vec3 ro,vec3 rd,inout float nearest,inout vec3 color) {
    if(breakAge<0.) return;
#ifdef VYBOJ_VORTEX_ONLY
    crystalSovereign(ro,rd,nearest,color);
#ifdef VESPER_KING_ASCENT
    crystalPyramid(ro,rd,nearest,color);
#endif
#else
    vec3 field=vec3(.2,3.,0.)-ro;
    if(length(cross(field,rd))>4.1 || dot(field,rd)<0.) return;
    for(int i=0;i<9;i++){
        float k=float(i), seed=hash(k+23.);
        float age=breakAge-.80*seed;
        if(age<0.) continue;
        float settle=smoothstep(.52,3.08,age);
        float angle=k*(TAU/9.);
        vec2 dir=vec2(cos(angle),sin(angle));
        vec3 anchor=vec3(MOUTH.x+.58*dir.x,LIP_Y-.09,MOUTH.y+.58*dir.y);
        float spin=angle+.38*(age+.65*sin(age*.74))*settle;
        float radius=i==0 ? 0. : 1.02+.72*seed+.13*music.x;
        float height=i==0 ? 2.5 : 2.08+.93*sin(1.64*k);
        vec3 orbit=vec3(radius*cos(spin),height+.16*sin(age*1.3+k),radius*sin(spin));
        float drop=min(age*age*2.15,2.65);
        vec3 pos=mix(anchor+vec3(dir.x*.22*drop,-drop,dir.y*.22*drop),orbit,settle);
        vec3 size=mix(vec3(.30,.095,.23),
                      i==0 ? vec3(.86,1.68,.81) : vec3(.29+.14*seed,.60+.47*seed,.32),
                      settle);
        float bound=max(size.x,max(size.y,size.z));
        vec3 offset=ro-pos;
        if(length(cross(offset,rd))>bound || dot(-offset,rd)-bound>=nearest) continue;
        mat2 yaw=rotate2(spin+.54*age), tilt=rotate2(mix(.28*dir.x,.72*sin(spin+.6),settle));
        vec3 o=offset, d=rd;
        o.xz=yaw*o.xz; d.xz=yaw*d.xz;
        o.xy=tilt*o.xy; d.xy=tilt*d.xy;
        o/=size; d/=size;
        float enter=0., leave=nearest;
        vec3 face=vec3(0.);
        for(int j=0;j<8;j++){
            vec3 n=vec3((j&1)==0?-1.:1.,(j&2)==0?-1.:1.,(j&4)==0?-1.:1.);
            float speed=dot(n,d), gap=1.-dot(n,o);
            if(abs(speed)<.00001){ if(gap<0.) leave=-1.; continue; }
            float t=gap/speed;
            if(speed<0. && t>enter){enter=t;face=n;}
            if(speed>0.) leave=min(leave,t);
        }
        if(enter<=0. || enter>=leave) continue;
        vec3 hit=o+d*enter, n=normalize(face/size);
        n.xy=transpose(tilt)*n.xy; n.xz=transpose(yaw)*n.xz;
        float edge=min(abs(hit.x),min(abs(hit.y),abs(hit.z)));
        float bevel=1.-smoothstep(.012,.052+min(.06,enter*exposurePx/size.x),edge);
        float facet=.5+.5*dot(n,normalize(vec3(-.7,.6,.3)));
        float stripe=exp(-14.*abs(fract(1.55*hit.y+.65*hit.x+.17*sin(5.*hit.z)+.21*age)-.5));
        float deep=1.-exp(-1.7*(leave-enter));
        color=vec3(.028,.087,.21)+vec3(.11,.40,1.10)*facet*facet
            +vec3(.08,.42,1.7)*deep*(.33+.98*stripe)*(.65+.45*music.y)
            +vec3(.85,1.30,2.10)*bevel*(.50+.65*music.z)
            +vec3(1.6)*pow(max(dot(reflect(rd,n),normalize(vec3(-.35,.65,.58))),0.),18.);
        nearest=enter;
    }
#endif
}

// Packed RGB8 atlas: grid offsets, Gaussian IDs, then seven texels per
// anisotropic Gaussian. Walk cells in depth order; blend the tiny depth
// interval within each cell as a group. No point sprites or extra pass.
ivec3 atlasBytes(int i){
    return ivec3(texelFetch(iChannel1,ivec2(i%1024,i/1024),0).rgb*255.+.5);
}
int atlasIndex(int i){
    ivec3 b=atlasBytes(i);
    return b.x+256*b.y+65536*b.z;
}
vec4 shellSplats(vec3 origin,vec3 direction,float formation){
    float scale=4.85+.09*music.x;
    vec3 o=(origin-vec3(.12,2.50,0.))/scale;
    vec3 d=direction;
    mat2 spin=rotate2(.15*iTime);
    o.xz=spin*o.xz; d.xz=spin*d.xz;
    o+=.5;
    vec3 inv=1./mix(vec3(1.e-7),d,greaterThan(abs(d),vec3(1.e-7)));
    vec3 a=-o*inv,b=(1.-o)*inv;
    vec3 nearSide=min(a,b),farSide=max(a,b);
    float t=max(0.,max(nearSide.x,max(nearSide.y,nearSide.z)));
    float end=min(farSide.x,min(farSide.y,farSide.z));
    if(t>=end) return vec4(0.);
    const int N=44;
    ivec3 cell=clamp(ivec3((o+(t+.00001)*d)*float(N)),ivec3(0),ivec3(N-1));
    ivec3 stepCell=ivec3(sign(d));
    vec3 next=(vec3(cell)+step(vec3(0.),d))/float(N);
    vec3 crossing=(next-o)*inv, stride=abs(inv)/float(N);
    int list=atlasIndex(1), data=atlasIndex(2);
    vec4 accum=vec4(0.);
    for(int k=0;k<3*N;k++){
        float edge=min(end,min(crossing.x,min(crossing.y,crossing.z)));
        int c=3+cell.x+N*(cell.y+N*cell.z);
        int start=atlasIndex(c), stop=atlasIndex(c+1);
        vec3 weighted=vec3(0.); float sum=0.,trans=1.;
        for(int j=start;j<stop;j++){
            int id=data+7*atlasIndex(list+j);
            vec3 mean=vec3(256*atlasBytes(id)+atlasBytes(id+1))/65535.;
            // The reveal front climbs the long axis, tracing individual splats.
            float front=formation*1.16-mean.y;
            if(front<0.) continue;
            vec3 widths=exp2(vec3(atlasBytes(id+2))*(8./255.)-12.);
            vec3 u=normalize(vec3(atlasBytes(id+3))*(2./255.)-1.);
            vec3 v=vec3(atlasBytes(id+4))*(2./255.)-1.;
            v=normalize(v-u*dot(u,v));
            mat3 basis=transpose(mat3(u,v,cross(u,v)));
            vec3 ray=basis*d/widths, delta=basis*(o-mean)/widths;
            float peak=-dot(ray,delta)/dot(ray,ray);
            if(peak<t || peak>=edge) continue;
            vec3 q=delta+peak*ray;
            float power=dot(q,q);
            if(power>7.84) continue;
            float alpha=float(atlasBytes(id+6).x)/255.*exp(-.5*power)*smoothstep(0.,.065,front);
            vec3 pigment=vec3(atlasBytes(id+5))/255.;
            float pearl=dot(pigment,vec3(.30,.52,.18));
            vec3 tint=mix(vec3(.045,.18,.54),vec3(.93,.98,1.),smoothstep(.22,.77,pearl));
            // The shortest covariance axis approximates a surface normal.
            vec3 n=dot(u,d)>0. ? -u : u;
            float light=max(dot(n,normalize(vec3(-.65,.74,-.25))),0.);
            float rim=pow(1.-abs(dot(n,d)),3.);
            tint=mix(tint,pigment,.20)*(.39+.91*light)
                +vec3(.11,.35,.68)*rim+vec3(.13,.52,1.55)*exp(-40.*abs(front-.06));
            weighted+=tint*alpha; sum+=alpha; trans*=1.-alpha;
        }
        float cover=1.-trans;
        accum+=vec4(weighted/max(sum,.00001),1.)*cover*(1.-accum.a);
        if(accum.a>.985 || edge>=end) break;
        if(crossing.x<=crossing.y && crossing.x<=crossing.z){
            cell.x+=stepCell.x; crossing.x+=stride.x;
        }else if(crossing.y<=crossing.z){
            cell.y+=stepCell.y; crossing.y+=stride.y;
        }else{cell.z+=stepCell.z; crossing.z+=stride.z;}
        if(any(lessThan(cell,ivec3(0))) || any(greaterThanEqual(cell,ivec3(N)))) break;
        t=edge;
    }
    return accum;
}

#ifdef VYBOJ_VORTEX_ONLY
// Radius, eye height, look-at height and bank for four exterior viewpoints.
vec4 vortexShot(float index){
    float i=mod(index,4.);
    if(i<1.) return vec4(7.45,.62,3.05,-.16);
    if(i<2.) return vec4(4.15,4.62,1.53,.31);
    if(i<3.) return vec4(5.45,.55,2.95,-.30);
    return vec4(8.65,3.66,2.24,.16);
}
#endif
void mainImage(out vec4 fragColor,in vec2 fragCoord) {
#ifdef VESPER_PASSAGE
    vec3 bands=uSongBands;
#else
    vec3 bands=vec3(.62*fftAt(2.)+.38*fftAt(4.),
                    .48*fftAt(9.)+.32*fftAt(21.)+.20*fftAt(43.),
                    .43*fftAt(74.)+.34*fftAt(121.)+.23*fftAt(181.));
    #endif
    music=smoothstep(vec3(.09,.06,.025),vec3(.97,.88,.73),bands);
#ifdef VYBOJ_VORTEX_ONLY
#ifdef VESPER_PASSAGE
    float flightClock=uPassageClock;
#ifdef VESPER_CONCERT
    float courtAge=flightClock-1.25*BREAK_TIME;
    bool encounter=courtAge>=0.;
    breakAge=encounter ? 4.+courtAge : -BREAK_TIME;
    setRitual(max(0.,courtAge));
#else
    bool encounter=false;
    breakAge=-BREAK_TIME;
#endif
    float chamber=1.;
    vec4 tunnelExit=vec4(0.);
#ifdef VESPER_CONCERT
    if(encounter && courtAge<2.3){
        lomMain(tunnelExit,fragCoord);
        float cut=mix(42.,-.8,pow(smoothstep(0.,2.3,courtAge),.58));
        chamber=smoothstep(cut,cut+max(.4,.12*cut),lomWallDepth);
        if(chamber==0.){fragColor=tunnelExit;return;}
    }
#endif
#else
    // Image-only fallback uses the same longer exterior before joining the rail.
    const float extraExterior=2.*10.29;
    float cycleTime=2.*BREAK_TIME+extraExterior;
    float elapsed=mod(iTime,cycleTime);
    float flightClock=elapsed<4.55+extraExterior
        ? elapsed*(4.55/(4.55+extraExterior)) : elapsed-extraExterior;
    float exitTime=1.25*BREAK_TIME;
    float tripIndex=floor(iTime/cycleTime);
    if(textureSize(iChannel1,0).x>=6 && texelFetch(iChannel1,ivec2(1,0),0).r>0.){
        int section=int(cueState(3)), stage=section%5;
        const float cuts[6]=float[6](0.,4.55,10.6,1.25*BREAK_TIME,23.2,2.*BREAK_TIME);
        flightClock=mix(cuts[stage],cuts[stage+1],clamp(cueState(4),0.,.99999));
        tripIndex=float(section/5);
    }
    bool encounter=flightClock>=exitTime && crystalEncounter(tripIndex);
    float reveal=smoothstep(exitTime,exitTime+BREAK_TIME/8.,flightClock);
    float remaining=2.*BREAK_TIME-flightClock;
    breakAge=encounter ? mix(-BREAK_TIME,min(4.+flightClock-exitTime,1.7*remaining-.3),
                              smoothstep(0.,1.,remaining)) : -BREAK_TIME;
    setRitual(flightClock-exitTime);
    vec4 tunnelExit=vec4(0.);
    float chamber=1.;
    if(encounter && reveal<1.){
        lomMain(tunnelExit,fragCoord);
        // The far wall opens first; close teeth pass the eye last.
        float cut=mix(42.,-.8,pow(reveal,.58));
        chamber=smoothstep(cut,cut+max(.4,.12*cut),lomWallDepth);
        if(chamber==0.){ fragColor=tunnelExit; return; }
    }
    #endif
    rimAge=0.;
    float cycle=mod(flightClock,BREAK_TIME);
    rope=smoothstep(4.55,7.85,cycle)*(1.-smoothstep(9.85,13.1,cycle));
    liftFront=1.18*smoothstep(6.30,9.12,cycle)*(1.-smoothstep(9.25,12.42,cycle));
#else
    breakAge=iTime-BREAK_TIME;
#endif
    float conductor=floor(iTime*.92/5.)*5.;
    float bearing=TAU*hash(conductor+.42);
    ruptureAxis=vec2(cos(bearing),sin(bearing));
    float impactAge=iTime-(conductor+.18*hash(conductor+28.))/.92-.47;
    skyStrikeSeed=conductor; skyStrikeAge=impactAge;
    float recoil=smoothstep(-.075,.055,impactAge)*exp(-2.9*max(0.,impactAge));
    // Let the old fault close before the next stroke picks a new sector.
    float sectorLife=smoothstep(0.,.24,mod(iTime*.92,5.))*(1.-smoothstep(4.55,5.,mod(iTime*.92,5.)));
    ruptureLoad=min(1.,sectorLife*(.57*music.x+.18*music.z+.79*recoil*(.5+.5*music.y)));
#ifdef VESPER_WRITE_SPRAY
    fragColor=sparkTexel(fragCoord);
    return;
#endif
    if(breakAge<3.6) stormLamp();
    vec2 uv=(2.*fragCoord-iResolution.xy)/iResolution.y;

    float pathTime=iTime;
    float drive=dot(bands,vec3(.56,.32,.12));
    vec2 steer=vec2(0.);
#ifdef VYBOJ_CAMERA_INPUT
    pathTime=uOrbit.z;
    drive=uOrbit.w;
    steer=uOrbit.xy;
#else
    // Shadertoy supplies pixel-space mouse and the signed click origin.
    if(abs(iMouse.z)+abs(iMouse.w)>0.)
        steer=(iMouse.xy-abs(iMouse.zw))/iResolution.xy*vec2(-TAU,1.7);
#endif
#ifdef VYBOJ_VORTEX_ONLY
#ifdef VESPER_PHRASE_CAMERA
    float movement=uStormCamera.x, energy=uStormCamera.y, accent=uStormCamera.z;
    float phrase=.22*movement+.075*iTime;
    float orbit=-1.15+.69*phrase+.18*sin(.82*phrase);
    vec2 outwards=vec2(sin(orbit),cos(orbit)),side=vec2(outwards.y,-outwards.x);
    float stormLook=smoothstep(-.45,-.02,skyStrikeAge)*(1.-smoothstep(.45,1.85,skyStrikeAge));
    float eyeFocus=accent+(.78+.16*energy)*stormLook*(1.-accent);
    float pass=.5+.5*sin(phrase-.85);
    float approach=clamp(.52*energy*pass+.65*eyeFocus,0.,1.);
    float distance=mix(8.9,5.45,approach)*max(1.,.96*iResolution.y/iResolution.x);
    float height=1.25+(1.22+.64*energy)*(.5+.5*sin(.73*phrase))-.45*eyeFocus;
    vec2 eye=outwards*distance+side*(.63+.62*energy)*sin(.88*phrase);
    vec3 ro=vec3(eye.x,height,eye.y);
    float aimY=mix(2.45+.35*sin(.76*phrase),4.46,eyeFocus);
    vec2 aim=mix(vec2(.18,-.07),MOUTH,eyeFocus)+side*.23*sin(.9*phrase);
    vec3 target=vec3(aim.x,aimY,aim.y);
    float roll=(.025+.065*energy)*sin(.87*phrase);
    float lens=mix(1.72,1.88,approach);
#else
    // The web player supplies eased audio cues. A single Shadertoy Image
    // has no previous frame: spectral balance directly selects its viewpoint.
    float shot=2.*smoothstep(.14,.62,bands.z)+smoothstep(.47,.79,bands.y);
    vec3 cue=bands;
#ifdef VYBOJ_CAMERA_INPUT
    shot=uVortexShot.x;
    cue=uVortexShot.yzw;
#endif
    float segment=floor(shot);
    vec4 pose=mix(vortexShot(segment),vortexShot(segment+1.),smoothstep(0.,1.,fract(shot)));
    float wide=smoothstep(.02,.58,rimAge)*(1.-smoothstep(1.10,2.18,rimAge))
              *smoothstep(.06,.35,drive);
    // Offset the eye along the tangent as well as across the orbit. The
    // camera leads into the pass, then the subject crosses back into frame.
    float sweep=sin(.91*pathTime+.83*shot);
    float crane=sin(.76*pathTime-.94*shot);
    float activity=smoothstep(.04,.50,drive);
    float orbit=-1.29+shot*(TAU/4.)+.43*cue.y+.19*sin(.39*pathTime)+steer.x;
    vec2 radial=vec2(sin(orbit),cos(orbit)), tangent=vec2(radial.y,-radial.x);
    float distance=mix(pose.x-.67*cue.x+.75*crane*activity,11.8,wide)
                   *max(1.,.97*iResolution.y/iResolution.x);
    float truck=activity*(1.50*sweep+.42*sin(1.43*pathTime))*(1.-.75*wide);
    float height=mix(pose.y,2.54+1.76*crane,.34*activity)+.45*cue.z-.24*cue.x;
    height=mix(height,2.55,wide);
    height=clamp(height+mix(4.8,2.3,wide)*steer.y,.20,LIP_Y-.16);
    vec2 eye=radial*distance+tangent*truck;
    vec3 ro=vec3(eye.x,height,eye.y);

    float aimY=mix(clamp(pose.z+activity*(.84*sin(.82*pathTime+.97*shot)+.33*cue.z),1.18,4.30),2.55,wide);
    vec2 subject=axisAtTime(aimY/H,iTime,.5);
    float lead=activity*(.55*cos(.91*pathTime+.83*shot)-.30*sweep);
    vec2 aim=mix(.80*subject+tangent*lead,vec2(.22,-.09),wide);
    vec3 target=vec3(aim.x,aimY,aim.y);
    float bank=pose.w+activity*(.19*sweep+.09*sin(1.52*pathTime-.66*shot));
    // Align below the small collar before lifting. The entire rail remains
    // within .13 units of its axis, inside both the lip and the tooth tips.
    float approach=smoothstep(6.85,9.10,flightClock)*(1.-smoothstep(19.7,22.,flightClock));
    float trip=clamp((flightClock-9.10)/10.6,0.,1.);
    float climb=sin(3.14159265*trip);
    float railY=LIP_Y-.86+7.20*climb*climb;
    float twist=1.40*railY+.42*pathTime+steer.x;
    vec2 rail=MOUTH+.12*vec2(cos(twist),sin(twist));
    ro=mix(ro,vec3(rail.x,railY,rail.y),approach);
    // Keep a horizontal component even when looking straight up the bore.
    vec2 ahead=rail-radial*(.23+.06*cue.z)+.045*tangent*clamp(steer.y,-1.,1.);
    target=mix(target,vec3(ahead.x,railY+1.8,ahead.y),approach);
    float roll=mix(bank*(1.-wide),.15*sin(twist),approach);
    float lens=mix(mix(1.75,1.58,wide),1.43,approach);
    if(encounter){
        // Push out of the passage facing the original orbiting crystal court.
        float weight=smoothstep(-1.5,3.4,breakAge);
        float age=flightClock-1.25*BREAK_TIME;
        float yaw=.33+.25*sin(.67*age)+.055*cue.y+steer.x;
        float pitch=clamp(.085+.075*sin(.66*age)+steer.y,-.20,.89);
        float dist=(6.6-1.1*exp(-.9*age)-.42*drive)*max(1.,.87*iResolution.y/iResolution.x);
        vec3 pivot=vec3(.08,1.96-.17*bow,0.);
#ifdef VESPER_KING_ASCENT
        float liftShot=smoothstep(1.3,3.3,uAscent.x)*(1.-smoothstep(4.45,5.22,uAscent.x));
        pivot.y+=.50*liftShot;
        dist+=.76*liftShot;
        float h=pyramidHeight(),follow=smoothstep(.4,4.5,h);
        pivot.y=mix(pivot.y,max(.43*h,h-4.2),follow);
        pitch=mix(pitch,.28,follow);
        yaw=mix(yaw,.67+.17*sin(.14*uAscent.x),follow);
        dist=max(dist,(2.65*min(h,5.5)+1.)*max(1.,iResolution.y/iResolution.x));
        pivot.y-=.15*exp(-6.*abs(uAscent.x-5.));
#endif
        roll=mix(roll,.018*sin(.7*age),weight);
        vec3 view=vec3(sin(yaw)*cos(pitch),sin(pitch),cos(yaw)*cos(pitch));
        ro=mix(ro,pivot+dist*view,weight);
        target=mix(target,pivot,weight);
        lens=mix(lens,2.22,weight);
    }
#endif
    uv=rotate2(roll)*uv;
#else
    float orbit=.49*pathTime+.17*sin(.67*pathTime)+steer.x;
    float rise=clamp(.49+.40*sin(.50*pathTime-.95)+steer.y,0.,1.);
    float radius=(8.58+.46*cos(.31*pathTime)-.90*drive)
                 *max(1.,.85*iResolution.y/iResolution.x);
    // Before the break the eye stays between the slab and the ceiling.
    // Once the roof is gone the same control reaches above the crystal.
    float openSky=smoothstep(.65,4.2,breakAge);
    float pivot=2.48;
    float low=asin((.60-pivot)/radius);
    float high=mix(asin((LIP_Y-.35-pivot)/radius),1.39,openSky);
    float elevation=mix(low,high,rise);
    vec2 around=vec2(sin(orbit),cos(orbit))*radius*cos(elevation);
    vec3 target=vec3(.12,pivot,0.);
    vec3 ro=target+vec3(around.x,radius*sin(elevation),around.y);
    float roll=.022*sin(.42*pathTime);
    uv=mat2(cos(roll),sin(roll),-sin(roll),cos(roll))*uv;
    float lens=1.96;
#endif
#ifdef VESPER_SOLO_STORM
    // Steer the actual exterior camera under the mouth, then cross the lip.
    if(uDoorTravel>0.){
        float steerIn=smoothstep(0.,.80,uDoorTravel),lift=smoothstep(.42,1.,uDoorTravel);
        vec3 mouth=vec3(MOUTH.x,LIP_Y,MOUTH.y);
        vec3 below=mouth+vec3(.08,-1.36+1.40*lift,.07);
        ro=mix(ro,below,steerIn);
        target=mix(target,mouth+vec3(0.,.58,0.),steerIn);
    }
#endif
    vec3 f=normalize(target-ro), right=normalize(cross(f,vec3(0.,1.,0.))), up=cross(right,f);
    vec3 rd=normalize(lens*f+uv.x*right+uv.y*up);
    exposurePx=1.4/(lens*iResolution.y);
#ifdef VESPER_SOLO_STORM
    if(uDoorTravel>0. && ro.y>=LIP_Y){
        fragColor=texture(uDoorView,fragCoord/iResolution.xy);
        return;
    }
#endif
#if defined(VYBOJ_VORTEX_ONLY) && !defined(VESPER_SOLO_STORM)
    // The aperture is a portal to the actual LOM / PIXEL Image. Once the eye
    // crosses the plane, render its native view with no extra colour grading.
    if(!encounter && ro.y>=LIP_Y){
        lomMain(fragColor,fragCoord);
        return;
    }
    float portalT=(LIP_Y-ro.y)/max(rd.y,.00001);
    vec2 apertureHit=ro.xz+rd.xz*portalT-MOUTH;
    float portalRadius=APERTURE;
    if(dot(apertureHit,apertureHit)<.63*.63) portalRadius=openingRadius(apertureHit);
    bool seesLom=!encounter && rd.y>.00001 && dot(apertureHit,apertureHit)<portalRadius*portalRadius;
#endif
    float floorT=rd.y<-.001 ? -ro.y/rd.y : 1.e4;
    float formation=0.;
    #ifndef VYBOJ_VORTEX_ONLY
    if(breakAge>24.-BREAK_TIME && textureSize(iChannel1,0).x==1024 && all(equal(atlasBytes(0),ivec3(83,71,1)))){
        float revealTime=24.;
#ifdef VYBOJ_CAMERA_INPUT
        revealTime=max(revealTime,uShellReadyAt);
#endif
        formation=smoothstep(0.,3.4,iTime-revealTime);
    }
    #endif
    vec3 sky=vec3(.011,.020,.026)+vec3(.078,.065,.051)*exp(-4.*abs(rd.y));
    vec3 col=sky;
    float nextScene=smoothstep(.45,3.40,breakAge);
    float roofT=1.e4;
    if(breakAge<3.){
        roofT=ceiling(ro,rd,col);
#ifndef VESPER_SOLO_STORM
        brokenRim(ro,rd,roofT,col);
        col+=rimSplinters(ro,rd,roofT)*(1.-smoothstep(0.,1.,breakAge));
#endif
    }
    float scenery=min(roofT,floorT), nearest=scenery;
    vec3 crystal=vec3(0.);
    if(formation<1.) crystalPieces(ro,rd,nearest,crystal);
    bool hitCrystal=nearest<scenery;
    float limit=min(35.,nearest);
    vec4 funnel=vec4(0.);
    vec3 lightning=vec3(0.), sparks=vec3(0.);
    vec2 visible=vec2(1.);
    if(breakAge<3.6 && (ro.y<H+.19 || rd.y<0.)){
        float strikeDepth,sparkDepth;
        lightning=bolts(ro,rd,limit,floorT<min(roofT,34.9) && !hitCrystal,strikeDepth);
        sparks=spray(ro,rd,limit,sparkDepth);
        funnel=storm(ro,rd,limit,false,vec2(strikeDepth,sparkDepth),visible);
        lightning*=1.-nextScene; sparks*=1.-nextScene;
    }
    vec3 reflection=vec3(0.);
    if(!hitCrystal && floorT<min(roofT,90.)){
        vec3 p=ro+floorT*rd;
        vec3 ground=vec3(0.);
        if(nextScene<1.) ground=floorLight(p,ro,rd,1.-funnel.a);
        if(nextScene>0.){
            float r=length(p.xz);
            float wave=exp(-5.*abs(r-2.5*max(0.,breakAge-.7)));
            float halo=exp(-.28*r*r);
            float ripple=pow(.5+.5*cos(8.*r-2.8*iTime+.32*rough(p.xz)),8.);
            vec3 iceFloor=vec3(.03+.52*halo+(.25+.42*music.x)*halo*ripple+wave);
            ground=mix(ground,iceFloor,nextScene);
            if(r<5.2 && formation<1.){
                vec3 bounce=reflect(rd,normalize(vec3(.012*rough(p.xz),1.,.012*rough(p.zx))));
                float depth=30.;
                crystalPieces(p+vec3(0.,.003,0.),bounce,depth,reflection);
                reflection*=nextScene*.33*exp(-.11*r*r);
            }
        }
        col=mix(sky,ground,exp(-.00055*floorT*floorT));
    }
    if(hitCrystal) col=vec3(0.);
    col=funnel.rgb+(1.-funnel.a)*col+sparks*visible.y;
    float silver=dot(max(col,0.),vec3(.26,.57,.17));
    silver*=mix(.91,1.06,smoothstep(.10,.95,silver));

    // Dense emissive folds carry cobalt light. Exposed silver crests and
    // the hottest part of the floor pool recover a near-white spectrum.
    float charge=clamp((col.r-col.g)/max(.10,silver),0.,1.);
    vec3 ice=mix(vec3(.23,.66,1.65),vec3(.055,.34,2.15),charge);
    float white=smoothstep(.52,1.75,silver)*(1.-.59*charge);
    col=silver*mix(ice,vec3(1.,1.06,1.14),white);
    col+=lightning*visible.x+(crystal+reflection)*(1.-funnel.a);
    if(formation>0.){
        vec4 shell=shellSplats(ro,rd,formation);
        vec3 stage=sky*vec3(.72,.95,1.6);
        if(floorT<70.){
            float r=length((ro+floorT*rd).xz);
            stage+=vec3(.04,.17,.43)*exp(-.24*r*r)*(.78+.3*music.x);
        }
        col=mix(col,stage,smoothstep(.04,.91,formation))*(1.-shell.a)+shell.rgb;
    }
    col*=1.-.12*smoothstep(.35,1.5,length(uv));
    fragColor=vec4(pow(1.-exp(-1.48*col),vec3(.79)),1.);
#ifdef VYBOJ_VORTEX_ONLY
    if(chamber<1.) fragColor=mix(tunnelExit,fragColor,chamber);
#ifdef VESPER_SOLO_STORM
    if(uDoorTravel>0. && rd.y>.0001){
        vec2 atLip=ro.xz+rd.xz*((LIP_Y-ro.y)/rd.y)-MOUTH;
        float gate=1.-smoothstep(APERTURE-.025,APERTURE,length(atLip));
        float entry=smoothstep(.08,.47,uDoorTravel);
        vec2 screen=fragCoord/iResolution.xy;
        vec2 view=mix(.5+.48*atLip/APERTURE,screen,smoothstep(.50,.94,uDoorTravel));
        float cover=gate*entry*mix(1.-funnel.a,1.,smoothstep(.76,.97,uDoorTravel));
        if(cover>0.)fragColor.rgb=mix(fragColor.rgb,texture(uDoorView,view).rgb,cover);
    }
#endif
#ifndef VESPER_SOLO_STORM
    if(seesLom && roofT>portalT+.001 && funnel.a<.999){
        vec2 windowUv=vec2(dot(apertureHit,tangent),dot(apertureHit,-radial))/APERTURE;
        vec2 windowPixel=.5*(iResolution.xy+windowUv*iResolution.y);
        vec4 tunnel;
        lomMain(tunnel,mix(windowPixel,fragCoord,approach));
        fragColor.rgb=mix(fragColor.rgb,tunnel.rgb,1.-funnel.a);
    }
#endif
#endif
}
