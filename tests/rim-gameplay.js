import {installRuntimeInfo} from '../examples/shared/runtime-info.js';
import {configureAppearance} from '../examples/rim/appearance.js';
import {initEngine} from '../js/engine.js';
import {createStressLoad} from '../examples/rim/stress.js';
import {createScene,createGameplay} from '../examples/rim/scene.js';
import {installCutTransport} from '../examples/rim/cut-transport.js';
import {installGPUProjection} from './xr/gpu-projection.js';
import {installXRSimulator} from './xr/simulator.js';
import {XRTestRun,publishXRReport} from './xr/testkit.js';
import {assertForeground,assertChanged} from './pixels.js';
const query=new URLSearchParams(location.search),gpu=query.get('backend')!=='webgl';
const hz=Number(query.get('hz')||(query.has('measureCuts')?72:90));
const sim=installXRSimulator({profile:{id:'aurelia-'+hz,hz,eyeWidth:320,eyeHeight:320},clockTimers:false});
const projection=gpu?installGPUProjection():null;
const pixels=eye=>projection?projection.pixels(eye):Promise.resolve(sim.pixels(eye));
const run=new XRTestRun('rim-gameplay',sim,publishXRReport);let engine,stop,info;
const browserRAF=window.requestAnimationFrame;let documentRAFCalls=0;
const testXR=engine=>new URLSearchParams(location.search).has('omitXRHook')?{get isPresenting(){return engine.xr.isPresenting;},addEventListener:(name,fn)=>{if(name!=='frame')engine.xr.addEventListener(name,fn);},removeEventListener:(name,fn)=>engine.xr.removeEventListener(name,fn)}:engine.xr;
const measureOnly=new URLSearchParams(location.search).has('measureCuts');
const xrOptions={resolutionScale:1,...(query.get('presentation')==='shared-framebuffer'?{webglPresentation:'shared-framebuffer'}:{})};
const cutTiming={acceptMs:[],prepareMs:[],framesMs:[],frameEvents:[]};let cutMeasuring=false,deathAt=0,cutRequestedAt=0,cutPublishedAt=0;
await run.execute(async test=>{
 engine=await initEngine(document.querySelector('#gpu-canvas'),{backend:new URLSearchParams(location.search).get('backend')==='webgl'?'webgl':'auto'});
 document.documentElement.dataset.rendererBackend=engine.renderer_backend();engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
 configureAppearance(engine);
 const content=await createScene(engine.scene);const game=await createGameplay(engine,content);
 const stressCount=Number(query.get('stress')||0),stressLoad=stressCount?createStressLoad(engine.scene,content):null;
 stressLoad?.setCount(stressCount);
 const transportScene={rimCutFailed:()=>engine.scene.rimCutFailed()};
 for(const [name,values] of [['rimAcceptCut',cutTiming.acceptMs],['rimTakeCutJob',cutTiming.prepareMs]]){
  const original=engine.scene[name].bind(engine.scene);
  transportScene[name]=(...args)=>{const start=performance.now();const value=original(...args);if(name==='rimTakeCutJob'&&value.length&&!cutRequestedAt)cutRequestedAt=start;if(name==='rimAcceptCut'||value.length)values.push(performance.now()-start);return value;};
 }
 stop=installCutTransport(transportScene,testXR(engine));const input=v=>engine.scene.rimInput(v),state=()=>engine.scene.rimState;
 info=installRuntimeInfo(engine);
 input({start:true});await engine.xr.enter(xrOptions);
 // Immersive browsers may suspend document RAF while XRSession RAF continues.
 window.requestAnimationFrame=()=>{documentRAFCalls++;return 0;};
 const frame=(v={})=>{const start=performance.now();sim.frame(v);if(cutMeasuring){const cpuMs=performance.now()-start;cutTiming.framesMs.push(cpuMs);const s=state();if(!deathAt&&s.kills>0)deathAt=performance.now();if(cutRequestedAt&&!cutPublishedAt&&s.cuts.pieces>0)cutPublishedAt=performance.now();if(measureOnly)cutTiming.frameEvents.push({cpuMs,kills:s.kills,cuts:s.cuts,uploadBytes:engine.renderer_stats().uploadBytes});}if(engine.xr.lastError)throw Error(String(engine.xr.lastError));};
 frame({head:{position:[0,1.7,0]},controllers:{left:{position:[-.3,1.4,-.1]},right:{position:[.3,1.4,-.1]}}});frame();
 test.assert(state().enemies.length===6,'six-orbiter first wave');
 test.assert(engine.xr.player.head.object?.exists===true,'shared native diagnostics panel exists in XR');
 test.assert(!document.querySelector('[data-runtime-info]')||document.querySelector('[data-runtime-info]').hidden,'desktop diagnostics hide in XR');
 test.assert(Math.abs(engine.xr.player.head.position[2]-42)<.1,'XR eyes/head share original gate spawn');
 test.assert(engine.xr.player.state.leftHand.object.id===game.swords[0].id&&engine.xr.player.state.rightHand.object.id===game.swords[1].id,'both tracked hands hold original gladii');
 assertForeground(await pixels('left'),'Aurelia left eye');assertForeground(await pixels('right'),'Aurelia right eye');
 if(!measureOnly){
  test.assert(game.blobShadows.length===14,'one pooled ground shadow per enemy');
  const enemy=state().enemies[0];
  input({teleport:[enemy.position[0],0,enemy.position[2]+3]});
  frame({dt:0,head:{orientation:[-Math.sin(.2),0,0,Math.cos(.2)]}});
  const withBlobs=[await pixels('left'),await pixels('right')],cached=engine.effects_stats();
  for(const blob of game.blobShadows)blob.setOpacity(0);
  frame({dt:0});
  for(const [i,eye] of ['left','right'].entries()){
   const without=await pixels(eye);let darker=0;
   for(let j=0;j<without.data.length;j+=4){const delta=without.data[j]+without.data[j+1]+without.data[j+2]-withBlobs[i].data[j]-withBlobs[i].data[j+1]-withBlobs[i].data[j+2];if(delta>12)darker++;}
   test.assert(darker>=12,'ground shadows darken pavement in '+eye+' eye',{darker});
  }
  const after=engine.effects_stats();
  test.assert(after.staticUpdates===cached.staticUpdates&&after.dynamicUpdates===cached.dynamicUpdates,'blob visibility leaves both shadow maps cached');
  for(const blob of game.blobShadows)blob.setOpacity(.08);
  input({teleport:[0,0,42]});frame({dt:0,head:{orientation:[0,0,0,1]}});
 }
 if(stressLoad){
  const load=stressLoad,before=engine.debugStats().gltf.trianglesDrawn;
  frame({dt:0});frame({dt:0});
  test.assert(state().stressModels===stressCount,'native stress count survives immersive entry');
  test.assert(load.objects.length===stressCount&&load.objects.every(o=>o.exists),'stress model handles exist in actual native scene');
  const withLoad=[await pixels('left'),await pixels('right')],after=engine.debugStats().gltf.trianglesDrawn;
  test.assert(after>before,'extra stress geometry is submitted to real GPU');
  // Timing replay retains the requested fleet through the actual worker cut.
  // The ordinary correctness scenario still verifies removal and visible pixels.
  const changed=[];if(!measureOnly){
   load.dispose();frame({dt:0});
   for(const [i,eye] of ['left','right'].entries())changed.push(assertChanged(withLoad[i],await pixels(eye),'visible stress fleet '+eye,20));
   test.assert(state().stressModels===0,'stress removal restores native panel count without resetting combat');
  }
  window.rimStressEvidence={count:stressCount,retainedForCut:measureOnly,changedPixels:changed,trianglesDrawnDelta:after-before,mode:'simulated XR, actual native renderer'};
 }
 const z=state().player.position.z;for(let i=0;i<Math.ceil(hz*15/72);i++)frame({controllers:{left:{axes:[0,0,0,-1]}}});frame({controllers:{left:{axes:[0,0,0,0]}}});
 test.assert(state().player.position.z<z-.3,'left stick moves native player through city',{z,player:state().player});
 frame({controllers:{right:{buttons:[1,0,0,0,0,0]}}});test.assert(state().player.aiming,'right trigger starts bullet-time jump aiming');
 frame({controllers:{right:{buttons:[0,0,0,0,0,0]}}});test.assert(state().player.jump.count===1,'trigger release launches ballistic jump');
 input({teleport:[0,0,42]});frame();test.assert(Math.abs(engine.xr.player.head.position[2]-42)<.1,'native teleport updates same XR origin');
 frame({controllers:{right:{buttons:[0,0,0,0,1,0]}}});for(let i=0;i<16;i++)frame();frame({controllers:{right:{buttons:[0,0,0,0,0,0]}}});
 test.assert(state().player.dash.count===1&&state().player.stamina.value<3,'A dashes and spends native stamina');
 input({teleport:[0,0,42]});frame();const enemy=state().enemies[0];input({teleport:[enemy.position[0],0,enemy.position[2]+1.05]});frame({controllers:{right:{tracked:false}}});frame({controllers:{right:{tracked:true,position:[.6,1.4,-.4]}}});test.assert(state().kills===0,'tracking reentry never invents a slash');
 const stroke=async(from,to,pace=false)=>{const steps=Math.ceil(Math.abs(to-from)/10.8*hz);for(let i=1;i<=steps;i++){const x=from+(to-from)*i/steps;frame({controllers:{right:{position:[x,1.4,-.4]}}});if(pace)await new Promise(r=>setTimeout(r,1000/hz));}};
 await stroke(.6,-.45);
 const wounded=state().enemies.find(e=>e.id===enemy.id);
 test.assert(wounded?.hp===50&&wounded.phase!==3&&state().kills===0,'first tracked blade swing removes 50 HP without killing');
 test.assert(state().cuts.queued===0&&state().cuts.pending===0&&state().cuts.pieces===0,'nonfatal tracked strike never requests worker cuts');
 for(let i=0;i<Math.ceil(.35*hz);i++)frame({controllers:{right:{position:[-.9,1.4,-.4]}}});
 const target=state().enemies.find(e=>e.id===enemy.id);input({teleport:[target.position[0],0,target.position[2]+1.05]});frame({controllers:{right:{position:[-.9,1.4,-.4]}}});
 // Timing mode paces the gesture and drains the initial simulator burst.
 if(measureOnly)await new Promise(r=>setTimeout(r,100));
 cutMeasuring=true;
 await stroke(-.9,.45,measureOnly);
 test.assert(state().kills>=1&&state().enemies.find(e=>e.id===enemy.id)?.hp===0,'second separated tracked blade swing kills original orbiter');test.assert(state().frame.simulation===state().frame.visual,'gameplay updates before the current stereo frame');
 // Let the platform worker run while simulation remains explicitly frame driven.
 const cutStages=[];let previousCommits=state().cuts.commits;
 const cutWaitLimit=new URLSearchParams(location.search).has('omitXRHook')?40:250;
 for(let i=0;i<cutWaitLimit&&state().cuts.pieces===0;i++){
  await new Promise(r=>setTimeout(r,measureOnly?14:10));frame();const cut=state().cuts;
  cutStages.push({...cut,uploadBytes:engine.renderer_stats().uploadBytes,frameCpuMs:cutTiming.framesMs.at(-1)});
  if(!measureOnly)test.assert(cut.preparedTriangles<=3500&&cut.stagedVertices<=9000,'cut work stays bounded inside large primitives');
  if(!measureOnly&&cut.commits===previousCommits)test.assert(cut.pieces===0,'staged halves remain invisible before atomic commit');
 }
 if(new URLSearchParams(location.search).has('omitXRHook')){
  test.assert(state().cuts.pieces===0&&state().cuts.queued>0,'negative control: suspended document RAF without XR hook leaves cut queued');
  return;
 }
 test.assert(state().cuts.pieces>0&&state().cuts.errors===0&&(measureOnly||state().cuts.commits>previousCommits),'worker Rust slice atomically publishes uploaded visible halves');
 cutMeasuring=false;
 test.assert(cutRequestedAt>0&&cutPublishedAt>=cutRequestedAt,'cut timing observes a real serialized job and atomic publication frame');
 window.rimCutEvidence={...cutTiming,stressCount:state().stressModels,completionMs:performance.now()-deathAt,requestToPublishMs:cutPublishedAt-cutRequestedAt,requestToPublishScope:'first nonempty rimTakeCutJob invocation through end of first simulated XR frame exposing complete uploaded pieces; excludes GPU completion/compositor',completionScope:'first observed kill through post-stroke publication check; includes observation delay',stages:cutStages,transport:window.rimCutTransportDiagnostics};
 // Preserve actual renderer pixels before XR exit/disposal clears its canvas.
 // Native state asserts the motion; these snapshots make its visual stages reviewable.
 const snapshots=[];
 const capture=async label=>{const image=await pixels('left');assertForeground(image,label);snapshots.push({label,pixels:image,age:state().cuts.motion[0]?.age});};
 await capture('HP 0: intact impact pose');
 const advanceTo=age=>{for(let i=0;i<300&&(state().cuts.motion[0]?.age??Infinity)<age;i++)frame();};
 advanceTo(.9);
 test.assert(state().cuts.motion.some(p=>p.direction.some(v=>Math.abs(v)>.1)&&p.distance>.45),'published cut halves visibly separate before dissolution');
 await capture('Separated halves: 0.9 s');
 // Isolate only fragment geometry: hold simulation/tracking fixed, shrink each
 // opposing group away, and compare both real eye targets before restoring it.
 const halves=state().cuts.motion.map(m=>({...m,object:engine.scene.getObject(m.id)}));
 test.assert(halves.length>=2,'death publishes at least two actual fragment meshes');
 const axis=halves[0].direction;
 const groups=[halves.filter(h=>h.direction.reduce((n,v,i)=>n+v*axis[i],0)>0),halves.filter(h=>h.direction.reduce((n,v,i)=>n+v*axis[i],0)<0)];
 test.assert(groups.every(group=>group.length>0),'actual halves move to opposite sides of cut plane');
 const visible=[await pixels('left'),await pixels('right')],halfVisibility=[];
 for(const [side,group]of groups.entries()){
  for(const half of group)half.object.setScale([.00001,.00001,.00001]);
  frame({dt:0});
  const leftPixels=assertChanged(visible[0],await pixels('left'),'isolated half '+side+' left eye',12);
  const rightPixels=assertChanged(visible[1],await pixels('right'),'isolated half '+side+' right eye',12);
  halfVisibility.push({side,meshes:group.length,leftPixels,rightPixels});
  for(const half of group)half.object.setScale([1,1,1]);
  frame({dt:0});
 }

 advanceTo(2.05);await capture('Evaporation: 2.05 s');
 assertChanged(snapshots[1].pixels,snapshots[2].pixels,'separation and evaporation rendered stages',30);
 const preview=document.querySelector('#stereo'),w=snapshots[0].pixels.width,h=snapshots[0].pixels.height;
 preview.width=w*snapshots.length;preview.height=h+28;preview.style.width='min(100%, 960px)';preview.style.height='auto';
 const ctx=preview.getContext('2d');ctx.fillStyle='#10251f';ctx.fillRect(0,0,preview.width,preview.height);ctx.font='14px system-ui';ctx.fillStyle='white';
 snapshots.forEach((v,i)=>{ctx.fillText(v.label,i*w+8,19);ctx.putImageData(v.pixels,i*w,28);});
 window.rimCutEvidence.halfVisibility=halfVisibility;
 window.rimCutEvidence.visualStages=snapshots.map(({label,age})=>({label,age}));
 advanceTo(3.1);frame();test.assert(state().cuts.motion.length===0&&state().cuts.pieces===0,'evaporated halves are removed after three seconds');
 await engine.xr.exit();test.assert(engine.xr.player.head.object===null,'diagnostics panel releases head binding on XR exit');await engine.xr.enter(xrOptions);frame();test.assert(state().kills>=1,'XR reentry preserves encounter state');await engine.xr.exit();
});window.requestAnimationFrame=browserRAF;stop?.();info?.dispose();engine?.dispose();projection?.dispose();
