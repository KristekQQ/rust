/** @typedef {[number,number,number]} Vec3 */
/** @typedef {{name:string,nodeIndex:number,position:Vec3,target:Vec3,fovDegrees:number}} RimCamera */
/** @typedef {{slug:string,name:string,file:string,height:number,holder:{position:Vec3},root:{position:Vec3,scale:Vec3}}} RimModel */
/** @typedef {{city:{meshes:number,triangles:number,materials:number},cityFile:string,decorationsFile:string,cameras:RimCamera[],models:RimModel[]}} RimLayout */

/** Original Aurelia geometry and original animated models; all glTF decoding,
 * poses, PBR and rendering belong to Rust. No Three.js dependency at runtime.
 * @param {import('../../js/engine.js').Scene} scene */
export async function createScene(scene){
    scene.clear();
    const base=new URL('../../assets/models/rim/',import.meta.url);
    const response=await fetch(new URL('layout.json',base));
    if(!response.ok)throw new Error(`Rozvržení Říma: HTTP ${response.status}`);
    const layout=/** @type {RimLayout} */(await response.json());
    const environment=await scene.loadModel(new URL(layout.cityFile,base));
    environment.setRenderOptions({isStatic:true,castsShadow:true,receivesShadow:true});
    const decorations=await scene.loadModel(new URL(layout.decorationsFile,base));
    decorations.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
    const characters=[];
    for(const spec of layout.models){
        const model=await scene.loadModel(new URL(spec.file,base));
        const position=/** @type {Vec3} */(spec.holder.position.map((v,i)=>v+spec.root.position[i]));
        model.setPosition(position).setScale(spec.root.scale).playAnimation('Idle',{loop:true});
        model.setRenderOptions({isStatic:false,castsShadow:false,receivesShadow:true,opaqueReorderable:true});
        characters.push({model,spec});
    }
    // Authored sun. The explicit appearance profile supplies sky/ground diffuse fill.
    const linear=(/** @type {number} */c)=>{const v=c/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;};
    scene.createLight({position:[-45,75,35],color:[linear(255)*3.1,linear(224)*3.1,linear(171)*3.1]});
    return {environment,decorations,characters,layout};
}

/** Explicit authoring only: IDs, imported original assets and native encounter configuration.
 * @param {import('../../js/engine.js').Engine} engine
 * @param {Awaited<ReturnType<typeof createScene>>} content */
export async function createGameplay(engine,content){
 const scene=engine.scene,base=new URL('../../assets/models/rim/',import.meta.url);
 const response=await fetch(new URL('collisions.json',base));if(!response.ok)throw Error('Kolize města nejsou dostupné.');const collision=await response.json();
 content.decorations.setPosition([0,-10000,0]);
 const enemies=[];
 for(const kind of ['drone-orbiter','core-tank','blade-wraith']){
  const source=content.characters.find(c=>c.spec.slug===kind);if(!source)throw Error('Chybí původní model '+kind);
  const count=kind==='drone-orbiter'?8:3;
  for(let i=0;i<count;i++){
   const model=i===0?source.model:source.model.createInstance();
   model.setRenderOptions({castsShadow:false,receivesShadow:true,opaqueReorderable:true});
   enemies.push({model,kind,height:source.spec.height,offset:source.spec.root.position,scale:source.spec.root.scale});
  }
 }
 const swordAsset=await scene.loadModel(new URL('gladius.glb',base));
 const swords=/** @type {[import('../../js/engine.js').ModelObject,import('../../js/engine.js').ModelObject]} */([swordAsset,swordAsset.createInstance()]);
 for(const sword of swords)sword.setCutter({base:[0,0,-.14],tip:[0,0,-1.02]}).setBladeTrail(true).setRenderOptions({castsShadow:false,receivesShadow:false});
 const bolts=Array.from({length:32},()=>scene.createSphere([0,-10000,0]).setScale([.055,.055,.055]).setEmission(1.5).setRenderOptions({castsShadow:false,receivesShadow:false}));
 const beams=Array.from({length:14},()=>scene.createCube([0,-10000,0]).setEmission(3).setRenderOptions({castsShadow:false,receivesShadow:false}));
 const charges=Array.from({length:14},()=>scene.createSphere([0,-10000,0]).setEmission(2).setRenderOptions({castsShadow:false,receivesShadow:false}));
 // Fixed colored cube geometry; native code owns all bar transforms and lifetime.
 const positions=[-.5,-.5,-.5,.5,-.5,-.5,.5,.5,-.5,-.5,.5,-.5,-.5,-.5,.5,.5,-.5,.5,.5,.5,.5,-.5,.5,.5];
 const indices=[0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,3,7,6,3,6,2,0,4,7,0,7,3,1,2,6,1,6,5];
 const bar=(/** @type {Vec3} */color)=>scene.createMesh({positions,indices,color}).setPosition([0,-10000,0]).setEmission(1).setRenderOptions({castsShadow:false,receivesShadow:false});
 const healthBars=enemies.map(()=>/** @type {[import('../../js/engine.js').SceneObject,import('../../js/engine.js').SceneObject]} */([bar([.025,.035,.05]),bar([.12,.9,.3])]));
 // Shared thin closed disk: the mesh pool validates closed solids. Flattened
 // geometry gives the same ground circle; two alpha faces compose to ~.16.
 const diskPositions=[0,0,0],diskIndices=[],segments=32,bottom=segments+1;
 for(let i=0;i<segments;i++){const a=i*Math.PI*2/segments;diskPositions.push(Math.cos(a),0,Math.sin(a));}
 diskPositions.push(0,-.001,0);
 for(let i=0;i<segments;i++){const a=i*Math.PI*2/segments;diskPositions.push(Math.cos(a),-.001,Math.sin(a));}
 for(let i=0;i<segments;i++){const t=i+1,n=(i+1)%segments+1,b=bottom+t,bn=bottom+n;diskIndices.push(0,n,t,bottom,b,bn,t,n,b,n,bn,b);}
 const disk=scene.createMeshVariants({positions:diskPositions,indices:diskIndices,variants:[new Array(diskIndices.length).fill(0)]});
 const blobShadows=enemies.map(()=>disk.createInstance(0).setPosition([0,-10000,0]).setOpacity(.08).setRenderOptions({isStatic:false,castsShadow:false,receivesShadow:false}));
 disk.remove();
 const hud=Array.from({length:5},()=>scene.createCube([0,-10000,0]).setEmission(1).setRenderOptions({castsShadow:false,receivesShadow:false}));
 const marker=scene.createSphere([0,-10000,0]).setScale([.3,.025,.3]).setEmission(1).setRenderOptions({castsShadow:false,receivesShadow:false});
 scene.configureRim({enemies,swords,bolts,beams,charges,healthBars,blobShadows,hud,marker,shapes:collision.shapes,obstacles:collision.obstacles.map((/** @type {{minX:number,maxX:number,minZ:number,maxZ:number}} */r)=>[r.minX,r.maxX,r.minZ,r.maxZ])});
 for(const [i,hand] of /** @type {const} */(['left','right']).entries())engine.xr.player.equip(hand,swords[i],{requireTrigger:false});
 return {enemies,swords,bolts,beams,charges,healthBars,blobShadows,hud,marker,collision};
}
