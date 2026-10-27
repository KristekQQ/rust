import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground,assertChanged} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);};
const frames=async(n=5)=>{while(n--)await new Promise(requestAnimationFrame);};
// Small authored fixture; the oracle is the independent existing uncached
// preparation path in the very same scene, never a second cache implementation.
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

const evidence=[];
const equal=(a,b,label)=>{assert(a.width===b.width&&a.height===b.height,label+' dimensions');let max=0;for(let i=0;i<a.data.length;i++)max=Math.max(max,Math.abs(a.data[i]-b.data[i]));assert(max===0,label+' exact pixels, max='+max);return max;};
let e;
try{
 const init=async()=>{
  e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
  document.documentElement.dataset.rendererBackend=e.renderer_backend();
  e.resize(640,400);e.set_camera_mode('free');e.set_render_effects(false,false);e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);e.setCameraView([0,.1,3],[0,0,0]);
 };
 await init();
 const load=async spec=>{const url=URL.createObjectURL(new Blob([JSON.stringify(spec)],{type:'model/gltf+json'}));try{return await e.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}};
 const options=enabled=>({isStatic:true,castsShadow:true,receivesShadow:true,opaqueReorderable:enabled});
 let light=e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
 const a=await load(fixture()),models=[a,a.createInstance(),a.createInstance()];
 models.forEach((m,i)=>m.setPosition([(i-1)*.42,0,i*.07]).setScale([.7,.8,.9]).setRenderOptions(options(true)));
 const check=async(label,{warm=false,foreground=true}={})=>{
  await frames(8);
  const before=e.debugStats().gltf;await frames(4);const after=e.debugStats().gltf;
  const cached=await capturePixels(canvas);if(foreground)assertForeground(cached,label);
  const draws=e.debugStats().mainDrawCalls;
  if(warm){
   assert(after.preparedPlanCache.hits>before.preparedPlanCache.hits,label+' actual cache hits');
   assert(after.prepareOpaqueGroupRebuilds===before.prepareOpaqueGroupRebuilds,label+' no opaque group rebuilds');
   assert(after.preparePayloadRebuilds===before.preparePayloadRebuilds,label+' no payload rebuilds');
   assert(after.prepareRecordRebuilds===before.prepareRecordRebuilds,label+' no record rebuilds');
  }
  e.setPreparedPlanCacheEnabled(false);const uncached=await capturePixels(canvas);
  assert(e.debugStats().mainDrawCalls===draws,label+' identical actual draws');equal(cached,uncached,label);
  assert(e.debugStats().gltf.preparedPlanCache.retainedBytes===0,label+' disabled releases plan');
  e.setPreparedPlanCacheEnabled(true);await frames();equal(cached,await capturePixels(canvas),label+' reenabled');
  const cache=e.debugStats().gltf.preparedPlanCache;
  assert(cache.retainedBytes<=cache.byteLimit&&cache.retainedSlots<=cache.slotLimit,label+' bounded storage');
  evidence.push({label,draws,cache,before,after});return cached;
 };
 const first=await check('warm immutable opaque',{warm:true});
 models[2].setPosition([.65,.2,.14]);assertChanged(first,await check('root transform',{warm:true}),'root transform');
 models[1].setRotation([.1,.22,.04]);await check('affine basis change',{warm:true});
 models[1].setScale([-.7,.8,.9]);await check('mirrored variant',{warm:true});
 models[1].setRenderOptions(options(false));await check('semantic reorder barrier',{warm:true});
 models[1].setRenderOptions({...options(true),receivesShadow:false,reflectivity:.4});await check('render material options',{warm:true});
 // Tiny changes must cause misses even when their pixels quantize identically.
 let start=e.debugStats().gltf.preparedPlanCache;
 e.setCameraView([.000001,.1,3],[0,0,0]);await frames(2);
 assert(e.debugStats().gltf.preparedPlanCache.misses>start.misses,'exact subpixel camera invalidation');
 await check('tiny camera');
 for(const eye of [-.032,.032]){e.setCameraView([eye,.1,3],[eye,0,0]);await check('desktop eye '+eye,{warm:true});}
 e.set_frustum_culling(false);await check('culling off',{warm:true});e.set_frustum_culling(true);
 light.set([-2,3,5],[0,0,0]);await check('zero local light class',{warm:true});light.set([-2,3,5],[10,10,10]);
 const added=a.createInstance().setPosition([-.8,.2,.2]).setScale([.5,.5,.5]);await check('add model',{warm:true});added.remove();await check('remove model',{warm:true});
 // Geometry/material replacement is the supported immutable asset revision.
 const changed=fixture();changed.materials[0].pbrMetallicRoughness.baseColorFactor=[.1,.8,.2,1];
 const replacement=await load(changed);replacement.setPosition([0,.4,.4]).setScale([.5,.5,.5]);await check('immutable material replacement',{warm:true});replacement.remove();
 const geometry=fixture();const bytes=Uint8Array.from(atob(geometry.buffers[0].uri.split(',')[1]),c=>c.charCodeAt(0));new Float32Array(bytes.buffer,0,12)[0]=-.8;geometry.accessors[0].min[0]=-.8;geometry.buffers[0].uri='data:application/octet-stream;base64,'+btoa(String.fromCharCode(...bytes));const reshaped=await load(geometry);reshaped.setPosition([0,.4,.4]).setScale([.5,.5,.5]);await check('immutable geometry replacement',{warm:true});reshaped.remove();
 const mirror=e.scene.createPlane([0,-.7,0]).setScale([5,1,5]).setRenderOptions({isStatic:true,castsShadow:false,reflectivity:.8});e.scene.setMirror(mirror);
 e.set_render_effects(true,true);await check('shadow reflection passes');assert(e.effects_stats().reflectionUpdates>0,'actual reflection rendering');
 e.postprocessing.enable('ssr',{strength:.3});e.postprocessing.enable('bloom',{strength:.2});await check('postprocessing auxiliary formats');
 e.postprocessing.disable('bloom');e.postprocessing.disable('ssr');e.set_render_effects(false,false);mirror.remove();
 const transparent=await load(fixture({alpha:'BLEND'}));transparent.setPosition([.2,.1,.5]);await check('transparent ordered fallback');
 transparent.setPosition([-.1,.2,-.5]);await check('transparent depth reorder');transparent.remove();
 e.scene.clear();await frames();assert(e.debugStats().gltf.preparedPlanCache.retainedBytes===0,'clear releases all plans');
 const full=await e.scene.loadModel(new URL('fixtures/gltf/full/full.gltf',location.href));e.scene.createLight({position:[0,5,5],color:[20,20,20]});e.setCameraView([0,3,12],[0,1,0]);
 full.playAnimation('Morph',{loop:false}).pauseAnimation().seekAnimation(.2);await check('animation pose');
 full.seekAnimation(.8);await check('changed animation pose');
 full.stopAnimation().setMorphWeights(1,[.75]);await check('morph payload revision');
 full.selectScene(1);await check('active primitives scene1');full.selectScene(0);await check('active primitives restored');
 full.remove();await frames();assert(e.debugStats().gltf.preparedPlanCache.retainedBytes===0,'last model removal releases cache');
 const fox=await e.scene.loadModel(new URL('../assets/models/fox/Fox.glb',location.href));e.setCameraView([3,2,5],[0,1,0]);fox.setScale([.025,.025,.025]).playAnimation('Walk').pauseAnimation().seekAnimation(0);await check('skin palette');fox.seekAnimation(.3);await check('changed skin palette');
 e.dispose();await init();e.scene.createLight({position:[-2,3,5],color:[10,10,10]});await load(fixture());await check('dispose and reinitialize',{warm:true});
 window.planCacheEvidence=evidence;result.textContent='PASS: exact opaque plan cache · '+evidence.length+' cache-off pixel oracles · bounded storage · mutations/lifecycle';
}catch(error){result.textContent='FAIL: '+(error.stack||error);throw error;}
finally{e?.dispose();}
