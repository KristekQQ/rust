import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);};
const frames=async(n=4)=>{while(n--)await new Promise(requestAnimationFrame);};
const exact=(a,b,label)=>{let n=0,max=0;assert(a.data.length===b.data.length,label+' size');for(let i=0;i<a.data.length;i++){const d=Math.abs(a.data[i]-b.data[i]);if(d)n++;max=Math.max(max,d);}assert(!n,label+': differing channels '+n+', max '+max);};
// Each primitive includes an unindexed fifth vertex. It participates in the
// proof even though it cannot affect the fixture image.
function fixture({cpu=false}={}){
 const views=[],accessors=[],chunks=[];let offset=0;
 const add=(values,type,componentType=5126,minmax={})=>{const bytes=new Uint8Array(values.buffer);const view=views.length;views.push({buffer:0,byteOffset:offset,byteLength:bytes.length});chunks.push(bytes);offset+=bytes.length;const id=accessors.length;accessors.push({bufferView:view,componentType,count:values.length/({SCALAR:1,VEC2:2,VEC3:3,VEC4:4}[type]),type,...minmax});return id;};
 const pos=add(new Float32Array([-.11,-.11,0,.11,-.11,0,.11,.11,0,-.11,.11,0,0,0,0]),'VEC3',5126,{min:[-.11,-.11,0],max:[.11,.11,0]});
 const normal=add(new Float32Array(Array.from({length:5},()=>[0,0,1]).flat()),'VEC3');
 const uv=add(new Float32Array([0,0,1,0,1,1,0,1,.5,.5]),'VEC2');
 const tangent=add(new Float32Array(Array.from({length:5},()=>[1,0,0,1]).flat()),'VEC4');
 const index=add(new Uint32Array([0,1,2,0,2,3]),'SCALAR',5125);
 const zero=add(new Float32Array(15),'VEC3',5126,{min:[0,0,0],max:[0,0,0]});
 const tex=document.createElement('canvas');tex.width=2;tex.height=2;const ctx=tex.getContext('2d');ctx.fillStyle='#c08040';ctx.fillRect(0,0,2,2);const base=tex.toDataURL();ctx.fillStyle='#8080ff';ctx.fillRect(0,0,2,2);const norm=tex.toDataURL();
 const expected=[true,true,false,false,false,true,true,true,true,false];
 const meshes=expected.map((_,i)=>{const attributes={POSITION:pos,NORMAL:normal,TEXCOORD_0:uv,TANGENT:tangent};
  if(i!==0){const width=i===5?3:4,colors=new Float32Array(5*width).fill(1);if(i===2)colors[4]=.25;if(i===3)colors[3]=.4;if(i===4)colors[0]=1-2**-24;if(i===9)colors[4*width]=.2;attributes.COLOR_0=add(colors,width===3?'VEC3':'VEC4');}
  return {primitives:[{attributes,indices:index,material:i,...(cpu?{targets:[{POSITION:zero}]}:{})}]};});
 const bytes=new Uint8Array(offset);let cursor=0;for(const c of chunks){bytes.set(c,cursor);cursor+=c.length;}
 const materials=expected.map((_,i)=>({doubleSided:true,alphaMode:i===7?'BLEND':i===8?'MASK':'OPAQUE',alphaCutoff:.3,pbrMetallicRoughness:{baseColorFactor:[.7,.3,.15,i===7?.6:1],metallicFactor:0,roughnessFactor:.7,...(i>=6&&i<=8?{baseColorTexture:{index:0}}:{})},...(i===6?{normalTexture:{index:1}}:{})}));
 return {expected,json:{asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...bytes)),byteLength:bytes.length}],bufferViews:views,accessors,images:[{uri:base},{uri:norm}],textures:[{source:0},{source:1}],materials,meshes,nodes:meshes.map((_,mesh)=>({mesh,translation:[(mesh%5-2)*.27,Math.floor(mesh/5)*.28-.14,0]})),scenes:[{nodes:meshes.map((_,i)=>i)}],scene:0}};
}
let e;const evidence=[];
try{
 e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});document.documentElement.dataset.rendererBackend=e.renderer_backend();
 e.resize(640,400);e.set_camera_mode('free');e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);e.setCameraView([0,0,3],[0,0,0]);e.set_render_effects(false,false);
 const load=async spec=>{const u=URL.createObjectURL(new Blob([JSON.stringify(spec)],{type:'model/gltf+json'}));try{return await e.scene.loadModel(u);}finally{URL.revokeObjectURL(u);}};
 const stage=async(enabled,{effects=false,cpu=false,affine=false}={})=>{
  e.scene.clear();await frames(2);assert(e.setGltfWhiteColorOmissionEnabled(enabled),'empty scene accepts layout policy');
  assert(e.gltfVertexLayouts().length===0,'clear releases live rigid geometry');
  e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
  const f=fixture({cpu}),before=e.debugStats().gltf.vertexColors,model=await load(f.json),models=[model,model.createInstance(),model.createInstance()];
  models.forEach((m,i)=>m.setPosition([0,(i-1)*.7,0]).setScale([1,1,1]).setRenderOptions({isStatic:true,castsShadow:true,receivesShadow:true,opaqueReorderable:true}));
  if(affine)models.forEach((m,i)=>m.setScale([i===1?-1.1:1.1,.8,1.2]).setRotation([.05,.1,.02]));
  assert(e.setGltfWhiteColorOmissionEnabled(!enabled)===false,'cannot mutate live immutable layout');
  const effectsBefore=e.effects_stats();
  e.set_render_effects(effects,effects);if(effects){const mirror=e.scene.createPlane([0,-1.2,0]).setScale([5,1,5]).setRenderOptions({isStatic:true,castsShadow:false,reflectivity:.6});e.scene.setMirror(mirror);e.postprocessing.enable('ssr',{strength:.2});e.postprocessing.enable('bloom',{strength:.1});}
  await frames(8);const image=await capturePixels(canvas);assertForeground(image,'white colors',200);
  if(effects){const after=e.effects_stats();assert(after.reflectionUpdates>effectsBefore.reflectionUpdates,'actual glTF reflection update');assert(after.staticUpdates+after.dynamicUpdates>effectsBefore.staticUpdates+effectsBefore.dynamicUpdates,'actual glTF shadow update');}
  const stats=e.debugStats().gltf,info=stats.vertexColors;const layouts=[...e.gltfVertexLayouts()].sort((a,b)=>a.primitive-b.primitive);
  if(!cpu){assert(layouts.length===10,'one shared rigid buffer per primitive across three roots');for(const [i,l] of layouts.entries()){
   assert(l.omitColor===(enabled&&f.expected[i]),'exact eligibility primitive '+i);
   const expectedStride=(i===6?72:i===7||i===8?48:40)-(l.omitColor?16:0);
   assert(l.stride===expectedStride&&l.bufferBytes===5*expectedStride,'actual GPU stride/size primitive '+i+': '+JSON.stringify(l));
  }}else assert(layouts.length===0,'morph reference stays on original 96-byte mutable layout');
  if(enabled&&!cpu){assert(info.colorlessShaders>0&&info.coloredShaders>0&&info.colorlessPipelines>0,'mixed shader/pipeline variants actually compiled');assert(stats.instancedDrawCalls>0,'instancing actually used');}
  evidence.push({enabled,effects,cpu,affine,initialVertexUploadBytes:info.rigidVertexUploadBytes-before.rigidVertexUploadBytes,omittedColorBytes:info.omittedColorBytes-before.omittedColorBytes,info,layouts});
  e.postprocessing.disable('ssr');e.postprocessing.disable('bloom');return image;
 };
 for(const options of [{},{affine:true},{effects:true},{effects:true,affine:true}]){
  const reference=await stage(false,options),candidate=await stage(true,options);exact(reference,candidate,'original color layout '+JSON.stringify(options));
 }
 // Separate original CPU fallback keeps source colors and all geometry intact.
 const cpuReference=await stage(false,{cpu:true}),cpuCandidate=await stage(true,{cpu:true});exact(cpuReference,cpuCandidate,'unchanged morph CPU fallback');
 e.scene.clear();await frames();assert(e.setGltfWhiteColorOmissionEnabled(false),'after final clear policy changes');
 e.dispose();e=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});assert(e.debugStats().gltf.vertexColors.enabled,'fresh renderer restores default');
 window.whiteColorEvidence=evidence;result.textContent='PASS: exact-white rigid attributes, real buffer bytes, original-layout exact pixels, mixed classes, alpha/textures/normal maps, affine instances, auxiliary passes, fallback and lifecycle';
}catch(error){result.textContent='FAIL: '+(error.stack||error);throw error;}finally{e?.dispose();}
