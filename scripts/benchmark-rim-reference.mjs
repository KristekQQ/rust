#!/usr/bin/env node
// Actual authored Rome diagnostic using the maintained runner's browser/server
// and asynchronous GL timer. AA/shadow/gameplay differences remain explicit.
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {gzipSync} from 'node:zlib';
import puppeteer from 'puppeteer';
import {serveFrozen,stats,hash} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
import {installGlRafTimer} from './render-performance-gl-timer.mjs';
import {installRuntimeLossMonitor} from './render-performance-api.mjs';
const [nativeRoot,threeRoot,outArg,scaleArg='1']=process.argv.slice(2),scale=Number(scaleArg),root=process.cwd();
if(!nativeRoot||!threeRoot||!outArg||![1,2,3].includes(scale))throw Error('Usage: node scripts/benchmark-rim-reference.mjs FROZEN_RUST FROZEN_ORIGINAL_THREE OUTPUT [1|2|3]');
const out=path.resolve(outArg);try{await fs.stat(out);throw Error('Refuse existing output');}catch(e){if(e.code!=='ENOENT')throw e;}
await fs.mkdir(out,{recursive:true});
const servers={},raw=[],results=[],errors=[],width=960*scale,height=640*scale;
const driverPaths=['scripts/benchmark-rim-reference.mjs','scripts/render-performance-lib.mjs','scripts/render-performance-gl-timer.mjs','scripts/render-performance-api.mjs','scripts/browser-options.mjs','tests/render-performance.js','tests/render-performance.html','tests/pixels.js','tests/render-performance-three-rim.js','tests/render-performance-three-rim.html'];
const identities=async()=>Object.fromEntries(await Promise.all(driverPaths.map(async f=>[f,hash(await fs.readFile(f))])));
const driverFiles=await identities();
let browser;
try{
 servers.rust=await serveFrozen(nativeRoot,root);servers.three=await serveFrozen(threeRoot,root);
 const executablePath=execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();
 browser=await puppeteer.launch({headless:true,executablePath,protocolTimeout:60000,args:browserArgs()});
 // AB/BA paired diagnostic; short repeated GPU windows are not primary CPU data.
 for(let pair=0;pair<2;pair++)for(const label of pair%2?['three','rust']:['rust','three']){
  const page=await browser.newPage(),pageErrors=[];try{
   await page.setViewport({width,height,deviceScaleFactor:1});page.on('pageerror',e=>pageErrors.push(String(e)));page.on('console',m=>{if(m.type()==='error')pageErrors.push(m.text())});page.on('response',r=>{if(r.status()>=400)pageErrors.push(r.status()+' '+r.url())});
   await page.evaluateOnNewDocument(installRuntimeLossMonitor);await page.evaluateOnNewDocument(installGlRafTimer,true);
   await page.evaluateOnNewDocument(()=>{window.rimDiagnostic={active:false,frames:[]};const original=requestAnimationFrame.bind(window);window.requestAnimationFrame=fn=>original(t=>{const m=window.rimDiagnostic;if(!m.active)return fn(t);let row=m.frames.at(-1);if(!row||row.stamp!==t){row={stamp:t,cpu:0};m.frames.push(row);}const start=performance.now();try{window.renderGlRafTimer.beginFrame(t);return fn(t);}finally{row.cpu+=performance.now()-start;}});});
   const url=servers[label].url+(label==='three'?'/tests/render-performance-three-rim.html':'/tests/render-performance.html')+'?family=rim-static&load=1&backend=webgl&profile=0';
   await page.goto(url,{waitUntil:'domcontentloaded',timeout:60000});await page.waitForFunction('!!window.bench',{timeout:60000});const actual=await page.evaluate(()=>({backend:bench.engine.renderer_backend(),profiling:bench.engine.frameProfile().enabled}));if(actual.backend!==(label==='three'?'ThreeWebGL2':'Gl')||actual.profiling!==false)throw Error('Unexpected backend/profile '+JSON.stringify(actual));
   await page.evaluate(({width,height})=>{
    // Attach the common timer to the original application's already-created GL
    // context. The ID is restored immediately; no context/AA state is changed.
    const canvas=document.querySelector('canvas'),id=canvas.id;canvas.id='gpu-canvas';canvas.getContext('webgl2');canvas.id=id;
    if(window.cityDebug){cityDebug.renderer.setSize(width,height,false);cityDebug.camera.aspect=width/height;cityDebug.camera.updateProjectionMatrix();}else bench.engine.resize(width,height);
    bench.start();
   },{width,height});
   await new Promise(r=>setTimeout(r,3000));
   const data=await page.evaluate(async()=>{
    const m=window.rimDiagnostic,errors=()=>window.renderRuntimeLosses||[];m.frames=[];m.active=true;renderGlRafTimer.start();const start=performance.now();
    await new Promise((resolve,reject)=>{function sample(){renderGlRafTimer.endFrame();if(errors().length)return reject(Error(JSON.stringify(errors())));if(performance.now()-start>=12000)return resolve();requestAnimationFrame(sample);}requestAnimationFrame(sample);});
    m.active=false;renderGlRafTimer.stop();bench.stop();return {frames:m.frames,timer:renderGlRafTimer.snapshot(),metadata:bench.metadata,quality:{canvas:{width:document.querySelector('canvas').width,height:document.querySelector('canvas').height},postprocessing:bench.engine.postprocessing.stats()},work:bench.engine.renderer_stats(),elapsedMs:performance.now()-start};
   });
   if(pageErrors.length)throw Error(JSON.stringify(pageErrors));if(!data.timer.supported||!data.timer.samples.length||data.timer.invalid.length||data.timer.disjointDiscarded)throw Error('Unavailable/invalid/disjoint GL timer');if(data.quality.canvas.width!==width||data.quality.canvas.height!==height)throw Error('Requested render dimensions differ');
   await page.evaluate(()=>bench.capturePose());const encoded=await page.evaluate(async()=>{const {capturePixels}=await import('/tests/pixels.js');const image=await capturePixels(document.querySelector('canvas'));const c=document.createElement('canvas');c.width=image.width;c.height=image.height;c.getContext('2d').putImageData(image,0,0);return c.toDataURL('image/png').split(',')[1];});const png=Buffer.from(encoded,'base64');await fs.writeFile(path.join(out,`${label}-pair${pair}.png`),png);
   raw.push({label,pair,...data});results.push({label,pair,cpuInstrumentedMs:stats(data.frames.map(f=>f.cpu)),rafMs:stats(data.frames.slice(1).map((f,i)=>f.stamp-data.frames[i].stamp)),gpuCommandIntervalMs:stats(data.timer.samples.map(s=>s.elapsedMs)),timer:{...data.timer,samples:undefined},metadata:data.metadata,quality:data.quality,work:data.work,capture:{file:`${label}-pair${pair}.png`,sha256:hash(png)}});
   console.log(JSON.stringify({label,pair,scale,gpuMedian:results.at(-1).gpuCommandIntervalMs.median}));
  }finally{await page.close();}
 }
}catch(e){errors.push(String(e.stack||e));process.exitCode=1;}
finally{
 try {
 if(browser)await browser.close();const serverErrors=Object.fromEntries(Object.entries(servers).map(([k,s])=>[k,s.errors()]));if(Object.values(serverErrors).some(v=>v.length))process.exitCode=1;
 const finalDriverFiles=await identities(),driverInputsChanged=Object.keys(driverFiles).filter(f=>driverFiles[f]!==finalDriverFiles[f]);if(driverInputsChanged.length){errors.push('Driver changed: '+driverInputsChanged.join(','));process.exitCode=1;}
 await fs.writeFile(path.join(out,'report.json'),JSON.stringify({scope:'Actual authored static Rome at gate camera; same render resolution; original observed hardware MSAA versus Rust FXAA, shading/shadow/runtime differences prevent a parity claim. External asynchronous GL command interval includes CPU/driver gaps, not pure GPU busy time. Instrumented CPU is diagnostic only.',viewport:{width,height,deviceScaleFactor:1},protocol:{pairs:2,order:'AB/BA',warmupMs:3000,sampleMs:12000},driverFiles,finalDriverFiles,driverInputsChanged,errors,serverErrors,results},null,2)+'\n');await fs.writeFile(path.join(out,'raw.json.gz'),gzipSync(JSON.stringify(raw)));
 for(const [k,s]of Object.entries(servers)){await fs.writeFile(path.join(out,`manifest-${k}.json`),JSON.stringify(s.manifest(),null,2)+'\n');}
 } finally {await Promise.allSettled(Object.values(servers).map(async s=>{s.forceClose();await s.close();}));}
}
