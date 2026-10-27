// Benchmark adapter around the unchanged original Three Rome application.
// This exposes observed rendering work; it does not claim identical AA/shaders
// or replace its gameplay with a synthetic workload.
import '/examples/rim/main.js';
import {REVISION} from 'three';
const d=window.cityDebug;
await d.characters.ready;await d.combat.ready;
if(d.characters.state.failed.length)throw Error('Original Three character loading failed');
const q=new URLSearchParams(location.search),width=Number(q.get('width')||960),height=Number(q.get('height')||640);if(!Number.isInteger(width)||!Number.isInteger(height)||width<64||height<64||width>4096||height>4096)throw Error('Invalid render dimensions');
d.setManualTime(true);
for(const entry of d.characters.state.entries)if(entry.loaded)entry.mixer.setTime(0);
const family=q.get('family')||'rim-static',load=Number(q.get('load')||1);
let stressModels=0;
if(family==='rim-stress'){
 const {stressPosition,MAX_STRESS_MODELS}=await import('/examples/rim/stress.js');
 if(!Number.isInteger(load)||load<1||load>MAX_STRESS_MODELS)throw Error('Invalid Three stress load');
 const original=d.characters.state.entries.find(e=>e.slug==='drone-orbiter'&&e.loaded);
 if(!original)throw Error('Missing original Three drone');
 for(let i=0;i<load;i++){
  const model=original.root.clone(true);model.position.set(...stressPosition(i));
  model.traverse(o=>{if(o.isMesh){o.castShadow=false;o.receiveShadow=true;}});
  d.scene.add(model);model.updateMatrixWorld(true);stressModels++;
 }
}
d.renderer.setPixelRatio(1);d.renderer.setSize(width,height,false);
d.rig.position.set(0,0,42);d.rig.rotation.set(0,0,0);
d.camera.position.set(0,1.7,0);d.camera.rotation.set(0,0,0);d.camera.fov=65;d.camera.aspect=width/height;d.camera.near=.08;d.camera.far=350;d.camera.updateProjectionMatrix();
const render=()=>d.renderer.render(d.scene,d.camera);
render();
const stats=()=>({...d.renderer.info.render,drawCalls:d.renderer.info.render.calls,mainDrawCalls:d.renderer.info.render.calls,memory:{...d.renderer.info.memory}});
const gl=d.renderer.getContext();
const engine={renderer_backend:()=> 'ThreeWebGL2',renderer_stats:stats,debugStats:stats,frameProfile:()=>({enabled:false,cpu:{},gpu:{supported:false,available:false,passesMs:null,sampleFrame:null,ageFrames:null,error:null}}),postprocessing:{stats:()=>({enabled:false,antialias:d.renderer.getContext().getContextAttributes().antialias,samples:gl.getParameter(gl.SAMPLES)}),graph:()=>({passes:[]})}};
window.bench={engine,family,load,operations:{transforms:0,spawns:0,removes:0,cuts:0},start(){d.setManualTime(true);},stop(){d.setManualTime(true);},capturePose(){d.setManualTime(true);render();},phaseState:()=>({kind:'original-three-rim',stressModels,gameplay:d.combat.describe().phase,characters:d.characters.describe()}),metadata:{family,load,stressModels,scene:'unchanged original Three Rome application, static gate camera; original character mixers at time0',viewport:{width,height,deviceScaleFactor:1},camera:{position:[0,1.7,42],target:[0,1.7,-20],fov:65,near:.08,far:350},renderer:{version:REVISION,antialias:gl.getContextAttributes().antialias,samples:gl.getParameter(gl.SAMPLES),shadows:d.renderer.shadowMap.enabled,shadowMapSize:2048,shadowAutoUpdate:d.renderer.shadowMap.autoUpdate,runtimeInfoIncluded:true,toneMappingExposure:d.renderer.toneMappingExposure},unmatchedThreeReference:true,differences:'Same authored city/camera/viewport and lighting profile, but Three uses hardware MSAA instead of Rust FXAA; shaders/material normal semantics, shadow policy and independently authored gameplay differ. Static render diagnostic only, not proven whole-game or pixel equivalence.'}};
