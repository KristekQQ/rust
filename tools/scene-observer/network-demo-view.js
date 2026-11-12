import {initEngine} from '../../js/engine.js';
const params=new URLSearchParams(location.search),canvas=document.querySelector('canvas'),side=params.get('side');
let engine,pending=[],received=0,cutChecks=0,duration=side==='smooth'?250:0;
const report=(type,extra={})=>parent.postMessage({type,side,...extra},location.origin);
try{
 engine=await initEngine(canvas,{backend:params.get('backend')==='webgl'?'webgl':'auto'});
 document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 engine.setDebugOverlay(false);engine.set_grid_visible(true);engine.set_light_helpers_visible(false);engine.set_render_effects(false,false);
 engine.postprocessing.disable('ssr');engine.postprocessing.disable('bloom');engine.postprocessing.disable('fxaa');
 engine.setBackgroundColor([.025,.045,.075]);engine.setNeonGlow({strength:.18,radius:.025});
 engine.setCameraView([0,1.65,3.2],[0,1.15,-1.3]);
 engine.setSceneInterpolation({durationMs:duration,maxDistance:3});
 function render(){
  try{for(const frame of pending){engine.applySimulationPublication(frame.wire,[]);
   if(frame.cut){
    for(const id of frame.cut.pieces)if(!engine.scene.getObject(id).exists)throw Error('Missing cut half '+id);
    let removed=false;try{engine.scene.getObject(frame.cut.removed);}catch{removed=true;}
    if(!removed)throw Error('Uncut parent remained after split');cutChecks++;
   }
   received++;}pending=[];}
  catch(e){document.querySelector('#error').textContent=e.message;report('demo-error',{error:e.message});}
  requestAnimationFrame(render);
 }
 window.addEventListener('message',event=>{
  if(event.origin!==location.origin||event.source!==parent)return;
  if(event.data.type==='demo-frame'){if(pending.length>=128)throw Error('Viewer frame queue exceeded128');pending.push(event.data);}
  if(event.data.type==='demo-config'){duration=side==='smooth'&&event.data.enabled?event.data.duration:0;engine.setSceneInterpolation({durationMs:duration,maxDistance:3});document.documentElement.dataset.interpolationMs=String(duration);}
 });
 document.documentElement.dataset.interpolationMs=String(duration);
 globalThis.networkDemoFrame={engine,get received(){return received;},get cutChecks(){return cutChecks;},async capture(){const {capturePixels}=await import('../../tests/pixels.js');return capturePixels(canvas);}};
 requestAnimationFrame(render);report('demo-ready',{backend:engine.renderer_backend()});
}catch(e){document.querySelector('#error').textContent=e.message;report('demo-error',{error:e.message});console.error(e);}
