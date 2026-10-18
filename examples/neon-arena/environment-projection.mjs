// Offline proxy used only to bake a geometrically consistent cube from the
// illustrated facade. Runtime has no architectural mesh or camera calibration.
export const arena = Object.freeze({radius:7, height:12, eyeHeight:1.65});
export const faces = [
    ['px',(u,v)=>[1,-v,-u]], ['nx',(u,v)=>[-1,-v,u]],
    ['py',(u,v)=>[u,1,v]], ['ny',(u,v)=>[u,-1,-v]],
    ['pz',(u,v)=>[u,-v,1]], ['nz',(u,v)=>[-u,-v,-1]],
];
export function projectArenaRay([x,y,z]) {
    const radial=Math.hypot(x,z);
    const height=radial>1e-12 ? arena.eyeHeight+y*arena.radius/radial : (y>0 ? Infinity : -Infinity);
    const u=((.5+Math.atan2(x,-z)/(2*Math.PI))%1+1)%1;
    if(height<0) return {surface:'floor',u,v:1};
    if(height<=arena.height) {
        // The source is a perspective illustration, NOT equirectangular.
        // Use only its architectural facade as a cylindrical wall texture.
        // Exclude the illustrated floor; runtime provides one real plane.
        return {surface:'wall',u,v:.075+(1-height/arena.height)*.535};
    }
    const topAngle=Math.atan2(arena.height-arena.eyeHeight,arena.radius);
    const angle=Math.atan2(y,radial);
    const sky=Math.min(1,Math.max(0,(angle-topAngle)/(Math.PI/2-topAngle)));
    return {surface:'sky',u,v:.075*(1-sky)};
}
