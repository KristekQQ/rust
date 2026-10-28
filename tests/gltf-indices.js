import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);},frames=async(n=3)=>{while(n--)await new Promise(requestAnimationFrame);};
const exact=(a,b,label)=>{let count=0,max=0;for(let i=0;i<a.data.length;i++){const d=Math.abs(a.data[i]-b.data[i]);if(d)count++;max=Math.max(max,d);}assert(count===0,label+JSON.stringify({count,max}));};
// Original accessor is Uint32, so65535 is valid source data. Normals keep the
// importer from flattening strips or high-valued source indices.
function fixture(high,{many=false,unindexed=false,cpu=false,skin=false}={}){
 const count=unindexed?3:high+1,positions=new Float32Array(count*3),normals=new Float32Array(count*3);
 for(let i=0;i<count;i++)normals[i*3+2]=1;
 const point=(i,x,y)=>positions.set([x,y,0],i*3);point(0,-.13,-.13);point(1,.13,-.13);point(unindexed?2:high,.13,.13);if(count>3)point(3,-.13,.13);
 const chunks=[],views=[],accessors=[];let offset=0;
 const add=(data,type,componentType=5126)=>{const pad=(-offset)&3;if(pad){chunks.push(new Uint8Array(pad));offset+=pad;}const bytes=new Uint8Array(data.buffer);views.push({buffer:0,byteOffset:offset,byteLength:bytes.length});chunks.push(bytes);offset+=bytes.length;accessors.push({bufferView:views.length-1,componentType,count:data.length/({VEC3:3,VEC4:4,MAT4:16,SCALAR:1}[type]),type,...(data===positions?{min:[-.13,-.13,0],max:[.13,.13,0]}:{})});return accessors.length-1;};
 const pos=add(positions,'VEC3'),normal=add(normals,'VEC3');
 const lists=unindexed?[[0,1,2]]:many?[Array.from({length:70002},(_,i)=>i%3)]:[[0,1,high],[0,high],[0,1,high],[0,1,high,3],[0,1,high],[0,1,3,high],[0,1,high,3]];
 const attributes={POSITION:pos,NORMAL:normal};if(skin){attributes.JOINTS_0=add(new Uint16Array(count*4),'VEC4',5123);const weights=new Float32Array(count*4);for(let i=0;i<count;i++)weights[i*4]=1;attributes.WEIGHTS_0=add(weights,'VEC4');}
 const zero=cpu?add(new Float32Array(count*3),'VEC3'):null;
 const meshes=lists.map((list,i)=>({primitives:[{attributes,...(cpu?{targets:[{POSITION:zero}]}:{}),...(!unindexed?{indices:add(new Uint32Array(list),'SCALAR',5125)}:{}),mode:many||unindexed?4:i,material:0}]}));
 const inverse=skin?add(new Float32Array([1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]),'MAT4'):null;
 const nodes=meshes.map((_,mesh)=>({mesh,translation:[(mesh%4-1.5)*.4,Math.floor(mesh/4)*.4,0],...(skin?{skin:0}:{})}));if(skin)nodes.push({});
 const bytes=new Uint8Array(offset);let o=0;for(const c of chunks){bytes.set(c,o);o+=c.length;}let binary='';for(let i=0;i<bytes.length;i+=32768)binary+=String.fromCharCode(...bytes.subarray(i,i+32768));
 return {asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(binary),byteLength:bytes.length}],bufferViews:views,accessors,materials:[{doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[.6,.2,.05,1],metallicFactor:0,roughnessFactor:.7}}],meshes,nodes,...(skin?{skins:[{joints:[nodes.length-1],inverseBindMatrices:inverse}]}:{}),scenes:[{nodes:nodes.map((_,i)=>i)}],scene:0};
}
let e;const evidence=[];window.indexEvidence=evidence;
try{
 const backend=new URLSearchParams(location.search).get('backend')||'auto';e=await initEngine(canvas,{backend});document.documentElement.dataset.rendererBackend=e.renderer_backend();
 e.resize(640,400);e.set_camera_mode('free');e.set_grid_visible(false);e.set_light_helpers_visible(false);e.set_simulation_paused(true);e.setCameraView([0,.2,3],[0,.2,0]);e.set_render_effects(false,false);
 const stage=async(compact,high,options={})=>{
  e.scene.clear();await frames();assert(e.gltfIndexLayouts().length===0,'clear releases shared index geometry');assert(e.setGltfCompactIndicesEnabled(compact),'empty scene selects index policy');e.scene.createLight({position:[-2,3,5],color:[10,10,10]});
  const beforeGltf=e.debugStats().gltf,before=beforeGltf.indices,url=URL.createObjectURL(new Blob([JSON.stringify(fixture(high,options))],{type:'model/gltf+json'}));let model;try{model=await e.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}
  const models=[model,model.createInstance(),model.createInstance()];models.forEach((m,i)=>m.setPosition([0,(i-1)*.9,0]).setScale([i===1?-1:1,.8,1.1]).setRotation([.03,0,.01]).setRenderOptions({isStatic:true,castsShadow:true,receivesShadow:true,opaqueReorderable:true}));
  assert(!e.setGltfCompactIndicesEnabled(!compact),'live shared assets reject format mutation');
  if(options.effects){e.set_render_effects(true,true);const mirror=e.scene.createPlane([0,-1.7,0]).setScale([5,1,5]).setRenderOptions({isStatic:true,castsShadow:false,reflectivity:.6});e.scene.setMirror(mirror);}await frames(6);
  const image=await capturePixels(canvas);assertForeground(image,'index fixture '+JSON.stringify({compact,high,options}),100);const layouts=e.gltfIndexLayouts().sort((a,b)=>a.primitive-b.primitive),stats=e.debugStats().gltf.indices;
  if(options.cpu)assert(e.debugStats().gltf.cpuDeformedVertices>beforeGltf.cpuDeformedVertices,'morph fixture executes CPU deformation');
  if(options.skin)assert(e.debugStats().gltf.skinPaletteUploadBytes>beforeGltf.skinPaletteUploadBytes,'skin fixture uploads actual joint palettes');
  const use16=compact&&(options.many||options.unindexed||high<65535);
  const counts=options.many?[70002]:options.unindexed?[3]:[3,2,6,4,3,4,6];
  assert(layouts.length===counts.length,'one shared index allocation per primitive across all roots');
  layouts.forEach((l,i)=>{assert(l.format===(use16?'Uint16':'Uint32'),'actual index format '+JSON.stringify(l));assert(l.count===counts[i],'logical count after conversion');assert(l.payloadBytes===l.count*(use16?2:4),'actual logical index bytes');assert(l.bufferBytes===Math.max(4,Math.ceil(l.payloadBytes/4)*4),'aligned real Buffer.size');if(!options.many&&!options.unindexed)assert(l.max===high,'actual sparse maximum preserved');});
  assert(stats.uploadBytes-before.uploadBytes===layouts.reduce((n,l)=>n+l.payloadBytes,0),'actual initial index payload upload');assert(stats.bufferBytes===layouts.reduce((n,l)=>n+l.bufferBytes,0),'aggregate real allocation bytes');
  evidence.push({compact,high,options,layouts,stats,initialUploadBytes:stats.uploadBytes-before.uploadBytes});
  // Removing the source preserves shared instance ownership until final clear.
  model.remove();await frames();assert(e.gltfIndexLayouts().length===layouts.length,'source removal preserves geometry owned by instances');
  e.set_render_effects(false,false);return image;
 };
 for(const [high,options] of [[65534,{}],[65535,{}],[65536,{}],[2,{many:true}],[2,{unindexed:true}],[2,{unindexed:true,cpu:true}],[2,{unindexed:true,skin:true}],[65534,{effects:true}]]){const original=await stage(false,high,options),compact=await stage(true,high,options);exact(original,compact,'Uint32 oracle '+JSON.stringify({high,options}));}
 e.scene.clear();await frames();assert(e.debugStats().gltf.indices.bufferBytes===0,'clear frees live index storage');e.dispose();e=await initEngine(canvas,{backend});assert(e.debugStats().gltf.indices.enabled,'dispose/reinit restores default compact policy');
 window.indexEvidence=evidence;result.textContent='PASS: exact Uint32 pixel oracle, real index formats/bytes/padding, all topologies and boundaries, sparse/dense/nonindexed data, affine instances, source removal, auxiliary passes and lifecycle';
}catch(error){result.textContent='FAIL: '+(error.stack||error);throw error;}finally{e?.dispose();}
