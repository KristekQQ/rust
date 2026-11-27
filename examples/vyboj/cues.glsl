// Additional cells of Buffer A for the Vír → Lom → crystals sequence.
#ifdef VYBOJ_CAMERA_INPUT
uniform highp uint uCueSeed;
#endif
float cueVariation(uint section) {
#ifdef VYBOJ_CAMERA_INPUT
    section+=uCueSeed;
#endif
    section=section*747796405u+2891336453u;
    uint word=((section>>((section>>28u)+4u))^section)*277803737u;
    return float(((word>>22u)^word)>>8u)/16777216.;
}
void mainImage(out vec4 color,in vec2 coord) {
    ivec2 cell=ivec2(coord);
    if(cell.y!=0 || cell.x>=6){ color=vec4(0.); return; }
    if(cell.x<3){ lomMotion(color,coord); return; }
    if(iFrame==0){ color=packMotion(0.); return; }
    float dt=clamp(iTimeDelta,0.,.12);
    int section=int(previousMotion(3));
    int stage=section%5;
    float progress=previousMotion(4);
    float low=.62*musicBin(2.)+.38*musicBin(4.);
    float air=.56*musicBin(74.)+.44*musicBin(121.);
    float accent=.80*low*low+.20*air*air;
    float envelope=previousMotion(5);
    bool attack=accent>envelope+.045 && accent>.15;
    envelope=mix(accent,envelope,exp(-7.*dt));

    // Wait for the next accent in the wide shot, the tunnel, and the court.
    // The approach and return are travelling shots and play straight through.
    const vec2 seconds[5]=vec2[5](vec2(3.8,9.),vec2(4.6,6.6),vec2(3.8,10.8),
                                  vec2(5.1,9.0),vec2(3.3,4.8));
    float lengthSec=mix(seconds[stage].x,seconds[stage].y,cueVariation(uint(section)));
    if(stage==0){
        // Triple the time outside, putting the extra bars before the approach.
        // The eye crosses the lip about 95% of the way through the flying shot.
        float approachSec=mix(seconds[1].x,seconds[1].y,cueVariation(uint(section+1)));
        lengthSec+=2.*(lengthSec+.95*approachSec);
    }
    float energy=clamp(previousMotion(2),0.,1.);
    progress+=dt*mix(.84,1.20,energy)/lengthSec;
    bool travelling=stage==1 || stage==4;
    if(progress>=1. && (travelling || attack || (progress-1.)*lengthSec>=2.1)){
        section++; progress=0.;
    }
    color=packMotion(cell.x==3 ? float(section) : cell.x==4 ? progress : envelope);
}
