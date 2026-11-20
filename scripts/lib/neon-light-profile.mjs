// Authoring-only IES-style angular distribution. Not measured luminaire data.
// Both shaft direction and baked floor illumination consume these same axes.
export const profile = {
    mask: {width:128,height:128,center:[0,-1.5],size:[6,7.5]},
    floorLobes:{forwardOffset:.65,sideOffset:.68,width:.5,length:1.35,strength:.9},
    stageSpill:{center:[0,-.2],innerRadius:1.65,outerRadius:2.4},
    distribution: {kind:'authored-angular-profile', measured:false,
        degrees:[0,7,12,17,22,27,32], relativeIntensity:[1,.98,.82,.48,.18,.035,0]},
    lights: [
        {position:[-4.2,6.2,-4.8],aimTarget:[0,1.1,0]},
        {position:[4.2,6.2,-4.8],aimTarget:[0,1.1,0]},
    ],
    ssr:{strength:.92,resolutionScale:.5,receiverBounds:{min:[-2.3,-.03,-2.55],max:[2.3,.03,2.05]}},
};
export function floorIntersection(light) {
    const t=light.position[1]/(light.position[1]-light.aimTarget[1]);
    return light.position.map((v,i)=>v+(light.aimTarget[i]-v)*t);
}
export function angularIntensity(degrees) {
    const d=profile.distribution;
    if(degrees<=0)return d.relativeIntensity[0];
    for(let i=1;i<d.degrees.length;i++)if(degrees<d.degrees[i]){
        let t=(degrees-d.degrees[i-1])/(d.degrees[i]-d.degrees[i-1]);t=t*t*(3-2*t);
        return d.relativeIntensity[i-1]*(1-t)+d.relativeIntensity[i]*t;
    }
    return 0;
}
export function lightContribution(light,x,z) {
    const axis=light.aimTarget.map((v,i)=>v-light.position[i]);
    const ray=[x-light.position[0],-light.position[1],z-light.position[2]];
    const axisLength=Math.hypot(...axis),distance=Math.hypot(...ray);
    const cosine=Math.max(-1,Math.min(1,axis.reduce((sum,v,i)=>sum+v*ray[i],0)/(axisLength*distance)));
    const angle=Math.acos(cosine)*180/Math.PI;
    // Baked floor incidence and inverse-square falloff, normalized at the
    // floor intersection. The axis itself aims at the actor's torso, not the floor.
    const groundDistance=axisLength*light.position[1]/(light.position[1]-light.aimTarget[1]);
    return .30*angularIntensity(angle)*(groundDistance/distance)**3;
}
// Authoring-only theatrical gobo: two elongated pools ahead of the feet.
// Intentionally art-directed, not a measured photometric projection.
export function floorLobe(light,x,z) {
    const f=profile.floorLobes,sign=Math.sign(light.position[0]);
    const dx=x-sign*f.sideOffset,dz=z+f.forwardOffset;
    const len=Math.hypot(light.position[0],light.position[2]);
    const ax=-light.position[0]/len,az=-light.position[2]/len;
    const along=dx*ax+dz*az,across=dx*az-dz*ax;
    return f.strength*Math.exp(-2.2*((along/f.length)**2+(across/f.width)**2));
}
export function lightAt(x,z) {
    // Offline art-directed stage spill fade keeps the finite floor edge black.
    const fade=profile.stageSpill;
    const t=Math.max(0,Math.min(1,(Math.hypot(x-fade.center[0],z-fade.center[1])-fade.innerRadius)/(fade.outerRadius-fade.innerRadius)));
    return (1-t*t*(3-2*t))*Math.min(1,profile.lights.reduce((sum,light)=>sum+.08*lightContribution(light,x,z)+floorLobe(light,x,z),0));
}
