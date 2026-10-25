// Headless, simulated-XR cut benchmark using the maintained gameplay scenario.
// Usage: node scripts/benchmark-rim-cuts.mjs <label> <frozen-pkg-dir>
import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {browserArgs} from './browser-options.mjs';

const repo=process.cwd(), label=process.argv[2], pkg=path.resolve(process.argv[3]||'pkg');
if(!label||!/^[-a-zA-Z0-9_]+$/.test(label)||!process.argv[3]) {
 throw Error('Usage: node scripts/benchmark-rim-cuts.mjs <safe-label> <frozen-pkg-dir>');
}
const sourceOverlay=path.join(path.dirname(pkg),'source');
const output=path.join(repo,'output/review-optimization',`${label}-cuts`);
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const report={label,date:new Date().toISOString(),pkg,sourceOverlay,requestedBackend:'webgl',
 evidenceType:'headless browser with simulated XR; no physical Quest measurement',
 browser:null,gpuDevice:null,hashes:{},runs:[],summary:null,status:'RUNNING'};
const served=new Map();
const summary=values=>{
 const samples=values.filter(Number.isFinite), sorted=[...samples].sort((a,b)=>a-b), n=sorted.length;
 return {samples, count:n,mean:n?samples.reduce((a,b)=>a+b,0)/n:null,
  p95:n?sorted[Math.ceil(n*.95)-1]:null,p99:n?sorted[Math.ceil(n*.99)-1]:null,max:n?sorted[n-1]:null};
};
const array=value=>Array.isArray(value)?value.filter(Number.isFinite):[];
const metrics=evidence=>{
 const stages=Array.isArray(evidence?.stages)?evidence.stages:[];
 const transport=evidence?.transport||{};
 const result={
  stageMs:stages.map(s=>s.stageMs),uploadBytes:stages.map(s=>s.uploadBytes),
  frameCpuMs:stages.map(s=>s.frameCpuMs),
  acceptMs:array(evidence?.acceptMs??transport.acceptMs),
  prepareMs:array(evidence?.prepareMs??transport.prepareMs),
  framesMs:array(evidence?.framesMs),
  preDeathFrameMs:(evidence?.frameEvents||[]).filter(e=>e.kills===0).map(e=>e.cpuMs),
  deathAndCutFrameMs:(evidence?.frameEvents||[]).filter(e=>e.kills>0).map(e=>e.cpuMs),
 };
 // Preserve the test's exact latency field and units without deriving latency from polling.
 for(const [key,value] of Object.entries(evidence||{})) {
  if(/(?:complet|latency).*ms$/i.test(key)) result[key]=Array.isArray(value)?value:[value];
 }
 return Object.fromEntries(Object.entries(result).map(([key,values])=>[key,summary(values)]));
};
const localOption=key=>{try{return execFileSync('git',['config','--local','--get',key],
 {encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();}catch{return undefined;}};
const within=(base,file)=>file.startsWith(base+path.sep);
const server=http.createServer(async(req,res)=>{
 try{
  const name=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  if(name==='/favicon.ico'){res.writeHead(204);res.end();return;}
  const packageRequest=name.startsWith('/pkg/');
  const base=packageRequest?pkg:repo;
  const relative=packageRequest?name.slice(4):name;
  let file=path.resolve(base,'.'+relative+(name.endsWith('/')?'index.html':''));
  if(!within(base,file))throw Error('Path outside source root');
  // Tests always use the same current instrumentation for before/after packages.
  if(!packageRequest&&!name.startsWith('/tests/')){
   const overlay=path.resolve(sourceOverlay,'.'+name);
   if(within(sourceOverlay,overlay))try{if((await fs.stat(overlay)).isFile())file=overlay;}catch{}
  }
  const bytes=await fs.readFile(file),hash=digest(bytes), previous=served.get(file);
  if(previous&&previous!==hash)throw Error(`Input changed during benchmark: ${file}`);
  served.set(file,hash);
  res.setHeader('Cache-Control','no-store');
  res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript',
   '.html':'text/html','.wasm':'application/wasm','.json':'application/json',
   '.gltf':'model/gltf+json'})[path.extname(file)]||'application/octet-stream');
  res.end(bytes);
 }catch(error){res.writeHead(404);res.end(String(error));}
});
let browser;
await fs.mkdir(output,{recursive:true});
const save=()=>fs.writeFile(path.join(output,'report.json'),JSON.stringify(report,null,2)+'\n');
try{
 for(const name of ['webgpu_wasm.js','webgpu_wasm_bg.wasm']){
  report.hashes[name]=digest(await fs.readFile(path.join(pkg,name)));
  served.set(path.join(pkg,name),report.hashes[name]);
 }
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
 const origin=`http://127.0.0.1:${server.address().port}`;
 const {default:puppeteer}=await import(path.join(repo,'node_modules/puppeteer/lib/puppeteer/puppeteer.js'));
 browser=await puppeteer.launch({headless:true,executablePath:process.env.BROWSER_EXECUTABLE||localOption('verification.browser'),
  args:browserArgs(),protocolTimeout:120000});
 report.browser=await browser.version();
 try{
  const session=await browser.target().createCDPSession();
  try{const info=await session.send('SystemInfo.getInfo');report.gpuDevice=info.gpu.devices.map(
   ({vendorString,deviceString,driverVendor,driverVersion})=>({vendorString,deviceString,driverVendor,driverVersion}));}
  finally{await session.detach();}
 }catch{/* Hardware detail is optional; backend acceptance is mandatory. */}
 for(let index=0;index<3;index++){
  const page=await browser.newPage(), errors=[];
  const abortedRequests=[];
  const run={index:index+1,status:'RUNNING',backend:null,errors,abortedRequests,evidence:null,metrics:null};
  report.runs.push(run);
  page.on('pageerror',error=>errors.push(`JS: ${error.message||error}`));
  page.on('console',message=>{if(message.type()==='error')errors.push(`console: ${message.text()}`);});
  page.on('requestfailed',request=>{const message=`request: ${request.url()}: ${request.failure()?.errorText}`;
   // dispose() intentionally cancels background decoration fetches, as in the maintained runner.
   (request.failure()?.errorText==='net::ERR_ABORTED'?abortedRequests:errors).push(message);});
  page.on('response',response=>{if(response.status()>=400)errors.push(`HTTP ${response.status()}: ${response.url()}`);});
  try{
   await page.setViewport({width:960,height:640,deviceScaleFactor:1});
   await page.goto(`${origin}/tests/rim-gameplay.html?backend=webgl&measureCuts=1`,{waitUntil:'load',timeout:120000});
   await page.waitForFunction(()=>['PASS','FAIL'].includes(window.xrTestReport?.status),{timeout:120000,polling:100});
   const data=await page.evaluate(()=>({backend:document.documentElement.dataset.rendererBackend,
    xrReport:window.xrTestReport,evidence:window.rimCutEvidence||null}));
   run.backend=data.backend;run.xrReport=data.xrReport;run.evidence=data.evidence;
   run.metrics=metrics(data.evidence);
   if(data.xrReport.status!=='PASS')throw Error(data.xrReport.error||'XR scenario failed');
   if(data.backend!=='Gl')throw Error(`Requested WebGL, observed ${data.backend||'unknown'}; fallback rejected`);
   if(!data.evidence)throw Error('Scenario passed without window.rimCutEvidence');
   if(errors.length)throw Error('Browser errors recorded');
   run.status='PASS';
  }catch(error){run.status='FAIL';run.error=String(error.message||error);
   // Recover diagnostics even when the scenario times out or reports failure.
   try{const data=await page.evaluate(()=>({backend:document.documentElement.dataset.rendererBackend,
    xrReport:window.xrTestReport||null,evidence:window.rimCutEvidence||null}));
    run.backend=data.backend;run.xrReport=data.xrReport;run.evidence=data.evidence;run.metrics=metrics(data.evidence);
   }catch{}
  }finally{await page.close();await save();}
  const compact=Object.fromEntries(Object.entries(run.metrics||{}).map(([key,{samples,...stats}])=>[key,stats]));
  console.log(JSON.stringify({label,run:run.index,status:run.status,backend:run.backend,metrics:compact,errors}));
 }
 const combined={};
 for(const run of report.runs)for(const [key,value] of Object.entries(run.metrics||{})){
  (combined[key]??=[]).push(...value.samples);
 }
 report.summary=Object.fromEntries(Object.entries(combined).map(([key,values])=>[key,summary(values)]));
 report.inputs=[];
 for(const [file,hash] of served){
  const current=digest(await fs.readFile(file));
  report.inputs.push({file,sha256:hash,stable:current===hash});
 }
 report.status=report.runs.length===3&&report.runs.every(r=>r.status==='PASS')&&report.inputs.every(i=>i.stable)?'PASS':'FAIL';
}catch(error){report.status='FAIL';report.error=String(error.stack||error);}
finally{
 if(browser)await browser.close().catch(error=>{report.cleanupError=String(error);report.status='FAIL';});
 if(server.listening)await new Promise(resolve=>server.close(resolve));
 await save();
}
console.log(`${report.status}: ${path.join(output,'report.json')}`);
if(report.status!=='PASS')process.exitCode=1;
