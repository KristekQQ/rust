import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import {createHash} from 'node:crypto';
export const hash=b=>createHash('sha256').update(b).digest('hex');
export const families=['batching','batching-heterogeneous','primitives','heterogeneous','rigid','skin','morph','fragment','effects','dynamic','rim-static','rim-active','rim-stress','rim-camera','rim-clustered','arena-static','arena-active'];
export const scalableLoadFamilies=['batching','batching-heterogeneous','primitives','heterogeneous','rigid','skin','morph','fragment','effects','dynamic','rim-stress','rim-clustered'];
export function fixedLoadFamily(family){return !scalableLoadFamilies.includes(family);}
export function stats(values,budget=1000/60){const a=values.filter(Number.isFinite).sort((a,b)=>a-b);if(!a.length)return null;const q=p=>a[Math.max(0,Math.ceil(a.length*p)-1)];return {count:a.length,median:q(.5),mean:a.reduce((s,v)=>s+v,0)/a.length,p95:q(.95),p99:q(.99),max:a.at(-1),budgetMs:budget,overrunToleranceMs:.5,rawExactOverruns:a.filter(v=>v>budget).length,rawExactOverrunDurationMs:a.reduce((s,v)=>s+Math.max(0,v-budget),0),overruns:a.filter(v=>v>budget+.5).length,overrunDurationMs:a.reduce((s,v)=>s+(v>budget+.5?v-budget:0),0)};}
// CPU histories have no invocation sequence. An unchanged history cannot prove
// that its last value belongs to this window; cold-only scopes remain visible.
export function cpuScopeEvidence(data){
 const previous=new Map(Object.entries(data.boundaryStart?.profile?.cpu||{}).map(([label,value])=>[label,JSON.stringify(value)]));
 const values=new Map([...previous.keys()].map(label=>[label,[]]));
 for(const sample of data.samples||[])for(const [label,value] of Object.entries(sample.profile?.cpu||{})){
  if(!values.has(label))values.set(label,[]);
  const signature=JSON.stringify(value);
  if(signature!==previous.get(label)&&Number.isFinite(value.lastMs))values.get(label).push(value.lastMs);
  previous.set(label,signature);
 }
 return {method:'changed reported CPU histories relative to the measurement boundary',perInvocationTimesAvailable:false,
  limitation:'Identically remeasured and untouched histories are indistinguishable; intermediate invocations are not recovered. These are sparse observed last-sample changes, not per-frame scope histograms.',
  scopes:Object.fromEntries([...values].map(([label,samples])=>[label,{observedHistoryChanges:samples.length,freshness:samples.length?'observed-history-changes':'unproven',lastSampleMs:stats(samples)}]))};
}
export function geometricLoads(max){const a=[];for(let n=1;n<=max;n*=2)a.push(n);if(a.at(-1)!==max)a.push(max);return a;}
export function parseArgs(argv){
 const o={command:argv[0]||'help',backend:'webgpu',family:'primitives',width:960,height:640,warmup:8000,sample:25000,pairs:3,order:'spatial',seed:1337,overlap:0,level:'near',load:1,maxLoad:1024,maxMemory:512,maxTime:1800,startupTimeout:30000,slowFrame:250,maxSlowFrames:20,budget:1000/60,threeRoot:null,threeRepresentation:'instanced',profiling:true,diagnostics:false,smokeMatrix:false,pixelChannel:8,pixelFraction:.001,out:'output/render-performance'};
 const names={'three-representation':'threeRepresentation','three-root':'threeRoot','baseline-root':'baselineRoot','candidate-root':'candidateRoot','max-load':'maxLoad','max-memory':'maxMemory','max-time':'maxTime','startup-timeout':'startupTimeout','slow-frame':'slowFrame','max-slow-frames':'maxSlowFrames','smoke-matrix':'smokeMatrix','pixel-channel':'pixelChannel','pixel-fraction':'pixelFraction'};
 for(let i=1;i<argv.length;i++){const key=argv[i];if(key==='--help'){o.command='help';continue;}if(key==='--smoke-matrix'){o.smokeMatrix=true;continue;}if(key==='--diagnostics'){o.diagnostics=true;continue;}if(key==='--no-profile'){o.profiling=false;continue;}if(!key.startsWith('--')||!argv[i+1]||argv[i+1].startsWith('--'))throw Error('Expected --option value: '+key);const name=names[key.slice(2)]||key.slice(2);if(!(name in o)&&!['config','baselineRoot','candidateRoot','threshold'].includes(name))throw Error('Unknown '+key);o[name]=typeof o[name]==='number'||name==='threshold'?Number(argv[++i]):argv[++i];}
 for(const k of ['warmup','sample','pairs','load','maxLoad','maxMemory','maxTime','startupTimeout','slowFrame','maxSlowFrames','budget','width','height'])if(!Number.isFinite(o[k])||o[k]<=0)throw Error('Invalid '+k);
 if(!Number.isInteger(o.width)||!Number.isInteger(o.height)||o.width<64||o.height<64||o.width>4096||o.height>4096)throw Error('Invalid render dimensions');
 if(!Number.isInteger(o.load)||o.load>o.maxLoad||!Number.isInteger(o.maxLoad)||!Number.isInteger(o.pairs)||o.pairs>20||o.maxLoad>65536||o.sample>300000||o.warmup>300000||o.maxTime>14400||o.startupTimeout>120000)throw Error('Bounds exceeded');
 if(!Number.isFinite(o.pixelChannel)||o.pixelChannel<0||o.pixelChannel>255||!Number.isFinite(o.pixelFraction)||o.pixelFraction<0||o.pixelFraction>1)throw Error('Invalid pixel tolerance');
 if(['rim-stress','rim-clustered'].includes(o.family)&&(o.maxLoad>2048||o.load>2048))throw Error('Rim stress model budget is 2048');
 if(!['spatial','shuffled','alternating'].includes(o.order)||!Number.isInteger(o.seed)||o.seed<0||o.seed>0xffffffff||!Number.isFinite(o.overlap)||o.overlap<0||o.overlap>1)throw Error('Invalid order/seed/overlap');
 if(!['instanced','objects'].includes(o.threeRepresentation))throw Error('Invalid Three representation');
 if(o.threeRepresentation!=='instanced'&&!o.threeRoot)throw Error('--three-representation objects requires --three-root');
 if(!['light','near','over'].includes(o.level))throw Error('Unknown calibration level');
 if(!['webgpu','webgl'].includes(o.backend)||!families.includes(o.family))throw Error('Unknown backend/family');return o;
}
export async function serveFrozen(root,harnessRoot){
 root=await fs.realpath(root);harnessRoot=await fs.realpath(harnessRoot);const manifest=new Map(),errors=[];
 const server=http.createServer(async(req,res)=>{try{
 const name=decodeURIComponent(new URL(req.url,'http://local').pathname);if(name==='/favicon.ico'){res.writeHead(204);res.end();return;}
 const harness=['/tests/normal-map-probe.html','/tests/normal-map-probe.js','/tests/shadow-filter-probe.html','/tests/shadow-filter-probe.js','/tests/render-performance.html','/tests/render-performance.js','/tests/pixels.js','/tests/render-performance-three.html','/tests/render-performance-three.js','/tests/render-performance-three-rim.html','/tests/render-performance-three-rim.js','/examples/rim-three/stress.js','/tests/rim-three-stress-check.js','/scripts/render-performance-order.mjs'].includes(name),base=harness?harnessRoot:root;
 const file=await fs.realpath(path.resolve(base,'.'+name+(name.endsWith('/')?'index.html':'')));if(!file.startsWith(base+path.sep))throw Error('Outside frozen root');
 const bytes=await fs.readFile(file),entry={url:name,bytes:bytes.length,sha256:hash(bytes),source:harness?'harness':'frozen'};
 const old=manifest.get(name);if(old&&old.sha256!==entry.sha256)throw Error('Frozen bytes changed '+name);manifest.set(name,entry);
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.html':'text/html','.wasm':'application/wasm','.json':'application/json','.gltf':'model/gltf+json'})[path.extname(file)]||'application/octet-stream');res.setHeader('Cache-Control','no-store');res.end(bytes);
 }catch(e){errors.push(String(e));res.writeHead(404);res.end(String(e));}});await new Promise(r=>server.listen(0,'127.0.0.1',r));
 return {root,url:'http://127.0.0.1:'+server.address().port,errors:()=>[...errors],manifest:()=>[...manifest.values()].sort((a,b)=>a.url.localeCompare(b.url)),close:()=>new Promise(r=>server.close(r)),forceClose:()=>server.closeAllConnections()};
}
export function summarize(data,budget){const interval=data.frames.slice(1).map((v,i)=>v.stamp-data.frames[i].stamp),keys=[...new Set(data.gpu.flatMap(g=>Object.keys(g.passesMs||{})))],work=workDeltas(data),culled=work?.gltf?.primitiveCulled?.delta??null;return {cpuScopeEvidence:cpuScopeEvidence(data),glRafGpu:data.glRafGpu?{...data.glRafGpu,samples:undefined}:null,glRafGpuMs:data.glRafGpu?.samples?.length?stats(data.glRafGpu.samples.map(s=>s.elapsedMs),budget):null,work,phaseEvidence:data.phaseBoundaries?{...data.phaseBoundaries,commandScope:'requested workload commands and observed native state at measured boundaries; fields depend on workload family',nativeGltfPrimitiveCulledDelta:culled,cullingObserved:culled===null?null:culled>0,cullingRequiredForShortSmoke:false}:null,missedFrameFraction:interval.length?interval.filter(v=>v>budget*1.5+.5).length/interval.length:null,rafInterval:stats(interval,budget),rafCallbackCpu:stats(data.frames.map(r=>r.cpu),budget),gpuPassMs:data.gpu.length?Object.fromEntries(keys.map(k=>[k,stats(data.gpu.map(g=>g.passesMs?.[k]))])):null,gpuSamples:data.gpu.length,gpuAvailable:data.gpu.length>0,cpuScopes:data.samples.at(-1)?.profile.cpu||null,sampledCpuScopeMs:Object.fromEntries([...new Set(data.samples.flatMap(s=>Object.keys(s.profile.cpu||{})))].map(k=>[k,stats(data.samples.map(s=>s.profile.cpu?.[k]?.lastMs),budget)])),gpuSupported:data.samples.at(-1)?.profile.gpu?.supported??null,gpuError:data.samples.at(-1)?.profile.gpu?.error??null,renderer:data.samples.at(-1)?.renderer||null,debug:data.samples.at(-1)?.debug||null,heapBytes:data.heapBytes??null,backend:data.backend,profiling:data.profiling,frames:data.frames.length,commands:data.commands};}

export function withinCapacity(result,budget){const r=result.rafInterval,c=result.rafCallbackCpu;return !!r&&!!c&&r.median<=budget+.5&&r.p95<=budget*2+.5&&result.missedFrameFraction<=.1&&c.p95<=budget+.5;}

// The renderer's glTF fields below are monotonic event counters. Gauges such as
// instanceBufferBytes/pagesActive and scratch capacities are deliberately excluded.
export const gltfEventCounters=['preparedPlanCache.hits','preparedPlanCache.misses',...Array.from({length:12},(_,i)=>'preparedPlanCache.reasons.'+i),'prepareEligibleRebuilds','prepareGroupRebuilds','prepareOpaqueGroupRebuilds','preparePayloadRebuilds','prepareRecordRebuilds','prepareTimedCalls',...['eligible','groups','basis','payload','records','resources'].map(name=>'prepareStageMs.'+name),'cpuDeformedVertices','rigidUploadBytes','uniformUploadBytes','bufferUploadCalls','skinUploadBytes','skinPaletteUploadBytes','drawCalls','primitiveTests','primitiveCulled','rootBoundsTests','rootFullyInside','instancedDrawCalls','translatedDrawCalls','translatedInstances','instancesDrawn','trianglesDrawn','overlapTests','overlapNodeTests','instancePagesCreated','instanceBuffersCreated','instanceBindGroupsCreated','instanceUploadBytes','queueScratchGrowths','pipelineChanges','bindGroupChanges','geometryCacheHits','materialCacheHits',...['rootBlend','rootUnsupported','rootBounds','rootAsset','rootOverlap','rootSpatial','drawCap','drawOrdinal','drawAsset','drawDeformed','drawMirror','drawMaterial','drawPrimitive','drawOptions'].map(reason=>'batchBreaks.'+reason)];
export function workDeltas(data){
 const a=data.boundaryStart,b=data.boundaryEnd;if(!a||!b)return null;
 const profilerFrames=Number.isInteger(a.profile.frame)&&Number.isInteger(b.profile.frame)?b.profile.frame-a.profile.frame:null;
 const applicationRafFrames=data.frames.length,seconds=data.elapsedMs/1000;
 const gltf=Object.fromEntries(gltfEventCounters.map(key=>{const start=key.split('.').reduce((value,part)=>value?.[part],a.debug?.gltf),end=key.split('.').reduce((value,part)=>value?.[part],b.debug?.gltf),delta=Number.isFinite(start)&&Number.isFinite(end)&&end>=start?end-start:null;return [key,{start:start??null,end:end??null,delta,perProfilerFrame:delta!==null&&profilerFrames>0?delta/profilerFrames:null,perApplicationRaf:delta!==null&&applicationRafFrames>0?delta/applicationRafFrames:null,perSecond:delta!==null&&seconds>0?delta/seconds:null}];}));
 return {boundary:'immediately before activation and after deactivation of measured RAF window',profilerFrames,applicationRafFrames,elapsedMs:data.elapsedMs,gltf,scope:'glTF counters only; not whole-renderer allocation totals',gauges:{start:a.debug?.gltf??null,end:b.debug?.gltf??null}};
}
export function pixelDifference(a,b,{channelTolerance=8,pixelFractionTolerance=.001}={}){
 if(a.width!==b.width||a.height!==b.height)return {pass:false,reason:'dimensions differ',baseline:[a.width,a.height],candidate:[b.width,b.height]};
 let maxChannelDelta=0,sum=0,changedPixels=0;for(let i=0;i<a.data.length;i+=4){let changed=false;for(let j=0;j<4;j++){const delta=Math.abs(a.data[i+j]-b.data[i+j]);sum+=delta;maxChannelDelta=Math.max(maxChannelDelta,delta);if(delta>channelTolerance)changed=true;}if(changed)changedPixels++;}
 const pixels=a.width*a.height,changedFraction=changedPixels/pixels;
 return {pass:changedFraction<=pixelFractionTolerance,exact:maxChannelDelta===0,width:a.width,height:a.height,maxChannelDelta,meanAbsoluteChannelDelta:sum/a.data.length,changedPixels,changedFraction,channelTolerance,pixelFractionTolerance};
}

export function errorEvidence(error){return {name:error?.name||typeof error,message:error?.message||String(error),stack:error?.stack||null,string:String(error)};}

export function calibrationSnapshot(options,results,{firstFailedLoad=null,upperBound=null,reason=null,incompleteFailure=null}={}){
 const probes=results.filter(r=>r.label==='baseline').map(r=>({load:r.load,capacityPass:withinCapacity(r,options.budget)}));
 const loads=[...new Set(probes.map(p=>p.load))].sort((a,b)=>a-b),passing=probes.filter(p=>p.capacityPass).map(p=>p.load),failing=probes.filter(p=>!p.capacityPass).map(p=>p.load);
 const near=passing.length?Math.max(...passing):loads[0]??null,over=failing.length?Math.max(...failing):null;
 const light=loads.filter(n=>n<=Math.max(1,(near||1)/4)).at(-1)??loads[0]??null;
 return {version:1,...Object.fromEntries(['backend','family','order','seed','overlap','profiling','budget','warmup','sample','width','height'].map(k=>[k,options[k]])),load:near,levels:{light,near,over},loads,underBudget:passing.length>0,incomplete:incompleteFailure!==null,unsafeLoad:incompleteFailure,overloadConfirmation:{firstFailedLoad,upperBound,reason:incompleteFailure?'Probe incomplete; retained largest completed failed capacity load.':reason},capacityProbes:probes,selectionMetric:'frame capacity: RAF median <= budget+.5ms, p95 <= 2*budget+.5ms, missed intervals <= 10%, callback CPU p95 <= budget+.5ms; RAF is not compositor presentation'};
}

export function protocolTimeoutMs({sample,startupTimeout}){if(!Number.isFinite(sample)||sample<=0||sample>300000||!Number.isFinite(startupTimeout)||startupTimeout<=0||startupTimeout>120000)throw Error('Invalid protocol duration bounds');return Math.max(120000,sample+startupTimeout+30000);}


export function ratioStats(values){const s=stats(values);return s?Object.fromEntries(['count','median','mean','p95','p99','max'].map(k=>[k,s[k]])):null;}
export function calibratedLoad(config,{explicitLoad=false,load,level='near'}={}){
 const selected=explicitLoad?load:config.levels?.[level]??(level==='near'?config.load:null);
 if(!Number.isInteger(selected)||!config.loads.includes(selected))throw Error('Requested load was not calibrated or requested capacity level unavailable');
 return selected;
}
export async function validateCalibrationInputs(config,{runtimeRoot,harnessRoot,harnessOnly=false}){
 if(!Array.isArray(config.baselineManifest)||!config.baselineManifest.some(e=>e.source==='harness'))throw Error('Calibration lacks common harness identity');
 for(const entry of config.baselineManifest){if(harnessOnly&&entry.source!=='harness')continue;const bytes=await fs.readFile(path.join(entry.source==='harness'?harnessRoot:runtimeRoot,entry.url));if(hash(bytes)!==entry.sha256)throw Error('Baseline differs from calibration: '+entry.url);}
}

export async function boundedCleanup(action,{label,timeoutMs=5000,force=()=>{}}){
 let timer;
 try{await Promise.race([Promise.resolve().then(action),new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error(label+' cleanup timeout')),timeoutMs);})]);return {label,complete:true,forced:false};}
 catch(error){let forceError=null;try{force();}catch(e){forceError=errorEvidence(e);}return {label,complete:false,forced:true,error:errorEvidence(error),forceError};}
 finally{clearTimeout(timer);}
}
