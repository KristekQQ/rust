import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';

const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const frames=async(n=1)=>{while(n--)await new Promise(requestAnimationFrame);};
function fixture(){
 const data=new Uint8Array(108);
 new Float32Array(data.buffer,0,9).set([-.45,-.4,0,.45,-.4,0,0,.45,0]);
 new Float32Array(data.buffer,48,9).set([0,0,1,0,0,1,0,0,1]);
 new Uint16Array(data.buffer,96,3).set([0,1,2]);
 return {asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...data)),byteLength:108}],
  bufferViews:[{buffer:0,byteOffset:0,byteLength:36},{buffer:0,byteOffset:48,byteLength:36},{buffer:0,byteOffset:96,byteLength:6}],
  accessors:[{bufferView:0,componentType:5126,count:3,type:'VEC3',min:[-.45,-.4,0],max:[.45,.45,0]},{bufferView:1,componentType:5126,count:3,type:'VEC3'},{bufferView:2,componentType:5123,count:3,type:'SCALAR'}],
  materials:[{doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[.9,.12,.025,1],roughnessFactor:.7,metallicFactor:0}}],
  meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1},indices:2,material:0}]}],
  nodes:[{mesh:0}],scenes:[{nodes:[0]}],scene:0};
}
const evidence=[];let e;
try{
 e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 document.documentElement.dataset.rendererBackend=e.renderer_backend();e.resize(640,400);e.set_camera_mode('free');
 e.set_render_effects(false,false);e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);
 e.setCameraView([0,.1,4],[0,0,0]);e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
 const url=URL.createObjectURL(new Blob([JSON.stringify(fixture())],{type:'model/gltf+json'}));
 let source;try{source=await e.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}
 const roots=[source,...Array.from({length:11},()=>source.createInstance())];
 const opts={isStatic:true,castsShadow:false,receivesShadow:true,opaqueReorderable:true};
 roots.forEach((r,i)=>r.setPosition([(i%4-1.5)*.65,(Math.floor(i/4)-1)*.65,i*.015]).setScale([.5,.5,.5]).setRenderOptions(opts));
 const stats=()=>e.debugStats().gltf;
 const mode=(m)=>{assert(e.setInstanceUploadPolicy(m,0,true),'policy rejected: '+m);};
 const read=()=>{const s=stats();for(const k of ['instanceUploadCalls','instanceUploadBytes','instanceChangedBytes','instanceDirtyRecordBytes','instanceGapUploadBytes','instanceRangeOverflows'])assert(Number.isFinite(s[k]),'missing statistic '+k);assert(Number.isFinite(s.instanceUploadPolicy?.mode),'missing upload policy mode');assert(s.instanceUploadPolicy.rangeLimit===4096,'diagnostic range capacity');assert(Number.isFinite(s.instanceUploadPolicy.scratchBytes)&&s.instanceUploadPolicy.scratchBytes>=0&&s.instanceUploadPolicy.scratchBytes<=4096*8,'bounded wasm32 range scratch');return {...s};};
 const pixels=async(label,foreground=true)=>{const p=await capturePixels(canvas);if(foreground)assertForeground(p,label,30);return p;};
 const exact=(a,b,label)=>{assert(a.width===b.width&&a.height===b.height,label+' dimensions');let max=0;for(let i=0;i<a.data.length;i++)max=Math.max(max,Math.abs(a.data[i]-b.data[i]));assert(max===0,label+' exact pixels max='+max);};
 const oracle=async(label,policy=0)=>{mode(policy);const original=await pixels(label+' original');
  e.setPreparedPlanCacheEnabled(false);e.set_frustum_culling(false);const uncached=await pixels(label+' uncached union');exact(original,uncached,label+' original/cache-off');
  e.setPreparedPlanCacheEnabled(true);e.set_frustum_culling(true);exact(original,await pixels(label+' restored'),label+' restored');return original;};
 await frames(8);await oracle('initial');
 for(const [label,change] of [
  ['far sparse translations',()=>{roots[0].setPosition([-.93,-.65,0]);roots[11].setPosition([.93,.65,.165]);roots[6].setPosition([.4,-.3,.09]);}],
  ['nonuniform scale and rotation',()=>roots[11].setRotation([.13,.25,.07]).setScale([.31,.49,.5])],
 ]){
  await frames(3);await oracle(label+' baseline');const restore=()=>{roots[0].setPosition([-.975,-.65,0]);roots[6].setPosition([.325,0,.09]);roots[11].setPosition([.975,.65,.165]).setRotation([0,0,0]).setScale([.5,.5,.5]);};
  restore();await frames(2);change();await frames(2);const expected=await oracle(label+' changed rebuilt oracle');
  mode(0);restore();await frames(2);change();await frames(2);const union=await pixels(label+' union');exact(expected,union,label+' union reference');
  let modeEvidence=[];
  for(const m of [1,2]){restore();await frames(2);mode(m);await frames(2);const before=read();
   // Reapply a real mutation after policy selection so the counters measure its upload.
   change();await frames(2);const after=read(),image=await pixels(label+' mode '+m);exact(expected,image,label+' mode '+m);
   assert(after.instanceUploadCalls>before.instanceUploadCalls,'mode '+m+' emitted upload '+JSON.stringify({label,beforeCalls:before.instanceUploadCalls,afterCalls:after.instanceUploadCalls,instanced:after.instancedDrawCalls,instances:after.instancesDrawn,trans:after.translatedInstances,breaks:after.batchBreaks,dirty:after.instanceDirtyRecordBytes,changed:after.instanceChangedBytes}));
   assert(after.instanceChangedBytes>before.instanceChangedBytes,'mode '+m+' reports changed record bytes');
   assert(after.instanceUploadBytes>before.instanceUploadBytes,'mode '+m+' uploaded instance bytes');
   assert(after.instanceRangeOverflows===before.instanceRangeOverflows,'mode '+m+' range overflow');
   assert(after.instanceUploadPolicy.mode===m,'policy diagnostic '+m);
   modeEvidence.push({mode:m,calls:after.instanceUploadCalls-before.instanceUploadCalls,bytes:after.instanceUploadBytes-before.instanceUploadBytes,
    changedBytes:after.instanceChangedBytes-before.instanceChangedBytes,gapBytes:after.instanceGapUploadBytes-before.instanceGapUploadBytes});
  }
  evidence.push({label,modes:modeEvidence});
 }
 // A 16-byte stream cap is valid and forces these 16-byte multi-instance
 // records onto standalone page buffers. Restore the documented default after.
 assert(e.configureGltfInstancePool({streamByteLimit:16})===true,'page fallback limit accepted');
 const resetSparse=()=>{roots[0].setPosition([-.975,-.65,0]);roots[6].setPosition([.325,0,.09]);roots[11].setPosition([.975,.65,.165]).setRotation([0,0,0]).setScale([.5,.5,.5]);};
 await frames(6);assert(stats().instancePool.streamByteLimit===16&&stats().instancePool.activeStreams===0,'standalone page path selected');
 await oracle('page path baseline');
 const pageChange=()=>{roots[0].setPosition([-.93,-.65,0]);roots[11].setPosition([.93,.65,.165]);roots[6].setPosition([.4,-.3,.09]);};
 resetSparse();await frames(3);pageChange();await frames(3);const pageExpected=await oracle('page changed rebuilt oracle');
 for(const m of [1,2]){resetSparse();await frames(2);mode(m);pageChange();await frames(3);const image=m===1?await oracle('page mode 1 rebuilt oracle',1):await pixels('page mode 2');exact(pageExpected,image,'page mode '+m);}
 evidence.push({label:'standalone page fallback',exact:true});
 assert(e.configureGltfInstancePool({streamByteLimit:16*1024*1024})===true,'default stream limit restored');
 // Repeated stable capture must not cause an instance upload.
 mode(1);await frames(5);const warm=await pixels('warm stable');const before=read();await frames(5);exact(warm,await pixels('unchanged'), 'unchanged');
 const after=read();assert(after.instanceUploadCalls===before.instanceUploadCalls,'no-change frame emitted upload');
 evidence.push({label:'no-change',uploadCalls:after.instanceUploadCalls-before.instanceUploadCalls});
 // Camera movement, reordering and membership edits remain exact across policies.
 for(const [label,mutate] of [['camera',()=>e.setCameraView([.04,.1,4],[0,0,0])],
  ['reorder',()=>{roots[2].setPosition([.4,.4,.3]);roots[5].setPosition([-.4,-.4,-.3]);}],
  ['negative scale barrier',()=>roots[11].setScale([-.31,.49,.5])],['remove',()=>roots[3].remove()],['clear/reload',()=>e.scene.clear()]]){
  mode(1);mutate();await frames(4);const empty=label==='clear/reload';const a=await pixels(label+' mode1',!empty);mode(2);const b=await pixels(label+' mode2',!empty);exact(a,b,label+' policy');evidence.push({label,exact:true});
  if(label==='clear/reload'){
   assert(read().instanceUploadPolicy.scratchBytes===0,'clear releases retained upload-range scratch');evidence.push({label:'clear-range-scratch',bytes:0});
   const reloadUrl=URL.createObjectURL(new Blob([JSON.stringify(fixture())],{type:'model/gltf+json'}));
   try{const reloaded=await e.scene.loadModel(reloadUrl);reloaded.setPosition([0,0,0]).setScale([.6,.6,.6]).setRenderOptions(opts);await frames(4);await pixels('reloaded');}finally{URL.revokeObjectURL(reloadUrl);}
  }
 }
 window.uploadRangeEvidence=evidence;result.dataset.status='pass';result.textContent='PASS: '+JSON.stringify(evidence);
}catch(error){window.uploadRangeEvidence=evidence;result.dataset.status='fail';result.textContent='FAIL: '+(error.stack||error);throw error;}
finally{e?.dispose();}
