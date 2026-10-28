import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);};
// The zero morph target forces the established exact CPU reference path.
export function fixture({cpu=false,far=false,offsets=null,alpha='OPAQUE'}={}){
 const data=new Uint8Array(128+48);new Float32Array(data.buffer,0,12).set([-.3,-.3,0,.3,-.3,0,.3,.3,0,-.3,.3,0]);
 new Float32Array(data.buffer,48,12).set([0,0,1,0,0,1,0,0,1,0,0,1]);new Uint16Array(data.buffer,96,6).set([0,1,2,0,2,3]);
 const a={asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...data)),byteLength:data.length}],
 bufferViews:[{buffer:0,byteOffset:0,byteLength:48},{buffer:0,byteOffset:48,byteLength:48},{buffer:0,byteOffset:96,byteLength:12},{buffer:0,byteOffset:128,byteLength:48}],
 accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.3,-.3,0],max:[.3,.3,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5123,count:6,type:'SCALAR'},{bufferView:3,componentType:5126,count:4,type:'VEC3'}],
 materials:[{pbrMetallicRoughness:{baseColorFactor:[.85,.15,.07,1],metallicFactor:.2,roughnessFactor:.6},alphaMode:alpha,doubleSided:true,emissiveFactor:[.05,.01,0]}],
 meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1},indices:2,material:0,...(cpu?{targets:[{POSITION:3}]}:{})}]}],nodes:(offsets|| (far?[0,35]:[0])).map(x=>({mesh:0,translation:[x,0,0]})),scenes:[{nodes:(offsets|| (far?[0,35]:[0])).map((_,i)=>i)}],scene:0};
 return URL.createObjectURL(new Blob([JSON.stringify(a)],{type:'model/gltf+json'}));
}
function delta(a,b){let max=0,changed=0;for(let i=0;i<a.data.length;i+=4){let d=0;for(let j=0;j<3;j++){const v=Math.abs(a.data[i+j]-b.data[i+j]);max=Math.max(max,v);d+=v;}if(d>12)changed++;}return{max,changed};}
const frames=async(n=5)=>{for(let i=0;i<n;i++)await new Promise(requestAnimationFrame);};
try{
 const engine=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 engine.resize(640,400);engine.set_camera_mode('free');engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_simulation_paused(true);engine.setCameraView([0,.2,5],[0,0,0]);
 const reset=()=>{engine.scene.clear();engine.scene.createLight({position:[-2,3,5],color:[12,12,12]});};
 const captures=[];const evidence=[];
 for(const cpu of [false,true]){
  reset();const rigidBytesBefore=engine.debugStats().gltf.rigidUploadBytes;const url=fixture({cpu}),source=await engine.scene.loadModel(url);URL.revokeObjectURL(url);const models=[source,source.createInstance(),source.createInstance()];
  for(const mirrored of [false,true]){
   models.forEach((m,i)=>m.setPosition([(i-1)*1.4,0,0]).setScale([mirrored?-.9:.9,1.2,.8]).setRotation([.1,i*.13,0]));
   const pixels=await capturePixels(canvas);assertForeground(pixels,'three actual model instances',300);
   const stats=engine.debugStats();
   if(!cpu)assert(stats.gltf.rigidUploadBytes-rigidBytesBefore===4*40+6*4,'normal-map-free shared geometry uploads40-byte vertices and original indices');
   evidence.push({cpu,mirrored,draws:stats.mainDrawCalls,gltf:stats.gltf});captures.push(pixels);
   assert(stats.mainDrawCalls===(cpu?3:1),`${cpu?'CPU reference':'rigid instances'} ${mirrored}: ${stats.mainDrawCalls} draws`);
   if(cpu)assert(stats.gltf.batchBreaks.rootBounds>0&&stats.gltf.batchBreaks.drawDeformed===0,'morph fallback stops at unavailable bounds before draw-merge checks '+JSON.stringify(stats.gltf.batchBreaks));
  }
  if(!cpu){
   assert(engine.debugStats().gltf.batchBreaks.rootBlend===0,'empty blend queue is not a rejected visible primitive');const overlapBefore=engine.debugStats().gltf.batchBreaks.rootOverlap;models.forEach(m=>m.setPosition([0,0,0]));await frames();assert(engine.debugStats().mainDrawCalls===3,'overlapping roots preserve original separate draw order');assert(engine.debugStats().gltf.batchBreaks.rootOverlap>overlapBefore,'overlap barrier counter follows actual separate draws');
   models.forEach((m,i)=>m.setPosition([(i-1)*1.4,0,0]));await frames();assert(engine.debugStats().gltf.instanceBufferBytes>0,'instance page allocated');
  }
 }
 for(let i=0;i<2;i++){const d=delta(captures[i],captures[i+2]);assert(d.changed<15&&d.max<8,'instanced/CPU pixel parity '+JSON.stringify(d));evidence.push({pixelDelta:d});}
 reset();const url=fixture({far:true}),model=await engine.scene.loadModel(url);URL.revokeObjectURL(url);
 engine.set_frustum_culling(false);const unculled=await capturePixels(canvas);assert(engine.debugStats().mainDrawCalls===2,'whole object submits both nodes with culling disabled');
 const before=engine.debugStats().gltf;engine.set_frustum_culling(true);const culled=await capturePixels(canvas);const after=engine.debugStats();
 assert(after.mainDrawCalls===1&&after.gltf.primitiveCulled>before.primitiveCulled,'offscreen primitive rejected within visible root');assert(delta(unculled,culled).changed===0,'primitive culling preserves pixels');
 engine.setCameraView([35,.2,5],[35,0,0]);await frames();assert(engine.debugStats().mainDrawCalls===1,'camera selects other primitive without pose update');
 // Stable capacity is narrower than zero allocation: only the seven reusable
 // planning vectors are counted, not prepared GPU handle/diagnostic lists.
 await frames();const growth=engine.debugStats().gltf.queueScratchGrowths;await frames(10);
 assert(engine.debugStats().gltf.queueScratchGrowths===growth,'stable queue scratch capacities');
 assert(engine.debugStats().gltf.queueScratchBytes>0,'scratch memory is reported');
 // Unequal culling gaps must preserve ordinal, then model order across a shared
 // asset run. Stable repeated frames and camera changes exercise reused indices.
 reset();engine.setDebugOverlay(true);engine.setCameraView([0,0,2],[0,0,0]);
 const gapUrl=fixture({offsets:[-.65,0,.65]}),gapSource=await engine.scene.loadModel(gapUrl);
 const gaps=[gapSource,gapSource.createInstance(),gapSource.createInstance()];gaps.forEach((m,i)=>m.setPosition([(i-1)*2.1,0,0]));
 const records=blend=>engine.debugStats().gltfDraws.draws.filter(d=>!d.shadow&&!d.reflected&&d.blend===blend&&d.viewSlot===0);
 const grouped=records=>records.map(d=>({node:d.node,primitive:d.primitive,objects:d.objects,instances:d.instances}));
 const references=[];for(let i=0;i<3;i++)references.push(await engine.scene.loadModel(gapUrl));URL.revokeObjectURL(gapUrl);
 const position=(models,visible)=>models.forEach((m,i)=>m.setPosition([(visible?0:10000)+(i-1)*2.1,0,0]));position(references,false);
 const pairs=(draws,models)=>draws.flatMap(d=>d.objects.map(id=>[d.node,models.findIndex(m=>m.id===id)])).sort((a,b)=>a[0]-b[0]||a[1]-b[1]);
 const gapEvidence=[];
 for(const camera of [[0,0,2],[.8,0,2]]){
  engine.setCameraView(camera,[camera[0],0,0]);await frames();const draws=records(false);
  assert(draws.length>0&&draws.some(d=>d.instances<3),'unequal culling columns');
  let previous=-1;for(const d of draws){assert(d.node>=previous,'ascending draw ordinal');previous=d.node;assert(d.objects.every((id,i)=>!i||id>d.objects[i-1]),'model order within a column');}
  const before=JSON.stringify(grouped(draws));await frames();assert(JSON.stringify(grouped(records(false)))===before,'reused scratch preserves ordered diagnostics');
  const expectedPairs=pairs(draws,gaps);engine.setDebugOverlay(false);const image=await capturePixels(canvas);position(gaps,false);position(references,true);engine.setDebugOverlay(true);await frames();const reference=records(false);
  assert(reference.every(d=>d.instances===1),'independent assets bypass ordinal merge');
  assert(JSON.stringify(pairs(reference,references))===JSON.stringify(expectedPairs),'all visible node/object pairs match independent draws');
  engine.setDebugOverlay(false);const diff=delta(image,await capturePixels(canvas));assert(diff.changed<15&&diff.max<8,'sparse merged/reference pixels '+JSON.stringify(diff));
  gapEvidence.push(expectedPairs);position(references,false);position(gaps,true);engine.setDebugOverlay(true);

 }
 assert(JSON.stringify(gapEvidence[0])!==JSON.stringify(gapEvidence[1]),'camera changes sparse visible pairs');evidence.push({gapColumns:gapEvidence});
 // Stable blend sorting must retain import order at exactly equal distances.
 reset();engine.setCameraView([0,0,5],[0,0,0]);const blendUrl=fixture({alpha:'BLEND'}),blendSource=await engine.scene.loadModel(blendUrl);URL.revokeObjectURL(blendUrl);
 const blendOther=blendSource.createInstance();blendSource.setPosition([-.9,0,0]);blendOther.setPosition([.9,0,0]);await frames();
 assert(JSON.stringify(records(true).map(d=>d.object))===JSON.stringify([blendSource.id,blendOther.id]),'equal-distance blend order is stable');assert(engine.debugStats().gltf.batchBreaks.rootBlend>0,'blend barrier is reported');
 // The root-run itself is not capped at1024: only individual draw groups are.
 reset();engine.set_frustum_culling(false);const manyUrl=fixture(),manySource=await engine.scene.loadModel(manyUrl);URL.revokeObjectURL(manyUrl);
 const many=[manySource];for(let i=1;i<1025;i++)many.push(manySource.createInstance());many.forEach((m,i)=>m.setPosition([i,0,0]));await frames(2);
 const batches=records(false);assert(JSON.stringify(batches.map(d=>d.instances))==='[1024,1]','1025 disjoint roots split at1024');
 assert(JSON.stringify(batches.flatMap(d=>d.objects))===JSON.stringify(many.map(m=>m.id)),'group cap preserves every object in order');
 assert(engine.debugStats().gltf.batchBreaks.drawCap>0,'1024 boundary reports an actual capacity barrier');evidence.push({groupBoundary:batches.map(d=>d.instances),batchBreaks:engine.debugStats().gltf.batchBreaks});engine.setDebugOverlay(false);
 engine.scene.clear();await frames();assert(engine.debugStats().gltf.instanceBufferBytes===0,'scene.clear releases instance pages');
 assert(engine.debugStats().gltf.queueScratchBytes===0,'scene.clear releases queue scratch storage');
 // Several instanced columns use byte-offset slices of the same pass/view
 // stream. Grow, shrink and change one root without changing draw ordering.
 reset();engine.setDebugOverlay(true);engine.set_frustum_culling(true);engine.setCameraView([0,0,6],[0,0,0]);
 const streamUrl=fixture({offsets:[-.65,0,.65]}),streamSource=await engine.scene.loadModel(streamUrl);URL.revokeObjectURL(streamUrl);
 const streamModels=[streamSource];for(let i=1;i<9;i++)streamModels.push(streamSource.createInstance());
 let previousBytes=0;const streamEvidence=[];
 for(const count of [2,3,9,3]){
  streamModels.forEach((m,i)=>m.setPosition([i<count?(i-(count-1)/2)*.7:10000,0,0]).setScale([.28,.28,.28]).setRotation([0,0,0]));await frames();
  const compactBefore=engine.debugStats().gltf;
  streamModels[count-1].setPosition([((count-1)-(count-1)/2)*.7+.01,0,0]);await frames(1);const compactAfter=engine.debugStats().gltf;
  assert(compactAfter.translatedDrawCalls>compactBefore.translatedDrawCalls,'actual shared-basis GPU commands required');
  const compactUploaded=compactAfter.instanceUploadBytes-compactBefore.instanceUploadBytes;
  assert(compactUploaded>0&&compactUploaded<=compactAfter.fineLayoutReservedBytes,'translation upload stays inside bounded reserved slots: '+compactUploaded);
  assert(compactAfter.fineLayoutActiveBytes===count*3*16,'all live translation instance records remain sixteen bytes');
  assert(compactAfter.finePayloadBytesWritten-compactBefore.finePayloadBytesWritten===3*16,'one translated root patches exactly its three16-byte records');
  streamModels[count-1].setPosition([((count-1)-(count-1)/2)*.7,0,0]);await frames();
  const before=engine.debugStats().gltf;streamModels[count-1].setRotation([.07,.11,0]);await frames(1);const after=engine.debugStats().gltf;
  assert(after.bufferUploadCalls-before.bufferUploadCalls===4,'full-matrix fallback updates three material uniforms and one shared stream');
  const affineUploaded=after.instanceUploadBytes-before.instanceUploadBytes;
  assert(affineUploaded>0&&affineUploaded<=after.fineLayoutReservedBytes,'affine upload covers live records within bounded reserved slots: '+affineUploaded);
  assert(after.fineLayoutActiveBytes===count*3*112,'all live affine instance records retain112-byte representation');
  assert(after.instanceBufferBytes>=previousBytes,'stream capacity survives shrink');previousBytes=after.instanceBufferBytes;
  const draws=records(false);assert(draws.length===3&&draws.every(d=>d.instances===count),'stream slices preserve primitive/instance counts');streamEvidence.push({count,uploadCalls:after.bufferUploadCalls-before.bufferUploadCalls,uploadBytes:after.instanceUploadBytes-before.instanceUploadBytes,bufferBytes:after.instanceBufferBytes});
 }
 engine.setDebugOverlay(false);await frames();const streamPixels=await capturePixels(canvas);streamModels.forEach(m=>m.setPosition([10000,0,0]));const streamReferenceUrl=fixture({cpu:true,offsets:[-.65,0,.65]}),streamReference=await engine.scene.loadModel(streamReferenceUrl);URL.revokeObjectURL(streamReferenceUrl);
 const streamReferences=[streamReference,streamReference.createInstance(),streamReference.createInstance()];streamReferences.forEach((m,i)=>m.setPosition([(i-1)*.7,0,0]).setScale([.28,.28,.28]).setRotation(i===2?[.07,.11,0]:[0,0,0]));await frames();const streamDelta=delta(streamPixels,await capturePixels(canvas));assert(streamDelta.max<8&&streamDelta.changed<15,'stream offsets match CPU deformation reference '+JSON.stringify(streamDelta));evidence.push({instanceStream:streamEvidence,cpuReferenceDelta:streamDelta});
 // Distinct material factors/UV sets must share identical texture resources,
 // while a different sampler must still require its own group. Draw order and
 // each primitive's material uniform are deliberately left unchanged.
 function bindingsFixture(textured=false,uniqueBindings=false){
  const bytes=new Uint8Array(172);new Float32Array(bytes.buffer,0,12).set([-.3,-.3,0,.3,-.3,0,.3,.3,0,-.3,.3,0]);
  new Float32Array(bytes.buffer,48,12).set([0,0,1,0,0,1,0,0,1,0,0,1]);new Float32Array(bytes.buffer,96,8).set([0,0,1,0,1,1,0,1]);new Uint16Array(bytes.buffer,128,6).set([0,1,2,0,2,3]);new Float32Array(bytes.buffer,140,8).set([1,0,0,0,0,1,1,1]);
  const count=textured?4:5,colors=[[.9,.1,.1,1],[.1,.9,.1,1],[.1,.1,.9,1],[.9,.8,.1,1],[.8,.1,.8,1]];
  const textureCanvas=document.createElement('canvas');textureCanvas.width=textureCanvas.height=2;const context=textureCanvas.getContext('2d');context.fillStyle='#fff';context.fillRect(0,0,2,2);if(textured){context.fillStyle='#777';context.fillRect(0,0,1,1);}
  const a={asset:{version:'2.0'},buffers:[{byteLength:bytes.length,uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...bytes))}],bufferViews:[{buffer:0,byteOffset:0,byteLength:48},{buffer:0,byteOffset:48,byteLength:48},{buffer:0,byteOffset:96,byteLength:32},{buffer:0,byteOffset:128,byteLength:12},{buffer:0,byteOffset:140,byteLength:32}],accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.3,-.3,0],max:[.3,.3,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5126,count:4,type:'VEC2'},{bufferView:3,componentType:5123,count:6,type:'SCALAR'},{bufferView:4,componentType:5126,count:4,type:'VEC2'}],materials:Array.from({length:count},(_,i)=>({pbrMetallicRoughness:{baseColorFactor:colors[i],metallicFactor:.1*i,roughnessFactor:.8,...(textured?{baseColorTexture:{index:i===2?1:0,texCoord:i===1?1:0}}:{})},doubleSided:true})),meshes:Array.from({length:count},(_,i)=>({primitives:[{attributes:{POSITION:0,NORMAL:1,TEXCOORD_0:2,TEXCOORD_1:4},indices:3,material:i}]})),nodes:Array.from({length:count},(_,i)=>({mesh:i,translation:[(i-(count-1)/2)*.75,0,0]})),scenes:[{nodes:Array.from({length:count},(_,i)=>i)}],scene:0};
  if(textured){a.images=[{uri:textureCanvas.toDataURL()}];a.samplers=[{magFilter:9728,minFilter:9728},{magFilter:9729,minFilter:9729}];a.textures=[{source:0,sampler:0},{source:0,sampler:1}];}
  if(uniqueBindings){a.images=[{uri:textureCanvas.toDataURL()}];a.samplers=[{magFilter:9728,minFilter:9728},{magFilter:9729,minFilter:9729}];a.textures=Array.from({length:count},(_,i)=>({source:0,sampler:textured&&i===2?1:0}));a.materials.forEach((m,i)=>{if(textured)m.pbrMetallicRoughness.baseColorTexture.index=i;else m.occlusionTexture={index:i};});}
  return URL.createObjectURL(new Blob([JSON.stringify(a)],{type:'model/gltf+json'}));
 }
 for(const textured of [false,true]){
  reset();engine.setCameraView([0,0,4],[0,0,0]);engine.scene.createLight({position:[0,3,5],color:[6,6,6]});
  const url=bindingsFixture(textured),model=await engine.scene.loadModel(url);URL.revokeObjectURL(url);const materialModels=[model,model.createInstance(),model.createInstance()];materialModels.forEach((m,i)=>m.setPosition([(i-1)*1.3,0,0]).setScale([.3,.3,.3]));await frames();
  const pixels=await capturePixels(canvas);assertForeground(pixels,'material bindings');
  const start=engine.debugStats().gltf;await frames(3);const end=engine.debugStats().gltf;
  const draws=end.drawCalls-start.drawCalls,bindings=end.bindGroupChanges-start.bindGroupChanges,n=textured?4:5,expected=textured?7:6;
  assert(draws>0&&bindings*n===draws*expected,`actual texture bindings ${textured}: ${bindings}/${draws}, expected ${expected}/${n}`);
  // Independent resource keys deliberately prevent bind-group sharing in the
  // reference. White AO maps preserve untextured math; textured references
  // retain the same pixels/sampler/UV while using separate texture indices.
  const referenceUrl=bindingsFixture(textured,true),reference=await engine.scene.loadModel(referenceUrl);URL.revokeObjectURL(referenceUrl);const materialReferences=[reference,reference.createInstance(),reference.createInstance()];materialReferences.forEach((m,i)=>m.setPosition([(i-1)*1.3,0,0]).setScale([.3,.3,.3]));materialModels.forEach(m=>m.setPosition([10000,0,0]));await frames();const other=await capturePixels(canvas);
  assert(delta(pixels,other).max===0,'shared binding factors/UV/sampler pixels');reference.remove();evidence.push({textureBindingReuse:{textured,draws,bindings,expectedBindingsPerFrame:expected,pixelsExact:true}});
 }
 engine.scene.clear();await frames();
 window.gltfBatchEvidence=evidence;result.textContent='PASS: '+engine.renderer_backend()+' · actual instanced draws, mirrored/nonuniform transforms, exact CPU pixel reference, overlap ordering, per-primitive culling, queue gaps/blend ties/1024 boundary and cleanup';
}catch(e){result.textContent='FAIL: '+e.message;console.error(e);}
