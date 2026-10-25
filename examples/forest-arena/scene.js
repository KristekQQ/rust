import {VR_DEBUG_PANEL_OFFSET,VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
export const PERFORMANCE_PANEL_OFFSET=VR_DEBUG_PANEL_OFFSET;
/** @typedef {[number,number,number]} Vec3 */
/** @typedef {{walkableBounds:[number,number,number,number],spawn:Vec3,landingPads:{id:string,position:Vec3,radius:number}[],obstacles:{id:string,center:Vec3,halfExtents:Vec3}[],modelSpawns:{id:string,slug:string,position:Vec3}[]}} ArenaLayout */
/** Large woodland arena; all architecture is authored Blender/glTF geometry.
 * @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const layoutResponse=await fetch(new URL('../../assets/models/forest-arena/layout.json',import.meta.url));
    if(!layoutResponse.ok)throw new Error(`Rozvržení arény: HTTP ${layoutResponse.status}`);
    /** @type {ArenaLayout} */
    const authored=await layoutResponse.json();
    // Move the authored valley into native locomotion's [-90,90] coordinate domain.
    // The complete model and navigation use the same fixed translation; spawn stays at zero.
    const shift=20;
    /** @type {ArenaLayout} */
    const layout={...authored,walkableBounds:[authored.walkableBounds[0],authored.walkableBounds[1],authored.walkableBounds[2]+shift,authored.walkableBounds[3]+shift],spawn:[0,0,0],
        landingPads:[{id:'player-spawn',position:[0,0,0],radius:.85},...authored.landingPads.map(p=>({...p,position:/** @type {Vec3} */([p.position[0],p.position[1],p.position[2]+shift])}))],
        obstacles:authored.obstacles.map(o=>({...o,center:/** @type {Vec3} */([o.center[0],o.center[1],o.center[2]+shift])})),
        modelSpawns:authored.modelSpawns.map(p=>({...p,position:/** @type {Vec3} */([p.position[0],p.position[1],p.position[2]+shift])}))};
    const environment=await scene.loadModel(new URL('../../assets/models/forest-arena/forest-arena.glb',import.meta.url));
    environment.setPosition([0,0,shift]).setRenderOptions({isStatic:true,castsShadow:true,receivesShadow:true});
    scene.createLight({position:[-25,55,12],color:[1,.86,.64]});
    scene.createLight({position:[35,35,-50],color:[.47,.67,.88]});
    scene.createLight({position:[-35,25,-85],color:[.55,.72,.46]});
    const enemies=[];
    for(const spawn of layout.modelSpawns){
        const model=await scene.loadModel(new URL(`../../assets/models/forest-arena/${spawn.slug}.glb`,import.meta.url));
        model.setPosition(spawn.position).playAnimation('Idle',{loop:true});
        enemies.push(model);
    }
    const marker=createEllipseRing(scene,[.45,.65],[.55,1,.36]);
    const b=layout.walkableBounds;
    // Distant silhouette trees outside the navigation region cannot block a valid path.
    const obstacles=layout.obstacles.map(({center:c,halfExtents:h})=>/** @type {[number,number,number,number]} */([c[0]-h[0],c[0]+h[0],c[2]-h[2],c[2]+h[2]]))
        .filter(r=>r[1]>=b[0]-.4&&r[0]<=b[1]+.4&&r[3]>=b[2]-.4&&r[2]<=b[3]+.4)
        .map(r=>/** @type {[number,number,number,number]} */(r.map(v=>Math.max(-90,Math.min(90,v)))));
    const performancePanel=scene.createInfoPanel('diagnostics',{position:[0,-10000,0],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    return {environment,layout,enemies,marker,performancePanel,locomotion:{bounds:layout.walkableBounds,obstacles,speed:2.5,teleportRange:10,slowScale:.15}};
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
