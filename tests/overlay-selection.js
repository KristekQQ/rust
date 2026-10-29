import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground,assertChanged} from './pixels.js';
import {installXRSimulator} from './xr/simulator.js';
import {createOverlayFixture as fixture} from './gltf-overlay-fixture.js';
const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(value,message)=>{if(!value)throw Error(message);};
const equal=(a,b,label)=>{
 assert(a.width===b.width&&a.height===b.height,label+': dimensions');
 for(let i=0;i<a.data.length;i++)if(a.data[i]!==b.data[i])throw Error(`${label}: byte ${i}: ${a.data[i]} != ${b.data[i]}`);
};

let engine,sim;
const evidence=[];
try{
 engine=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto',profiling:true});
 document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 assert(engine.debugStats().overlaySelectionReference===true,'safe default: original overlay selection on both backends');
 engine.resize(640,400);engine.set_camera_mode('free');engine.set_simulation_paused(true);
 engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
 engine.setBackgroundColor([.008,.014,.032]);engine.setCameraView([0,.1,3],[0,0,0]);
 // FXAA keeps the physical final composition/overlay selection path active.
 const setup=()=>{engine.set_simulation_paused(true);engine.postprocessing.enable('fxaa',{strength:1});engine.scene.createLight({position:[-2,3,5],color:[10,10,10]});};
 setup();
 const load=async blend=>{
  const url=URL.createObjectURL(new Blob([JSON.stringify(fixture(blend))],{type:'model/gltf+json'}));
  try{return await engine.scene.loadModel(url);}finally{URL.revokeObjectURL(url);}
 };
 const shot=async reference=>{
  engine.setOverlaySelectionReference(reference);
  const before=engine.debugStats().overlaySelection;
  assert(before&&Number.isFinite(before.calls),'overlay selection profiling counters available');
  const pixels=await capturePixels(canvas),stats=engine.debugStats();
  const delta=Object.fromEntries(Object.keys(before).filter(key=>typeof before[key]==='number').map(key=>[key,stats.overlaySelection[key]-before[key]]));
  assert(delta.calls>0,'actual overlay selection calls');
  assert(Number.isFinite(stats.drawCalls),'actual draw counters available');
  return {pixels,delta,draws:stats.drawCalls};
 };
 const check=async(label,{reject=false}={})=>{
  const original=await shot(true),optimized=await shot(false);
  assertForeground(original.pixels,label);assertForeground(optimized.pixels,label);
  equal(original.pixels,optimized.pixels,label+' original/optimized');
  assert(original.draws===optimized.draws,label+': actual draws preserved');
  const restored=await shot(true);equal(original.pixels,restored.pixels,label+' reference restored');
  engine.setOverlaySelectionReference(false);
  if(reject){
   assert(original.delta.gltfPrimitivesVisited>0,label+': original scans authored primitives');
   assert(optimized.delta.gltfPrimitivesVisited===0,label+': optimized skips immutable OPAQUE/MASK primitives');
   assert(optimized.delta.gltfImmutableRejects>0,label+': immutable guard exercised');
  }
  evidence.push({label,original:original.delta,optimized:optimized.delta,draws:optimized.draws,exact:true});
  return optimized.pixels;
 };
 engine.scene.createCube([-.9,-.72,-.25]).setScale([.25,.25,.25]);
 await check('native opaque, no overlays');
 const opaque=await load(false);
 await check('immutable OPAQUE and MASK, no overlays',{reject:true});
 engine.postprocessing.enable('bloom',{strength:.25});
 await check('immutable OPAQUE and MASK, bloom',{reject:true});
 engine.postprocessing.disable('bloom');opaque.remove();
 const model=await load(true);
 const blended=await check('active OPAQUE MASK BLEND');
 model.selectScene(1);const inactive=await check('inactive BLEND node');
 assertChanged(blended,inactive,'BLEND genuinely contributes visible pixels');
 model.selectScene(0);equal(blended,await check('active BLEND restored'),'scene restoration');
 model.selectScene(2);await check('BLEND-only active scene');model.selectScene(0);
 const instance=model.createInstance().setPosition([.2,.3,.35]).setScale([.55,.55,.55]);
 await check('add shared model instance');instance.remove();
 equal(blended,await check('remove shared model instance'),'instance removal restoration');
 const transparent=engine.scene.createCube([.05,-.15,.6]).setScale([.3,.3,.3]).setOpacity(.35);
 const alpha=await check('non-glTF alpha');transparent.setOpacity(1);
 assertChanged(alpha,await check('non-glTF opaque'),'non-glTF opacity changes rendered pixels');transparent.remove();
 // Emissive mesh supplies real geometry glow, including a live state change.
 const glow=engine.scene.createMesh({positions:[-.35,-.65,.45,.35,-.65,.45,0,-.35,.45,0,-.55,.8],indices:[0,2,1,0,1,3,1,2,3,2,0,3],color:[1,.2,.05]}).setEmission(2);
 engine.setNeonGlow({strength:.7,radius:.025});await check('geometry glow on');
 engine.postprocessing.enable('bloom',{strength:.2});await check('geometry glow and bloom');
 glow.setPosition([.1,.1,0]).setEmission(.8);await check('live glow geometry change');
 engine.setNeonGlow({strength:0});await check('glow disabled, bloom retained');
 engine.postprocessing.disable('bloom');glow.remove();
 for(const [name,eye,target] of [['left',-.032,-.06],['right',.032,.08]]){
  engine.setCameraView([eye,.1,3],[target,.025,0]);await check('asymmetric desktop eye '+name);
 }
 engine.setCameraView([0,.1,3],[0,0,0]);model.remove();
 await check('last glTF removal');
 const reloaded=await load(true);equal(blended,await check('glTF reload'),'reload restoration');reloaded.remove();
 engine.scene.clear();setup();
 engine.scene.createCube([0,0,0]).setScale([.45,.45,.45]);
 await check('clear resets overlay membership');
 const afterClear=await load(false);await check('opaque reload after clear',{reject:true});afterClear.remove();
 if(engine.renderer_backend()==='Gl'){
  // Real GL stereo framebuffer, asymmetric eye dimensions. Diagnostic readback
  // stays inside this fixture, never in timed/production rendering.
  sim=installXRSimulator({clockTimers:false,profile:{eyeWidth:180,eyeHeight:190,viewports:[{x:0,y:0,width:180,height:190},{x:180,y:3,width:164,height:178}]}});
  engine.scene.clear();setup();
  const xrModel=await load(true);xrModel.setPosition([0,1.6,-2]);
  const xrCube=engine.scene.createCube([0,1.3,-1.6]).setScale([.25,.25,.25]).setOpacity(.4);
  await engine.xr.enter();
  const xrShot=reference=>{
   engine.setOverlaySelectionReference(reference);
   const before=engine.debugStats().overlaySelection.calls;
   sim.frame({head:{position:[0,1.6,0]}});
   assert(!engine.xr.lastError,'XR renderer has no error');
   const debug=engine.debugStats();
   assert(debug.overlaySelection.calls-before===2,'one selection per asymmetric XR eye');
   return {left:sim.pixels('left'),right:sim.pixels('right'),draws:debug.drawCalls};
  };
  const xrCheck=label=>{
   const a=xrShot(true),b=xrShot(false),c=xrShot(true);
   for(const eye of ['left','right']){
    assertForeground(a[eye],label+' '+eye);equal(a[eye],b[eye],label+' '+eye+' optimized');equal(a[eye],c[eye],label+' '+eye+' restored');
   }
   assert(a.draws===b.draws,label+' stereo draws preserved');
   evidence.push({label,exact:true,simulatedXR:true,eyeSizes:[[a.left.width,a.left.height],[a.right.width,a.right.height]],draws:b.draws});
  };
  xrCheck('asymmetric XR mixed alpha');
  xrModel.selectScene(1);xrCheck('asymmetric XR BLEND inactive');
  xrModel.selectScene(0);xrCube.setOpacity(1);xrCheck('asymmetric XR alpha restored and native opaque');
  xrModel.setPosition([1,1.6,-2]);xrCheck('asymmetric XR moved model');
  await engine.xr.exit();sim.dispose();sim=null;
 }
 window.overlaySelectionEvidence=evidence;
 result.textContent='PASS: '+evidence.length+' exact overlay-selection oracles; immutable rejection, active alpha scenes, lifecycle, glow/bloom and desktop eye views';
}catch(error){result.textContent='FAIL: '+(error.stack||error);throw error;}
finally{engine?.dispose();sim?.dispose();}
