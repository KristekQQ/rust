import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground,assertChanged} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const exact=(a,b,label)=>{
 assert(a.width===b.width&&a.height===b.height,label+' dimensions');
 for(let i=0;i<a.data.length;i++)if(a.data[i]!==b.data[i])throw Error(label+' pixel byte '+i);
};
// One visible root spans the frustum: the second node must be rejected by
// primitive culling. Another instance lies wholly outside the main frustum.
function fixture(){
 const data=new Uint8Array(80);
 new Float32Array(data.buffer,0,9).set([-.4,0,0,.4,0,0,0,.8,0]);
 new Float32Array(data.buffer,36,9).set([0,0,1,0,0,1,0,0,1]);
 new Uint16Array(data.buffer,72,3).set([0,1,2]);
 return {asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...data)),byteLength:80}],
 bufferViews:[{buffer:0,byteOffset:0,byteLength:36},{buffer:0,byteOffset:36,byteLength:36},{buffer:0,byteOffset:72,byteLength:6}],
 accessors:[{bufferView:0,componentType:5126,count:3,type:'VEC3',min:[-.4,0,0],max:[.4,.8,0]},{bufferView:1,componentType:5126,count:3,type:'VEC3'},{bufferView:2,componentType:5123,count:3,type:'SCALAR'}],
 materials:[{doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[.8,.1,.1,1],roughnessFactor:1,metallicFactor:0}}],
 meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1},indices:2,material:0}]}],nodes:[{mesh:0},{mesh:0,translation:[100,0,0]}],scenes:[{nodes:[0,1]}],scene:0};
}
let e;
try{
 e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 document.documentElement.dataset.rendererBackend=e.renderer_backend();
 e.resize(640,400);e.scene.clear();e.set_simulation_paused(true);
 e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_camera_mode('free');
 e.setCameraView([4,4,6],[0,0,0]);e.set_render_effects(true,false);
 e.scene.createLight({position:[-3,6,3],color:[2,2,2]});
 e.scene.createPlane([0,-.51,0]).setScale([8,1,8]).setRenderOptions({castsShadow:false,receivesShadow:true,isStatic:true});
 const staticCaster=e.scene.createCube([-.9,0,0]).setRenderOptions({castsShadow:true,receivesShadow:true,isStatic:true});
 const dynamicCaster=e.scene.createCube([.9,0,-.7]).setRenderOptions({castsShadow:true,receivesShadow:true,isStatic:false});
 const url=URL.createObjectURL(new Blob([JSON.stringify(fixture())],{type:'model/gltf+json'}));
 let model;try{model=await e.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}
 model.setPosition([0,0,1]).setRenderOptions({castsShadow:true,receivesShadow:true,isStatic:true});
 model.createInstance().setPosition([10000,0,0]).setRenderOptions({castsShadow:false,receivesShadow:true,isStatic:true});
 const baseline=await capturePixels(canvas);assertForeground(baseline,'lit geometry',100);
 model.setRenderOptions({castsShadow:false,isStatic:true});
 assertChanged(baseline,await capturePixels(canvas),'actual glTF caster shadow',1);
 model.setRenderOptions({castsShadow:true,isStatic:true});
 exact(baseline,await capturePixels(canvas),'restored glTF shadow');
 // Prove that the occupied shadow layers affect the image while geometry stays
 // present. Restore before the main-only isolation series.
 staticCaster.setRenderOptions({castsShadow:false,isStatic:true});dynamicCaster.setRenderOptions({castsShadow:false,isStatic:false});
 assertChanged(baseline,await capturePixels(canvas),'actual caster shadows',1);
 staticCaster.setRenderOptions({castsShadow:true,isStatic:true});dynamicCaster.setRenderOptions({castsShadow:true,isStatic:false});
 exact(baseline,await capturePixels(canvas),'restored shadow content');
 const shadows=e.effects_stats(),visible=e.renderer_stats().visible;
 assert(shadows.staticVisible>0&&shadows.dynamicVisible>0,'both shadow layers occupied');
 const stable=s=>{
  for(const k of ['staticUpdates','dynamicUpdates','staticVisible','dynamicVisible','shadowLightCount'])assert(s[k]===shadows[k],'main-only changed shadow '+k);
  for(const k of ['staticUpdatesByLight','dynamicUpdatesByLight'])assert(JSON.stringify(s[k])===JSON.stringify(shadows[k]),'main-only changed '+k);
 };
 const evidence=[];
 for(const cache of [true,false]){
  e.setPreparedPlanCacheEnabled(cache);
  for(const [roots,primitives] of [[false,true],[true,false],[false,false],[true,true]]){
   const before=e.debugStats().gltf.primitiveCulled;
   e.setMainViewCulling(roots,primitives);
   const image=await capturePixels(canvas);exact(baseline,image,'main-only '+[cache,roots,primitives]);
   exact(image,await capturePixels(canvas),'consecutive capture');stable(e.effects_stats());
   const after=e.debugStats().gltf.primitiveCulled,now=e.renderer_stats();
   assert(roots?now.visible===visible:now.visible>visible,'root switch must change actual root selection');
   assert(primitives?after>before:after===before,'primitive switch must change actual primitive tests');
   evidence.push({cache,roots,primitives,visible:now.visible,primitiveCulled:after-before});
  }
 }
 // Force new maps as well: selection/content must remain equal after real
 // invalidation, rather than merely preserving an already cached map.
 const move=dx=>{staticCaster.setPosition([-.9+dx,0,0]);dynamicCaster.setPosition([.9+dx,0,-.7]);model.setPosition([dx,0,1]);};
 e.setMainViewCulling(true,true);move(.08);const movedReference=await capturePixels(canvas);
 move(0);exact(baseline,await capturePixels(canvas),'caster pose restored');
 for(const [roots,primitives] of [[false,true],[true,false],[false,false],[true,true]]){
  e.setMainViewCulling(roots,primitives);const before=e.effects_stats();move(.08);
  exact(movedReference,await capturePixels(canvas),'fresh shadow maps '+[roots,primitives]);
  const after=e.effects_stats();
  assert(after.staticUpdates===before.staticUpdates+1&&after.dynamicUpdates===before.dynamicUpdates+1,'both occupied shadow maps regenerated once');
  assert(after.staticVisible===shadows.staticVisible&&after.dynamicVisible===shadows.dynamicVisible,'fresh shadow caster selection unchanged');
  move(0);exact(baseline,await capturePixels(canvas),'fresh shadow maps restored');
 }
 // Preserve global semantics: it overrides both main controls.
 e.setMainViewCulling(true,true);e.set_frustum_culling(false);
 await capturePixels(canvas);assert(e.renderer_stats().visible>visible,'global off still disables root culling');
 e.set_frustum_culling(true);e.setPreparedPlanCacheEnabled(true);
 exact(baseline,await capturePixels(canvas),'global restored');
 window.mainViewCullingEvidence={shadows,states:evidence};
 result.textContent='PASS: independent main root/primitive controls; unchanged occupied shadow maps; exact images; global restore';
}catch(error){result.textContent='FAIL: '+(error.stack||error);console.error(error);}
finally{e?.dispose();}
