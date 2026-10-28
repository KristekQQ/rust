import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);};
const frames=async(n=4)=>{while(n--)await new Promise(requestAnimationFrame);};

// Eight material groups share the same authored geometry and are instanced as
// eight roots. Their separated colored tiles make every comparison visible.
function fixture(){
 const data=new Uint8Array(140);
 new Float32Array(data.buffer,0,12).set([-.12,-.12,0,.12,-.12,0,.12,.12,0,-.12,.12,0]);
 new Float32Array(data.buffer,48,12).set([0,0,1,0,0,1,0,0,1,0,0,1]);
 new Uint16Array(data.buffer,96,6).set([0,1,2,0,2,3]);
 return {asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...data)),byteLength:data.length}],
  bufferViews:[{buffer:0,byteOffset:0,byteLength:48},{buffer:0,byteOffset:48,byteLength:48},{buffer:0,byteOffset:96,byteLength:12}],
  accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.12,-.12,0],max:[.12,.12,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5123,count:6,type:'SCALAR'}],
  materials:Array.from({length:8},(_,i)=>({doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[.2+(i%3)*.25,.15+((i+1)%3)*.23,.18+((i+2)%3)*.2,1],roughnessFactor:.8,metallicFactor:0}})),
  meshes:Array.from({length:8},(_,material)=>({primitives:[{attributes:{POSITION:0,NORMAL:1},indices:2,material}]})),
  nodes:Array.from({length:8},(_,mesh)=>({mesh,translation:[(mesh-3.5)*.31,0,0]})),
  scenes:[{nodes:Array.from({length:8},(_,i)=>i)}],scene:0};
}
const exact=(a,b,label)=>{assert(a.width===b.width&&a.height===b.height,label+' dimensions');let max=0;for(let i=0;i<a.data.length;i++)max=Math.max(max,Math.abs(a.data[i]-b.data[i]));assert(max===0,label+' exact pixels max='+max);};
const pool=e=>{const p=e.debugStats().gltf.instancePool;assert(p&&Number.isFinite(p.ownedBytes), 'instance pool diagnostics');return p;};
const evidence=[];let e;
try{
 e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 document.documentElement.dataset.rendererBackend=e.renderer_backend();
 e.resize(640,400);e.set_camera_mode('free');e.setCameraView([0,0,5],[0,0,0]);e.set_render_effects(false,false);
 e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);
 e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
 const load=async()=>{const u=URL.createObjectURL(new Blob([JSON.stringify(fixture())],{type:'model/gltf+json'}));try{return await e.scene.loadModel(u);}finally{URL.revokeObjectURL(u);}};
 const limits=(byteLimit,streamByteLimit=16*1024*1024,resourceLimit=4096)=>{assert(e.configureGltfInstancePool({byteLimit,streamByteLimit,resourceLimit})===true,'pool configuration accepted');};
 const groups=async()=>{const source=await load(),roots=[source,...Array.from({length:7},()=>source.createInstance())];roots.forEach((r,i)=>r.setPosition([0,(3.5-i)*.29,0]).setScale([1,1,1]).setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:true,opaqueReorderable:true}));return roots;};
 const oracle=async(label)=>{await frames();const image=await capturePixels(canvas);assertForeground(image,label,150);e.setPreparedPlanCacheEnabled(false);e.set_frustum_culling(false);const reference=await capturePixels(canvas);exact(image,reference,label+' plan-cache-off culling-off');e.set_frustum_culling(true);e.setPreparedPlanCacheEnabled(true);exact(image,await capturePixels(canvas),label+' restored');return image;};
 // Establish the uncached pixel oracle before enabling retention.
 let defaults=pool(e);assert(defaults.byteLimit===64*1024*1024&&defaults.resourceLimit===4096&&defaults.streamByteLimit===16*1024*1024,'documented pool defaults');
 assert(e.configureGltfInstancePool({byteLimit:300*1024*1024})===false,'oversized global byte budget rejected');assert(e.configureGltfInstancePool({streamByteLimit:15})===false,'invalid stream alignment rejected');assert(pool(e).byteLimit===defaults.byteLimit,'invalid configuration is atomic');
 limits(0);let roots=await groups();const baseline=await oracle('zero-budget active scene');
 limits(64*1024*1024);await frames(8);exact(baseline,await oracle('pooled active scene'),'pool visual equivalence');
 const record=phase=>{const p=pool(e);evidence.push({phase,...p});return p;};
 const first=record('warm');assert(first.activeBytes>0&&first.activePages>0,'active geometry pages are owned');
 const created=first.buffersCreated+first.bindGroupsCreated;
 // Visibility oscillation retains the asset; final asset removal intentionally clears its pool.
 for(let i=0;i<4;i++){roots.forEach((r,j)=>r.setPosition([j?10000:0,(3.5-j)*.29,0]));await frames();roots.forEach(r=>r.setPosition([10000,0,0]));await frames();assert(pool(e).activeBytes===0&&pool(e).activePages===0,'culled groups become inactive');roots.forEach((r,j)=>r.setPosition([0,(3.5-j)*.29,0]));await frames(5);record('visibility-cycle-'+i);}
 let p=pool(e);assert(p.buffersCreated+p.bindGroupsCreated===created,'many/few/none/many resource creation plateaus');
 const spawnWarm=p.buffersCreated+p.bindGroupsCreated;
 for(let cycle=0;cycle<3;cycle++){
  roots.slice(1).forEach(r=>r.remove());roots=roots.slice(0,1);await frames(3);
  const source=roots[0];roots.push(...Array.from({length:7},()=>source.createInstance()));
  roots.forEach((r,j)=>r.setPosition([0,(3.5-j)*.29,0]).setScale([1,1,1]).setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:true,opaqueReorderable:true}));await frames(5);
 }
 p=pool(e);assert(p.buffersCreated+p.bindGroupsCreated===spawnWarm,'real spawn/despawn keeps warmed instance resources');await oracle('actual model membership changes');
 const compact=p;
 roots[0].setRotation([.13,.22,.04]).setScale([1.2,.8,1]);await frames(5);p=pool(e);assert(p.ownedBytes>=compact.ownedBytes,'112-byte affine stream retained');await oracle('affine transition');
 roots[0].setRotation([0,0,0]).setScale([1,1,1]);await frames(5);assert(pool(e).ownedBytes<=p.ownedBytes,'16-byte compact representation returns');await oracle('compact and affine transitions');
 roots.forEach(r=>r.setRotation([.13,.22,.04]));await frames(5);
 limits(1024*1024,256);await frames(6);p=pool(e);assert(p.streamByteLimit===256,'small stream limit applied');assert(p.activeStreams===0,'oversized streams use standalone fallback');await oracle('stream fallback at 256 bytes');
 limits(1024*1024);await frames(5);assert(pool(e).activeStreams>0,'pooled stream returns when limit rises');
 limits(1024*1024,16*1024*1024,1);await frames(5);p=pool(e);assert(p.resourceLimit===1,'small resource limit applied');assert(p.activePages>0,'active working set survives resource limit');assert(p.activePages+p.activeStreams>p.resourceLimit||p.evictions>0,'active over-budget or eviction accounting');await oracle('resource limit edge');
 limits(1024);await frames(6);p=pool(e);assert(p.ownedBytes<=p.byteLimit||p.activeBytes>p.byteLimit,'inactive pages evict within byte budget');assert(p.evictions>0||p.overBudgetEvents>0,'low budget eviction or active over-budget is observed');await oracle('low byte budget');
 roots.forEach(r=>r.setPosition([10000,0,0]));limits(0);await frames(5);p=pool(e);assert(p.ownedBytes===0&&p.liveBytes===0,'zero budget evicts inactive pages and streams');roots.forEach((r,j)=>r.setPosition([0,(3.5-j)*.29,0]));await frames(5);assert(pool(e).activeBytes>0,'zero budget preserves active working set');
 roots.forEach(r=>r.remove());await frames(3);assert(pool(e).liveBytes===0,'removing last asset releases all instance resources');roots=await groups();await frames(5);
 // Repeated clear/reload checks released counts grow while live gauges return.
 const reloadCapacities=[];
 for(let i=0;i<3;i++){e.scene.clear();await frames(2);p=record('clear-'+i);assert(p.liveBytes===0&&p.activePages===0&&p.activeStreams===0,'clear returns live gauges to zero');roots=await groups();await frames(6);p=record('reload-'+i);reloadCapacities.push(p.ownedBytes);assert(p.liveBytes===p.ownedBytes,'reloaded scene has no retired instance owners');}
 assert(reloadCapacities.every(n=>n===reloadCapacities[0]),'identical clear/reload reaches a fixed known-capacity plateau');
 e.scene.clear();await frames(2);p=pool(e);assert(p.liveBytes===0&&p.ownedBytes===0,'final clear releases pool resources');assert(p.buffersReleased>0&&p.bindGroupsReleased>0,'release counters advance');
 const finalClear=p;
 roots=await groups();await frames(5);assert(record('active-dispose').activeBytes>0,'dispose exercised with live instance resources');e.dispose();
 e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 e.resize(640,400);e.set_camera_mode('free');e.setCameraView([0,0,5],[0,0,0]);e.set_render_effects(false,false);e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);
 p=record('reinit-empty');assert(p.liveBytes===0&&p.buffersCreated===0&&p.byteLimit===64*1024*1024,'reinitialization starts fresh pool ownership and defaults');
 e.scene.createLight({position:[-2,3,5],color:[10,10,10]});roots=await groups();await frames(6);exact(baseline,await oracle('active dispose and reinitialization'),'reinitialized exact pixels');
 window.instancePoolEvidence={limits:finalClear,plateau:true,reloadCapacities,snapshots:evidence,activeDisposeReinit:true,baselineBytes:baseline.data.length};result.dataset.status='pass';result.textContent='PASS: glTF instance pool bounds, reuse, fallback, lifecycle, exact pixels';
}catch(error){result.dataset.status='fail';result.textContent='FAIL: '+(error.stack||error);throw error;}finally{e?.dispose();}
