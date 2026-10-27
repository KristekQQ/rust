#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import puppeteer from 'puppeteer';
import {PNG} from 'pngjs';
import {serveFrozen,pixelDifference,hash} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';

const [baseline,candidate,outArg,stateMode='all']=process.argv.slice(2);
if(!['all','uv-only','pbr'].includes(stateMode))throw Error('Invalid state mode');
if(!baseline||!candidate||!outArg)throw Error('Usage: node scripts/verify-normal-map.mjs BASELINE CANDIDATE NEW_OUTPUT');
const out=path.resolve(outArg);await fs.mkdir(out,{recursive:false});
const owned=['scripts/verify-normal-map.mjs','scripts/render-performance-lib.mjs','scripts/browser-options.mjs','tests/normal-map-probe.html','tests/normal-map-probe.js','tests/pixels.js'];
const identities=async()=>Object.fromEntries(await Promise.all(owned.map(async f=>[f,hash(await fs.readFile(f))])));
const before=await identities(),results=[],errors=[],manifests=[];let browser,server;
const states=[];
if(stateMode!=='uv-only')for(const view of ['front-single','back-double','mirror-front'])for(const normal of [false,true])for(const tangent of [false,true])for(const metal of [false,true])states.push({view,normal,tangent,metal});
for(const normal of [false,true])for(const tangent of [false,true])for(const metal of [false,true])states.push({view:'front-single',normal,tangent,metal,noUv:true});
if(stateMode==='pbr'){for(const normal of [false,true])for(const tangent of [false,true])states.push({view:'front-single',normal,tangent,metal:true,roughness:.05});for(const normal of [false,true])for(const tangent of [false,true])for(const metal of [false,true])states.push({view:'grazing-double',normal,tangent,metal});}
const save=async(label,backend,state,mode,data)=>{
 if(!data?.png)throw Error(`Missing image: ${label}/${backend}/${state.view}/n${+state.normal}t${+state.tangent}/${mode}`);
 const bytes=Buffer.from(data.png.split(',')[1],'base64'),image=PNG.sync.read(bytes);
 const name=`${label}-${backend}-${state.view}-n${+state.normal}-t${+state.tangent}-${state.metal?'metal':'dielectric'}-${state.noUv?'noUV':'UV'}-r${state.roughness??'default'}-${mode}.png`;
 await fs.writeFile(path.join(out,name),bytes);return image;
};
try{
 const executablePath=execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();
 browser=await puppeteer.launch({headless:true,executablePath,protocolTimeout:60000,args:browserArgs()});
 for(const backend of ['webgpu','webgl']){
  const images={baseline:new Map(),candidate:new Map()};
  for(const [label,root] of [['baseline',baseline],['candidate',candidate]]){
   server=await serveFrozen(root,process.cwd());const page=await browser.newPage(),pageErrors=[];
   try{
    page.on('pageerror',e=>pageErrors.push(String(e)));page.on('console',m=>{if(m.type()==='error')pageErrors.push(m.text())});page.on('response',r=>{if(r.status()>=400)pageErrors.push(`${r.status()} ${r.url()}`)});
    await page.setViewport({width:640,height:400,deviceScaleFactor:1});
    await page.goto(server.url+'/tests/normal-map-probe.html?backend='+backend,{waitUntil:'domcontentloaded',timeout:60000});
    if(pageErrors.length)throw Error('Startup errors: '+JSON.stringify(pageErrors));
    await Promise.race([page.waitForFunction('!!window.normalMapProbe',{timeout:60000}),new Promise((_,reject)=>page.once('pageerror',reject))]);
    const actual=await page.evaluate(()=>normalMapProbe.backend),expected=backend==='webgl'?'Gl':'BrowserWebGpu';
    if(actual!==expected)throw Error(`Backend fallback: expected ${expected}, got ${actual}`);
    for(const state of states)for(const mode of ['noMap','neutralRef','mapped']){
     const data=await page.evaluate(async({state,mode})=>normalMapProbe.capture(state,mode),{state,mode});
     images[label].set(`${JSON.stringify(state)}|${mode}`,await save(label,backend,state,mode,data));
    }
    await page.evaluate(()=>normalMapProbe.dispose());
    if(pageErrors.length)throw Error('Browser errors: '+JSON.stringify(pageErrors));
    if(server.errors().length)throw Error('HTTP errors: '+JSON.stringify(server.errors()));
   }catch(e){throw Error(`${String(e.stack||e)}\nPage errors: ${JSON.stringify(pageErrors)}`);}
   finally{await page.close();manifests.push({backend,label,files:server.manifest(),errors:server.errors()});server.forceClose();await server.close();server=null;}
  }
  let visibleOld=0,visibleMapped=0;
  for(const state of states){
   const key=JSON.stringify(state),baseNeutral=images.baseline.get(`${key}|neutralRef`),baseMapped=images.baseline.get(`${key}|mapped`),newNoMap=images.candidate.get(`${key}|noMap`),newNeutral=images.candidate.get(`${key}|neutralRef`),newMapped=images.candidate.get(`${key}|mapped`);
   const noMapNeutral=pixelDifference(newNoMap,newNeutral,{channelTolerance:0,pixelFractionTolerance:0});
   const neutralExact=pixelDifference(baseNeutral,newNeutral),mappedExact=pixelDifference(baseMapped,newMapped);
   const noMapBaseline=pixelDifference(images.baseline.get(`${key}|noMap`),newNoMap);
   if(stateMode==='pbr'&&noMapBaseline.maxChannelDelta>2)throw Error(`No-map baseline differs beyond 2/255: ${backend} ${key} ${JSON.stringify(noMapBaseline)}`);
   const oldEffect=pixelDifference(images.baseline.get(`${key}|noMap`),newNeutral,{channelTolerance:0,pixelFractionTolerance:0}),mapEffect=pixelDifference(newNeutral,newMapped);
   visibleOld+=oldEffect.changedPixels;visibleMapped+=mapEffect.changedPixels;
   results.push({backend,state,differences:{baselineNoMapVsCandidate:noMapBaseline,candidateNoMapVsNeutralReference:noMapNeutral,baselineNeutralReferenceVsCandidate:neutralExact,baselineMappedVsCandidate:mappedExact,baselineNoMapVsCandidateNeutralReference:oldEffect,candidateNeutralReferenceVsMapped:mapEffect}});
   if(noMapNeutral.maxChannelDelta>1)throw Error(`noMap vs neutral reference exceeds 1/255: ${backend} ${key} ${JSON.stringify(noMapNeutral)}`);
   if(stateMode==='pbr'?(neutralExact.maxChannelDelta>2||mappedExact.maxChannelDelta>2):(!neutralExact.exact||!mappedExact.exact))throw Error(`Full normal-map path differs: ${backend} ${key} ${JSON.stringify({neutralExact,mappedExact})}`);
  }
  if((stateMode!=='pbr'&&!visibleOld)||!visibleMapped)throw Error(`Oracle is visually blank: ${backend} oldEffect=${visibleOld}, mappedEffect=${visibleMapped}`);
  console.log(JSON.stringify({backend,states:states.length,visibleOldNoMapPixels:visibleOld,visibleMappedPixels:visibleMapped}));
 }
}catch(e){errors.push(String(e.stack||e));process.exitCode=1;}
finally{
 if(browser)await browser.close();if(server){server.forceClose();await server.close();}
 const after=await identities(),changed=Object.keys(before).filter(f=>before[f]!==after[f]);if(changed.length){errors.push('Inputs changed during verification: '+changed.join(', '));process.exitCode=1;}
 const report={version:1,stateMode,scope:'Generated glTF normal-map oracle on frozen baseline and candidate releases; both actual WebGPU/WebGL backends. Candidate no-map must match neutral-reference scale=0 within max 1/255; neutral and mapped paths must match baseline exactly (pbr mode: max2/255 predeclared, including grazing/roughness.05 controls). Not a Quest or performance claim.',stateCount:states.length,modes:['noMap','neutralRef','mapped'],completedBackends:['webgpu','webgl'].filter(backend=>results.filter(r=>r.backend===backend).length===states.length),before,after,changed,errors,manifests,results};
 await fs.writeFile(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n');
}
