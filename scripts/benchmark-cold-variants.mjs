#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import puppeteer from 'puppeteer';
import {serveFrozen,stats,hash,boundedCleanup} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
import {installApiCounters,installRuntimeLossMonitor} from './render-performance-api.mjs';

// Separate bounded observers: maintained API counters do not wrap these methods.
function installQueryCreationCounters(){
 const output={webgl:{installed:false,calls:0,created:0,overflow:false},webgpu:{installed:false,calls:0,created:0,overflow:false},errors:[]};
 window.renderQueryCreationCounters=output;
 const limit=1000000;
 for(const [backend,prototype,name]of [['webgl',globalThis.WebGL2RenderingContext?.prototype,'createQuery'],['webgpu',globalThis.GPUDevice?.prototype,'createQuerySet']]){
  const target=output[backend],original=prototype?.[name];if(typeof original!=='function')continue;
  try{
   const increment=key=>{if(target[key]<limit)target[key]++;else target.overflow=true;};
   prototype[name]=function(...args){increment('calls');const result=original.apply(this,args);if(result)increment('created');return result;};
   target.installed=prototype[name]!==original;
  }catch(error){output.errors.push(backend+': '+String(error));}
 }
}

const options={};
for(let i=2;i<process.argv.length;i+=2){
 const key=process.argv[i],value=process.argv[i+1];
 if(!['--baseline-root','--candidate-root','--out'].includes(key)||!value||value.startsWith('--'))throw Error('Expected --baseline-root PATH --candidate-root PATH --out NEW_DIRECTORY');
 options[key.slice(2)]=path.resolve(value);
}
if(Object.keys(options).length!==3)throw Error('Required --baseline-root, --candidate-root and --out');
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
for(const key of ['baseline-root','candidate-root'])options[key]=await fs.realpath(options[key]);
await fs.mkdir(options.out); // Deliberately refuse an existing evidence directory.
const write=(name,data)=>fs.writeFile(path.join(options.out,name),JSON.stringify(data,null,2)+'\n');
const report={version:1,status:'RUNNING',startedAt:new Date().toISOString(),options,runs:[],errors:[],cleanup:[],
 scope:'Instrumented desktop actual API calls and CPU wall times; 3 AB/BA/AB fresh pages per backend. Browser process reused: page/engine cold use, not device/driver/OS-cache cold. No GPU timing, no physical headset evidence.',
 timerQueryPolicy:'profiling false; assert zero query creation calls on active backend',profiling:false,driverHashes:{}};
for(const name of ['scripts/benchmark-cold-variants.mjs','scripts/render-performance-lib.mjs','scripts/render-performance-api.mjs','scripts/browser-options.mjs','tests/render-cold-variants.html','tests/render-cold-variants.js','tests/pixels.js']){
 report.driverHashes[name]=hash(await fs.readFile(path.join(repo,name)));
}
// Numeric API deltas only. Preserve complete snapshots for gauges and metadata.
function delta(before,after){
 if(typeof before==='number'&&typeof after==='number')return after-before;
 if(before===null||after===null||typeof after!=='object')return null;
 return Object.fromEntries(Object.keys(after??{}).filter(key=>key in (before??{})).map(key=>[key,delta(before[key],after[key])]));
}
function summarize(result){
 const stage=(before,after)=>({actualApiDelta:delta(before,after)});
 return {initialized:{wallMs:result.initialized.wallMs,...stage(result.initialized.apiBefore,result.initialized.apiAfter)},
  variants:result.variants.map(v=>({name:v.name,load:{wallMs:v.load.wallMs,engineCallbacks:v.load.callbacks,callbackWallMs:stats(v.load.observedCallbacks.map(f=>f.callbackWallMs)),...stage(v.load.apiBefore,v.load.apiAfter)},
   first:{callbackWallMs:v.first.callbackWallMs,activationToCallbackEndMs:v.first.activationToCallbackEndMs,...stage(v.first.apiBefore,v.first.apiAfter)},
   warm:{callbackWallMs:stats(v.warm.callbacks.map(f=>f.callbackWallMs)),
    rafIntervalsMs:stats(v.warm.callbacks.map((f,i)=>f.stamp-(i?v.warm.callbacks[i-1].stamp:v.first.stamp))),
    ...stage(v.warm.apiBefore,v.warm.apiAfter)},native:v.native,foregroundPixels:v.foregroundPixels}))};
}
let browser;const servers={};
try{
 for(const label of ['baseline','candidate'])servers[label]=await serveFrozen(options[label+'-root'],repo);
 const executablePath=process.env.BROWSER_EXECUTABLE||execFileSync('git',['config','--local','--get','verification.browser'],{cwd:repo,encoding:'utf8'}).trim();
 browser=await puppeteer.launch({headless:true,executablePath,args:browserArgs(),protocolTimeout:120000});
 report.browser=await browser.version();
 for(const backend of ['webgpu','webgl'])for(let pair=0;pair<3;pair++)for(const label of pair===1?['candidate','baseline']:['baseline','candidate']){
  const id=`${backend}-${pair+1}-${label}`,record={id,backend,pair:pair+1,label,status:'RUNNING',consoleErrors:[],pageErrors:[],requestFailures:[],abortedRequests:[]};
  report.runs.push(record);await write('report.json',report);
  let page;
  try{
   page=await browser.newPage();await page.setViewport({width:640,height:400,deviceScaleFactor:1});
   await page.evaluateOnNewDocument(installApiCounters);
   await page.evaluateOnNewDocument(installQueryCreationCounters);
   await page.evaluateOnNewDocument(installRuntimeLossMonitor);
   page.on('console',msg=>{if(msg.type()==='error')record.consoleErrors.push(msg.text());});
   page.on('pageerror',error=>record.pageErrors.push(String(error)));
   page.on('requestfailed',request=>{const failure={url:request.url(),error:request.failure()?.errorText};
    (failure.error==='net::ERR_ABORTED'?record.abortedRequests:record.requestFailures).push(failure);});
   await page.goto(servers[label].url+'/tests/render-cold-variants.html?backend='+backend,{waitUntil:'load',timeout:60000});
   await page.waitForFunction(()=>['PASS','FAIL'].includes(window.coldVariants?.status),{timeout:90000});
   const state=await page.evaluate(()=>({status:window.coldVariants.status,error:window.coldVariants.error,failure:window.coldVariants.failure,result:window.coldVariants.result}));
   if(state.status!=='PASS')throw Error(state.error||'Probe failed');
   const result=state.result;
   const expectedBackend={webgl:'Gl',webgpu:'BrowserWebGpu'}[backend],query=result.queryCreation[backend];
   if(result.requestedBackend!==backend||result.backend!==expectedBackend||result.profiling!==false||result.timerQueriesEnabled!==false)throw Error('Backend/profiling/query assertion failed');
   if(!query.installed||query.calls!==0||query.created!==0||query.overflow||result.queryCreation.errors.length)throw Error('Query creation observation failed');
   record.queryCreation=result.queryCreation;
   if(result.apiErrors.length||result.losses.length)throw Error('API instrumentation or runtime loss error');
   if(record.consoleErrors.length||record.pageErrors.length||record.requestFailures.length)throw Error('Browser errors occurred');
   for(const variant of result.variants){
    if(variant.warm.callbacks.length!==120)throw Error('Expected 120 warm engine callbacks');
    await fs.writeFile(path.join(options.out,id+'-'+variant.name+'.png'),Buffer.from(variant.png.split(',')[1],'base64'));
    delete variant.png;
   }
   record.summary=summarize(result);record.rawFile=id+'.json';await write(record.rawFile,result);
   record.status='PASS';
  }catch(error){
   record.status='FAIL';record.error=String(error);report.errors.push(id+': '+error);
   if(page){
    try{record.failureState=await page.evaluate(()=>({status:window.coldVariants?.status,error:window.coldVariants?.error,failure:window.coldVariants?.failure,queryCreation:window.renderQueryCreationCounters??null}));}
    catch(diagnosticError){record.failureStateError=String(diagnosticError);}
    try{record.failureScreenshot=id+'-failure.png';await page.screenshot({path:path.join(options.out,record.failureScreenshot)});}
    catch(screenshotError){record.failureScreenshotError=String(screenshotError);}
   }
  }
  finally{
   record.servedFiles=servers[label].manifest();record.serverErrors=servers[label].errors();
   if(record.serverErrors.length){record.status='FAIL';report.errors.push(id+': frozen server errors');}
   if(page){
    record.engineCleanup=await boundedCleanup(()=>page.evaluate(()=>window.coldVariants?.dispose()),{label:id+' engine'});
    record.pageCleanup=await boundedCleanup(()=>page.close(),{label:id+' page'});
   }
   await write('report.json',report);
  }
 }
 report.comparisons=['webgpu','webgl'].map(backend=>({backend,
  initialized:Object.fromEntries(['baseline','candidate'].map(label=>[label,stats(report.runs.filter(r=>r.backend===backend&&r.label===label&&r.status==='PASS').map(r=>r.summary.initialized.wallMs))])),
  variants:['white-helmet','colored-rgba'].map(name=>({name,...Object.fromEntries(['baseline','candidate'].map(label=>{
   const rows=report.runs.filter(r=>r.backend===backend&&r.label===label&&r.status==='PASS').map(r=>r.summary.variants.find(v=>v.name===name));
   return [label,{loadWallMs:stats(rows.map(v=>v.load.wallMs)),firstCallbackWallMs:stats(rows.map(v=>v.first.callbackWallMs)),
    firstActivationWallMs:stats(rows.map(v=>v.first.activationToCallbackEndMs)),warmCallbackMedianMs:stats(rows.map(v=>v.warm.callbackWallMs.median))}];
  }))}))}));
}catch(error){report.errors.push(String(error));}
finally{
 if(browser)report.cleanup.push(await boundedCleanup(()=>browser.close(),{label:'browser',force:()=>browser.process()?.kill('SIGKILL')}));
 for(const [label,server]of Object.entries(servers))report.cleanup.push(await boundedCleanup(()=>server.close(),{label:label+' server',force:()=>server.forceClose()}));
 report.status=report.errors.length||report.runs.length!==12||report.runs.some(r=>r.status!=='PASS'||!r.engineCleanup?.complete||!r.pageCleanup?.complete)||report.cleanup.some(c=>!c.complete)?'FAIL':'PASS';
 report.finishedAt=new Date().toISOString();await write('report.json',report);
 console.log(JSON.stringify({status:report.status,runs:report.runs.length,out:options.out,errors:report.errors}));
 if(report.status!=='PASS')process.exitCode=1;
}
