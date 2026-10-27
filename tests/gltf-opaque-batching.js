import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);};
const frames=async(n=5)=>{for(let i=0;i<n;i++)await new Promise(requestAnimationFrame);};
// Two differently colored parts: independent asset loads prevent instancing and
// provide a root-order oracle, rather than mirroring the queue implementation.
function fixture({coplanar=false,alpha='OPAQUE',cpu=false}={}){
 const data=new Uint8Array(208);
 const mask=document.createElement('canvas');mask.width=2;mask.height=2;const ctx=mask.getContext('2d');ctx.fillStyle='white';ctx.fillRect(0,0,1,1);ctx.fillRect(1,1,1,1);
 new Float32Array(data.buffer,176,8).set([0,0,1,0,1,1,0,1]);
 new Float32Array(data.buffer,0,12).set([-.45,-.4,0,.45,-.4,0,.45,.4,0,-.45,.4,0]);
 new Float32Array(data.buffer,48,12).set([0,0,1,0,0,1,0,0,1,0,0,1]);
 new Uint16Array(data.buffer,96,6).set([0,1,2,0,2,3]);
 return {asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...data)),byteLength:208}],
 bufferViews:[{buffer:0,byteOffset:0,byteLength:48},{buffer:0,byteOffset:48,byteLength:48},{buffer:0,byteOffset:96,byteLength:12},{buffer:0,byteOffset:128,byteLength:48},{buffer:0,byteOffset:176,byteLength:32}],
 accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.45,-.4,0],max:[.45,.4,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5123,count:6,type:'SCALAR'},{bufferView:3,componentType:5126,count:4,type:'VEC3'},{bufferView:4,componentType:5126,count:4,type:'VEC2'}],
 ...(alpha==='MASK'?{images:[{uri:mask.toDataURL()}],textures:[{source:0,sampler:0}],samplers:[{magFilter:9728,minFilter:9728}]}:{}),
 materials:[[.85,.04,.02],[.02,.1,.8]].map(c=>({doubleSided:true,alphaMode:alpha,alphaCutoff:.2,pbrMetallicRoughness:{baseColorFactor:[...c,alpha==='BLEND'?.55:1],roughnessFactor:.7,metallicFactor:0,...(alpha==='MASK'?{baseColorTexture:{index:0}}:{})},emissiveFactor:c.map(x=>x*.15)})),
 meshes:[0,1].map(material=>({primitives:[{attributes:{POSITION:0,NORMAL:1,TEXCOORD_0:4},indices:2,material,...(cpu?{targets:[{POSITION:3}]}:{})}]})),
 nodes:[{mesh:0,translation:[-.17,0,coplanar?0:-.12]},{mesh:1,translation:[.17,0,coplanar?0:.12]}],scenes:[{nodes:[0,1]}],scene:0};
}
const delta=(a,b)=>{let max=0,changed=0;for(let i=0;i<a.data.length;i+=4){let d=0;for(let c=0;c<3;c++){const v=Math.abs(a.data[i+c]-b.data[i+c]);max=Math.max(max,v);d+=v;}if(d>12)changed++;}return{max,changed};};
const compare=(a,b,label)=>{const d=delta(a,b);assert(d.max<8&&d.changed<15,label+' '+JSON.stringify(d));return d;};
try{
 const e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 document.documentElement.dataset.rendererBackend=e.renderer_backend();
 e.resize(640,400);e.set_camera_mode('free');e.set_render_effects(false,false);e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);
 const reset=()=>{e.scene.clear();e.scene.createLight({position:[-2,3,5],color:[10,10,10]});};
 const load=async spec=>{const url=URL.createObjectURL(new Blob([JSON.stringify(spec)],{type:'model/gltf+json'}));try{return await e.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}};
 const options=(enabled,isStatic=true)=>({isStatic,castsShadow:false,receivesShadow:true,opaqueReorderable:enabled});
 const positions=[[-.23,-.1,0],[.12,.12,.071],[.4,-.04,.163]];
 const place=(m,i)=>m.setPosition(positions[i]).setScale([.8,1.1,.9]);
 const evidence=[];
 for(const alpha of ['OPAQUE','MASK']){
  for(const mirrored of [false,true]){
   reset();const spec=fixture({alpha}),a=await load(spec),models=[a,a.createInstance(),a.createInstance()];
   models.forEach((m,i)=>place(m,i).setScale([mirrored?-.8:.8,1.1,.9]).setRenderOptions(options(true)));
   for(const eye of [-.032,.032]){
    e.setCameraView([eye,.1,3],[eye,0,0]);const pixels=await capturePixels(canvas);assertForeground(pixels,'overlapping parts',300);
    assert(e.debugStats().mainDrawCalls===2,'opted overlapping rigid roots: '+e.debugStats().mainDrawCalls);
    models.forEach(m=>m.setPosition([10000,0,0]));
    const reference=[];for(let i=0;i<3;i++)reference.push(place(await load(spec),i).setScale([mirrored?-.8:.8,1.1,.9]).setRenderOptions(options(false)));
    const oracle=await capturePixels(canvas);assert(e.debugStats().mainDrawCalls===6,'independent root-order oracle draws');
    evidence.push({alpha,mirrored,eye,delta:compare(pixels,oracle,'independent opaque pixels')});
    reference.forEach(m=>m.remove());models.forEach(place);
    models.forEach(m=>m.setScale([mirrored?-.8:.8,1.1,.9]));
   }
   // Policy-only mutations must invalidate static prepared caches, no transform change.
   const before=await capturePixels(canvas);
   for(const enabled of [false,true,false,true]){
    models.forEach(m=>m.setRenderOptions(options(enabled)));await frames();
    assert(e.debugStats().mainDrawCalls===(enabled?2:6),'static policy transition '+enabled);
    compare(before,await capturePixels(canvas),'toggle pixels');
   }
   models[1].setRenderOptions(options(false));await frames();assert(e.debugStats().mainDrawCalls===6,'non-opted root is an ordering barrier');
   models[1].setRenderOptions(options(true,false));models[2].remove();models[0].setPosition([-.3,.1,.02]);await frames();
   assert(e.debugStats().mainDrawCalls===2,'moving and removal retain one shared group per part');
   models[1].setRenderOptions({...options(true,false),receivesShadow:false});await frames();assert(e.debugStats().mainDrawCalls===4,'different shader options split groups');
  }
 }
 for(const spec of [fixture({coplanar:true}),fixture({alpha:'BLEND'}),fixture({cpu:true})]){
  reset();e.setCameraView([0,.1,3],[0,0,0]);const a=await load(spec),models=[a,a.createInstance(),a.createInstance()];
  models.forEach((m,i)=>m.setPosition([i*.12,0,0]).setRenderOptions(options(!spec.nodes.every(n=>n.translation[2]===0))));
  const actual=await capturePixels(canvas);assert(e.debugStats().mainDrawCalls===6,'default coplanar/blend/deformed fallback');
  models.forEach(m=>m.setPosition([10000,0,0]));
  for(let i=0;i<3;i++)(await load(spec)).setPosition([i*.12,0,0]).setRenderOptions(options(false));
  compare(actual,await capturePixels(canvas),'fallback independent oracle');
 }
 // A completed reorderable run may not leap across an intervening default root.
 reset();const a=await load(fixture()),roots=[a,a.createInstance(),a.createInstance(),a.createInstance()];
 roots.forEach((m,i)=>m.setPosition([i*.05,0,i*.017]).setRenderOptions(options(i!==2)));await frames();
 assert(e.debugStats().mainDrawCalls===6,'two-root run then barrier then singleton');
 // Sparse high UV-slot masks and the full five-map layout must copy the
 // same Float32 bytes as the 96-byte CPU deformation reference.
 for(const textureClass of ['emissive-only','all-five']){
  const captures=[];
  for(const cpu of [false,true]){
   reset();e.setCameraView([0,.1,3],[0,0,0]);const spec=fixture({cpu,alpha:'MASK'});
   for(const m of spec.materials){
    if(textureClass==='emissive-only')delete m.pbrMetallicRoughness.baseColorTexture;
    m.emissiveTexture={index:0};m.emissiveFactor=[.3,.7,.4];
    if(textureClass==='all-five'){
     m.pbrMetallicRoughness.metallicRoughnessTexture={index:0};
     m.normalTexture={index:0,scale:.2};m.occlusionTexture={index:0,strength:.6};
    }
   }
   const source=await load(spec),models=[source,source.createInstance()];
   models.forEach((m,i)=>m.setPosition([i*.4-.2,0,i*.07]).setScale([.8,1.1,.9]).setRenderOptions(options(true)));
   captures.push(await capturePixels(canvas));
  }
  evidence.push({textureClass,compactLayoutDelta:compare(captures[0],captures[1],'sparse/full material layout')});
 }
 // The same primitive is referenced by both a rigid node and a skinned node.
 // Rigid compact and skin/deformed 96-byte buffers must remain distinct.
 const sharedSkinSpec=cpu=>{
  const spec=fixture({cpu}),uri=spec.buffers[0].uri;
  const bytes=new Uint8Array(304);bytes.set(Uint8Array.from(atob(uri.split(',')[1]),c=>c.charCodeAt(0)));
  new Float32Array(bytes.buffer,240,16).set([1,0,0,0,1,0,0,0,1,0,0,0,1,0,0,0]);
  spec.buffers[0]={uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...bytes)),byteLength:304};
  spec.bufferViews.push({buffer:0,byteOffset:208,byteLength:32},{buffer:0,byteOffset:240,byteLength:64});
  spec.accessors.push({bufferView:5,componentType:5123,count:4,type:'VEC4'},{bufferView:6,componentType:5126,count:4,type:'VEC4'});
  spec.meshes[0].primitives[0].attributes.JOINTS_0=5;spec.meshes[0].primitives[0].attributes.WEIGHTS_0=6;
  spec.nodes.push({mesh:0,skin:0,translation:[.17,.1,.3]},{translation:[-.4,.3,.09]});
  spec.skins=[{joints:[3]}];spec.scenes[0].nodes.push(2,3);return spec;
 };
 const sharedCaptures=[];
 for(const cpu of [false,true]){
  reset();e.setCameraView([0,.1,3],[0,0,0]);const source=await load(sharedSkinSpec(cpu));
  const models=[source,source.createInstance(),source.createInstance()];
  models.forEach((m,i)=>m.setPosition([(i-1)*.7,0,i*.07]).setScale([.4,.4,.4]).setRenderOptions(options(true)));
  const pixels=await capturePixels(canvas);assertForeground(pixels,'rigid and skin shared primitive',100);sharedCaptures.push(pixels);
 }
 evidence.push({sharedRigidSkinDelta:compare(sharedCaptures[0],sharedCaptures[1],'shared rigid/skin vs CPU morph oracle')});
 // Nearby roots share part groups beyond the old 128-root scheduling cap.
 // Toggling the policy retains a separately ordered root path as pixel oracle.
 reset();e.setCameraView([0,.1,3],[0,0,0]);const windowSource=await load(fixture());
 const windowRoots=[windowSource,...Array.from({length:128},()=>windowSource.createInstance())];
 windowRoots.forEach((m,i)=>m.setPosition([i*.0001,0,i*.00001]).setScale([.3,.3,.3]).setRenderOptions(options(true)));
 const containedBefore=e.debugStats().gltf.rootFullyInside;
 const widePixels=await capturePixels(canvas);assert(e.debugStats().mainDrawCalls===2,'129 nearby roots share two part groups');
 assert(e.debugStats().gltf.rootFullyInside>containedBefore,'cached root containment skips primitive frustum checks');
 windowRoots.forEach(m=>m.setRenderOptions(options(false)));
 const orderedPixels=await capturePixels(canvas);assert(e.debugStats().mainDrawCalls===258,'overlapping root-order oracle is unbatched');
 evidence.push({nearby129:compare(widePixels,orderedPixels,'129-root independent-order pixels')});
 windowRoots.forEach(m=>m.setRenderOptions(options(true)));
 windowRoots.at(-1).remove();await frames();assert(e.debugStats().mainDrawCalls===2,'removal retains nearby grouping');
 // A distant moved root splits by view depth, not count. Camera movement must
 // reevaluate that decision even if every model pose/cache remains unchanged.
 reset();e.setCameraView([0,.1,3],[0,0,0]);const depthSource=await load(fixture());
 const depthRoots=[depthSource,depthSource.createInstance(),depthSource.createInstance()];
 depthRoots.forEach((m,i)=>m.setPosition([0,0,-i]).setScale([.3,.3,.3]).setRenderOptions(options(true,false)));
 await frames();assert(e.debugStats().mainDrawCalls===2,'three nearby roots share a depth window');
 const spatialBefore=e.debugStats().gltf.batchBreaks.rootSpatial;
 depthRoots[2].setPosition([0,0,-30]);await frames();assert(e.debugStats().mainDrawCalls===4,'far moved root starts a separate depth window');
 assert(e.debugStats().gltf.batchBreaks.rootSpatial>spatialBefore,'spatial barrier counter records actual split');
 e.setCameraView([0,.1,50],[0,0,0]);await frames();assert(e.debugStats().mainDrawCalls===2,'camera alone changes depth grouping');
 e.setCameraView([0,.1,3],[0,0,0]);depthRoots[2].setPosition([0,0,-2]);await frames();assert(e.debugStats().mainDrawCalls===2,'model motion restores nearby grouping');
 // Authored glTF cameras have negative projection determinant in this engine.
 // PipelineKey.reflected records winding here; this is still the main color pass.
 reset();const cameraSpec=fixture();cameraSpec.cameras=[{type:'perspective',perspective:{yfov:1,znear:.08,zfar:100}}];
 cameraSpec.nodes.push({camera:0,translation:[0,.1,3]});cameraSpec.scenes[0].nodes.push(2);
 const cameraSource=await load(cameraSpec),cameraModels=[cameraSource,cameraSource.createInstance(),cameraSource.createInstance()];
 cameraModels.forEach((m,i)=>m.setPosition([i*.13,0,i*.071]).setRenderOptions(options(false)));cameraSource.useCamera(2);
 const cameraBefore=await capturePixels(canvas);assert(e.debugStats().mainDrawCalls===6,'authored camera legacy commands');
 cameraModels.forEach(m=>m.setRenderOptions(options(true)));e.setDebugOverlay(true);await frames();
 const cameraRecords=e.debugStats().gltfDraws.draws.filter(r=>!r.shadow&&!r.blend);
 assert(cameraRecords.length===2&&cameraRecords.every(r=>r.reflected&&r.instances===3),'negative-determinant main camera actually batches');
 e.setDebugOverlay(false);evidence.push({authoredCameraDelta:compare(cameraBefore,await capturePixels(canvas),'authored camera independent ordering')});
 // Auxiliary passes keep legacy ordering even when main color opts into batching.
 reset();e.setCameraView([0,.8,4],[0,0,0]);
 const auxSource=await load(fixture()),auxModels=[auxSource,auxSource.createInstance(),auxSource.createInstance()];
 auxModels.forEach(place);
 const mirror=e.scene.createPlane([0,0,-2]).setRotation([Math.PI/2,0,0]).setScale([4,1,3]).setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,reflectivity:.9});
 e.scene.setMirror(mirror);e.set_render_effects(true,true);e.setDebugOverlay(true);
 const auxSnapshots=[];
 for(const enabled of [false,true]){
  e.setDebugOverlay(true);
  auxModels.forEach(m=>m.setRenderOptions({...options(enabled),castsShadow:true}));
  let records=[];
  for(let i=0;i<6;i++){
   await frames(1);const d=e.debugStats().gltfDraws.draws;
   if(d.some(r=>r.shadow)&&d.some(r=>r.reflected))records=d.filter(r=>r.shadow||r.reflected);
  }
  assert(records.some(r=>r.shadow)&&records.some(r=>r.reflected),'actual shadow and reflection commands observed');
  assert(records.every(r=>r.instances===1),'auxiliary overlapping roots remain unbatched');
  // Diagnostics also draw changing HUD text; exclude it from image comparison.
  e.setDebugOverlay(false);
  auxSnapshots.push({records,pixels:await capturePixels(canvas)});
 }
 assert(JSON.stringify(auxSnapshots[0].records)===JSON.stringify(auxSnapshots[1].records),'auxiliary pass ordering unchanged');
 evidence.push({auxiliaryDelta:compare(auxSnapshots[0].pixels,auxSnapshots[1].pixels,'shadow/reflection image'),auxiliaryCommands:auxSnapshots[1].records.length});
 e.setDebugOverlay(false);e.set_render_effects(false,false);
 e.scene.clear();await frames();assert(e.debugStats().gltf.instanceBufferBytes===0,'clear releases all instance storage');assert(e.debugStats().gltf.activeDrawIndexBytes===0,'clear releases active-draw index caches');
 result.dataset.status='pass';result.textContent='PASS: '+JSON.stringify(evidence);
}catch(error){console.error(error);result.dataset.status='fail';result.textContent='FAIL: '+(error.stack||error);}
