import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground} from './pixels.js';

const canvas=document.querySelector('canvas'),backend=new URLSearchParams(location.search).get('backend')==='webgl'?'webgl':'auto';
const engine=await initEngine(canvas,{backend});engine.resize(640,400);engine.setDebugOverlay(false);engine.set_camera_mode('free');engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_simulation_paused(true);engine.set_frustum_culling(false);engine.scene.clear();engine.set_render_effects(false,false);engine.setBackgroundColor([0,0,0]);
const texCanvas=document.createElement('canvas');texCanvas.width=texCanvas.height=1;const texContext=texCanvas.getContext('2d',{willReadFrequently:true});texContext.putImageData(new ImageData(new Uint8ClampedArray([255,0,255,255]),1,1),0,0);const normalPng=texCanvas.toDataURL('image/png');const preload=new Image();preload.src=normalPng;await preload.decode();
let lastAppearance, lightHandles=[];
const normalize=v=>{const m=Math.hypot(...v);return v.map(x=>x/m)};
const cornerNormals=[normalize([-.22,-.12,1]),normalize([.22,-.08,1]),normalize([.18,.2,1]),normalize([-.16,.15,1])];
const stateId=s=>`${s.view}-n${+s.normal}-t${+s.tangent}-${s.metal?'metal':'dielectric'}-${s.noUv?'noUV':'UV'}-r${s.roughness??'default'}`;
function makeAsset(state,mode){
 const arrays=[['POSITION',new Float32Array([-1,-1,0,1,-1,0,1,1,0,-1,1,0]),5126,3],...(state.noUv&&mode==='noMap'?[]:[['TEXCOORD_0',new Float32Array([0,0,1,0,1,1,0,1]),5126,2]]),['INDICES',new Uint16Array([0,1,2,0,2,3]),5123,1]];
 if(state.normal)arrays.push(['NORMAL',new Float32Array(cornerNormals.flat()),5126,3]);
 if(state.tangent){const tangents=cornerNormals.map(n=>normalize([n[2],0,-n[0]]).concat(1));arrays.push(['TANGENT',new Float32Array(tangents.flat()),5126,4]);}
 const byteLength=arrays.reduce((n,a)=>n+a[1].byteLength,0),buffer=new ArrayBuffer(byteLength),views=[],accessors=[],attributes={};let offset=0;
 for(const [name,data,componentType,size] of arrays){new Uint8Array(buffer,offset,data.byteLength).set(new Uint8Array(data.buffer));const view=views.length;views.push({buffer:0,byteOffset:offset,byteLength:data.byteLength});const accessor=accessors.length;accessors.push({bufferView:view,componentType,count:data.length/size,type:size===1?'SCALAR':`VEC${size}`});if(name==='POSITION'){accessors[accessor].min=[-1,-1,0];accessors[accessor].max=[1,1,0];}if(name!=='INDICES')attributes[name]=accessor;offset+=data.byteLength;}
 const material={doubleSided:state.view==='back-double'||state.view==='grazing-double',pbrMetallicRoughness:{baseColorFactor:[.72,.68,.58,1],metallicFactor:state.metal ? .7 : 0,roughnessFactor:state.roughness??(state.metal ? .25 : .8)}};
 if(mode!=='noMap')material.normalTexture={index:0,scale:mode==='neutralRef'?0:1};
 const asset={asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...new Uint8Array(buffer))),byteLength}],bufferViews:views,accessors,images:[{uri:normalPng}],samplers:[{magFilter:9729,minFilter:9729,wrapS:10497,wrapT:10497}],textures:[{sampler:0,source:0}],materials:[material],meshes:[{primitives:[{attributes,indices:arrays.findIndex(a=>a[0]==='INDICES'),material:0}]}],nodes:[{mesh:0}],scenes:[{nodes:[0]}],scene:0};
 // The index accessor is not necessarily the final accessor when optional attributes are present.
 const indexArray=accessors.findIndex(a=>a.componentType===5123);asset.meshes[0].primitives[0].indices=indexArray;
 return asset;
}
window.normalMapProbe={backend:engine.renderer_backend(),stateId,async capture(state,mode){
 if(!['noMap','neutralRef','mapped'].includes(mode))throw Error('Unknown normal-map mode '+mode);
 engine.scene.clear();engine.clear_lights();engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_simulation_paused(true);
 const back=state.view==='back-double',mirror=state.view==='mirror-front';engine.setCameraView(state.view==='grazing-double'?[4,0,.45]:[0,0,back?-4:4],[0,0,0]);lastAppearance={sky:[.012,.016,.025],ground:[.008,.01,.016],sunDirection:back?[0,0,-1]:[.12,.18,1],exposure:1,fogColor:[0,0,0],fogNear:100,fogFar:200,shadowExtent:8,shadowTarget:[0,0,0],shadowNear:.1,shadowFar:30,specularAA:true};engine.setSceneAppearance(lastAppearance);
 if(state.directional===false)engine.setSceneAppearance(null);
 const lights=state.lights??[{position:back?[0,0,-6]:[1,2,6],color:[3,2.7,2.3]}];
 lightHandles=lights.map(light=>engine.scene.createLight(light));
 const asset=makeAsset(state,mode),url=URL.createObjectURL(new Blob([JSON.stringify(asset)],{type:'model/gltf+json'}));let model;
 try{model=await engine.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}
 model.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});if(mirror)model.setScale([-1.25,.72,1]).setRotation([.16,.24,.08]);
 if(state.instances){
  model.setPosition([-.7,0,0]).setScale([.6,.7,1]).setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,opaqueReorderable:true});
  model.createInstance().setPosition([.7,0,-.2]).setScale([.6,.7,1]).setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false,opaqueReorderable:true});
 }
 const image=await capturePixels(canvas);assertForeground(image,`${stateId(state)} ${mode}`,100);const copy=document.createElement('canvas');copy.width=image.width;copy.height=image.height;copy.getContext('2d').putImageData(image,0,0);
 return{png:copy.toDataURL('image/png'),backend:engine.renderer_backend(),state:stateId(state),mode};
},async mutateLights(lights,directional){
 if(lights.length!==lightHandles.length)throw Error('Mutation preserves light slot count');
 lights.forEach((light,i)=>lightHandles[i].set(light.position,light.color));
 engine.setSceneAppearance(directional?lastAppearance:null);
 const image=await capturePixels(canvas);
 const copy=document.createElement('canvas');copy.width=image.width;copy.height=image.height;copy.getContext('2d').putImageData(image,0,0);
 return{png:copy.toDataURL('image/png'),backend:engine.renderer_backend(),stats:engine.debugStats().gltf};
},dispose(){engine.dispose();}};
