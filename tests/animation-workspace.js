import {initEngine} from '../js/engine.js';
import {installXRSimulator} from './xr/simulator.js';
import {installGPUProjection} from './xr/gpu-projection.js';
import {XRTestRun,publishXRReport} from './xr/testkit.js';
import {assertForeground,assertChanged} from './pixels.js';
const gpu=new URLSearchParams(location.search).get('backend')!=='webgl';
const sim=installXRSimulator({clockTimers:false,profile:{id:'animation-workspace-90',hz:90,eyeWidth:320,eyeHeight:320}});
const projection=gpu?installGPUProjection():null,run=new XRTestRun('animation-workspace',sim,publishXRReport);
let engine;const captures=[];
function png(image){const canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;canvas.getContext('2d').putImageData(image,0,0);return canvas.toDataURL();}
const report=await run.execute(async test=>{
 engine=await initEngine(document.querySelector('canvas'),{backend:gpu?'auto':'webgl',profiling:false});document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 engine.set_simulation_paused(true);engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.resize(320,320);
 const layout=await (await fetch('../assets/models/rim/layout.json')).json(),tank=layout.models.find(m=>m.slug==='core-tank');
 await engine.xr.enter({resolutionScale:1});
 const device=projection?.bindings.at(-1).device;device?.pushErrorScope('validation');
 for(const spec of [
  {name:'rigid-tank',path:'../assets/models/rim/core-tank.glb',clip:'Idle',scale:tank.root.scale,position:tank.root.position.map((v,i)=>v+(i===2?-3:0))},
  {name:'skin-fox',path:'../assets/models/fox/Fox.glb',clip:'Walk',scale:[.02,.02,.02],position:[0,0,-4]},
  {name:'morph',path:'fixtures/gltf/full/full.gltf',clip:'Morph',scale:[.25,.25,.25],position:[0,1.5,-3]}
 ]){
  engine.set_simulation_paused(true);engine.scene.clear();engine.scene.createLight({position:[-2,4,0],color:[16,16,16]});
  const model=await engine.scene.loadModel(new URL(spec.path,import.meta.url));model.setPosition(spec.position).setScale(spec.scale).playAnimation(spec.clip,{loop:true});
  engine.set_simulation_paused(false);let first;
  for(let frame=1;frame<=24;frame++){
   sim.frame({dt:1000/90,head:{position:[0,1.6,0]}});test.assert(!engine.xr.lastError,'hot animation stereo frame completes',String(engine.xr.lastError));
   if(frame===6||frame===24){
    const images=await Promise.all(['left','right'].map(eye=>projection?projection.pixels(eye):Promise.resolve(sim.pixels(eye))));
    images.forEach((image,i)=>assertForeground(image,spec.name+' actual eye'+i));
    captures.push({name:spec.name,frame,playback:model.info.playback,images:images.map(png)});
    if(first&&spec.name!=='rigid-tank')assertChanged(first,images[0],spec.name+' hot updates change image',10);
    first=images[0];
   }
  }
  test.assert(model.info.playback.time>0&&model.info.lastError===null,'native playback progresses without errors');
 }
 await engine.xr.exit();await engine.xr.enter({resolutionScale:1});
 sim.frame({dt:1000/90,head:{position:[0,1.6,0]}});test.assert(!engine.xr.lastError,'warm animation workspace survives XR reentry');await engine.xr.exit();
 if(device){await device.queue.onSubmittedWorkDone();const error=await device.popErrorScope();test.assert(!error,'actual GPU animation renders without validation errors',String(error));}
});
engine?.dispose();sim.dispose();projection?.dispose();
window.animationWorkspaceEvidence={captures,scope:'fixed90Hz simulation steps; actual rigid/skin/morph hot animation and stereo pixels; no headset performance inference'};
publishXRReport(report);
