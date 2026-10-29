import {initEngine} from '../js/engine.js';
import {capturePixels,assertForeground,assertChanged} from './pixels.js';
import {installXRSimulator} from './xr/simulator.js';

const canvas=document.querySelector('canvas'),result=document.querySelector('#result');
const assert=(value,message)=>{if(!value)throw Error(message);};
const equal=(a,b,label)=>{
 assert(a.width===b.width&&a.height===b.height,label+': dimensions');
 for(let i=0;i<a.data.length;i++)if(a.data[i]!==b.data[i])throw Error(`${label}: byte ${i}: ${a.data[i]} != ${b.data[i]}`);
};
const copyImage=image=>({width:image.width,height:image.height,data:new Uint8ClampedArray(image.data)});
const delta=(after,before)=>Object.fromEntries(Object.keys(before).filter(k=>typeof before[k]==='number').map(k=>[k,after[k]-before[k]]));

let engine,sim;
const evidence={runs:[],comparisons:[]};
try{
 engine=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')||'auto'});
 document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 assert(engine.debugStats().panelBufferReuse===(engine.renderer_backend()==='Gl'),'measured safe default: reuse only on WebGL');
 engine.resize(640,400);engine.set_camera_mode('free');engine.set_simulation_paused(true);
 engine.setBackgroundColor([.008,.014,.032]);engine.set_render_effects(false,false);
 engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.setCameraView([0,0,3],[0,0,0]);

 const run=async reuse=>{
  engine.scene.clear();engine.setPanelBufferReuse(reuse);
  engine.postprocessing.disable('bloom');engine.postprocessing.disable('fxaa');engine.setNeonGlow({strength:0});
  engine.scene.createLight({position:[-2,3,5],color:[10,10,10]});
  const panel=engine.scene.createInfoPanel('debug-text',{position:[0,0,0],width:1.5,height:.42});
  // Mixed signs and non-uniform scale exercise the same panel mesh upload path.
  panel.setScale([-1.05,.82,1.1]);
  const id=panel.id, snaps={}, steps=[];
  let previousStats=engine.debugStats().panelMeshes;
  const snapshot=async label=>{
   const image=copyImage(await capturePixels(canvas));
   const stats=engine.debugStats().panelMeshes;
   assert(stats&&stats.events&&Number.isFinite(stats.events.updates),label+': panel buffer counters available');
   assert(Number.isFinite(stats.liveBuffers)&&Number.isFinite(stats.capacityBytes)&&Number.isFinite(stats.payloadBytes),label+': panel buffer gauges available');
   snaps[label]=image;steps.push({label,events:delta(stats.events,previousStats.events),gauges:{liveBuffers:stats.liveBuffers,capacityBytes:stats.capacityBytes,payloadBytes:stats.payloadBytes,inactivePoolBytes:stats.inactivePoolBytes},byKind:{...stats.events.byKind}});
   previousStats=stats;return image;
  };
  const text=async(label,value)=>{const before=engine.debugStats().panelMeshes.events.updates;engine.diagnosticPanelText(id,value);const image=await snapshot(label);assert(engine.debugStats().panelMeshes.events.updates===before+1,label+': changed text performs exactly one bitmap update');return image;};
  await text('initial','ASYM PANEL 01');
  const changed=await text('changed','ASYM PANEL 02');
  assertChanged(snaps.initial,changed,'panel text mutation');
  const unchangedBefore=engine.debugStats().panelMeshes.events.updates;
  await snapshot('unchanged');
  assert(engine.debugStats().panelMeshes.events.updates===unchangedBefore,'unchanged text creates no bitmap updates');
  const short=await text('short','SHORT');assertChanged(changed,short,'shorter panel text');
  const empty=await text('empty','');assertChanged(short,empty,'empty panel text');
  const largeText='ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghijklmnopqrstuvwxyz!?';
  const large=await text('large',largeText);
  assert(large&&engine.debugStats().panelMeshes.events.updates>unchangedBefore,'64-character panel text updates bitmap');
  assertForeground(large,'64-character bitmap panel');
  // At a stable payload size, reuse must keep its logical GPU allocation flat;
  // the reference deliberately allocates a fresh pair on every dirty update.
  const plateauA=largeText;
  const plateauB=[...plateauA].reverse().join('');
  let currentPlateau=largeText;
  const nextPlateau=()=>{currentPlateau=currentPlateau===plateauA?plateauB:plateauA;return currentPlateau;};
  await text('plateau-start',nextPlateau());
  const plateauCapacity=engine.debugStats().panelMeshes.capacityBytes;
  const plateauSamples=[];
  for(let i=0;i<20;i++){
   const before=engine.debugStats().panelMeshes;
   await text(`cycle-${i}`,nextPlateau());
   const after=engine.debugStats().panelMeshes;
   const created=after.events.buffersCreated-before.events.buffersCreated;
   assert(after.liveBuffers===2,`cycle ${i}: panel retains two live buffers`);
   assert(after.capacityBytes<=4*Math.max(after.payloadBytes,8),`cycle ${i}: capacity bounded by current payload`);
   if(reuse){assert(after.events.writeCalls-before.events.writeCalls===1,`cycle ${i}: only vertex payload uploaded`);assert(after.events.indexUploadReuses-before.events.indexUploadReuses===1,`cycle ${i}: equal index bytes reused`);assert(after.indexShadowBytes<=after.capacityBytes,`cycle ${i}: CPU index shadow bounded by live GPU capacity`);}
   assert(reuse?created===0:created>=2,`cycle ${i}: ${reuse?'reuse keeps same-capacity buffers':'reference allocates replacement buffers'}`);
   assert(after.capacityBytes===plateauCapacity,`cycle ${i}: repeated peak capacity plateaus`);
   plateauSamples.push({cycle:i,createdBytes:after.events.createdBytes-before.events.createdBytes,createdBuffers:created,gauges:{liveBuffers:after.liveBuffers,capacityBytes:after.capacityBytes,payloadBytes:after.payloadBytes}});
   if(i%5===4){
    const small=await text(`cycle-${i}-small`,'TINY');
    assertForeground(small,'periodic short text');
    let gauges=engine.debugStats().panelMeshes;
    assert(gauges.liveBuffers===2&&gauges.capacityBytes<=4*Math.max(gauges.payloadBytes,8),`cycle ${i}: short text capacity bound`);
    const blank=await text(`cycle-${i}-empty`,'');
    gauges=engine.debugStats().panelMeshes;
    assert(gauges.liveBuffers===2&&gauges.capacityBytes<=4*Math.max(gauges.payloadBytes,8),`cycle ${i}: empty text capacity bound`);
    await text(`cycle-${i}-restore`,nextPlateau());
    gauges=engine.debugStats().panelMeshes;
    assert(gauges.liveBuffers===2&&gauges.capacityBytes<=4*Math.max(gauges.payloadBytes,8),`cycle ${i}: restored text capacity bound`);
    assertChanged(small,blank,`cycle ${i}: empty transition`);
   }
  }
  assert(plateauSamples.every(sample=>sample.gauges.capacityBytes===plateauCapacity),'same-size cycles repeat a stable peak allocation');
  await text('warm-1','WARM CYCLE 1');await text('warm-2','WARM CYCLE 2');
  await text('warm-1-again','WARM CYCLE 1');
  // These are asymmetric desktop camera poses, not physical XR eye captures.
  engine.setCameraView([-.032,0,3],[-.06,0,0]);await snapshot('desktop-left-view');
  engine.setCameraView([.032,0,3],[.08,0,0]);await snapshot('desktop-right-view');
  engine.setCameraView([0,0,3],[0,0,0]);
  engine.postprocessing.enable('fxaa',{strength:1});await snapshot('fxaa');
  engine.postprocessing.enable('bloom',{strength:.25});await snapshot('fxaa-bloom');
  engine.postprocessing.disable('bloom');await snapshot('fxaa-restored');
  engine.setNeonGlow({strength:.7,radius:.025});await snapshot('glow-on');
  engine.setNeonGlow({strength:0});await snapshot('glow-off');
  engine.postprocessing.disable('fxaa');await snapshot('effects-off');
  if(sim){
   await engine.xr.enter();
   assert(!engine.xr.lastError,'simulated XR entry succeeds');
   const xrSnapshot=(label,image)=>{
    assertForeground(image,`simulated XR ${label}`);
    snaps[`simulated-xr-${label}`]=copyImage(image);
   };
   const xrText=async(label,value)=>{
    const before=engine.debugStats().panelMeshes.events.updates;
    engine.diagnosticPanelText(id,value);
    sim.frame({head:{position:[0,0,.5],orientation:[0,0,0,1]}});
    assert(!engine.xr.lastError,`simulated XR ${label} renderer succeeds`);
    assert(engine.debugStats().panelMeshes.events.updates===before+1,`simulated XR ${label} updates panel once`);
    sim.frame({head:{position:[0,0,.5],orientation:[0,0,0,1]}});
    assert(engine.debugStats().panelMeshes.events.updates===before+1,`simulated XR ${label} unchanged stereo frame performs no panel update`);
    const left=sim.pixels('left'),right=sim.pixels('right');
    assert(left.width===180&&left.height===190,`simulated XR ${label} left viewport dimensions`);
    assert(right.width===164&&right.height===178,`simulated XR ${label} asymmetric right viewport dimensions`);
    xrSnapshot(`${label}-left`,left);xrSnapshot(`${label}-right`,right);
    return {left,right};
   };
   const stereo=[];
   stereo.push(await xrText('text','XR ASYMMETRIC PANEL'));
   stereo.push(await xrText('large',largeText));
   stereo.push(await xrText('short','TINY'));
   // Empty panels have no foreground; exact per-eye pixels are still compared.
   const beforeEmpty=engine.debugStats().panelMeshes.events.updates;
   engine.diagnosticPanelText(id,'');sim.frame({head:{position:[0,0,.5],orientation:[0,0,0,1]}});
   assert(engine.debugStats().panelMeshes.events.updates===beforeEmpty+1,'simulated XR empty text updates panel once');
   sim.frame({head:{position:[0,0,.5],orientation:[0,0,0,1]}});
   assert(engine.debugStats().panelMeshes.events.updates===beforeEmpty+1,'simulated XR empty unchanged frame performs no panel update');
   const emptyEyes={left:sim.pixels('left'),right:sim.pixels('right')};
   assert(emptyEyes.left.width===180&&emptyEyes.right.width===164,'simulated XR empty keeps asymmetric viewports');
   snaps['simulated-xr-empty-left']=copyImage(emptyEyes.left);snaps['simulated-xr-empty-right']=copyImage(emptyEyes.right);
   stereo.push(await xrText('restored','XR ASYMMETRIC PANEL'));
   await engine.xr.exit();
   assert(!engine.xr.lastError,'simulated XR exit succeeds');
   engine.diagnosticPanelText(id,'ASYM PANEL 01');
   snaps['simulated-xr-return-desktop']=copyImage(await capturePixels(canvas));
   assertForeground(snaps['simulated-xr-return-desktop'],'desktop rendering after simulated XR');
   await engine.xr.enter();assert(!engine.xr.lastError,'simulated XR reentry succeeds');
   sim.frame({head:{position:[0,0,.5],orientation:[0,0,0,1]}});
   const reentryLeft=sim.pixels('left'),reentryRight=sim.pixels('right');
   assert(reentryLeft.width===180&&reentryRight.width===164,'simulated XR reentry keeps asymmetric viewports');
   xrSnapshot('reentry-left',reentryLeft);xrSnapshot('reentry-right',reentryRight);
   await engine.xr.exit();assert(!engine.xr.lastError,'simulated XR reentry exit succeeds');
   evidence.simulatedXR={profile:sim.profile,eyeSizes:[[180,190],[164,178]],textStates:stereo.length+1,exactPerEye:true};
  }
  panel.remove();await snapshot('removed');
  assert(engine.debugStats().panelMeshes.liveBuffers===0,'panel removal releases its live bitmap buffers');
  const reloaded=engine.scene.createInfoPanel('debug-text',{position:[0,0,0],width:1.5,height:.42});
  engine.diagnosticPanelText(reloaded.id,'ASYM PANEL 01');await snapshot('reloaded');
  engine.scene.clear();await snapshot('cleared');
  assert(engine.debugStats().panelMeshes.liveBuffers===0,'scene clear releases panel buffers');
  for(let i=0;i<3;i++){
   const next=engine.scene.createInfoPanel('debug-text',{position:[0,0,0],width:1.5,height:.42});
   engine.diagnosticPanelText(next.id,'RELOAD '+i);await snapshot(`reload-${i}`);
   assert(engine.debugStats().panelMeshes.liveBuffers===2,`reload ${i}: panel buffers restored`);
   engine.scene.clear();await snapshot(`clear-${i}`);
   assert(engine.debugStats().panelMeshes.liveBuffers===0,`clear ${i}: panel buffers released`);
  }
  if(reuse)assert(steps.reduce((sum,step)=>sum+step.events.writeCalls,0)>0,'reused path performs queue buffer writes');
  evidence.runs.push({reuse,steps,plateauSamples});return snaps;
 };
 if(engine.renderer_backend()==='Gl')sim=installXRSimulator({clockTimers:false,profile:{eyeWidth:180,eyeHeight:190,viewports:[{x:0,y:0,width:180,height:190},{x:180,y:3,width:164,height:178}]}});
 else evidence.xrSkipped='The maintained simulator targets the WebGL XR path.';
 const original=await run(false),reused=await run(true);
 const labels=Object.keys(original);
 for(const label of labels){equal(original[label],reused[label],'original/reused '+label);const blank=label==='empty'||label.endsWith('-empty')||label.startsWith('simulated-xr-empty-')||label==='removed'||label==='cleared'||label.startsWith('clear-');if(!blank)assertForeground(reused[label],'reused '+label);evidence.comparisons.push({label,exact:true});}
 window.panelBufferEvidence=evidence;
 result.textContent=`PASS: ${engine.renderer_backend()} original/reused bitmap panels; ${labels.length} exact images, text updates, effects and lifecycle${evidence.simulatedXR?'; simulated asymmetric XR per-eye oracle (not headset acceptance)':''}`;
}catch(error){result.textContent='FAIL: '+(error.stack||error);throw error;}
finally{engine?.dispose();sim?.dispose();}
