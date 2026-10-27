#!/usr/bin/env node
// Compare mutable light program classes to an independently frozen generic loop.
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import puppeteer from 'puppeteer';
import {PNG} from 'pngjs';
import {serveFrozen,pixelDifference,hash} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
const [baseline,candidate,outArg]=process.argv.slice(2);
if(!baseline||!candidate||!outArg)throw Error('Usage: verify-light-classes.mjs BASELINE CANDIDATE NEW_OUTPUT');
const out=path.resolve(outArg);await fs.mkdir(out,{recursive:false});
const owned=['scripts/verify-light-classes.mjs','scripts/render-performance-lib.mjs','scripts/browser-options.mjs','tests/normal-map-probe.html','tests/normal-map-probe.js','tests/pixels.js'];
const identities=async()=>Object.fromEntries(await Promise.all(owned.map(async f=>[f,hash(await fs.readFile(f))])));
const before=await identities(),results=[],manifests=[],errors=[];let browser,server;
const lights=(mask,phase=0)=>Array.from({length:4},(_,i)=>({position:[-2+i*1.4+phase*.3,1+i*.2,4-i*.3],color:mask&(1<<i)?[.4+i*.2,.7-i*.1,.3+i*.15].map(v=>v*(phase?1.3:1)):[0,0,0]}));
const states=[];
for(const directional of [true,false])for(const mask of [15,0,1,2,4,8,3,5,9,6,10,12,7,11,13,14])states.push({mask,directional,phase:0});
states.push({mask:15,directional:true,phase:1},{mask:15,directional:false,phase:1},{mask:15,directional:true,phase:0});
try{
 const executablePath=execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();
 browser=await puppeteer.launch({headless:true,executablePath,protocolTimeout:60000,args:browserArgs()});
 for(const backend of ['webgpu','webgl']){
  const images={baseline:[],candidate:[]};
  for(const [label,root] of [['baseline',baseline],['candidate',candidate]]){
   server=await serveFrozen(root,process.cwd());const page=await browser.newPage(),pageErrors=[];
   try{
    page.on('pageerror',e=>pageErrors.push(String(e)));page.on('console',m=>{if(m.type()==='error')pageErrors.push(m.text())});page.on('response',r=>{if(r.status()>=400)pageErrors.push(`${r.status()} ${r.url()}`)});
    await page.setViewport({width:640,height:400,deviceScaleFactor:1});
    await page.goto(server.url+'/tests/normal-map-probe.html?backend='+backend,{waitUntil:'domcontentloaded',timeout:60000});
    await page.waitForFunction('!!window.normalMapProbe',{timeout:60000});
    const actual=await page.evaluate(()=>normalMapProbe.backend);
    if(actual!==(backend==='webgl'?'Gl':'BrowserWebGpu'))throw Error('Backend fallback '+actual);
    // One static loaded model: subsequent light/appearance mutations must update
    // the shader class even though object geometry and transforms stay unchanged.
    await page.evaluate(async lights=>normalMapProbe.capture({view:'front-single',normal:true,tangent:true,metal:true,roughness:.2,instances:true,lights},'mapped'),lights(15));
    for(let i=0;i<states.length;i++){
     const state=states[i],data=await page.evaluate(async({lights,directional})=>normalMapProbe.mutateLights(lights,directional),{lights:lights(state.mask,state.phase),directional:state.directional});
     const bytes=Buffer.from(data.png.split(',')[1],'base64');
     await fs.writeFile(path.join(out,`${backend}-${label}-${i}.png`),bytes);
     images[label].push(PNG.sync.read(bytes));
     manifests.push({backend,label,state,stats:data.stats});
     if(label==='candidate'&&i===0)page.firstInstanceBuffers=data.stats.instanceBuffersCreated;
     if(label==='candidate'&&data.stats.instanceBuffersCreated!==page.firstInstanceBuffers)throw Error('Lighting duplicated instance buffers');
    }
    await page.evaluate(()=>normalMapProbe.dispose());
    if(pageErrors.length)throw Error(JSON.stringify(pageErrors));
   } catch(error) {await page.screenshot({path:path.join(out,`${backend}-${label}-failure.png`)});throw Error(`${label}/${backend}: ${String(error.stack||error)}\nBrowser errors: ${JSON.stringify(pageErrors)}`);} finally {await page.close();manifests.push({backend,label,files:server.manifest(),errors:server.errors()});server.forceClose();await server.close();server=null;}
  }
  for(let i=0;i<states.length;i++){
   const difference=pixelDifference(images.baseline[i],images.candidate[i]);
   results.push({backend,state:states[i],difference});
   if(!difference.pass)throw Error(`Light class pixels differ: ${backend} ${i} ${JSON.stringify(difference)}`);
  }
  const colorChanged=pixelDifference(images.candidate[0],images.candidate[32],{channelTolerance:0,pixelFractionTolerance:0});
  const restored=pixelDifference(images.candidate[0],images.candidate[34],{channelTolerance:0,pixelFractionTolerance:0});
  if(colorChanged.changedPixels<100||!restored.exact)throw Error('Light mutation/restoration is not visible/exact');
 }
 if(JSON.stringify(before)!==JSON.stringify(await identities()))throw Error('Harness inputs changed');
 console.log(`PASS: ${results.length} light-class pixel comparisons; masks, light types, color/position mutations and exact restoration`);
}catch(e){errors.push(String(e.stack||e));console.error(errors.at(-1));process.exitCode=1;}
finally{
 if(server){server.forceClose();await server.close();}
 if(browser)await browser.close();
 await fs.writeFile(path.join(out,'report.json'),JSON.stringify({before,after:await identities(),results,manifests,errors,physicalQuest:false},null,2)+'\n');
}
