import {installGlRafTimer} from './render-performance-gl-timer.mjs';
import {insertionOrder} from './render-performance-order.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {stats,geometricLoads,parseArgs,serveFrozen,hash,summarize,withinCapacity,workDeltas,pixelDifference,errorEvidence,calibrationSnapshot,protocolTimeoutMs,ratioStats,calibratedLoad,validateCalibrationInputs,boundedCleanup,fixedLoadFamily,cpuScopeEvidence} from './render-performance-lib.mjs';
test('statistics retain tails and budget overruns; absent measurements null',()=>{assert.equal(stats([]),null);assert.deepEqual(stats([1,3,2,100],2),{count:4,median:2,mean:26.5,p95:100,p99:100,max:100,budgetMs:2,overrunToleranceMs:.5,rawExactOverruns:2,rawExactOverrunDurationMs:99,overruns:2,overrunDurationMs:99});});
test('bounded geometric loads and option validation',()=>{assert.deepEqual(geometricLoads(9),[1,2,4,8,9]);assert.equal(parseArgs(['smoke']).warmup,8000);assert.equal(parseArgs(['compare']).pairs,3);for(const args of [['smoke','--max-load','Infinity'],['smoke','--pairs','1.5'],['smoke','--backend','fallback'],['smoke','--wat','x']])assert.throws(()=>parseArgs(args));});
test('GPU unavailable is not zero timing',()=>{const s=summarize({frames:[{stamp:1,cpu:2},{stamp:18,cpu:3}],gpu:[],samples:[],commands:{}},16);assert.equal(s.gpuPassMs,null);assert.equal(s.gpuAvailable,false);assert.equal(s.heapBytes,null);assert.equal(s.rafInterval.mean,17);});
test('server manifest hashes actual bytes, rejects changed files and symlink escapes',async()=>{const dir=await fs.mkdtemp(path.join(os.tmpdir(),'render-tool-'));const outside=await fs.mkdtemp(path.join(os.tmpdir(),'render-out-'));let server;try{await fs.writeFile(path.join(dir,'a.js'),'abc');await fs.writeFile(path.join(outside,'secret.js'),'secret');await fs.symlink(path.join(outside,'secret.js'),path.join(dir,'escape.js'));server=await serveFrozen(dir,dir);assert.equal(await(await fetch(server.url+'/a.js')).text(),'abc');assert.equal(server.manifest()[0].sha256,hash('abc'));assert.equal((await fetch(server.url+'/escape.js')).status,404);await fs.writeFile(path.join(dir,'a.js'),'changed');assert.equal((await fetch(server.url+'/a.js')).status,404);}finally{await server?.close();await fs.rm(dir,{recursive:true,force:true});await fs.rm(outside,{recursive:true,force:true});}});

test('frame capacity accepts rounding jitter but rejects missed-frame cadence',()=>{const result={rafInterval:{median:16.7,p95:16.8},rafCallbackCpu:{p95:2},missedFrameFraction:0};assert.equal(withinCapacity(result,1000/60),true);assert.equal(withinCapacity({...result,missedFrameFraction:.2},1000/60),false);assert.equal(withinCapacity({...result,rafInterval:{median:33.3,p95:33.4}},1000/60),false);assert.equal(stats([16.7],1000/60).overruns,0);assert.equal(stats([16.7],1000/60).rawExactOverruns,1);});

test('work counters use exact boundaries and unsupported metrics remain null',()=>{const d=workDeltas({boundaryStart:{profile:{frame:10},debug:{gltf:{drawCalls:20,instanceBufferBytes:100}}},boundaryEnd:{profile:{frame:20},debug:{gltf:{drawCalls:50,instanceBufferBytes:200}}},frames:Array(10),elapsedMs:1000});assert.equal(d.gltf.drawCalls.delta,30);assert.equal(d.gltf.drawCalls.perProfilerFrame,3);assert.equal(d.gltf.drawCalls.perSecond,30);assert.equal(d.gltf.instancePagesCreated.delta,null);assert.equal(d.gltf.instanceBufferBytes,undefined);});
test('pixel tolerance distinguishes exact parity, rounding and changed geometry',()=>{const a={width:1,height:1,data:Uint8Array.from([50,50,50,255])};assert.equal(pixelDifference(a,a).exact,true);assert.equal(pixelDifference(a,{...a,data:Uint8Array.from([54,50,50,255])}).pass,true);assert.equal(pixelDifference(a,{...a,data:Uint8Array.from([100,50,50,255])}).pass,false);assert.equal(pixelDifference(a,{width:2,height:1,data:new Uint8Array(8)}).pass,false);});

test('insertion permutations preserve logical identities and reproducible seed',()=>{for(const n of [1,2,9,128])for(const order of ['spatial','alternating','shuffled']){const ids=insertionOrder(n,order,42);assert.equal(new Set(ids).size,n);assert.deepEqual([...ids].sort((a,b)=>a-b),Array.from({length:n},(_,i)=>i));assert.deepEqual(ids,insertionOrder(n,order,42));}assert.deepEqual(insertionOrder(5,'alternating'),[0,4,1,3,2]);assert.notDeepEqual(insertionOrder(64,'shuffled',1),insertionOrder(64,'shuffled',2));assert.equal(parseArgs(['smoke','--smoke-matrix']).smokeMatrix,true);assert.throws(()=>parseArgs(['smoke','--overlap','2']));});

test('diagnostic context metadata reads extension without issuing queries; loss observers report events',async()=>{
 const {installApiCounters,installRuntimeLossMonitor}=await import('./render-performance-api.mjs');
 const keys=['window','HTMLCanvasElement','WebGL2RenderingContext','GPUAdapter'],old=new Map(keys.map(k=>[k,Object.getOwnPropertyDescriptor(globalThis,k)]));let requestedExtension=null,deviceLost,uncaptured;
 try{
 const listeners={};globalThis.window={addEventListener:(name,callback)=>{listeners[name]=callback;}};
 const context={getExtension:name=>{requestedExtension=name;return {};},getContextAttributes:()=>({antialias:false}),isContextLost:()=>false};
 globalThis.HTMLCanvasElement=class{width=960;height=640;getContext(){return context;}};globalThis.WebGL2RenderingContext=class{};
 globalThis.GPUAdapter=class{requestDevice(){return Promise.resolve({lost:new Promise(resolve=>{deviceLost=resolve;}),addEventListener:(name,cb)=>{uncaptured=cb;}});}};
 installApiCounters();new HTMLCanvasElement().getContext('webgl2');assert.equal(requestedExtension,'EXT_disjoint_timer_query_webgl2');assert.equal(window.renderApiCounters.webglContexts[0].attributes.antialias,false);assert.equal(window.renderApiCounters.webglContexts[0].disjointTimerQueryWebgl2Supported,true);
 installRuntimeLossMonitor();await new GPUAdapter().requestDevice();listeners.webglcontextlost({statusMessage:'lost'});uncaptured({error:{message:'validation'}});deviceLost({reason:'unknown',message:'device lost'});await Promise.resolve();assert.deepEqual(window.renderRuntimeLosses.map(v=>v.kind),['webgl-context-lost','webgpu-uncaptured-error','webgpu-device-lost']);
 }finally{for(const [key,descriptor] of old)if(descriptor)Object.defineProperty(globalThis,key,descriptor);else delete globalThis[key];}
});

test('primitive WASM exception strings and Error stacks remain informative',()=>{assert.equal(errorEvidence('Cuttable requires a custom triangle mesh').message,'Cuttable requires a custom triangle mesh');const e=errorEvidence(new Error('failure'));assert.equal(e.message,'failure');assert.match(e.stack,/failure/);});

function mockGlTimer(enabled=true){
 const keys=['window','HTMLCanvasElement'],old=new Map(keys.map(k=>[k,Object.getOwnPropertyDescriptor(globalThis,k)]));
 let current=null,next=0;const control={available:false,disjoint:false,ns:2000000,resultReads:0,deleted:[],begins:0,ends:0};const ext={TIME_ELAPSED_EXT:333,GPU_DISJOINT_EXT:444};
 const gl={CURRENT_QUERY:8,QUERY_RESULT_AVAILABLE:1,QUERY_RESULT:2,getExtension:()=>ext,isContextLost:()=>false,getQuery:()=>current,createQuery:()=>({id:++next}),deleteQuery:q=>control.deleted.push(q.id),beginQuery:(_,q)=>{current=q;control.begins++;},endQuery:()=>{current=null;control.ends++;},getParameter:()=>control.disjoint,getQueryParameter:(_,key)=>{if(key===1)return control.available;control.resultReads++;return control.ns;}};
 globalThis.window={};globalThis.HTMLCanvasElement=class{id='gpu-canvas';getContext(){return gl;}};const before=HTMLCanvasElement.prototype.getContext;
 installGlRafTimer(enabled);new HTMLCanvasElement().getContext('webgl2');const timer=window.renderGlRafTimer;timer.start();
 return {timer,control,externalQuery(value){current=value;},before,restore(){for(const [key,descriptor] of old)if(descriptor)Object.defineProperty(globalThis,key,descriptor);else delete globalThis[key];}};
}
test('GL queries start before command interval, end at sampler and only read available results on later frames',()=>{const m=mockGlTimer();try{m.timer.beginFrame(10);m.timer.beginFrame(10);assert.equal(m.control.begins,1);m.timer.endFrame();assert.equal(m.control.ends,1);assert.equal(m.control.resultReads,0);m.timer.beginFrame(20);assert.equal(m.control.resultReads,0);m.control.available=true;m.timer.beginFrame(30);const s=m.timer.snapshot();assert.equal(s.samples.length,1);assert.equal(s.samples[0].elapsedMs,2);assert.equal(s.samples[0].sampleFrame,1);assert.equal(s.samples[0].availableAtFrame,3);assert.equal(m.control.resultReads,1);assert.equal(s.queriesDeleted,1);m.timer.stop();}finally{m.restore();}});
test('GL query inflight bound and stop cleanup preserve incomplete evidence',()=>{const m=mockGlTimer();try{for(let frame=1;frame<=92;frame++){m.timer.beginFrame(frame);m.timer.endFrame();}const s=m.timer.snapshot();assert.equal(s.queriesCreated,3);assert.equal(s.skippedInflight,1);assert.equal(s.pendingQueries,3);m.timer.stop();const end=m.timer.snapshot();assert.equal(end.incomplete.length,3);assert.equal(end.queriesDeleted,3);assert.equal(end.pendingQueries,0);}finally{m.restore();}});
test('GL disjoint results are discarded and other query owners remain untouched',()=>{const m=mockGlTimer();try{m.timer.beginFrame(1);m.timer.endFrame();m.control.disjoint=true;m.timer.beginFrame(2);assert.equal(m.timer.snapshot().disjointDiscarded,1);assert.equal(m.timer.snapshot().samples.length,0);m.control.disjoint=false;m.externalQuery({external:true});for(let frame=3;frame<=31;frame++)m.timer.beginFrame(frame);assert.equal(m.timer.snapshot().skippedBusy,1);assert.equal(m.control.ends,1);m.timer.stop();}finally{m.restore();}});
test('profiler-off GL helper issues no queries and does not wrap getContext',()=>{const m=mockGlTimer(false);try{assert.equal(HTMLCanvasElement.prototype.getContext,m.before);for(let i=0;i<40;i++){m.timer.beginFrame(i);m.timer.endFrame();}m.timer.stop();assert.equal(m.timer.snapshot().enabled,false);assert.equal(m.timer.snapshot().queriesCreated,0);assert.equal(m.control.begins,0);}finally{m.restore();}});

test('Three reference root is an explicit parsed option',()=>{assert.equal(parseArgs(['compare','--three-root','/private/tmp/frozen-three','--no-profile']).threeRoot,'/private/tmp/frozen-three');});

test('GL nonfinite and stale results are not published as durations',()=>{for(const mode of ['nonfinite','stale']){const m=mockGlTimer();try{m.timer.beginFrame(1);m.timer.endFrame();if(mode==='nonfinite'){m.control.available=true;m.control.ns=NaN;m.timer.beginFrame(2);}else for(let i=2;i<=242;i++){m.externalQuery({external:true});m.timer.beginFrame(i);}assert.equal(m.timer.snapshot().samples.length,0);assert.ok(m.timer.snapshot().invalid.some(v=>v.reason.includes(mode==='nonfinite'?'nonfinite':'stale')));m.timer.stop();}finally{m.restore();}}});

test('unsafe overload startup retains valid near/over probes without inventing a sample',()=>{
 const o=parseArgs(['calibrate']),probe=(load,pass)=>({label:'baseline',load,rafInterval:{median:pass?16.7:33.3,p95:pass?16.8:34},rafCallbackCpu:{p95:pass?5:20},missedFrameFraction:pass?0:.5});
 const failure={load:8192,stage:'navigate and initialize workload',error:{message:'startup timeout'}};
 const c=calibrationSnapshot(o,[probe(1,true),probe(3584,true),probe(3712,false),probe(4096,false)],{firstFailedLoad:4096,incompleteFailure:failure});
 assert.equal(c.incomplete,true);assert.equal(c.levels.near,3584);assert.equal(c.levels.over,4096);assert.equal(c.unsafeLoad.load,8192);assert.equal(c.loads.includes(8192),false);assert.equal(c.capacityProbes.length,4);
 const empty=calibrationSnapshot(o,[],{incompleteFailure:failure});assert.equal(empty.load,null);assert.deepEqual(empty.loads,[]);assert.equal(empty.underBudget,false);assert.equal(parseArgs(['smoke']).startupTimeout,30000);assert.throws(()=>parseArgs(['smoke','--startup-timeout','0']));
});

test('protocol deadline includes long samples, startup allowance and bounded margin',()=>{assert.equal(protocolTimeoutMs({sample:25000,startupTimeout:30000}),120000);assert.equal(protocolTimeoutMs({sample:120000,startupTimeout:30000}),180000);assert.equal(protocolTimeoutMs({sample:300000,startupTimeout:120000}),450000);assert.throws(()=>protocolTimeoutMs({sample:Infinity,startupTimeout:30000}));assert.throws(()=>protocolTimeoutMs({sample:300001,startupTimeout:30000}));});
test('nested batch rejection deltas retain missing fields and reset counters as null',()=>{const d=workDeltas({boundaryStart:{profile:{frame:10},debug:{gltf:{batchBreaks:{rootAsset:3,drawCap:9}}}},boundaryEnd:{profile:{frame:20},debug:{gltf:{batchBreaks:{rootAsset:13,drawCap:1}}}},frames:Array(10),elapsedMs:2000});assert.equal(d.gltf['batchBreaks.rootAsset'].delta,10);assert.equal(d.gltf['batchBreaks.rootAsset'].perProfilerFrame,1);assert.equal(d.gltf['batchBreaks.rootAsset'].perSecond,5);assert.equal(d.gltf['batchBreaks.drawCap'].delta,null);assert.equal(d.gltf['batchBreaks.rootBlend'].delta,null);});
test('dynamic culling evidence comes from native boundary counters, not requested drift',()=>{const base={frames:[{stamp:1,cpu:1},{stamp:18,cpu:1}],gpu:[],samples:[],commands:{frustumDriftUpdates:100},elapsedMs:17,phaseBoundaries:{start:{elapsedSeconds:8},end:{elapsedSeconds:8.017}},boundaryStart:{profile:{frame:10},debug:{gltf:{primitiveCulled:3}}},boundaryEnd:{profile:{frame:11},debug:{gltf:{primitiveCulled:3}}}};assert.equal(summarize(base,16.667).phaseEvidence.cullingObserved,false);base.boundaryEnd.debug.gltf.primitiveCulled=5;const s=summarize(base,16.667);assert.equal(s.phaseEvidence.nativeGltfPrimitiveCulledDelta,2);assert.equal(s.phaseEvidence.cullingObserved,true);assert.equal(s.phaseEvidence.start.elapsedSeconds,8);});


test('API diagnostics count real pass/submit/state calls and only successful asynchronous pipeline creations',async()=>{
 const {installApiCounters}=await import('./render-performance-api.mjs');
 const keys=['window','GPUDevice','GPUCommandEncoder','GPURenderPassEncoder','GPUQueue'],saved=new Map(keys.map(k=>[k,Object.getOwnPropertyDescriptor(globalThis,k)]));
 try{
 globalThis.window={};
 globalThis.GPURenderPassEncoder=class{setPipeline(){} setBindGroup(){} drawIndexed(){}};
 globalThis.GPUCommandEncoder=class{beginRenderPass(){return new GPURenderPassEncoder();}finish(){return {};}};
 globalThis.GPUDevice=class{createCommandEncoder(){return new GPUCommandEncoder();}createRenderPipelineAsync(reject=false){return reject?Promise.reject(Error('invalid pipeline')):Promise.resolve({});}};
 globalThis.GPUQueue=class{submit(){}};installApiCounters();
 const device=new GPUDevice(),encoder=device.createCommandEncoder(),pass=encoder.beginRenderPass();pass.setPipeline();pass.setBindGroup();pass.drawIndexed();new GPUQueue().submit([encoder.finish()]);
 const c=window.renderApiCounters.webgpu;for(const key of ['createCommandEncoder','beginRenderPass','setPipeline','setBindGroup','drawIndexed','finish','submit'])assert.equal(c[key],1,key);
 assert.equal(c.beginComputePass,null,'missing API stays unavailable');
 const pending=device.createRenderPipelineAsync();assert.equal(c.createRenderPipelineAsync,0);await pending;assert.equal(c.createRenderPipelineAsync,1);await assert.rejects(device.createRenderPipelineAsync(true));assert.equal(c.createRenderPipelineAsync,1);
 }finally{for(const [k,v]of saved)if(v)Object.defineProperty(globalThis,k,v);else delete globalThis[k];}
});

test('Three representation is explicit and bounded',()=>{assert.equal(parseArgs(['compare','--three-root','/private/tmp/three','--three-representation','objects']).threeRepresentation,'objects');assert.throws(()=>parseArgs(['compare','--three-representation','objects']));assert.throws(()=>parseArgs(['compare','--three-root','/private/tmp/three','--three-representation','bad']));});

test('dimensionless ratios do not carry millisecond budgets',()=>{assert.deepEqual(ratioStats([1,2,3]),{count:3,median:2,mean:2,p95:3,p99:3,max:3});assert.equal(ratioStats([]),null);});
test('calibrated soak rejects absent loads and stale common harness while permitting candidate runtime',async()=>{
 const dir=await fs.mkdtemp(path.join(os.tmpdir(),'render-cal-'));
 try{await fs.writeFile(path.join(dir,'harness.js'),'original');const config={loads:[1,4],load:4,levels:{near:4},baselineManifest:[{url:'/harness.js',source:'harness',sha256:hash('original')},{url:'/runtime.wasm',source:'frozen',sha256:'old'}]};
 assert.equal(calibratedLoad(config),4);assert.throws(()=>calibratedLoad({...config,loads:[],load:null,levels:{}}));assert.throws(()=>calibratedLoad(config,{explicitLoad:true,load:3}));assert.throws(()=>calibratedLoad(config,{level:'over'}));
 await validateCalibrationInputs(config,{runtimeRoot:dir,harnessRoot:dir,harnessOnly:true});await assert.rejects(validateCalibrationInputs(config,{runtimeRoot:dir,harnessRoot:dir}));await fs.writeFile(path.join(dir,'harness.js'),'changed');await assert.rejects(validateCalibrationInputs(config,{runtimeRoot:dir,harnessRoot:dir,harnessOnly:true}),/differs from calibration/);
 }finally{await fs.rm(dir,{recursive:true,force:true});}
});

test('completed measurements cannot hang forever during owned cleanup',async()=>{let forced=0;assert.deepEqual(await boundedCleanup(()=>Promise.resolve(),{label:'closed',timeoutMs:20,force:()=>forced++}),{label:'closed',complete:true,forced:false});assert.equal(forced,0);const timeout=await boundedCleanup(()=>new Promise(()=>{}),{label:'owned',timeoutMs:10,force:()=>forced++});assert.equal(timeout.complete,false);assert.equal(timeout.forced,true);assert.match(timeout.error.message,/cleanup timeout/);assert.equal(forced,1);const failed=await boundedCleanup(()=>{throw Error('close failed');},{label:'failed',timeoutMs:20,force:()=>{throw Error('force failed');}});assert.equal(failed.error.message,'close failed');assert.equal(failed.forceError.message,'force failed');});

test('a query spanning a missing sampler is discarded rather than published as frame timing',()=>{const m=mockGlTimer();try{m.timer.beginFrame(1);m.control.available=true;m.timer.beginFrame(2);m.timer.beginFrame(3);const s=m.timer.snapshot();assert.equal(s.samples.length,0);assert.equal(s.queriesEnded,1);assert.equal(s.queriesDeleted,1);assert.ok(s.invalid.some(v=>v.reason==='previous frame missing sampler'));m.timer.stop();}finally{m.restore();}});

test('render dimensions are explicit bounded calibration inputs',()=>{const d=parseArgs(['smoke']);assert.equal(d.width,960);assert.equal(d.height,640);const o=parseArgs(['calibrate','--width','2880','--height','1920']);assert.equal(o.width,2880);assert.equal(o.height,1920);for(const v of ['0','63','4097','NaN','120.5'])assert.throws(()=>parseArgs(['smoke','--width',v]));const c=calibrationSnapshot({...o,budget:16.667},[]);assert.equal(c.width,2880);assert.equal(c.height,1920);});

test('Rim stress and clustered model workloads stay inside their native model budget',()=>{assert.equal(parseArgs(['smoke','--family','rim-stress','--max-load','2048']).maxLoad,2048);assert.equal(parseArgs(['smoke','--family','rim-clustered','--max-load','2048']).maxLoad,2048);assert.throws(()=>parseArgs(['smoke','--family','rim-stress','--max-load','2049']),/budget/);assert.throws(()=>parseArgs(['smoke','--family','rim-clustered','--max-load','2049']),/budget/);});
test('authored Rome camera is fixed-load while clustered shared drones calibrate by model load',()=>{assert.equal(fixedLoadFamily('rim-camera'),true);assert.equal(fixedLoadFamily('rim-static'),true);assert.equal(fixedLoadFamily('rim-clustered'),false);assert.equal(fixedLoadFamily('rim-stress'),false);assert.equal(parseArgs(['calibrate','--family','rim-clustered','--max-load','2048']).maxLoad,2048);});

test('fixed calibration probe stays within explicit budget',()=>{assert.throws(()=>parseArgs(['calibrate','--load','128','--max-load','64']),/Bounds exceeded/);assert.equal(parseArgs(['calibrate','--load','128','--max-load','128']).load,128);});

// A cold scope must not be promoted to a recurring warm cost just because its
// historical lastMs survives in every profiler snapshot.
test('CPU scope evidence excludes unchanged cold histories and deduplicates snapshots',()=>{
 const cold={lastMs:90,meanMs:90,samples:1},zero={lastMs:0,meanMs:0,samples:60};
 const a={lastMs:2,meanMs:1,samples:60},b={lastMs:3,meanMs:1.1,samples:60};
 const d={boundaryStart:{profile:{cpu:{cold,zero}}},samples:[{profile:{cpu:{cold,zero,queue:a}}},{profile:{cpu:{cold,zero,queue:a}}},{profile:{cpu:{cold,zero,queue:b}}}]};
 const e=cpuScopeEvidence(d);assert.equal(e.perInvocationTimesAvailable,false);
 assert.equal(e.scopes.cold.freshness,'unproven');assert.equal(e.scopes.cold.lastSampleMs,null);
 assert.equal(e.scopes.zero.lastSampleMs,null);assert.equal(e.scopes.queue.observedHistoryChanges,2);
 assert.equal(e.scopes.queue.lastSampleMs.count,2);assert.equal(e.scopes.queue.lastSampleMs.max,3);
});

// New batch3 families reuse the maintained frozen-runtime/common-harness path.
test('camera-stress and one-moving-root Rome are scalable frozen workload families',()=>{
 for(const family of ['rim-camera-stress','rim-mixed']){
  assert.equal(fixedLoadFamily(family),false);
  assert.equal(parseArgs(['smoke','--family',family,'--load','1024']).family,family);
 }
});
test('fine counters use exact boundaries, while cache-byte gauges stay out of event deltas',()=>{
 const d=workDeltas({boundaryStart:{profile:{frame:1},debug:{gltf:{finePayloadRecordsWritten:10,finePreparedRowsReused:20,fineCacheBytes:300}}},boundaryEnd:{profile:{frame:3},debug:{gltf:{finePayloadRecordsWritten:12,finePreparedRowsReused:30,fineCacheBytes:400}}},frames:Array(2),elapsedMs:100});
 assert.equal(d.gltf.finePayloadRecordsWritten.delta,2);
 assert.equal(d.gltf.finePreparedRowsReused.perApplicationRaf,5);
 assert.equal(d.gltf.fineCacheBytes,undefined);
 assert.equal(d.gauges.end.fineCacheBytes,400);
 assert.equal(d.gltf.fineDynamicFallbacks.delta,null);
});


test('allocation diagnostic duration is bounded independently from primary',()=>{
 assert.equal(parseArgs(['smoke','--diagnostic-sample','25000']).diagnosticSample,25000);
 for(const n of ['0','-1','300001','Infinity'])assert.throws(()=>parseArgs(['smoke','--diagnostic-sample',n]));
});
test('actual GL bufferData allocation events use the selected source range',async()=>{
 const {installApiCounters}=await import('./render-performance-api.mjs');
 const names=['window','HTMLCanvasElement','WebGL2RenderingContext'],old=new Map(names.map(k=>[k,Object.getOwnPropertyDescriptor(globalThis,k)]));
 try{
  globalThis.window={};delete globalThis.HTMLCanvasElement;
  class GL{bufferData(){} }
  globalThis.WebGL2RenderingContext=GL;installApiCounters();window.renderApiCounters.allocationEventsEnabled=true;const gl=new GL();
  gl.bufferData(1,new Uint16Array(100),2,10,20);
  gl.bufferData(1,new Float32Array(20),2,3,0);
  gl.bufferData(1,new Uint8Array(12),2);
  gl.bufferData(1,64,2);
  assert.deepEqual(window.renderApiCounters.allocationEvents.filter(e=>e.api==='bufferData').map(e=>e.bytes),[40,68,12,64]);
  window.renderApiCounters.allocationEventsEnabled=false;gl.bufferData(1,16,2);assert.equal(window.renderApiCounters.allocationEvents.length,4);assert.deepEqual(window.renderApiCounters.allocationTotals.bufferData,{calls:5,knownBytes:200,unknownSizeCalls:0});
 }finally{for(const [k,d] of old)if(d)Object.defineProperty(globalThis,k,d);else delete globalThis[k];}
});
