#!/usr/bin/env node
// Diagnostic replay only. Instrumented wall times are never performance evidence.
import fs from 'node:fs/promises';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {gzipSync,gunzipSync} from 'node:zlib';
import {fileURLToPath} from 'node:url';
import puppeteer from 'puppeteer';
import {PNG} from 'pngjs';
import {serveFrozen,pixelDifference} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';

const usage='node scripts/render-image-replay.mjs --config replay.json --out NEW_DIR';
const argv=process.argv.slice(2),args={};
for(let i=0;i<argv.length;i+=2){if(!['--config','--out'].includes(argv[i])||!argv[i+1])throw Error(usage);args[argv[i].slice(2)]=argv[i+1];}
if(!args.config||!args.out)throw Error(usage);
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const inputBytes=await fs.readFile(args.config),config=JSON.parse(inputBytes);
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const resolve=p=>{if(typeof p!=='string'||!p)throw Error('Required path missing from config');return path.resolve(p);};
const baselineRoot=resolve(config.roots?.baseline??config.baselineRoot),candidateRoot=resolve(config.roots?.candidate??config.candidateRoot),harnessRoot=resolve(config.harnessRoot);
const originalDir=resolve(config.originalReportDir),out=resolve(args.out);
const family=config.family,load=config.load,pair=config.failingPair;
if(!/^rim-(clustered|camera-stress|moving|pool-cycle|mixed)$/.test(family)||!Number.isSafeInteger(load)||load<1||!Number.isSafeInteger(pair)||pair<0)throw Error('Explicit supported family, positive integer load and failingPair required');
if(config.backend&&config.backend!=='webgpu')throw Error('The reused capture probe traces WebGPU only');
const candidateSessions=config.candidateSessions??3;
if(!Number.isSafeInteger(candidateSessions)||candidateSessions<1||candidateSessions>3)throw Error('candidateSessions must be 1..3');
await fs.mkdir(out); // Intentionally rejects an existing output directory.
const report={schema:1,purpose:'bounded instrumented image diagnosis; no timing/performance claim',config,roots:{baselineRoot,candidateRoot,harnessRoot},sessions:[],errors:[],limitations:['Buffer hashes are CPU shadows of reachable observed writes/maps/copies, not GPU readback. Texture content and arbitrary GPU writes are not traced.','Legacy capture copies native/frame state immediately after its fifth-RAF image; callback-ack captures at the submitting callback boundary.','Input roots are selected and historically verified by the caller; this tool records bytes served, not historical provenance.'],inputHashes:[],bounds:{candidateFresh:3,baselineFresh:1,candidateHistory:1,maxHistoryFrames:6000,maxRawBytes:256*1024*1024,maxCapturesPerSession:12}};
const writeReport=()=>fs.writeFile(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n');
async function archiveInput(name,file,bytes){bytes??=await fs.readFile(file);await fs.writeFile(path.join(out,name),bytes);report.inputHashes.push({file:resolve(file),archived:name,bytes:bytes.length,sha256:sha(bytes)});return bytes;}
const timeout=(promise,ms,label)=>{let timer;return Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error(label+' timed out')),ms);})]).finally(()=>clearTimeout(timer));};
await archiveInput('config.json',args.config,inputBytes);
let old,raw,archived,history;
try{
 old=JSON.parse(await archiveInput('original-report.json',path.join(originalDir,'report.json')));
 const find=label=>old.results.find(r=>r.label===label&&r.pair===pair&&r.load===load);
 const a=find('baseline'),b=find('candidate');if(!a?.capture?.file||!b?.capture?.file)throw Error('Archived baseline/candidate capture records not found');
 if(old.settings?.family!==family||old.settings?.backend!=='webgpu')throw Error('Original family/backend mismatch');
 archived={};
 for(const [label,row] of [['baseline',a],['failure',b]]){
  const source=path.resolve(originalDir,row.capture.file);
  if(!source.startsWith(originalDir+path.sep))throw Error('Archived capture escapes originalReportDir');
  const bytes=await archiveInput('archived-'+label+'.png',source);archived[label]=PNG.sync.read(bytes);
 }
 const rawBytes=await archiveInput('original-raw.json.gz',path.join(originalDir,'raw.json.gz'));
 raw=JSON.parse(gunzipSync(rawBytes,{maxOutputLength:report.bounds.maxRawBytes}));
 const id=`candidate-${family}-${load}-pair${pair}`,row=raw.find(r=>r.id===id);
 if(!row)throw Error('Missing historical raw session '+id);
 const frames=row.frames,intervals=frames.slice(1).map((f,i)=>f.stamp-frames[i].stamp);
 if(frames.length<2||frames.length>6000||intervals.some(x=>!Number.isFinite(x)||x<=0||x>10000))throw Error('Invalid or unbounded historical RAF interval sequence');
 const end=row.phaseBoundaries?.end?.elapsedSeconds;
 const firstElapsed=end*1000-intervals.reduce((a,b)=>a+b,0);
 const source=await fs.readFile(path.join(harnessRoot,'tests/render-performance.js'),'utf8');
 // The only supported clock contract is the fixture's t - epoch, with epoch
 // sampled by bench.start. Do not replace pose(), seekAnimation(), or a fixture.
 const contract=/epoch\s*=\s*performance\.now\(\)/.test(source)&&/\(t-epoch\)\/1000/.test(source)&&/capturePose\(\)/.test(source);
 if(!contract||!Number.isFinite(firstElapsed)||firstElapsed<8000||family!=='rim-clustered'){
  history={available:false,reason:'Cannot safely align this fixture/raw clock; history is not fabricated',contract,family,firstElapsed};
 }else{
  const warmup=Array.from({length:480},()=>8000/480),bridge=firstElapsed-8000;
  history={available:true,exactOriginalHistory:false,syntheticWarmup:{durationMs:8000,frames:480,intervalMs:8000/480},syntheticBridgeMs:bridge,inferredFirstElapsedMs:firstElapsed,originalPhaseBoundaries:row.phaseBoundaries,intervals,steps:[...warmup,...(bridge>1e-6?[bridge]:[]),...intervals],alignmentAssumption:'Last archived RAF stamp is assumed to share the fixture tick of phaseBoundaries.end. Original epoch and warmup RAF history were not recorded; absolute elapsed alignment is inferred, not proven.',limitations:['Recorded measured RAF intervals are replayed; callback work, native scheduling, driver history and original warmup cannot be reconstructed.']};
 }
 await fs.writeFile(path.join(out,'history-plan.json'),JSON.stringify(history,null,2)+'\n');
}catch(error){report.errors.push({stage:'inputs/history',message:error.stack||String(error)});await writeReport();throw error;}
const probePath=path.join(repo,'scripts/render-capture-probe.js');
const probe=await archiveInput('render-capture-probe.js',probePath);
for(const name of ['render-image-replay.mjs','render-performance-lib.mjs','browser-options.mjs'])await archiveInput('driver-'+name,path.join(repo,'scripts',name));
report.archivedStrictDifference=pixelDifference(archived.baseline,archived.failure,{channelTolerance:0,pixelFractionTolerance:0});
report.history={...history,steps:undefined,intervals:undefined};
let executable;
try{executable=config.browserExecutable||execFileSync('git',['config','--local','--get','verification.browser'],{cwd:repo,encoding:'utf8'}).trim();}
catch(error){report.errors.push({stage:'browser-configuration',message:String(error)});await writeReport();throw error;}

// Installed before fixture initialization. Replays one logical frame per native
// frame, preserving callback insertion order and a common timestamp for all
// callbacks. It changes elapsed-time input, never the production fixture.
function installReplayClock(){
 const nativeRaf=window.requestAnimationFrame.bind(window),nativeCancel=window.cancelAnimationFrame.bind(window),nativeNow=performance.now.bind(performance);
 const callbacks=new Map();let nextId=0,pending=null,playing=false,steps=[],index=0,stamp=0,done=null,exhausted=false;
 const schedule=()=>{if(pending===null&&callbacks.size)pending=nativeRaf(flush);};
 function flush(realStamp){
  pending=null;
  if(playing&&index<steps.length)stamp+=steps[index++];
  const time=playing?stamp:realStamp,rows=[...callbacks.values()];callbacks.clear();
  for(const callback of rows){try{callback(time);}catch(error){setTimeout(()=>{throw error;},0);}}
  if(playing&&!exhausted&&index===steps.length){exhausted=true;done?.({frames:index,elapsedMs:stamp,phase:window.bench?.phaseState()});done=null;}
  schedule();
 }
 window.requestAnimationFrame=callback=>{const id=++nextId;callbacks.set(id,callback);schedule();return id;};
 window.cancelAnimationFrame=id=>{callbacks.delete(id);if(!callbacks.size&&pending!==null){nativeCancel(pending);pending=null;}};
 Object.defineProperty(performance,'now',{configurable:true,value:()=>playing?stamp:nativeNow()});
 window.imageReplayClock={arm(sequence){if(playing)throw Error('Clock already armed');steps=sequence;index=0;stamp=0;exhausted=false;playing=true;},complete(){return exhausted?Promise.resolve({frames:index,elapsedMs:stamp,phase:window.bench?.phaseState()}):new Promise(resolve=>{done=resolve;});},state(){return{playing,index,total:steps.length,stamp,exhausted};}};
}

async function session(label,root,replay=false){
 const dir=path.join(out,label);await fs.mkdir(dir);
 const result={label,root,replay,stage:'initialization',errors:[],captures:[],status:'running',limitations:[]};report.sessions.push(result);await writeReport();
 const traces=[];let browser,server,page;
 const save=()=>fs.writeFile(path.join(dir,'trace.json.gz'),gzipSync(JSON.stringify({label,captures:traces}),{level:6}));
 const errorEvent=(kind,message)=>{if(result.errors.length<128)result.errors.push({kind,message,stage:result.stage,at:new Date().toISOString()});else if(!result.errors.some(e=>e.kind==='overflow'))result.errors.push({kind:'overflow',message:'error event retention exceeded128'});};
 try{
  server=await serveFrozen(root,harnessRoot);
  browser=await puppeteer.launch({headless:true,executablePath:executable,args:browserArgs(),protocolTimeout:180000});
  page=await browser.newPage();page.setDefaultTimeout(120000);
  page.on('pageerror',e=>errorEvent('pageerror',String(e)));page.on('console',m=>{if(m.type()==='error')errorEvent('console',m.text());});
  page.on('requestfailed',r=>errorEvent('requestfailed',r.url()+' '+r.failure()?.errorText));
  const viewport=old.viewport??{width:1280,height:720,deviceScaleFactor:1};await page.setViewport(viewport);
  if(replay)await page.evaluateOnNewDocument(installReplayClock);
  await page.evaluateOnNewDocument(probe.toString());
  const query={family,load:String(load),backend:'webgpu',profile:'0',width:String(viewport.width),height:String(viewport.height),order:old.settings.order??'spatial',seed:String(old.settings.seed??1337),overlap:String(old.settings.overlap??0)};
  await page.goto(server.url+'/tests/render-performance.html?'+new URLSearchParams(query),{waitUntil:'domcontentloaded',timeout:120000});
  await page.waitForFunction('!!window.bench && !!window.captureProbe');
  result.backend=await page.evaluate(()=>bench.engine.renderer_backend());
  if(result.backend!=='BrowserWebGpu')throw Error('WebGPU backend required, got '+result.backend);
  if(replay){
   result.stage='history';await page.evaluate(steps=>{imageReplayClock.arm(steps);bench.start();},history.steps);
   result.replayEnd=await timeout(page.evaluate(()=>imageReplayClock.complete()),150000,'historical intervals');
   result.replayClock=await page.evaluate(()=>imageReplayClock.state());
   // Retain the mismatch rather than relabel inferred history as exact.
   result.replayElapsedDeltaSeconds=result.replayEnd.phase?.elapsedSeconds-history.originalPhaseBoundaries.end.elapsedSeconds;
   if(!Number.isFinite(result.replayElapsedDeltaSeconds)||Math.abs(result.replayElapsedDeltaSeconds)>1e-6)result.limitations.push('Replayed elapsed phase differs from the archived final phase');
  }else{
   result.stage='warmup';await page.evaluate(()=>bench.start());await new Promise(resolve=>setTimeout(resolve,8000));
   result.warmup={kind:'fresh real-time diagnostic warmup',requestedMs:8000};
  }
  await page.evaluate(()=>bench.capturePose());
  async function capture(name,method='ack'){
   result.stage='capture:'+name;
   if(traces.length>=12)throw Error('Capture bound exceeded');
   const snap=await timeout(page.evaluate(method=>method==='legacy'?captureProbe.legacy():captureProbe.next(),method),60000,'capture '+name);
   const bytes=Buffer.from(snap.png,'base64');delete snap.png;
   const file=name+'.png';await fs.writeFile(path.join(dir,file),bytes);
   const image=PNG.sync.read(bytes),entry={name,method,file,sha256:sha(bytes),camera:snap.camera,phase:snap.phase,frame:snap.frame?{frame:snap.frame.frame,stamp:snap.frame.stamp,camera:snap.frame.camera,phase:snap.frame.phase}:null,epochs:{write:snap.trace?.epoch,submit:snap.trace?.submits,preparedPlanCache:snap.debug?.gltf?.preparedPlanCache},traceComplete:snap.trace?.complete,traceLimitations:snap.trace?.limitations,missingDefinitionIds:snap.trace?.missingDefinitions,vsArchivedBaseline:pixelDifference(archived.baseline,image,{channelTolerance:0,pixelFractionTolerance:0}),vsArchivedFailure:pixelDifference(archived.failure,image,{channelTolerance:0,pixelFractionTolerance:0})};
   traces.push({name,method,...snap});result.captures.push(entry);await save();await writeReport();
   if(snap.trace?.complete!==true)result.limitations.push(name+': bounded trace incomplete');
   return entry;
  }
  await capture('legacy','legacy');await capture('callback-ack');await capture('repeat');
  if(replay){
   const initial=await page.evaluate(()=>({cache:bench.engine.debugStats().gltf.preparedPlanCache.enabled,pool:bench.engine.debugStats().gltf.instancePool}));
   try{await page.evaluate(()=>bench.engine.setPreparedPlanCacheEnabled(false));await capture('cache-off');}
   finally{await page.evaluate(enabled=>bench.engine.setPreparedPlanCacheEnabled(enabled),initial.cache);}
   await capture('cache-restored');
   try{const accepted=await page.evaluate(()=>bench.engine.configureGltfInstancePool({byteLimit:0,resourceLimit:0}));if(!accepted)throw Error('Pool-zero control rejected');await capture('pool-zero');}
   finally{await page.evaluate(p=>bench.engine.configureGltfInstancePool({byteLimit:p.byteLimit,resourceLimit:p.resourceLimit,streamByteLimit:p.streamByteLimit}),initial.pool);}
   await capture('pool-restored');
   result.controlsRestored=await page.evaluate(initial=>{const g=bench.engine.debugStats().gltf;return g.preparedPlanCache.enabled===initial.cache&&['byteLimit','resourceLimit','streamByteLimit'].every(k=>g.instancePool[k]===initial.pool[k]);},initial);
   if(!result.controlsRestored)throw Error('Cache/pool controls not restored');
  }
  result.status=result.errors.length?'errors-preserved':'complete';
 }catch(error){result.errors.push({kind:'driver',message:error.stack||String(error)});result.status='failed';}
 finally{
  await save();result.stage='cleanup';
  if(page)try{await page.evaluate(()=>bench?.engine?.dispose());}catch(error){errorEvent('dispose',String(error));}
  if(browser)try{await browser.close();}catch(error){errorEvent('browser-close',String(error));}
  if(server){result.served=server.manifest();result.serverErrors=server.errors();await fs.writeFile(path.join(dir,'served-inputs.json'),JSON.stringify(result.served,null,2)+'\n');server.forceClose();await server.close();}
  await fs.writeFile(path.join(dir,'session.json'),JSON.stringify(result,null,2)+'\n');await writeReport();
 }
}
await session('baseline-fresh',baselineRoot);
for(let i=0;i<candidateSessions;i++)await session('candidate-fresh-'+i,candidateRoot);
if(history.available)await session('candidate-history',candidateRoot,true);
else report.sessions.push({label:'candidate-history',status:'not-replayed',limitation:history.reason});
report.status=report.sessions.some(s=>s.status==='failed'||s.errors?.length||s.serverErrors?.length)?'errors-preserved':'complete-diagnostic';
report.limits='Fresh/replayed image observations do not establish the cause or frequency of an archived intermittent failure. No tolerance/reference changes, performance claims or physical-device acceptance.';
await writeReport();
console.log(JSON.stringify({out,status:report.status,sessions:report.sessions.map(s=>({label:s.label,status:s.status,captures:s.captures?.length??0}))}));
if(report.status==='errors-preserved')process.exitCode=1;

// All owned pages/servers and evidence are awaited above; discard orphan library timers.
process.exit(process.exitCode||0);
