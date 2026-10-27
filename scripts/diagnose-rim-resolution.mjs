#!/usr/bin/env node
// Feature ablations are diagnostics, never retained quality reductions.
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {gzipSync} from 'node:zlib';
import puppeteer from 'puppeteer';
import {serveFrozen,stats,hash} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
const [frozen,outArg,widthArg="2880",heightArg="1920"]=process.argv.slice(2);const width=Number(widthArg),height=Number(heightArg);if(!Number.isInteger(width)||!Number.isInteger(height)||width<64||height<64||width>4096||height>4096)throw Error("Invalid dimensions");if(!frozen||!outArg)throw Error('Usage: node scripts/diagnose-rim-resolution.mjs FROZEN_ROOT NEW_OUTPUT [WIDTH HEIGHT]');
const out=path.resolve(outArg);await fs.mkdir(out,{recursive:false});
const paths=['scripts/diagnose-rim-resolution.mjs','scripts/render-performance-lib.mjs','scripts/browser-options.mjs','tests/render-performance.js','tests/render-performance.html'];
const identities=async()=>Object.fromEntries(await Promise.all(paths.map(async f=>[f,hash(await fs.readFile(f))]))),before=await identities(),raw=[],results=[],errors=[];
let server,browser;
try{
 server=await serveFrozen(frozen,process.cwd());const executablePath=execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();browser=await puppeteer.launch({headless:true,executablePath,protocolTimeout:60000,args:browserArgs()});
 for(const mode of ['full','no-fxaa','no-shadows','high-power']){
  const page=await browser.newPage(),pageErrors=[];try{
   await page.setViewport({width,height,deviceScaleFactor:1});page.on('pageerror',e=>pageErrors.push(String(e)));page.on('console',m=>{if(m.type()==='error')pageErrors.push(m.text())});page.on('response',r=>{if(r.status()>=400)pageErrors.push(r.status()+' '+r.url())});
   if(mode==='high-power')await page.evaluateOnNewDocument(()=>{const original=HTMLCanvasElement.prototype.getContext;HTMLCanvasElement.prototype.getContext=function(kind,options){return original.call(this,kind,kind==='webgl2'?{...options,powerPreference:'high-performance'}:options);};});
   await page.evaluateOnNewDocument(()=>{window.rimCadence={active:false,frames:[]};const original=requestAnimationFrame.bind(window);window.requestAnimationFrame=fn=>original(t=>{const m=window.rimCadence;if(!m.active)return fn(t);let row=m.frames.at(-1);if(!row||row.stamp!==t){row={stamp:t,cpu:0};m.frames.push(row);}const start=performance.now();try{return fn(t);}finally{row.cpu+=performance.now()-start;}});});
   await page.goto(server.url+`/tests/render-performance.html?family=rim-static&load=1&backend=webgl&profile=0&width=${width}&height=${height}`,{waitUntil:'domcontentloaded',timeout:60000});await page.waitForFunction('!!window.bench',{timeout:60000});
   await page.evaluate(mode=>{if(bench.engine.renderer_backend()!=='Gl'||bench.engine.frameProfile().enabled)throw Error('Unexpected backend/profiler');if(mode==='no-fxaa')bench.engine.postprocessing.disable('fxaa');if(mode==='no-shadows')bench.engine.set_render_effects(false,false);bench.start();},mode);await new Promise(r=>setTimeout(r,3000));
   const data=await page.evaluate(async()=>{const m=window.rimCadence;m.frames=[];m.active=true;const start=performance.now();await new Promise(resolve=>{function sample(){if(performance.now()-start>=8000)return resolve();requestAnimationFrame(sample);}requestAnimationFrame(sample);});m.active=false;bench.stop();const canvas=document.querySelector('canvas'),gl=canvas.getContext('webgl2');return{frames:m.frames,elapsedMs:performance.now()-start,backend:bench.engine.renderer_backend(),work:bench.engine.renderer_stats(),graph:bench.engine.postprocessing.graph(),quality:{width:canvas.width,height:canvas.height,context:gl.getContextAttributes()}};});
   if(pageErrors.length)throw Error(JSON.stringify(pageErrors));if(data.quality.width!==width||data.quality.height!==height||data.work.drawCalls<=0)throw Error('Missing actual workload/viewport');
   raw.push({mode,...data});const raf=stats(data.frames.slice(1).map((f,i)=>f.stamp-data.frames[i].stamp));results.push({mode,raf,cpu:stats(data.frames.map(f=>f.cpu)),elapsedMs:data.elapsedMs,work:data.work,graph:data.graph,quality:data.quality});console.log(JSON.stringify({mode,mean:raf.mean,p95:raf.p95,fps:1000/raf.mean}));
  }finally{await page.close();}
 }
}catch(e){errors.push(String(e.stack||e));process.exitCode=1;}
finally{
 try{if(browser)await browser.close();const after=await identities(),changed=Object.keys(before).filter(f=>before[f]!==after[f]);if(changed.length||server?.errors().length)process.exitCode=1;await fs.writeFile(path.join(out,'report.json'),JSON.stringify({scope:`Actual staticRome${width}x${height}, noEXT/internalprofiler. Feature ablations localize work; no-fxaa/no-shadows reduce quality and are NOT candidates for retention. One3s/8s run per mode; no speedup/parity acceptance.`,before,after,changed,errors,serverErrors:server?.errors()||[],results},null,2)+'\n');await fs.writeFile(path.join(out,'raw.json.gz'),gzipSync(JSON.stringify(raw)));if(server)await fs.writeFile(path.join(out,'manifest.json'),JSON.stringify(server.manifest(),null,2)+'\n');}finally{if(server){server.forceClose();await server.close();}}
}
