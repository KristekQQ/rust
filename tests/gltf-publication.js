import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground,assertChanged} from './pixels.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(v,m)=>{if(!v)throw Error(m);};
const frames=async(n=1)=>{while(n--)await new Promise(requestAnimationFrame);};
function fixture(){
 const data=new Uint8Array(188);
 new Float32Array(data.buffer,0,12).set([-.5,-.5,0,.5,-.5,0,.5,.5,0,-.5,.5,0]);
 new Float32Array(data.buffer,48,12).set([0,0,1,0,0,1,0,0,1,0,0,1]);
 new Uint16Array(data.buffer,96,6).set([0,1,2,0,2,3]);
 new Float32Array(data.buffer,108,2).set([0,1]);
 new Float32Array(data.buffer,116,6).set([-.3,0,0,.3,0,0]);
 new Float32Array(data.buffer,140,12).set([0,0,0,.4,0,0,.4,.3,0,0,.3,0]);
 return {asset:{version:'2.0'},buffers:[{uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...data)),byteLength:data.length}],
 bufferViews:[[0,48],[48,48],[96,12],[108,8],[116,24],[140,48]].map(([byteOffset,byteLength])=>({buffer:0,byteOffset,byteLength})),
 accessors:[{bufferView:0,componentType:5126,count:4,type:'VEC3',min:[-.5,-.5,0],max:[.5,.5,0]},{bufferView:1,componentType:5126,count:4,type:'VEC3'},{bufferView:2,componentType:5123,count:6,type:'SCALAR'},{bufferView:3,componentType:5126,count:2,type:'SCALAR',min:[0],max:[1]},{bufferView:4,componentType:5126,count:2,type:'VEC3'},{bufferView:5,componentType:5126,count:4,type:'VEC3'}],
 materials:[[.8,.02,.03],[.02,.2,.9]].map(c=>({doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[...c,1],roughnessFactor:.8,metallicFactor:0},emissiveFactor:c.map(x=>x*.25)})),
 meshes:[0,1].map(material=>({weights:[0],primitives:[{attributes:{POSITION:0,NORMAL:1},indices:2,material,targets:[{POSITION:5}]}]})),
 nodes:[{mesh:0},{mesh:1,translation:[.3,0,0]}],scenes:[{nodes:[0]},{nodes:[1]}],scene:0,
 animations:[{name:'Move',samplers:[{input:3,output:4,interpolation:'LINEAR'}],channels:[{sampler:0,target:{node:0,path:'translation'}}]}]};
}
const exact=(a,b,label)=>{assert(a.width===b.width&&a.height===b.height,label+' dimensions');let max=0;for(let i=0;i<a.data.length;i++)max=Math.max(max,Math.abs(a.data[i]-b.data[i]));assert(max===0,label+' max pixel difference '+max);};
const evidence=[];let e;
try{
 const backend=new URLSearchParams(location.search).get('backend')||'auto';
 const init=async()=>{const x=await initEngine(canvas,{backend,profiling:false});x.resize(640,400);x.set_camera_mode('free');x.set_simulation_paused(true);x.set_grid_visible(false);x.set_light_helpers_visible(false);x.set_render_effects(false,false);x.setCameraView([0,0,4],[0,0,0]);x.scene.createLight({position:[-2,3,4],color:[8,8,8]});return x;};
 e=await init();document.documentElement.dataset.rendererBackend=e.renderer_backend();
 const url=URL.createObjectURL(new Blob([JSON.stringify(fixture())],{type:'model/gltf+json'}));
 const load=async()=>{const m=await e.scene.loadModel(url);m.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:true,opaqueReorderable:true});return m;};
 const source=await load();const roots=[source,...Array.from({length:7},()=>source.createInstance())];
 roots.forEach((m,i)=>m.setPosition([(i%4-1.5)*.55,(Math.floor(i/4)-.5)*.8,0]).setScale([.4,.4,.4]));
 const snap=()=>e.debugStats().gltf.publication;
 const oracle=async label=>{
  const cached=await capturePixels(canvas);assertForeground(cached,label,20);
  e.setModelPublicationReuseEnabled(false);assert(!snap().enabled,'reference switch active');
  const reference=await capturePixels(canvas);exact(cached,reference,label+' full publication reference');
  e.setModelPublicationReuseEnabled(true);exact(cached,await capturePixels(canvas),label+' reuse restored');evidence.push({label,exact:true});return cached;
 };
 await frames(8);const initial=await oracle('initial pose');
 let before=snap();await frames(5);let after=snap();assert(after.modelVisits===before.modelVisits,'static models incur no publication visits');
 before=snap();e.setCameraView([.03,0,4],[0,0,0]);await frames(3);after=snap();assert(after.modelVisits===before.modelVisits,'camera changes incur no model publication visits');await oracle('camera change');
 const mutate=async(label,change,visits=1)=>{await frames(2);const a=snap();change();await frames(2);const b=snap();assert(b.modelVisits-a.modelVisits===visits,label+' expected '+visits+' visits, got '+(b.modelVisits-a.modelVisits));return oracle(label);};
 assertChanged(initial,await mutate('one translated model',()=>roots[2].setPosition([.45,.15,.1])),'translated pixels',10);
 await mutate('nonuniform negative affine',()=>roots[2].setRotation([.1,.3,.2]).setScale([-.5,.32,.4]));
 await mutate('render options and ordering policy',()=>roots[2].setRenderOptions({reflectivity:.25,receivesShadow:false,opaqueReorderable:false}));
 await mutate('explicit pose',()=>roots[2].playAnimation('Move',{loop:true}).pauseAnimation().seekAnimation(.7));
 await mutate('morph weights',()=>roots[2].setMorphWeights(0,[.8]));
 await mutate('active primitive scene',()=>roots[2].stopAnimation().selectScene(1));
 roots[2].remove();await frames(3);await oracle('model removal');
 // Replacement geometry and source ownership remain independent of queued IDs.
 const replacement=await load();replacement.setPosition([0,.15,.2]).setScale([.6,.6,.6]);await frames(3);await oracle('replacement geometry');
 e.scene.clear();const reloaded=await load();reloaded.setScale([.8,.8,.8]);await frames(3);await oracle('clear and reload');
 e.dispose();e=await init();const fresh=await load();fresh.setScale([.7,.7,.7]);await frames(3);await oracle('dispose and reinitialize');
 URL.revokeObjectURL(url);result.textContent='PASS: '+JSON.stringify(evidence);
}catch(error){result.textContent='FAIL: '+error.message;console.error(error);}finally{e?.dispose();}
