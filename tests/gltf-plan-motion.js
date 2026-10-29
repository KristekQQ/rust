import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground,assertChanged} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(value,message)=>{if(!value)throw Error(message);};
const frames=async(n=1)=>{while(n--)await new Promise(requestAnimationFrame);};
// Authored immutable assets keep the uncached, uncullled builder independent of
// the cached visible list. Distinct loads give unaffected groups their own keys.
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

const counters=['preparePayloadRebuilds','fineGroupsVisited','fineGroupMembershipHits','fineGroupMembershipMisses',
 'finePreparedRowsReused','finePreparedRowsCreated','finePayloadGroupsReused',
 'finePayloadGroupsRebuilt','finePayloadRecordsWritten','finePayloadBytesWritten','fineBasisComparisons',
 'fineWorldDataUpdates','fineFullRecordPacks','fineCompatibilityHits','fineDynamicFallbacks'];
const snapshot=e=>{
 const stats=e.debugStats().gltf;
 for(const key of [...counters,'fineCacheBytes'])assert(Number.isFinite(stats[key]),'missing numeric diagnostic '+key);
 return Object.fromEntries([...counters,'fineCacheBytes'].map(key=>[key,stats[key]]));
};
const difference=(before,after)=>Object.fromEntries(counters.map(key=>[key,after[key]-before[key]]));
const exactPixels=(a,b,label)=>{
 assert(a.width===b.width&&a.height===b.height,label+' dimensions');
 let max=0;for(let i=0;i<a.data.length;i++)max=Math.max(max,Math.abs(a.data[i]-b.data[i]));
 assert(max===0,label+' cache/culling independent pixels: max='+max);return max;
};
const evidence=[];let e;
try{
 e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 if(new URLSearchParams(location.search).has('uploadPolicy'))assert(e.setInstanceUploadPolicy(Number(new URLSearchParams(location.search).get('uploadPolicy')),0,true),'upload policy accepted');
 document.documentElement.dataset.rendererBackend=e.renderer_backend();
 e.resize(640,400);e.set_camera_mode('free');e.set_render_effects(false,false);
 e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);
 const camera=(x=0,z=4)=>e.setCameraView([x,.1,z],[x,0,0]);camera();
 e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
 const load=async spec=>{
  const url=URL.createObjectURL(new Blob([JSON.stringify(spec)],{type:'model/gltf+json'}));
  try{return await e.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}
 };
 const options={isStatic:true,castsShadow:false,receivesShadow:true,opaqueReorderable:true};
 const roots=[];
 for(let cohort=0;cohort<4;cohort++){
  const source=await load(fixture({alpha:cohort%2?'MASK':'OPAQUE'}));
  const group=[source,...Array.from({length:3},()=>source.createInstance())];
  group.forEach((root,i)=>root.setPosition([(cohort-1.5)*.65,(i-1.5)*.38,i*.055])
   .setScale([.36,.36,.36]).setRenderOptions(options));
  roots.push(...group);
 }
 // This helper must be called before capturePixels: that function waits several
 // RAFs and would otherwise disguise mutation work as stable-cache reuse.
 const mutate=async(label,change,verify=()=>{})=>{
  await frames(3);const before=snapshot(e);change();await frames(1);
  const after=snapshot(e),delta=difference(before,after);verify(delta,after);
  evidence.push({label,firstFrame:delta,fineCacheBytes:after.fineCacheBytes});
  return delta;
 };
 const oracle=async(label)=>{
  const cached=await capturePixels(canvas);assertForeground(cached,label,100);
  e.setPreparedPlanCacheEnabled(false);e.set_frustum_culling(false);
  const uncached=await capturePixels(canvas);
  const off=snapshot(e);assert(off.fineCacheBytes===0,label+' disabling frees fine cache');
  exactPixels(cached,uncached,label);
  e.set_frustum_culling(true);e.setPreparedPlanCacheEnabled(true);
  exactPixels(cached,await capturePixels(canvas),label+' restored');
  evidence.push({label,independentPixels:true});return cached;
 };
 const reused=(delta,label)=>{
  assert(delta.finePreparedRowsReused>0,label+' retained prepared rows');
  assert(delta.finePayloadGroupsReused>0,label+' retained payload groups');
 };
 await frames(8);const initial=await oracle('initial opaque and masked cohorts');
 await mutate('camera-only exact movement',()=>camera(.015),delta=>{
  reused(delta,'camera');assert(delta.fineGroupMembershipHits>0,'camera membership hits');
  assert(delta.finePreparedRowsCreated===0,'camera creates no prepared rows');
  assert(delta.finePayloadRecordsWritten===0,'camera writes no payload records');
  assert(delta.fineWorldDataUpdates===0,'camera republishes no world data');
  assert(delta.fineFullRecordPacks===0,'camera packs no full records');
 });
 await oracle('camera-only pose');
 await mutate('one translated root among sixteen',()=>roots[5].setPosition([-.23,-.12,.055]),delta=>{
  reused(delta,'one root');assert(delta.finePayloadRecordsWritten>0,'moved root writes changed records');
  assert(delta.fineWorldDataUpdates>0,'moved root rebuilds world data');
  assert(delta.fineWorldDataUpdates<roots.length,'unaffected roots retain world data');
  assert(delta.fineFullRecordPacks===0,'translated root packs no unused112B records');
 });
 assertChanged(initial,await oracle('one moving root pose'),'root motion pixels',20);
 // Uniform translation-only instances begin with compact records. Rotation,
 // nonuniform scale exercise affine records. Restore the exact common basis to
 // return to compact records, then test mirrored pipeline/group compatibility.
 for(const [label,rotation,scale] of [
  ['rotation affine',[.1,.24,.04],[.36,.36,.36]],
  ['nonuniform affine',[0,0,0],[.29,.45,.36]],
  ['compact restored',[0,0,0],[.36,.36,.36]],
  ['negative determinant',[0,0,0],[-.36,.36,.36]],
  ['mirror restored',[0,0,0],[.36,.36,.36]],
 ]){
  await mutate(label,()=>roots[5].setRotation(rotation).setScale(scale),delta=>{
   // Stride/capacity changes can replace the shared stream or shift every later
   // binding offset. Prepared rows and payload slots then legitimately rebuild;
   // unaffected per-draw world data must still survive that layout change.
   assert(delta.fineWorldDataUpdates>0,label+' rebuilds changed basis');
   assert(delta.fineWorldDataUpdates<roots.length,label+' retains unaffected world data');
   assert(delta.finePayloadRecordsWritten>0,label+' writes changed representation');
   if(label==='rotation affine'||label==='nonuniform affine'){
    assert(delta.fineFullRecordPacks>0,label+' packs actual dirty affine records');
    assert(delta.finePayloadBytesWritten>16*delta.finePayloadRecordsWritten,
     label+' actually copies 112-byte affine records');
   }
   if(label==='compact restored'||label==='mirror restored'){
    assert(delta.fineFullRecordPacks===0,label+' avoids full record packing');
    assert(delta.finePayloadBytesWritten===16*delta.finePayloadRecordsWritten,
     label+' copies only 16-byte translation records');
   }
  });await oracle(label);
 }
 // Moving through the actual viewport edge and completely outside exercises both
 // primitive and upstream root culling; the oracle disables both through the SDK.
 for(const [label,x] of [['near right edge',2.3],['across right edge',2.8],['outside frustum',20],['reentered frustum',.3]]){
  await mutate(label,()=>roots[5].setPosition([x,.15,.055]));await oracle(label);
 }
 // Membership/depth clustering changes are intentional here. The far root stays
 // visible in the independent uncullled reference and must rejoin on return.
 await mutate('far depth cluster',()=>roots[3].setPosition([-.975,.1,-30]));await oracle('far depth cluster');
 await mutate('camera changes depth clustering',()=>camera(0,50));await oracle('camera depth clustering');
 await mutate('restore camera and near cluster',()=>{camera();roots[3].setPosition([-.975,.57,.165]);});
 await oracle('near cluster restored');
 let spawned;
 await mutate('spawn rigid instance',()=>{spawned=roots[0].createInstance().setPosition([0,.8,.3]).setScale([.36,.36,.36]).setRenderOptions(options);});
 await oracle('spawned rigid instance');
 await mutate('remove rigid instance',()=>spawned.remove());await oracle('removed rigid instance');
 await mutate('order-sensitive opaque barrier',()=>roots[1].setRenderOptions({...options,opaqueReorderable:false}));
 await oracle('order-sensitive opaque barrier');
 await mutate('restore reorder policy',()=>roots[1].setRenderOptions(options));await oracle('restored opaque ordering');
 const blend=await load(fixture({alpha:'BLEND'}));
 blend.setPosition([0,0,.6]).setScale([.7,.7,.7]).setRenderOptions(options);
 const blendInstance=blend.createInstance().setPosition([.1,.05,-.4]).setScale([.7,.7,.7]);
 await oracle('transparent ordered fallback');
 await mutate('transparent depth order swap',()=>{blend.setPosition([0,0,-.4]);blendInstance.setPosition([.1,.05,.6]);});
 await oracle('transparent depth reordered');blend.remove();blendInstance.remove();
 // Paused explicit animation seeks invalidate pose without wall-clock drift.
 const pose=await e.scene.loadModel(new URL('fixtures/gltf/full/full.gltf',location.href));
 pose.setPosition([0,0,0]).setScale([.4,.4,.4]).playAnimation('Morph',{loop:false}).pauseAnimation().seekAnimation(.2);
 await oracle('explicit paused pose');
 await mutate('paused pose seek',()=>pose.seekAnimation(.8));await oracle('changed paused pose');
 await mutate('explicit morph weights',()=>pose.stopAnimation().setMorphWeights(1,[.75]));await oracle('changed morph weights');
 pose.remove();
 e.scene.clear();await frames(2);assert(snapshot(e).fineCacheBytes===0,'clear releases fine cache');
 // Regression for fleet-fallback accounting: 64 contributing static models,
 // each with 64 rigid draw nodes, must survive one unrelated 4096-node update.
 // All nodes share one mesh/accessor payload; only node descriptors multiply.
 camera();e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
 const nodeFixture=(side,spacing,scale)=>{
  const spec=fixture();
  spec.nodes=Array.from({length:side*side},(_,i)=>({mesh:0,
   translation:[(i%side-(side-1)/2)*spacing,(Math.floor(i/side)-(side-1)/2)*spacing,0],
   scale:[scale,scale,scale]}));
  spec.scenes=[{nodes:spec.nodes.map((_,i)=>i)}];return spec;
 };
 const fleetSource=await load(nodeFixture(8,.035,.03));
 const fleet=[fleetSource,...Array.from({length:63},()=>fleetSource.createInstance())];
 fleet.forEach((root,i)=>root.setPosition([(i%8-3.5)*.33,(Math.floor(i/8)-3.5)*.33,0])
  .setRenderOptions(options));
 const heavySpec=nodeFixture(64,.006,.005);
 const heavy=await load(heavySpec);heavy.setPosition([0,0,.25]).setRenderOptions(options);
 // Load/update its GPU records while visible, then settle both the culling
 // membership change and fleet cache before measuring the next offscreen move.
 await frames(5);assertForeground(await capturePixels(canvas),'large model initially visible',100);
 await mutate('one large visible root preserves static fleet',()=>heavy.setPosition([.003,0,.25]),delta=>{
  assert(delta.fineWorldDataUpdates>=4096,'visible heavy root publishes actual data');
  assert(delta.fineDynamicFallbacks===0,'one visible heavy root cannot discard static groups');
  assert(delta.finePreparedRowsReused>0,'visible heavy motion retains static rows');
 });
 await oracle('large visible root independent culling oracle');
 heavy.setPosition([100,0,.25]);await frames(5);
 await mutate('one large offscreen model preserves static fleet',()=>heavy.setPosition([101,0,.25]),delta=>{
  assert(delta.fineWorldDataUpdates>=4096,'offscreen root actually updates all 4096 GPU world data');
  assert(delta.fineWorldDataUpdates===heavySpec.nodes.length,'static fleet rebuilds no world data');
  assert(delta.fineDynamicFallbacks===0,'one offscreen model cannot trigger fleet fallback');
  assert(delta.finePreparedRowsReused>0,'static fleet retains prepared rows');
  assert(delta.finePayloadGroupsReused>0,'static fleet retains payload groups');
  assert(delta.finePayloadRecordsWritten===0,'offscreen motion writes no static fleet payload records');
  assert(delta.fineFullRecordPacks===0,'offscreen motion packs no unused112B records');
 });
 await oracle('large offscreen root independent culling oracle');
 heavy.remove();fleet.slice(16).forEach(root=>root.remove());await frames(8);
 await mutate('small fully moving fleet uses original builder',()=>{
  fleet.slice(0,16).forEach((root,i)=>root.setPosition([(i%8-3.5)*.33+.006,(Math.floor(i/8)-3.5)*.33,0]));
 },delta=>{
  assert(delta.fineWorldDataUpdates===1024,'all sixteen roots publish their64nodes');
  assert(delta.fineDynamicFallbacks>0,'small fully moving work uses measured fallback');
  assert(delta.preparePayloadRebuilds>0,'fallback actually executes original payload builder');
  assert(delta.fineFullRecordPacks===0,'compact fallback avoids unused112B packing');
 });
 await oracle('small moving fallback independent culling oracle');
 e.scene.clear();await frames(2);assert(snapshot(e).fineCacheBytes===0,'large fixture clear releases fine cache');
 window.planMotionEvidence=evidence;result.dataset.status='pass';
 result.textContent='PASS: '+JSON.stringify(evidence);
}catch(error){result.dataset.status='fail';result.textContent='FAIL: '+(error.stack||error);throw error;}
finally{e?.dispose();}
