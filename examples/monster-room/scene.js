import {swordGeometry} from '../neon-arena/geometry.js';
import {VR_DEBUG_PANEL_OFFSET,VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
export const PERFORMANCE_PANEL_OFFSET=VR_DEBUG_PANEL_OFFSET;
/** @typedef {[number,number,number]} Vec3 */
/** @typedef {{walkableBounds:[number,number,number,number],spawn:Vec3,landingPads:{id:string,position:Vec3,radius:number}[],obstacles:{id:string,center:Vec3,halfExtents:Vec3}[],modelSpawns:{id:string,position:Vec3}[]}} ArenaLayout */
/** Outdoor city arena; all architecture is authored Blender/glTF geometry.
 * @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene) {
    scene.clear();
    const layoutResponse=await fetch(new URL('../../assets/models/cyberpunk-alley/layout.json',import.meta.url));
    if(!layoutResponse.ok)throw new Error(`Rozvržení arény: HTTP ${layoutResponse.status}`);
    /** @type {ArenaLayout} */
    const layout=await layoutResponse.json();
    const environment=await scene.loadModel(new URL('../../assets/models/cyberpunk-alley/cyberpunk-alley.glb',import.meta.url));
    environment.setRenderOptions({isStatic:true,castsShadow:true,receivesShadow:true});
    const collisionBoxes=layout.obstacles.map(({center,halfExtents})=>({offset:center,halfExtents}));
    collisionBoxes.push({offset:[0,-.5,-12],halfExtents:[20,.5,25]},
        {offset:[0,9.2,-20],halfExtents:[12.5,.25,1.15]});
    // Match the roof caps, penthouses, balconies and canopies in build_blender.py.
    for(const building of layout.obstacles.filter(o=>o.id.startsWith('Building '))){
        const [x,,z]=building.center,h=building.halfExtents[1]*2,depth=building.halfExtents[2]*2;
        const side=Math.sign(x),i=[4,-5,-15,-26].indexOf(z),inner=x-side*2.2;
        collisionBoxes.push({offset:[x,h+.35,z],halfExtents:[2.3,.35,(depth+.2)/2]},
            {offset:[x,h+1.6,z+.8],halfExtents:[1,1.25,1.3]});
        if(i%2===0){
            for(const floor of [2,4]) if(floor<Math.trunc(h/3.3))
                collisionBoxes.push({offset:[inner-side*.75,floor*3.3-.15,z],halfExtents:[.825,.11,depth*.35]});
            collisionBoxes.push({offset:[inner-side*.9,3.6,z],halfExtents:[.9,.09,depth*.45]});
        }
    }
    environment.setColliders(collisionBoxes);
    scene.createLight({position:[-6,10,-3],color:[.40,.66,.95]});
    scene.createLight({position:[8,7,-14],color:[.75,.18,.42]});
    scene.createLight({position:[-3,9,-28],color:[.22,.45,.62]});
    scene.createLight({position:[0,5,7],color:[.34,.40,.52]});
    const enemies=[];
    const specs=[{slug:'drone-orbiter',color:'pink'},{slug:'blade-wraith',color:'green'},{slug:'core-tank',color:'amber'}];
    for(const [i,spec] of specs.entries()){
        const model=await scene.loadModel(new URL(`../../assets/models/neon-enemies/${spec.slug}-${spec.color}.glb`,import.meta.url));
        model.setPosition(layout.modelSpawns[i].position).playAnimation('Idle',{loop:true});
        enemies.push(model);
    }
    const swords=/** @type {[import('../../js/engine.js').SceneObject,import('../../js/engine.js').SceneObject]} */([
        scene.createMesh(swordGeometry()).setPosition([-.45,1.1,5.1]),
        scene.createMesh(swordGeometry()).setPosition([.45,1.1,5.1]),
    ]);
    for(const sword of swords)sword.setCutter({base:[0,0,-.1],tip:[0,0,-.95]}).setBladeTrail(true)
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    const marker=createEllipseRing(scene,[.45,.65],[.08,.65,1]);
    /** @type {[number,number,number,number][]} */
    const obstacles=layout.obstacles.map(({center:c,halfExtents:h})=>[c[0]-h[0],c[0]+h[0],c[2]-h[2],c[2]+h[2]]);
    const performancePanel=scene.createInfoPanel('diagnostics',{position:[0,-10000,0],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT})
        .setRenderOptions({castsShadow:false,receivesShadow:false});
    return {environment,layout,enemies,swords,marker,performancePanel,locomotion:{bounds:layout.walkableBounds,obstacles,sceneCollisions:true,teleportSpeed:30,speed:2.5,teleportRange:30,slowScale:.15}};
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

/** @param {import('../../js/engine.js').Engine} engine @param {Awaited<ReturnType<typeof createScene>>} world */
export function startCombat(engine,world){
    for(const [index,hand] of /** @type {const} */(['left','right']).entries())engine.xr.player.equip(hand,world.swords[index]);
    engine.scene.configureMelee({enemies:world.enemies,swords:world.swords,
        spawns:world.enemies.map((_,index)=>world.layout.modelSpawns[index].position),bounds:world.layout.walkableBounds,obstacles:world.locomotion.obstacles});
}
