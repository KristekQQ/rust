// Actual packaged Rome bootstrap, menu, worker, native gameplay and local log POSTs.
import {installXRSimulator} from './xr/simulator.js';
import {installGPUProjection} from './xr/gpu-projection.js';
import {XRTestRun,publishXRReport} from './xr/testkit.js';
const gpu=new URLSearchParams(location.search).get('backend')!=='webgl';
const sim=installXRSimulator({profile:{hz:90,eyeWidth:320,eyeHeight:320},clockTimers:false});
const projection=gpu?installGPUProjection():null;
const run=new XRTestRun('rim-device-logs',sim,report=>publishXRReport(report,{result:document.querySelector('#device-result')}));
const nativeFetch=window.fetch,nativeRAF=window.requestAnimationFrame,requests=[],posts=[];
window.fetch=(input,options)=>{
 const response=nativeFetch(input,options);
 if(input==='/__diagnostics'&&options?.body){requests.push(JSON.parse(options.body));posts.push(response);}
 return response;
};
let app;
await run.execute(async test=>{
 const response=await fetch('../examples/rim/');const source=new DOMParser().parseFromString(await response.text(),'text/html');
 for(const script of source.querySelectorAll('script'))script.remove();
 for(const node of source.head.querySelectorAll('style,meta[name="engine-build"],meta[name="app-build"]'))document.head.append(node);
 document.body.append(...source.body.childNodes);
 app=await import('../examples/rim/main.js');const {engine}=app;
 document.documentElement.dataset.rendererBackend=engine.renderer_backend();
 const build=document.querySelector('meta[name="app-build"]').content;
 const ready=requests.find(r=>r.stage==='rim-ready');test.assert(ready?.build===build,'actual Rome ready log identifies packaged application');
 const enter=document.querySelector('#enter');test.assert(!enter.disabled,'authored VR entry is enabled');
 enter.click();
 for(let i=0;i<200&&(!engine.xr.isPresenting||!sim.session?.pendingFrameCount);i++)await new Promise(r=>setTimeout(r,10));
 test.assert(engine.xr.isPresenting,'actual authored entry starts requested native XR backend');
 // Match a headset where document RAF does not run during immersion.
 window.requestAnimationFrame=()=>0;
 const frame=(controllers={})=>{sim.frame({head:{position:[0,1.7,0]},controllers});if(engine.xr.lastError)throw engine.xr.lastError;};
 const input=value=>engine.scene.rimInput(value),state=()=>engine.scene.rimState;
 frame({right:{position:[.6,1.4,-.4],buttons:[1,0,0,0,0,0]}});frame();
 const enemy=state().enemies[0];input({teleport:[enemy.position[0],0,enemy.position[2]+1.05]});
 frame({right:{position:[.6,1.4,-.4],buttons:[1,0,0,0,0,0]}});
 // The same 1.5m/s sweep failed the previous 1.8cm-per-frame gate at90Hz.
 // Real workers need browser tasks between simulated frames, just as in XR.
 for(let i=1;i<=72;i++){frame({right:{position:[.6-i/60,1.4,-.4]}});await new Promise(r=>setTimeout(r,0));}
 test.assert(state().enemies.find(e=>e.id===enemy.id)?.hp===50,'authored game accepts a continuous 1.5m/s swing at90Hz');
 for(let i=0;i<32;i++)frame({right:{position:[-.6,1.4,-.4]}});
 const target=state().enemies.find(e=>e.id===enemy.id);input({teleport:[target.position[0],0,target.position[2]+1.05]});
 frame({right:{position:[-.6,1.4,-.4]}});
 for(let i=1;i<=72;i++){frame({right:{position:[-.6+i/60,1.4,-.4]}});await new Promise(r=>setTimeout(r,0));}
 test.assert(state().enemies.find(e=>e.id===enemy.id)?.hp===0,'second continuous tracked swing reaches HP0 through actual bootstrap');
 for(let i=0;i<250&&state().cuts.pieces===0;i++){await new Promise(r=>setTimeout(r,10));frame();}
 test.assert(state().cuts.commits>0&&state().cuts.pieces>=2&&state().cuts.errors===0,'actual app worker publishes both halves without cut errors',{cuts:state().cuts,transport:window.rimCutTransportDiagnostics});
 await new Promise(r=>setTimeout(r,1050));frame();
 const samples=requests.filter(r=>r.stage==='rim-xr-sample');
 test.assert(samples.length>=2&&samples.some(r=>r.details.cuts.commits>0&&r.details.transport.status==='ready'&&r.details.blade.hits>=2),'server-bound sample includes real hit, worker and published-cut counters');
 test.assert(samples.every(r=>r.build===build&&r.wasmBuild&&r.backend===engine.renderer_backend()),'all actual XR samples carry correct backend and build identity');
 await engine.xr.exit();
 test.assert(requests.some(r=>r.stage==='rim-xr-end'),'actual session exit is logged');
 const responses=await Promise.all(posts);test.assert(responses.every(r=>r.status===204),'real local server accepted every diagnostic POST');
 window.rimDeviceLogEvidence={build,wasmBuild:ready.wasmBuild,backend:engine.renderer_backend(),runId:ready.runId,stages:requests.map(r=>r.stage),latestSample:samples.at(-1)};
});
window.requestAnimationFrame=nativeRAF;
window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:false}));app?.engine.dispose();projection?.dispose();window.fetch=nativeFetch;
