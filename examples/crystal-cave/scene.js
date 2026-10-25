import {VR_DEBUG_PANEL_OFFSET,VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
export const PERFORMANCE_PANEL_OFFSET=VR_DEBUG_PANEL_OFFSET;
/** @typedef {[number,number,number]} Vec3 */
/** @typedef {{walkableBounds:[number,number,number,number],spawn:Vec3,landingPads:{id:string,position:Vec3,radius:number}[],obstacles:{id:string,center:Vec3,halfExtents:Vec3}[]}} CaveLayout */
/** Enclosed crystal cave; navigation and geometry share the authored layout.
 * @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const layoutResponse=await fetch(new URL('../../assets/models/crystal-cave/layout.json',import.meta.url));
    if(!layoutResponse.ok)throw new Error(`Rozvržení jeskyně: HTTP ${layoutResponse.status}`);
    /** @type {CaveLayout} */
    const layout=await layoutResponse.json();
    const environment=await scene.loadModel(new URL('../../assets/models/crystal-cave/crystal-cave.glb',import.meta.url));
    environment.setRenderOptions({isStatic:true,castsShadow:true,receivesShadow:true});
    scene.createLight({position:[-3,4,0],color:[1,.65,.30]});
    scene.createLight({position:[4,5,-12],color:[.25,.65,.85]});
    scene.createLight({position:[-3,5,-26],color:[.25,.48,.75]});
    const marker=createEllipseRing(scene,[.45,.65],[.08,.65,1]);
    /** @type {[number,number,number,number][]} */
    const obstacles=layout.obstacles.map(({center:c,halfExtents:h})=>[c[0]-h[0],c[0]+h[0],c[2]-h[2],c[2]+h[2]]);
    const performancePanel=scene.createInfoPanel('diagnostics',{position:[0,-10000,0],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    return {environment,layout,marker,performancePanel,locomotion:{bounds:layout.walkableBounds,obstacles,speed:2,teleportRange:10,slowScale:1}};
}

/** Closed annular mesh on the XZ floor plane, positioned by native locomotion.
 * @param {import('../../js/engine.js').Scene} scene @param {[number,number]} radii @param {[number,number,number]} color */
function createEllipseRing(scene,radii,color) {
    const positions=[],indices=[],segments=48,[rx,rz]=radii,inner=.88,yBottom=.025,yTop=.045;
    for(let i=0;i<segments;i++) {
        const angle=i*Math.PI*2/segments;
        const x=Math.cos(angle),z=Math.sin(angle);
        // Shared vertex order per segment: outer top, inner top, outer bottom, inner bottom.
        positions.push(x*rx,yTop,z*rz,x*rx*inner,yTop,z*rz*inner,
            x*rx,yBottom,z*rz,x*rx*inner,yBottom,z*rz*inner);
    }
    for(let i=0;i<segments;i++) {
        const next=(i+1)%segments,a=i*4,b=next*4;
        // Outward outer wall, inward bore wall, upward crown, downward underside.
        indices.push(a+2,a,b,a+2,b,b+2);
        indices.push(a+3,b+3,b+1,a+3,b+1,a+1);
        indices.push(a,a+1,b+1,a,b+1,b);
        indices.push(a+2,b+2,b+3,a+2,b+3,a+3);
    }
    return scene.createMesh({positions,indices,color}).setPosition([100,0,100]).setEmission(1.2)
        .setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
}
