import {initEngine} from '/js/engine.js';
import {assertForeground} from './pixels.js';

const canvas=document.querySelector('canvas'),requested=new URLSearchParams(location.search).get('backend');
if(!['webgpu','webgl'].includes(requested))throw Error('Explicit backend required');
const clone=value=>JSON.parse(JSON.stringify(value));
const api=()=>clone(window.renderApiCounters??null);
if(!window.renderApiCounters)throw Error('Driver must install maintained API counters before initialization');
const nativeRaf=window.requestAnimationFrame.bind(window);
let phase='init',callbacks=0,armed=null,phaseRows=[],pendingCapture=null,latestFrame=null,pixelInfo=null;
// Observe real engine callbacks. Copy only after rendering, outside timing.
window.requestAnimationFrame=callback=>nativeRaf(stamp=>{
 const active=armed,before=active&&active.rows.length===0?api():null,start=performance.now();
 try{return callback(stamp);}finally{
  const end=performance.now();callbacks++;
  const row={stamp,startMs:start,endMs:end,callbackWallMs:end-start,phase};latestFrame=row;
  if(phase==='init'||phase.endsWith(' asset-load'))phaseRows.push(row);
  if(active){
   active.rows.push(row);
   if(active.rows.length===1)active.first={apiBefore:before,apiAfter:api(),activationToCallbackEndMs:end-active.activationMs};
   if(active.rows.length===121){armed=null;active.apiEnd=api();active.resolve(active);}
  }
  if(!armed&&pendingCapture){
   const capture=pendingCapture;pendingCapture=null;clearTimeout(capture.timer);
   try{
    const copy=document.createElement('canvas');copy.width=canvas.width;copy.height=canvas.height;
    const context=copy.getContext('2d',{willReadFrequently:true});
    if(!context)throw Error('Capture 2D context unavailable');
    context.drawImage(canvas,0,0);const image=context.getImageData(0,0,copy.width,copy.height);
    let foreground=0,nonzeroAlpha=0;const data=image.data;
    for(let i=0;i<data.length;i+=4){if(data[i+3])nonzeroAlpha++;if(Math.abs(data[i]-data[0])+Math.abs(data[i+1]-data[1])+Math.abs(data[i+2]-data[2])>20)foreground++;}
    pixelInfo={width:image.width,height:image.height,foreground,nonzeroAlpha,firstPixel:Array.from(data.slice(0,4)),capturedAfterFrame:row};
    capture.resolve(image);
   }catch(error){pixelInfo={captureError:String(error),capturedAfterFrame:row};capture.reject(error);}
  }
 }
});
const initStart=performance.now(),initApi=api();
const engine=await initEngine(canvas,{backend:requested==='webgl'?'webgl':'auto',profiling:false});
const initialized={wallMs:performance.now()-initStart,apiBefore:initApi,apiAfter:api(),callbacks,observedCallbacks:phaseRows};
const expectedBackend={webgl:'Gl',webgpu:'BrowserWebGpu'}[requested];
if(engine.renderer_backend()!==expectedBackend)throw Error('Backend fallback: '+engine.renderer_backend());
if(engine.frameProfile().enabled!==false||canvas.dataset.frameProfile!=='false')throw Error('Profiling must be disabled');
engine.resize(640,400);engine.setDebugOverlay(false);engine.set_camera_mode('free');
engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_simulation_paused(true);
engine.set_frustum_culling(false);engine.scene.clear();engine.set_render_effects(false,false);
engine.setBackgroundColor([0,0,0]);engine.setCameraView([0,0,4],[0,0,0]);
engine.scene.createLight({position:[-2,3,5],color:[12,12,12]});
const optional=name=>typeof engine[name]==='function'?engine[name]():null;
const diagnostics=()=>({debug:engine.debugStats(),indexLayouts:optional('gltfIndexLayouts'),vertexLayouts:optional('gltfVertexLayouts')});
function coloredAsset(){
 const bytes=new Uint8Array(126);
 new Float32Array(bytes.buffer,0,9).set([-1,-1,0,1,-1,0,0,1,0]);
 new Float32Array(bytes.buffer,36,9).set([0,0,1,0,0,1,0,0,1]);
 new Float32Array(bytes.buffer,72,12).set([1,.1,.05,.55,.05,1,.1,1,.1,.1,1,.8]);
 new Uint16Array(bytes.buffer,120,3).set([0,1,2]);
 return {asset:{version:'2.0'},buffers:[{byteLength:bytes.length,uri:'data:application/octet-stream;base64,'+btoa(String.fromCharCode(...bytes))}],
  bufferViews:[{buffer:0,byteOffset:0,byteLength:36},{buffer:0,byteOffset:36,byteLength:36},{buffer:0,byteOffset:72,byteLength:48},{buffer:0,byteOffset:120,byteLength:6}],
  accessors:[{bufferView:0,componentType:5126,count:3,type:'VEC3',min:[-1,-1,0],max:[1,1,0]},
   {bufferView:1,componentType:5126,count:3,type:'VEC3'},{bufferView:2,componentType:5126,count:3,type:'VEC4'},
   {bufferView:3,componentType:5123,count:3,type:'SCALAR'}],
  materials:[{doubleSided:true,alphaMode:'BLEND',pbrMetallicRoughness:{baseColorFactor:[1,1,1,1],metallicFactor:0,roughnessFactor:.8}}],
  meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1,COLOR_0:2},indices:3,material:0}]}],
  nodes:[{mesh:0,scale:[-1.15,.8,1.4]}],scenes:[{nodes:[0]}],scene:0};
}
const variants=[];
async function measureVariant(name,url){
 phase=name+' asset-load';phaseRows=[];const apiBefore=api(),start=performance.now(),callbackStart=callbacks;
 const model=await engine.scene.loadModel(url);
 const load={wallMs:performance.now()-start,callbacks:callbacks-callbackStart,observedCallbacks:phaseRows,apiBefore,apiAfter:api()};
 model.setRenderOptions({isStatic:true,castsShadow:false,receivesShadow:false});
 if(name==='colored-rgba')model.setRotation([.12,.18,.06]);
 phase=name+' live';
 const result=await new Promise(resolve=>{armed={resolve,rows:[],activationMs:performance.now()};});
 const native=diagnostics();
 // Timing has ended before any capture, readback or foreground assertion.
 const image=await new Promise((resolve,reject)=>{
  if(armed||pendingCapture)throw Error('Capture requires completed timing');
  const capture={resolve,reject,timer:null};pendingCapture=capture;
  capture.timer=setTimeout(()=>{if(pendingCapture===capture)pendingCapture=null;reject(Error('Post-render capture timeout'));},10000);
 });
 const foregroundPixels=assertForeground(image,name,100),copy=document.createElement('canvas');
 copy.width=image.width;copy.height=image.height;copy.getContext('2d').putImageData(image,0,0);
 variants.push({name,load,first:{...result.first,...result.rows[0]},warm:{callbacks:result.rows.slice(1),apiBefore:result.first.apiAfter,apiAfter:result.apiEnd},
  native,foregroundPixels,png:copy.toDataURL('image/png')});
 model.remove();
}
window.coldVariants={status:'RUNNING',dispose(){engine.dispose();window.requestAnimationFrame=nativeRaf;}};
try{
 await measureVariant('white-helmet','/assets/models/damaged-helmet/DamagedHelmet.glb');
 const asset=coloredAsset(),url=URL.createObjectURL(new Blob([JSON.stringify(asset)],{type:'model/gltf+json'}));
 try{await measureVariant('colored-rgba',url);}finally{URL.revokeObjectURL(url);}
 const queryCreation=clone(window.renderQueryCreationCounters??null),query=queryCreation?.[requested];
 if(!query?.installed||query.calls!==0||query.created!==0||query.overflow||queryCreation.errors.length)throw Error('Expected observed zero query creation calls');
 window.coldVariants.result={requestedBackend:requested,backend:engine.renderer_backend(),profiling:engine.frameProfile().enabled,
  timerQueriesEnabled:query.calls>0,queryCreation,initialized,variants,apiErrors:window.renderApiCounters.errors,
  losses:window.renderRuntimeLosses??[],scope:'Instrumented desktop CPU wall and actual API calls; no GPU durations. Asset-load may create shaders/pipelines. First live frame means first post-load engine RAF; earlier asset drawing during load is not excluded. Warm samples are the next 120 actual engine RAF callbacks.'};
 window.coldVariants.status='PASS';
}catch(error){
 let native=null;try{native=diagnostics();}catch(diagnosticError){native={error:String(diagnosticError)};}
 window.coldVariants.failure={phase,viewport:{innerWidth,innerHeight,devicePixelRatio,canvasWidth:canvas.width,canvasHeight:canvas.height,rect:canvas.getBoundingClientRect().toJSON()},
  backend:engine.renderer_backend(),native,latestFrame,pixelInfo,api:api(),queryCreation:clone(window.renderQueryCreationCounters??null),losses:window.renderRuntimeLosses??[]};
 window.coldVariants.status='FAIL';window.coldVariants.error=String(error);throw error;
}
