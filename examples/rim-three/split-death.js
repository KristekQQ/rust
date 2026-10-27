import * as THREE from 'three';
import {mergePrimitives} from './mesh-split.js?v=cut-sync-1';
import {createCutWorker} from './cut-worker-client.js?v=cut-sync-1';

function createDissolveMaterial(original,progress){
  const m=original.clone();
      m.onBeforeCompile=shader=>{
        shader.uniforms.splitProgress=progress;
        shader.vertexShader='varying vec3 vSplitPosition;\n'+shader.vertexShader;
        shader.vertexShader=shader.vertexShader.replace('#include <begin_vertex>','#include <begin_vertex>\nvSplitPosition = position;');
        shader.fragmentShader='uniform float splitProgress;\nvarying vec3 vSplitPosition;\nfloat splitNoise(vec3 p){return fract(sin(dot(p,vec3(12.9898,78.233,37.719)))*43758.5453);}\n'+shader.fragmentShader;
        shader.fragmentShader=shader.fragmentShader.replace('#include <alphatest_fragment>',`#include <alphatest_fragment>
          vec3 grid = vSplitPosition * 12.0;
          float field = 0.02 + 0.96 * (splitNoise(floor(grid * 0.5))*0.58 + splitNoise(floor(grid))*0.27 + dot(fract(grid),vec3(0.43,0.35,0.22))*0.15);
          if (splitProgress > 0.0 && field < splitProgress) discard;
          if (splitProgress > 0.0) diffuseColor.rgb += vec3(1.0,0.32,0.035) * (1.0-smoothstep(0.0,0.08,field-splitProgress)) * 2.0;
        `);
      };
      m.customProgramCacheKey=()=> 'rim-solid-dissolve-1';
  return m;
}

// Collision/death is immediate. A worker owns triangle clipping; completed
// geometry is installed atomically by update(), never by a message callback.
export function createSplitDeathSystem(scene,{renderer=null,camera=null}={}){
  const group=new THREE.Group();group.name='Rozříznutí nepřátelé';scene.add(group);
  const cutter=createCutWorker(),mergedCache=new Map(),slots=new Map(),dissolveSlots=new Map();
  let cuts=0,dissolves=0,released=0,failures=0,lastCommitMs=0,maxCommitMs=0,lastWorkerMs=0,lastLatencyMs=0,maxLatencyMs=0,lastDissolveMs=0,maxDissolveMs=0;
  function prepareCut(unit){
    if(slots.has(unit.id))return slots.get(unit.id);
    const source=unit.entry.root,sources=[],merged=new Map(),suppressed=new Set();source.traverse(o=>sources.push(o));
    source.traverse(parent=>{
      if(parent.children.length<2||!parent.children.every(o=>o.isMesh&&!o.children.length&&o.position.lengthSq()<1e-16&&o.quaternion.angleTo(new THREE.Quaternion())<1e-8&&o.scale.distanceToSquared(new THREE.Vector3(1,1,1))<1e-16))return;
      const children=parent.children,key=children.map(o=>o.geometry.uuid+':'+(Array.isArray(o.material)?o.material:[o.material]).map(m=>m.uuid).join(',')).join('|');
      if(!mergedCache.has(key))mergedCache.set(key,mergePrimitives(children));
      merged.set(children[0],mergedCache.get(key));children.slice(1).forEach(o=>suppressed.add(o));
    });
    const progress={value:0},materialCache=new Map();
    function dissolved(original){if(!materialCache.has(original))materialCache.set(original,createDissolveMaterial(original,progress));return materialCache.get(original);}
    const capOriginal=new THREE.MeshStandardMaterial({color:0xff893d,emissive:0xff410a,emissiveIntensity:1.1,roughness:.65,metalness:.15,side:THREE.DoubleSide}),cap=dissolved(capOriginal);
    const halves=[1,-1].map(sign=>{
      const holder=new THREE.Group();holder.visible=false;holder.matrixAutoUpdate=false;group.add(holder);
      const root=source.clone(true),nodes=[],meshes=[];root.traverse(o=>nodes.push(o));holder.add(root);
      nodes.forEach((o,i)=>{if(o.isMesh){
        const original=sources[i];if(suppressed.has(original)){o.visible=false;return;}
        const part=merged.get(original),geometry=part?.geometry||original.geometry,materials=part?.materials||(Array.isArray(original.material)?original.material:[original.material]);
        o.geometry=geometry;o.material=[...materials.map(dissolved),cap];
        meshes.push({source:original,geometry,geometryId:(()=>{try{return cutter.registerGeometry(geometry)}catch{return null}})(),mesh:o,materials:o.material,capIndex:materials.length});
      }});
      return {sign,holder,root,nodes,meshes,mixer:new THREE.AnimationMixer(root)};
    });
    const slot={unit,sources,halves,progress,owned:[],active:false,pending:false,generation:0,job:null,result:null,time:0,age:0,duration:3,normal:new THREE.Vector3(),origin:new THREE.Vector3(),caps:0,generated:0,animation:null,startedFrame:0,committedFrame:null,readyAt:0,fallback:false,error:null};
    slots.set(unit.id,slot);return slot;
  }
  function clear(slot){
    slot.generation++;slot.job?.cancel();slot.job=null;slot.result=null;slot.active=false;slot.pending=false;
    for(const h of slot.halves){h.holder.visible=false;h.mixer.stopAllAction();}
    for(const g of slot.owned){g.dispose();released++;}slot.owned.length=0;
    for(const h of slot.halves)for(const p of h.meshes)p.mesh.geometry=p.geometry;
  }
  function playDeath(slot){
    const clip=slot.unit.entry.animations.find(c=>c.name.toLowerCase()==='explode');
    for(const h of slot.halves)if(h.holder.visible&&clip){const action=h.mixer.clipAction(clip);action.reset().setLoop(THREE.LoopOnce,1);action.clampWhenFinished=true;action.play().fadeIn(.15);}
  }
  function fallback(slot,time,frame,error){
    slot.generation++;slot.job?.cancel();slot.job=null;slot.result=null;slot.pending=false;slot.fallback=true;slot.error=String(error);slot.readyAt=time;slot.committedFrame=frame;failures++;
    slot.halves[0].holder.visible=true;slot.halves[1].holder.visible=false;playDeath(slot);
  }
  function startCut(unit,plane,frame){
    const slot=slots.get(unit.id)||prepareCut(unit);clear(slot);unit.entry.holder.updateWorldMatrix(true,true);
    slot.normal.copy(plane.normal);unit.entry.holder.getWorldPosition(slot.origin);slot.progress.value=0;
    slot.time=0;slot.age=0;slot.active=true;slot.pending=true;slot.caps=0;slot.generated=0;slot.readyAt=null;slot.fallback=false;slot.error=null;slot.startedFrame=frame;slot.committedFrame=null;slot.submittedAt=performance.now();
    const clip=unit.entry.animations.find(c=>c.name.toLowerCase()==='explode');slot.animation=clip?.name||null;slot.duration=Math.max(2.2,Math.min(4,clip?.duration||3));
    slot.halves.forEach((half,side)=>{
      half.nodes.forEach((o,i)=>{const source=slot.sources[i];o.position.copy(source.position);o.quaternion.copy(source.quaternion);o.scale.copy(source.scale);});
      half.holder.matrix.copy(unit.entry.holder.matrixWorld);half.holder.matrix.decompose(half.holder.position,half.holder.quaternion,half.holder.scale);half.holder.visible=side===0;
      for(const p of half.meshes){p.mesh.geometry=p.geometry;p.mesh.material=p.geometry.groups.length?p.materials:p.materials[0];p.mesh.visible=p.source.visible;}
    });
    // Frozen impact-pose preview uses original buffers while the dead enemy has
    // already left all interaction/target lists. No new collision body is created.
    unit.entry.holder.visible=false;
    const meshes=slot.halves[0].meshes.map(p=>{const local=plane.clone().applyMatrix4(p.source.matrixWorld.clone().invert());return {geometryId:p.geometryId,plane:[...local.normal.toArray(),local.constant],capIndex:p.capIndex};});
    const generation=slot.generation;
    try{
      if(meshes.some(m=>m.geometryId===null))throw new Error('Cut worker unavailable');
      slot.job=cutter.cut(meshes);
      slot.job.promise.then(result=>{if(slot.active&&slot.generation===generation)slot.result={value:result};},error=>{if(slot.active&&slot.generation===generation)slot.result={error};});
    }catch(error){slot.result={error};}
    return slot.duration;
  }
  function geometryPart(part,original,owned){
    if(!part)return null;if(part.shared)return original;
    if(!(part.position instanceof Float32Array)||!(part.normal instanceof Float32Array)||part.position.length%9||part.normal.length!==part.position.length||(part.uv&&part.uv.length!==part.position.length/3*2))throw new Error('Invalid worker geometry');
    const geometry=new THREE.BufferGeometry();owned.push(geometry);
    geometry.setAttribute('position',new THREE.BufferAttribute(part.position,3));geometry.setAttribute('normal',new THREE.BufferAttribute(part.normal,3));
    if(part.uv)geometry.setAttribute('uv',new THREE.BufferAttribute(part.uv,2));
    for(const g of part.groups)geometry.addGroup(g.start,g.count,g.materialIndex);
    if(part.boundingBox)geometry.boundingBox=new THREE.Box3(new THREE.Vector3().fromArray(part.boundingBox.min),new THREE.Vector3().fromArray(part.boundingBox.max));else geometry.computeBoundingBox();
    if(part.boundingSphere)geometry.boundingSphere=new THREE.Sphere(new THREE.Vector3().fromArray(part.boundingSphere.center),part.boundingSphere.radius);else geometry.computeBoundingSphere();
    return geometry;
  }
  function commit(slot,time,frame){
    const reply=slot.result;slot.result=null;
    if(reply.error){fallback(slot,time,frame,reply.error);return;}
    const started=performance.now(),owned=[],pairs=[];
    try{
      const outputs=reply.value.meshes;if(outputs.length!==slot.halves[0].meshes.length)throw new Error('Incomplete worker result');
      for(let i=0;i<outputs.length;i++){
        const p=slot.halves[0].meshes[i],o=outputs[i];pairs.push({positive:geometryPart(o.positive,p.geometry,owned),negative:geometryPart(o.negative,p.geometry,owned),caps:o.capTriangles});
      }
    }catch(error){for(const g of owned)g.dispose();fallback(slot,time,frame,error);return;}
    // All results exist before any live mesh is replaced. Message handlers only
    // enqueue data; this transaction runs inside the same update as rendering.
    slot.halves.forEach((half,side)=>{half.holder.visible=true;half.meshes.forEach((p,i)=>{const geometry=side===0?pairs[i].positive:pairs[i].negative;p.mesh.geometry=geometry||p.geometry;p.mesh.visible=!!geometry;p.mesh.material=p.mesh.geometry.groups.length?p.materials:p.materials[0];});});
    slot.owned=owned;slot.generated=owned.length;slot.caps=pairs.reduce((n,p)=>n+p.caps,0);slot.pending=false;slot.job=null;slot.readyAt=time;slot.committedFrame=frame;cuts++;playDeath(slot);
    lastWorkerMs=reply.value.computeMs;lastLatencyMs=performance.now()-slot.submittedAt;maxLatencyMs=Math.max(maxLatencyMs,lastLatencyMs);lastCommitMs=performance.now()-started;maxCommitMs=Math.max(maxCommitMs,lastCommitMs);
  }
  function updateCut(unit,time,dt,frame){
    const slot=slots.get(unit.id);if(!slot?.active)return true;slot.age=time;
    if(slot.pending){
      if(slot.result)commit(slot,time,frame);else if(performance.now()-slot.submittedAt>1500)fallback(slot,time,frame,'Cut worker timeout');
      if(slot.pending)return false;
    }
    slot.time=Math.max(0,time-slot.readyAt);
    const moving=Math.max(0,slot.time-.45),distance=Math.min(.65,slot.time*.52);
    slot.progress.value=slot.fallback?THREE.MathUtils.clamp((slot.time-.1)/(slot.duration-.1),0,1):THREE.MathUtils.clamp((slot.time-slot.duration*.42)/(slot.duration*.58),0,1);
    for(const h of slot.halves){if(!h.holder.visible)continue;
      if(slot.fallback)h.mixer.update(dt);else if(slot.time>.45)h.mixer.update(Math.min(dt,moving));
      h.holder.position.copy(slot.origin);if(!slot.fallback){h.holder.position.addScaledVector(slot.normal,h.sign*distance);h.holder.position.y-=Math.min(.38,moving*moving*.1);}h.holder.updateMatrix();
    }
    if(slot.time>=slot.duration)clear(slot);return !slot.active;
  }
  function prepareDissolve(unit){
    if(dissolveSlots.has(unit.id))return dissolveSlots.get(unit.id);
    const source=unit.entry.root,sources=[],nodes=[],root=source.clone(true),holder=new THREE.Group(),progress={value:0},materials=new Map();
    source.traverse(o=>sources.push(o));root.traverse(o=>nodes.push(o));
    root.traverse(o=>{if(o.isMesh){const variant=original=>{if(!materials.has(original))materials.set(original,createDissolveMaterial(original,progress));return materials.get(original);};o.material=Array.isArray(o.material)?o.material.map(variant):variant(o.material);}});
    holder.visible=false;holder.matrixAutoUpdate=false;holder.add(root);group.add(holder);
    const slot={unit,sources,nodes,holder,root,progress,mixer:new THREE.AnimationMixer(root),active:false,time:0,duration:3,animation:null,startedFrame:0,committedFrame:0};dissolveSlots.set(unit.id,slot);return slot;
  }
  function prepare(unit,mode='dissolve'){if(mode==='cut')return prepareCut(unit);if(mode==='dissolve')return prepareDissolve(unit);throw new Error('Unknown enemy death mode: '+mode);}
  async function ready(){
    try{await cutter.prepare();}catch{/* Worker failure selects the cheap effect at impact. */}
    if(renderer&&camera)await renderer.compileAsync(group,camera,scene);
  }
  function clearDissolve(slot){slot.active=false;slot.holder.visible=false;slot.mixer.stopAllAction();}
  function cancel(unit){const cheap=dissolveSlots.get(unit.id),exact=slots.get(unit.id);if(cheap?.active)clearDissolve(cheap);if(exact?.active)clear(exact);}
  function start(unit,plane,mode='dissolve',frame=0){
    cancel(unit);if(mode==='cut')return startCut(unit,plane,frame);
    if(mode!=='dissolve')throw new Error('Unknown enemy death mode: '+mode);
    const started=performance.now(),slot=dissolveSlots.get(unit.id)||prepareDissolve(unit);slot.mixer.stopAllAction();unit.entry.holder.updateWorldMatrix(true,true);
    slot.nodes.forEach((o,i)=>{const source=slot.sources[i];o.position.copy(source.position);o.quaternion.copy(source.quaternion);o.scale.copy(source.scale);o.visible=source.visible;});
    slot.holder.matrix.copy(unit.entry.holder.matrixWorld);slot.holder.visible=true;slot.progress.value=0;slot.time=0;slot.active=true;slot.startedFrame=frame;slot.committedFrame=frame;
    const clip=unit.entry.animations.find(c=>c.name.toLowerCase()==='explode');slot.animation=clip?.name||null;slot.duration=Math.max(2.2,Math.min(4,clip?.duration||3));
    if(clip){const action=slot.mixer.clipAction(clip);action.reset().setLoop(THREE.LoopOnce,1);action.clampWhenFinished=true;action.play().fadeIn(.15);}
    unit.entry.holder.visible=false;dissolves++;lastDissolveMs=performance.now()-started;maxDissolveMs=Math.max(maxDissolveMs,lastDissolveMs);return slot.duration;
  }
  function update(unit,time,dt,frame=0){
    const slot=dissolveSlots.get(unit.id);if(!slot?.active)return updateCut(unit,time,dt,frame);
    slot.time=time;slot.mixer.update(dt);slot.progress.value=THREE.MathUtils.clamp((time-.1)/(slot.duration-.1),0,1);if(time>=slot.duration)clearDissolve(slot);return !slot.active;
  }
  function describe(){
    const active=[...slots.values()].filter(s=>s.active).map(s=>({id:s.unit.id,mode:s.fallback?'cut-fallback':'cut',pending:s.pending,startedFrame:s.startedFrame,committedFrame:s.committedFrame,time:s.time,age:s.age,duration:s.duration,progress:s.progress.value,halves:s.pending||s.fallback?0:2,capTriangles:s.caps,generatedGeometries:s.generated,animation:s.animation,normal:s.normal.toArray(),error:s.error}));
    for(const s of dissolveSlots.values())if(s.active)active.push({id:s.unit.id,mode:'dissolve',pending:false,startedFrame:s.startedFrame,committedFrame:s.committedFrame,time:s.time,duration:s.duration,progress:s.progress.value,halves:0,capTriangles:0,generatedGeometries:0,animation:s.animation});
    return {capacity:new Set([...slots.keys(),...dissolveSlots.keys()]).size,halfTrees:slots.size*2,dissolveTrees:dissolveSlots.size,mergedGeometries:mergedCache.size,cuts,dissolves,failures,releasedGeometries:released,lastCommitMs,maxCommitMs,lastWorkerMs,lastLatencyMs,maxLatencyMs,lastDissolveMs,maxDissolveMs,worker:cutter.describe(),active,liveGeometries:[...slots.values()].reduce((n,s)=>n+s.owned.length,0)};
  }
  return {prepare,ready,start,update,cancel,describe};
}
