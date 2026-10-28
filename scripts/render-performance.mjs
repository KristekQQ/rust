#!/usr/bin/env node
// Timing never forces GPU completion/readback. Captures happen after sampling.
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {gzipSync} from 'node:zlib';
import {installGlRafTimer} from './render-performance-gl-timer.mjs';
import {installApiCounters,installRuntimeLossMonitor} from './render-performance-api.mjs';
import {browserArgs} from './browser-options.mjs';
import {families,parseArgs,serveFrozen,summarize,stats,geometricLoads,hash,withinCapacity,pixelDifference,errorEvidence,calibrationSnapshot,protocolTimeoutMs,ratioStats,calibratedLoad,validateCalibrationInputs,boundedCleanup,fixedLoadFamily} from './render-performance-lib.mjs';
const root=fileURLToPath(new URL('../',import.meta.url)),o=parseArgs(process.argv.slice(2));
if(o.command==='help'||o.command==='--help'){
 console.log(`Renderer performance CLI: smoke | calibrate | compare | capacity | soak | regression
node scripts/render-performance.mjs COMMAND --baseline-root FROZEN_ROOT [--candidate-root FROZEN_ROOT] [--config FILE] [--out DIR]
Frozen roots must contain pkg/, js/, assets/, tests/fixtures and example source imports; no source overlay.
--order spatial|shuffled|alternating --seed N (1337) --overlap 0..1 (0; synthetic root position contraction)
--three-root FROZEN_THREE_ROOT (compare only, batching or rim-static/rim-stress + webgl/--no-profile required; reference limitations recorded)
--three-representation instanced|objects (default instanced; independent roots in objects mode)
--width N (960) --height N (640): explicit render viewport, DPR1, each64..4096
--backend webgpu|webgl --family ${families.join('|')}
--warmup MS (8000) --sample MS (25000) --pairs N (3; AB/BA/AB) --budget MS (16.667)
--level light|near|over (near) --load N (smoke or an exactly calibrated load) --max-load N (1024) --max-memory MB (JS heap only, 512) --max-time SECONDS (1800)
--smoke-matrix (smoke all workload families at one load, bounded by --max-time)
--pixel-channel N (8) --pixel-fraction N (.001), regression pixel tolerance
--diagnostics (separate instrumented page; never included in timing)
--startup-timeout MS (30000; navigation + engine ready)
--slow-frame MS (250) --max-slow-frames N (20) --no-profile --threshold PERCENT (5)
calibrate: baseline-only geometric sweep + bounded binary refinement; --load N selects one explicit fixed probe without refinement/doubling. Saves light/near/over frame-capacity levels.
compare: reuses calibrated load/settings; paired median ratios. capacity: paired sweep of baseline calibrated loads.
smoke: single load baseline; soak: calibrated baseline load for requested --sample duration.
regression: compare plus nonzero exit for RAF/CPU median degradation above threshold.
Raw distinct profiler samples + RAF records: raw.json.gz. Actual served bytes: manifest-*.json.
Captures use actual readback outside timing. GPU absence is null, never inferred from RAF.
Hardware timestamp support, desktop vs physical Quest and unmatched Three references are separate evidence.`);process.exit(0);
}
if(o.threeRoot){if(o.command!=='compare'||o.candidateRoot)throw Error('--three-root requires compare and no --candidate-root');o.candidateRoot=o.threeRoot;}
if(!['smoke','calibrate','compare','capacity','soak','regression'].includes(o.command)||!o.baselineRoot)throw Error('Use --help; --baseline-root is required');
if(['compare','capacity','regression'].includes(o.command)&&!o.candidateRoot)throw Error('--candidate-root required');
if(['compare','capacity','soak','regression'].includes(o.command)&&!o.config)throw Error('--config from baseline calibration required');
await fs.mkdir(path.resolve(o.out),{recursive:true});
const driverPaths=['scripts/benchmark-core.mjs','scripts/render-performance.mjs','scripts/render-performance-lib.mjs','scripts/render-performance-api.mjs','scripts/render-performance-gl-timer.mjs','scripts/render-performance-order.mjs','tests/render-performance.html','tests/render-performance.js','tests/pixels.js',...(o.threeRoot?['tests/render-performance-three.html','tests/render-performance-three.js','tests/render-performance-three-rim.html','tests/render-performance-three-rim.js','examples/rim-three/stress.js']:[])];
const driverFiles=Object.fromEntries(await Promise.all(driverPaths.map(async file=>[file,hash(await fs.readFile(path.join(root,file)))])));
const driverIdentity={capturedAt:new Date().toISOString(),nodeVersion:process.version,v8Version:process.versions.v8,files:driverFiles,sha256:hash(JSON.stringify(driverFiles))};
const config=o.config?JSON.parse(await fs.readFile(o.config,'utf8')):null;
if(config){for(const key of ['backend','family','order','seed','overlap','profiling','budget','width','height']){const flag=key==='profiling'?'--no-profile':'--'+key;if(config[key]!==undefined&&!process.argv.includes(flag))o[key]=config[key];}
 if(config.version!==1||!Array.isArray(config.loads)||config.loads.some(n=>!Number.isInteger(n)||n<1||n>o.maxLoad))throw Error('Invalid/out-of-bound calibrated config');for(const key of ['backend','family','order','seed','overlap','budget','width','height'])if((config[key]??(key==='width'?960:key==='height'?640:undefined))!==o[key])throw Error('Calibration setting mismatch: '+key);}
if(o.threeRoot&&(o.backend!=='webgl'||!['batching','rim-static','rim-stress'].includes(o.family)||o.profiling))throw Error('--three-root requires batching or rim-static/rim-stress + webgl + --no-profile (symmetric profiler-off control)');
const baseline=await serveFrozen(o.baselineRoot,root),candidate=o.candidateRoot?await serveFrozen(o.candidateRoot,root):null;
const {default:puppeteer}=await import(path.join(root,'node_modules/puppeteer/lib/puppeteer/puppeteer.js'));
let executablePath=process.env.BROWSER_EXECUTABLE;try{executablePath||=execFileSync('git',['config','--local','--get','verification.browser'],{cwd:root,encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();}catch{}
let browser;
const started=Date.now(),raw=[],report={version:1,driverIdentity,comparisonKind:o.threeRoot?(o.family.startsWith('rim-')?'authoredRomeDiagnostic':'matchedGeometryOnly'):'sameRendererRevisionPair',threeReference:o.threeRoot?{root:path.resolve(o.threeRoot),representation:o.threeRepresentation,scope:o.family.startsWith('rim-')?'original authored Rome at same gate camera/resolution; MSAA vs FXAA, shading, shadow and gameplay differences remain; no proven pixel/full-game parity':'matching visible quad geometry/load/grid/elapsed poses only; material, PBR and lighting implementations differ; no authored Rome or arena equivalence'}:null,command:o.command,date:new Date().toISOString(),settings:o,viewport:{width:o.width,height:o.height,deviceScaleFactor:1},baselineRoot:baseline.root,candidateRoot:candidate?.root||null,calibration:config,measurementProtocol:{warmup:o.warmup,sample:o.sample,profiling:o.profiling,protocolTimeoutMs:protocolTimeoutMs(o)},calibrationProtocol:config?{warmup:config.warmup,sample:config.sample,profiling:config.profiling}:null,profilerOffControl:!!config&&config.profiling===true&&o.profiling===false,results:[],limits:{maxTimeSeconds:o.maxTime,maxMemoryMB:o.maxMemory,memoryScope:'performance.memory JS heap only; GPU/process memory unavailable',maxLoad:o.maxLoad,slowFrameMs:o.slowFrame,maxSlowFrames:o.maxSlowFrames},evidence:'desktop headless; no physical headset acceptance; no Three equivalence'};
async function run(server,label,load,pair=0){
 if(Date.now()-started>o.maxTime*1000)throw Error('Wall-clock limit reached');
 const isThree=!!o.threeRoot&&label==='candidate',pagePath=isThree?(o.family.startsWith('rim-')?'/tests/render-performance-three-rim.html':'/tests/render-performance-three.html'):'/tests/render-performance.html';
 const coldStart=Date.now(),page=await browser.newPage(),errors=[];let stage='page initialization';await page.setViewport(report.viewport);page.setDefaultTimeout(o.startupTimeout);page.setDefaultNavigationTimeout(o.startupTimeout);
 page.on('pageerror',e=>errors.push({type:'pageerror',...errorEvidence(e)}));page.on('console',m=>{if(m.type()==='error')errors.push({type:'consoleerror',message:m.text()||'(empty console error)',location:m.location(),argumentDescriptions:m.args().map(a=>a.toString())});});
 page.on('response',response=>{if(response.status()>=400)errors.push('HTTP '+response.status()+' '+response.url());});
 try{
 await page.evaluateOnNewDocument(installRuntimeLossMonitor);await page.evaluateOnNewDocument(installGlRafTimer,o.profiling&&o.backend==='webgl');
 await page.evaluateOnNewDocument(()=>{
 const original=requestAnimationFrame.bind(window);window.perfMeasure={active:false,frames:[]};
 window.requestAnimationFrame=fn=>original(t=>{const m=window.perfMeasure;if(!m.active)return fn(t);let row=m.frames.at(-1);if(!row||row.stamp!==t){row={stamp:t,cpu:0};m.frames.push(row);}const start=performance.now();try{window.renderGlRafTimer?.beginFrame(t);return fn(t);}finally{row.cpu+=performance.now()-start;}});
 });
 const startupDeadline=coldStart+Math.min(o.startupTimeout,Math.max(1,o.maxTime*1000-(Date.now()-started)));stage='navigate and initialize workload';await page.goto(server.url+pagePath+'?'+new URLSearchParams({family:o.family,width:String(o.width),height:String(o.height),load:String(load),backend:o.backend,profile:String(Number(o.profiling)),order:o.order,seed:String(o.seed),overlap:String(o.overlap),representation:o.threeRepresentation}),{waitUntil:'domcontentloaded',timeout:Math.max(1,startupDeadline-Date.now())});
 await page.waitForFunction('!!window.bench',{timeout:Math.max(1,startupDeadline-Date.now())});const coldReadyMs=Date.now()-coldStart;
 const actual=await page.evaluate(()=>({backend:bench.engine.renderer_backend(),profiling:bench.engine.frameProfile().enabled,metadata:bench.metadata,startupProgress:window.benchProgress||null}));
 if(actual.backend!==(isThree?'ThreeWebGL2':o.backend==='webgl'?'Gl':'BrowserWebGpu')||actual.profiling!==o.profiling)throw Error('Requested backend/profiling mismatch '+JSON.stringify(actual));
 stage='warmup';await page.evaluate(()=>bench.start());await new Promise(r=>setTimeout(r,o.warmup));
 const remaining=o.maxTime*1000-(Date.now()-started);if(remaining<o.sample)throw Error('Insufficient wall-clock budget for sample');
 stage='measured sample';const data=await page.evaluate(async settings=>{
 const m=window.perfMeasure;m.frames=[];
 const boundaryStart={profile:bench.engine.frameProfile(),renderer:bench.engine.renderer_stats(),debug:bench.engine.debugStats()};const initial=boundaryStart.profile,initialCommands={...bench.operations},phaseStart=bench.phaseState?.()??null;window.renderGlRafTimer?.start();m.active=true;const samples=[],gpu=[];let lastCpu=initial.frame,lastGpu=initial.gpu.sampleFrame,slow=0,previous=performance.now(),lastPoll=-Infinity,abort=null;
 const start=performance.now();
 await new Promise(resolve=>{const sample=()=>{
 window.renderGlRafTimer?.endFrame();const now=performance.now();if(now-previous>settings.slowFrame)slow++;previous=now;
 const e=bench.engine,p=now-lastPoll>=100?e.frameProfile():null;
 if(p&&now-lastPoll>=100&&(p.frame!==lastCpu||!p.enabled)){lastPoll=now;samples.push({atMs:now-start,profile:p,renderer:e.renderer_stats(),debug:e.debugStats()});lastCpu=p.frame;}
 if(window.benchFailure)abort='Workload failure: '+JSON.stringify(window.benchFailure);
 if(window.renderRuntimeLosses?.length)abort='GPU/WebGL runtime loss: '+JSON.stringify(window.renderRuntimeLosses);
 if(p?.gpu.error)abort='GPU profiler error: '+p.gpu.error;
 if(p?.gpu.available){const g=p.gpu,maxAge=Math.max(128,(g.sampleEveryFrames||16)*4);if(!Number.isInteger(g.sampleFrame)||!Number.isInteger(g.ageFrames)||g.ageFrames<0||g.ageFrames>maxAge||!g.passesMs||Object.values(g.passesMs).some(v=>!Number.isFinite(v)||v<0))abort='Invalid or stale GPU sample';else if(g.sampleFrame!==lastGpu&&g.sampleFrame>initial.frame){gpu.push(g);lastGpu=g.sampleFrame;}}
 const heap=performance.memory?.usedJSHeapSize;
 if(m.frames.length>20000)abort='raw frame bound';if(heap>settings.maxMemory*1024*1024)abort='JS heap limit';if(slow>settings.maxSlowFrames)abort='slow frame limit';
 if(abort||now-start>=settings.sample){resolve();return;}requestAnimationFrame(sample);
 };requestAnimationFrame(sample);});
 m.active=false;const elapsedMs=performance.now()-start;window.renderGlRafTimer?.stop();const glRafGpu=window.renderGlRafTimer?.snapshot()||null;const phaseEnd=bench.phaseState?.()??null;const boundaryEnd={profile:bench.engine.frameProfile(),renderer:bench.engine.renderer_stats(),debug:bench.engine.debugStats()};bench.stop();
 return {phaseBoundaries:phaseStart||phaseEnd?{start:phaseStart,end:phaseEnd}:null,glRafGpu,actualQualityState:{canvas:{width:document.querySelector('canvas').width,height:document.querySelector('canvas').height},postprocessing:bench.engine.postprocessing?.stats?.()??null,graph:bench.engine.postprocessing?.graph?.()??null},boundaryStart,boundaryEnd,frames:m.frames,samples,gpu,backend:bench.engine.renderer_backend(),profiling:bench.engine.frameProfile().enabled,commands:Object.fromEntries(Object.entries(bench.operations).map(([key,value])=>[key,value-(initialCommands[key]||0)])),heapBytes:performance.memory?.usedJSHeapSize??null,abort,elapsedMs};
 },o);
 raw.push({id:`${label}-${o.family}-${load}-pair${pair}`,...data});stage='validate measured sample';
 if(data.abort)throw Error(data.abort);if(data.actualQualityState.canvas.width!==o.width||data.actualQualityState.canvas.height!==o.height)throw Error('Actual render dimensions differ from requested viewport');if(!o.profiling&&data.glRafGpu&&(data.glRafGpu.enabled||data.glRafGpu.queriesBegun))throw Error('Profiler-off control unexpectedly enabled GL timer');if(o.profiling&&o.backend==='webgl'&&data.glRafGpu?.supported&&!data.glRafGpu.samples.length)throw Error('Supported GL asynchronous timer yielded no valid samples');if(o.profiling&&data.samples.at(-1)?.profile.gpu?.supported&&!data.gpu.length)throw Error('Supported GPU profiler yielded no fresh sample');
 if(errors.length)throw Error(JSON.stringify(errors.slice(0,5),null,2));
 const debug=data.samples.at(-1)?.debug,renderer=data.samples.at(-1)?.renderer;
 if(!data.frames.length||!((debug?.mainDrawCalls||0)+(debug?.drawCalls||0)+(renderer?.drawCalls||0)>0))throw Error('No measured visible render commands');
 if(!isThree&&['batching','batching-heterogeneous'].includes(o.family)){const before=data.boundaryStart.debug.gltf,after=data.boundaryEnd.debug.gltf,drawn=after.instancesDrawn-before.instancesDrawn,culled=after.primitiveCulled-before.primitiveCulled,frames=data.boundaryEnd.profile.frame-data.boundaryStart.profile.frame;if(Number.isFinite(drawn)&&drawn<=0)throw Error('Batching submitted no model instances');if(Number.isFinite(culled)&&culled!==0)throw Error('Batching workload unexpectedly culled primitives');if(o.profiling&&frames>0&&Number.isFinite(drawn)&&Math.abs(drawn-frames*load)>load)throw Error('Batching submitted instance count does not match visible load/frame boundaries');}
 if(isThree&&o.family==='rim-stress'&&(actual.metadata?.stress?.count!==load||actual.metadata?.stress?.representation!==o.threeRepresentation))throw Error('Authored Three stress count/representation mismatch');
 if(isThree&&o.family==='batching'&&renderer?.drawCalls!==(o.threeRepresentation==='objects'?load:Math.ceil(load/1024)))throw Error('Three reference draw count does not match selected representation');
 if(isThree&&o.family==='batching'&&renderer?.triangles!==load*2)throw Error('Three reference triangle count does not match visible quad load');
 if(o.family==='dynamic'){if(data.commands.removes!==data.commands.cuts*3)throw Error('Dynamic explicit remove commands must equal cutter plus two pieces per cut');if(data.commands.cuts===0)throw Error('Dynamic cut workload did not cut');if(data.commands.cutAttempts!==data.commands.cuts)throw Error('Dynamic native cut attempts/results do not match');if(!(data.commands.frustumDriftUpdates>0))throw Error('Dynamic workload issued no frustum drift commands');const phases=data.phaseBoundaries;if(!Number.isFinite(phases?.start?.elapsedSeconds)||!Number.isFinite(phases?.end?.elapsedSeconds)||phases.end.elapsedSeconds<=phases.start.elapsedSeconds)throw Error('Dynamic measured pose clock did not advance');}
 if(o.family==='rim-camera'){const phases=data.phaseBoundaries;if(!(data.commands.cameraUpdates>0)||!Number.isFinite(phases?.start?.elapsedSeconds)||!Number.isFinite(phases?.end?.elapsedSeconds)||phases.end.elapsedSeconds<=phases.start.elapsedSeconds||phases.start.native?.active!==false||phases.end.native?.active!==false||phases.start.native?.paused!==true||phases.end.native?.paused!==true)throw Error('Rim camera path did not advance with native gameplay paused');}
 if(['rim-camera-stress','rim-mixed'].includes(o.family)){
  const phases=data.phaseBoundaries;
  if(phases?.start?.native?.stressModels!==load||phases?.end?.native?.stressModels!==load||phases?.start?.native?.paused!==true||phases?.end?.native?.paused!==true)throw Error('Paused stress baseline state mismatch');
  if(o.family==='rim-camera-stress'&&data.commands.cameraUpdates<10)throw Error('Camera-stress did not move the camera');
  if(o.family==='rim-mixed'&&(data.commands.mixedModelUpdates<10||data.commands.transforms!==data.commands.mixedModelUpdates))throw Error('Mixed workload must update exactly one root per tick');
 }
 if(o.family==='rim-clustered'){const phases=data.phaseBoundaries;if(data.commands.clusteredModelUpdates!==data.commands.animationSeeks||data.commands.clusteredModelUpdates<load||data.commands.frustumDriftUpdates!==data.commands.clusteredModelUpdates||phases?.start?.actualHandles!==load||phases?.end?.actualHandles!==load||phases?.start?.native?.stressModels!==load||phases?.end?.native?.stressModels!==load||phases?.end?.elapsedSeconds<=phases?.start?.elapsedSeconds||phases?.end?.cluster===phases?.start?.cluster||phases?.start?.native?.active!==false||phases?.end?.native?.active!==false||phases?.start?.native?.paused!==true||phases?.end?.native?.paused!==true)throw Error('Rim clustered handles, elapsed motion, or paused native state mismatch');}
 stage='deterministic capture';await page.evaluate(()=>bench.capturePose());
 const pixels=await page.evaluate(async()=>{const {capturePixels,assertForeground}=await import('/tests/pixels.js');const im=await capturePixels(document.querySelector('canvas'));assertForeground(im,'benchmark workload',20);const c=document.createElement('canvas');c.width=im.width;c.height=im.height;c.getContext('2d').putImageData(im,0,0);return c.toDataURL().split(',')[1];});
 const id=`${label}-${o.family}-${load}-pair${pair}`,png=Buffer.from(pixels,'base64');await fs.writeFile(path.join(o.out,id+'.png'),png);
 await page.close();
 stage='separate API diagnostics';let apiOperations=null;if(o.diagnostics){const diagnostic=await browser.newPage();try{diagnostic.setDefaultTimeout(o.startupTimeout);diagnostic.setDefaultNavigationTimeout(o.startupTimeout);const diagnosticDeadline=Date.now()+o.startupTimeout;await diagnostic.setViewport(report.viewport);await diagnostic.evaluateOnNewDocument(installRuntimeLossMonitor);await diagnostic.evaluateOnNewDocument(installApiCounters);await diagnostic.goto(server.url+pagePath+'?'+new URLSearchParams({family:o.family,width:String(o.width),height:String(o.height),load:String(load),backend:o.backend,profile:String(Number(o.profiling)),order:o.order,seed:String(o.seed),overlap:String(o.overlap),representation:o.threeRepresentation}),{waitUntil:'domcontentloaded',timeout:Math.max(1,diagnosticDeadline-Date.now())});await diagnostic.waitForFunction('!!window.bench',{timeout:Math.max(1,diagnosticDeadline-Date.now())});apiOperations=await diagnostic.evaluate(async()=>{bench.start();await new Promise(resolve=>setTimeout(resolve,1000));const cold=structuredClone(window.renderApiCounters);await new Promise(resolve=>setTimeout(resolve,3000));const snapshot=()=>({atMs:performance.now(),engineFrame:bench.engine.frameProfile().frame,counters:structuredClone(window.renderApiCounters)});const start=snapshot();await new Promise(resolve=>setTimeout(resolve,3000));const end=snapshot();return {counters:cold,backend:bench.engine.renderer_backend(),runtimeLosses:window.renderRuntimeLosses,renderer:bench.engine.renderer_stats(),debug:bench.engine.debugStats(),duration:'cold counters: initialization + 1s; final debug snapshot follows separate warm window',warm:{start,end,elapsedMs:end.atMs-start.atMs,engineFrames:end.engineFrame-start.engineFrame,warmupMs:4000,requestedSampleMs:3000,scope:'all wrapped API calls in a separate instrumented diagnostic page; excluded from primary timing'},separateInstrumentedPage:true};});if(apiOperations.backend!==(o.backend==='webgl'?'Gl':'BrowserWebGpu'))throw Error('Diagnostic backend mismatch: '+apiOperations.backend);if(apiOperations.runtimeLosses?.length)throw Error('Diagnostic GPU/WebGL runtime loss: '+JSON.stringify(apiOperations.runtimeLosses));}finally{await diagnostic.close();}}
 const result={id,label,load,pair,apiOperations,actualQualityState:data.actualQualityState,coldReadyMs,coldTimingScope:'page creation + navigation DOMContentLoaded + explicit bench readiness; not isolated shader compilation',metadata:actual.metadata,startupProgress:actual.startupProgress,...summarize(data,o.budget),capture:{file:id+'.png',sha256:hash(png)},elapsedMs:data.elapsedMs};report.results.push(result);
 await fs.writeFile(path.join(o.out,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({id,raf:result.rafInterval,cpu:result.rafCallbackCpu,gpuSamples:result.gpuSamples}));return result;
 }catch(error){let workloadFailure=null;if(!page.isClosed()){let diagnosticTimer;try{workloadFailure=await Promise.race([page.evaluate(()=>({documentReadyState:document.readyState,benchPresent:!!window.bench,progress:window.benchProgress||null,benchFailure:window.benchFailure||null,runtimeLosses:window.renderRuntimeLosses||null,heapBytes:performance.memory?.usedJSHeapSize??null,latestDebug:(window.bench?.engine??window.benchStartupEngine)?.debugStats?.()??null})),new Promise(resolve=>{diagnosticTimer=setTimeout(()=>resolve({unavailable:'failure diagnostic evaluation exceeded 1500ms'}),1500);})]);}catch(error){workloadFailure={unavailable:errorEvidence(error)};}finally{clearTimeout(diagnosticTimer);}}report.failures??=[];report.failures.push({family:o.family,label,load,pair,stage,error:errorEvidence(error),pageErrors:errors,workloadFailure});throw error;}finally{if(!page.isClosed())await page.close();}
}
try{
 browser=await puppeteer.launch({headless:true,executablePath,args:browserArgs(),protocolTimeout:protocolTimeoutMs(o)});report.browser=await browser.version();for(const [label,server] of [['baseline',baseline],['candidate',candidate]])if(server){try{report[label+'BuildFingerprint']=JSON.parse(await fs.readFile(path.join(server.root,'pkg/verification-build.json'),'utf8'));}catch{report[label+'BuildFingerprint']=null;}}
 try{const s=await browser.target().createCDPSession();report.hardware=(await s.send('SystemInfo.getInfo')).gpu;await s.detach();}catch{report.hardware=null;}
 if(o.command==='calibrate'){
 const fixedLoad=fixedLoadFamily(o.family),fixedProbe=process.argv.includes('--load');
 const loads=fixedLoad?[1]:fixedProbe?[o.load]:geometricLoads(o.maxLoad),tested=[];let low=0,high=null,firstFailedLoad=null,strongOver=null,overReason=null,upperBound=null,attemptingLoad=null,incompleteFailure=null;
 const calibrateProbe=async load=>{attemptingLoad=load;return run(baseline,'baseline',load);};
 try{
 for(const load of loads){const r=await calibrateProbe(load);tested.push(load);if(withinCapacity(r,o.budget)){low=load;}else{high=load;firstFailedLoad=load;break;}}
 // At most four extra probes; never calibrate against candidate.
 for(let i=0;high!==null&&!fixedProbe&&high-low>1&&i<4;i++){const mid=Math.floor((low+high)/2),r=await calibrateProbe(mid);tested.push(mid);if(withinCapacity(r,o.budget))low=mid;else high=mid;}
 // Confirm a visibly stronger overload than the refined knee, on baseline only.
 // A successful probe is an observed upper bound, never labelled overloaded.
 if(firstFailedLoad!==null&&!fixedLoad&&!fixedProbe){
 const target=Math.min(o.maxLoad,2*firstFailedLoad);let probe=report.results.find(r=>r.label==='baseline'&&r.load===target);
 if(!probe){probe=await calibrateProbe(target);tested.push(target);}
 const pass=withinCapacity(probe,o.budget);upperBound={load:target,capacityPass:pass,clampedToMaxLoad:target<2*firstFailedLoad};
 if(!pass){strongOver=target;overReason=target===firstFailedLoad?'First failed load is already the configured maximum; reused confirmed failure.':'Baseline overload confirmation failed frame-capacity criteria.';}
 else{low=Math.max(low,target);overReason='Doubled/clamped confirmation passed; upper bound observed, no confirmed strong overload.';}
 }else overReason=fixedProbe?'Explicit fixed-load probe; no refinement or doubled confirmation.':firstFailedLoad===null?'No failure observed within max-load; overload unavailable.':'Authored scene has a fixed load axis; overload replication is unavailable.';
 }catch(error){incompleteFailure={load:attemptingLoad,reason:'Probe did not produce a complete accepted measurement',...(report.failures?.at(-1)||{stage:'calibration probe',error:errorEvidence(error)})};report.error=error.stack||String(error);process.exitCode=1;}
 const saved={...calibrationSnapshot(o,report.results,{firstFailedLoad,upperBound,reason:overReason,incompleteFailure}),baselineManifest:baseline.manifest(),baselineRoot:baseline.root,driverIdentity};
 await fs.writeFile(path.join(o.out,'config.json'),JSON.stringify(saved,null,2)+'\n');report.calibration=saved;
 }else if(['compare','capacity','regression'].includes(o.command)){
 await validateCalibrationInputs(config,{runtimeRoot:baseline.root,harnessRoot:root});
 const selected=calibratedLoad(config,{explicitLoad:process.argv.includes('--load'),load:o.load,level:o.level});
 for(const load of o.command==='capacity'?config.loads:[selected])for(let pair=0;pair<o.pairs;pair++)for(const label of pair%2?['candidate','baseline']:['baseline','candidate'])await run(label==='baseline'?baseline:candidate,label,load,pair);
 report.comparisons=[];for(const load of [...new Set(report.results.map(r=>r.load))]){const rows=report.results.filter(r=>r.load===load),paired=[];for(let pair=0;pair<o.pairs;pair++){const a=rows.find(r=>r.label==='baseline'&&r.pair===pair),b=rows.find(r=>r.label==='candidate'&&r.pair===pair);let pixel=null;if(!['rim-active','arena-active'].includes(o.family)){const {PNG}=createRequire(import.meta.url)('pngjs');pixel=pixelDifference(PNG.sync.read(await fs.readFile(path.join(o.out,a.capture.file))),PNG.sync.read(await fs.readFile(path.join(o.out,b.capture.file))),{channelTolerance:o.pixelChannel,pixelFractionTolerance:o.pixelFraction});if(o.threeRoot)pixel={...pixel,withinDiagnosticTolerance:pixel.pass,pass:null,gateDisabled:true,reason:o.family.startsWith('rim-')?'original Rome diagnostic; MSAA/FXAA, shader, shadow and scene-state equivalence not established':'matched geometry only; material/PBR/lighting implementations differ'};}else pixel={pass:null,reason:o.threeRoot?'matched geometry only; renderer/material/lighting output differences are not a same-renderer pixel regression':'native gameplay is not reset to a deterministic capture state; functional acceptance required'};paired.push({pair,pixel,rafRatio:b.rafInterval.median/a.rafInterval.median,cpuRatio:a.rafCallbackCpu.median>0?b.rafCallbackCpu.median/a.rafCallbackCpu.median:null});}report.comparisons.push({load,paired,rafRatio:ratioStats(paired.map(p=>p.rafRatio)),cpuRatio:ratioStats(paired.map(p=>p.cpuRatio))});}
 if(o.command==='regression'&&report.comparisons.some(c=>c.rafRatio.median>1+(o.threshold??5)/100||c.cpuRatio?.median>1+(o.threshold??5)/100)){process.exitCode=1;report.regression=true;}if(o.command==='regression'&&report.comparisons.some(c=>c.paired.some(p=>p.pixel.pass===false))){process.exitCode=1;report.pixelRegression=true;}
 }else if(o.command==='soak'){await validateCalibrationInputs(config,{runtimeRoot:baseline.root,harnessRoot:root,harnessOnly:true});const load=calibratedLoad(config,{explicitLoad:process.argv.includes('--load'),load:o.load,level:o.level});report.calibrationValidation={commonHarness:true,selectedLoad:load,runtimeMayDiffer:true,reason:'candidate soak intentionally reuses original baseline calibrated workload'};await run(baseline,'baseline',load);
 }else if(o.smokeMatrix){if(o.command!=='smoke')throw Error('--smoke-matrix requires smoke');const selectedFamily=o.family;report.smokeFailures=[];report.smokeSkipped=[];for(const [index,family] of families.entries()){if(Date.now()-started>=o.maxTime*1000){report.smokeSkipped=families.slice(index);process.exitCode=1;break;}o.family=family;try{await run(baseline,'baseline',1);}catch(error){report.smokeFailures.push({family,backend:o.backend,error:error.message});process.exitCode=1;console.error('SMOKE FAIL '+family+': '+error.message);}}o.family=selectedFamily;}else await run(baseline,'baseline',config?.load||o.load);
}catch(e){report.error=e.stack;process.exitCode=1;console.error(e.message);}finally{
 await fs.writeFile(path.join(o.out,'raw.json.gz'),gzipSync(JSON.stringify(raw)));
 await fs.writeFile(path.join(o.out,'manifest-baseline.json'),JSON.stringify(baseline.manifest(),null,2)+'\n');if(candidate)await fs.writeFile(path.join(o.out,'manifest-candidate.json'),JSON.stringify(candidate.manifest(),null,2)+'\n');
 const finalDriverFiles=Object.fromEntries(await Promise.all(driverPaths.map(async file=>[file,hash(await fs.readFile(path.join(root,file)))])));report.driverInputsChangedDuringRun=driverPaths.filter(file=>driverFiles[file]!==finalDriverFiles[file]);if(report.driverInputsChangedDuringRun.length){report.error ||= 'Driver inputs changed during measurement';process.exitCode=1;}
 report.serverErrors={baseline:baseline.errors(),candidate:candidate?.errors()||null};if(baseline.errors().length||candidate?.errors().length){report.error ||= 'Frozen server rejected requests';process.exitCode=1;}report.durationSeconds=(Date.now()-started)/1000;await fs.writeFile(path.join(o.out,'report.json'),JSON.stringify(report,null,2)+'\n');report.cleanup=[];
 if(browser)report.cleanup.push(await boundedCleanup(()=>browser.close(),{label:'owned browser',force:()=>{browser.disconnect();browser.process()?.kill('SIGKILL');}}));
 for(const [label,server] of [['baseline server',baseline],['candidate server',candidate]])if(server)report.cleanup.push(await boundedCleanup(()=>server.close(),{label,force:()=>server.forceClose()}));
 if(report.cleanup.some(c=>!c.complete)){report.cleanupFailure=true;process.exitCode=1;}
 await fs.writeFile(path.join(o.out,'report.json'),JSON.stringify(report,null,2)+'\n');
 // This file is a standalone CLI: all output is awaited and all owned surfaces
 // were closed or force-closed. Do not let orphan library timers hang a series.
 process.exit(process.exitCode||0);
}
