#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import puppeteer from 'puppeteer';
import {PNG} from 'pngjs';
import {serveFrozen,pixelDifference,hash} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
const [baseline,candidate,outArg,qualityMode="exact"]=process.argv.slice(2);if(!["exact","pcf4","pcf-uniform","zero-light"].includes(qualityMode))throw Error("Invalid quality mode");if(!outArg)throw Error('Usage: node scripts/verify-shadow-filter.mjs BASELINE CANDIDATE NEW_OUTPUT [exact|pcf4|pcf-uniform]');
const out=path.resolve(outArg);await fs.mkdir(out,{recursive:false});
const paths=['scripts/verify-shadow-filter.mjs','scripts/render-performance-lib.mjs','scripts/browser-options.mjs','tests/shadow-filter-probe.html','tests/shadow-filter-probe.js','tests/pixels.js'];
const identities=async()=>Object.fromEntries(await Promise.all(paths.map(async f=>[f,hash(await fs.readFile(f))]))),before=await identities(),results=[],errors=[],manifests=[];
let browser,server;
try{
 const executablePath=execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();browser=await puppeteer.launch({headless:true,executablePath,protocolTimeout:60000,args:browserArgs()});
 for(const backend of ['webgpu','webgl']){
  const images=new Map();
  for(const [label,root] of [['baseline',baseline],['candidate',candidate]]){
   server=await serveFrozen(root,process.cwd());const page=await browser.newPage(),pageErrors=[];
   try{
    page.on('pageerror',e=>pageErrors.push(String(e)));page.on('console',m=>{if(m.type()==='error')pageErrors.push(m.text())});page.on('response',r=>{if(r.status()>=400)pageErrors.push(r.status()+' '+r.url())});
    await page.setViewport({width:640,height:400,deviceScaleFactor:1});await page.goto(server.url+'/tests/shadow-filter-probe.html?backend='+backend,{waitUntil:'domcontentloaded',timeout:60000});await page.waitForFunction('!!window.shadowProbe',{timeout:60000});
    if(await page.evaluate(()=>shadowProbe.backend)!==(backend==='webgl'?'Gl':'BrowserWebGpu'))throw Error('Backend fallback');
    const cases=[];for(const bias of [0,.12])for(let index=0;index<4;index++)for(let mask=0;mask<4;mask++)cases.push({bias,index,mask,mode:"both"});for(const mode of ["primitive","gltf"])for(let index=0;index<4;index++)for(const mask of [0,3])cases.push({bias:.12,index,mask,mode});
    for(const legacy of [false,true])for(const pattern of ['all','alternating'])for(const metallic of [false,true])for(const shadowCount of [0,1,4])cases.push({bias:.12,index:0,mask:3,mode:'both',options:{legacy,pattern,metallic,lightCount:shadowCount||4,shadows:shadowCount!==0},tag:`-legacy${legacy}-${pattern}-metal${metallic}-shadows${shadowCount}`});
    if(qualityMode==='pcf4'||qualityMode==='pcf-uniform'){
     for(const legacy of [false,true])for(const pattern of ['all','alternating'])for(const metallic of [false,true])for(const shadowCount of [0,1,4])for(const mask of [1,2])cases.push({bias:.12,index:0,mask,mode:'both',options:{legacy,pattern,metallic,lightCount:shadowCount||4,shadows:shadowCount!==0},tag:`-legacy${legacy}-${pattern}-metal${metallic}-shadows${shadowCount}`});
     for(const offset of [0,.13,.37])for(const mask of [1,2,3])cases.push({bias:.12,index:0,mask,mode:'both',options:{offset,rotation:[.1,.27,.15]},tag:`-moving${offset}-diagonal`});
    }
    if(qualityMode==='pcf4'||qualityMode==='pcf-uniform'){
     const key=c=>`${c.bias}|${c.index}|${c.mask}|${c.mode}|${c.tag||''}`,seen=new Set(cases.map(key));
     for(const c of [...cases])if(c.mask!==0&&c.options?.shadows!==false){const lit={...c,mask:0};if(!seen.has(key(lit))){seen.add(key(lit));cases.push(lit);}}
    }
    if(qualityMode==='zero-light')for(const legacy of [false,true])for(const metallic of [false,true])for(const lightElevation of [-6,-.001,0,.001,6])for(const shadows of [false,true])for(const mask of [1,2,3])cases.push({bias:.12,index:0,mask,mode:'both',options:{legacy,metallic,lightElevation,lightCount:1,shadows},tag:`-zero-light-legacy${legacy}-metal${metallic}-elevation${lightElevation}-shadows${shadows}`});
    for(const {bias,index,mask,mode,options={},tag=''} of cases){
     const name=`${backend}-bias${bias}-light${index}-layers${mask}-${mode}${tag}`,data=await page.evaluate(async({index,mask,bias,mode,options})=>shadowProbe.capture(index,mask,bias,mode,options),{index,mask,bias,mode,options}),bytes=Buffer.from(data.png.split(',')[1],'base64'),image=PNG.sync.read(bytes);await fs.writeFile(path.join(out,`${label}-${name}.png`),bytes);
     if(label==='baseline')images.set(name,image);else{const difference=pixelDifference(images.get(name),image);results.push({name,settings:{index,mask,bias,mode,options,tag},stats:data.stats,difference});if(qualityMode==='pcf4'?difference.maxChannelDelta>8:!difference.exact)throw Error('Pixel mismatch '+name+' '+JSON.stringify(difference));}
    }
    await page.evaluate(()=>shadowProbe.dispose());if(pageErrors.length)throw Error(JSON.stringify(pageErrors));if(server.errors().length)throw Error(JSON.stringify(server.errors()));
   }catch(e){throw Error(String(e.stack||e)+"\nPage errors: "+JSON.stringify(pageErrors));}finally{await page.close();manifests.push({backend,label,files:server.manifest(),errors:server.errors()});server.forceClose();await server.close();server=null;}
  }
  // The baseline itself must exhibit shadowing, not merely match two blank paths.
  for(const bias of [0,.12])for(let index=0;index<4;index++)for(let mask=1;mask<4;mask++){
   const a=images.get(`${backend}-bias${bias}-light${index}-layers0-both`),b=images.get(`${backend}-bias${bias}-light${index}-layers${mask}-both`),d=pixelDifference(a,b);if(d.changedPixels<50)throw Error('No visible baseline shadow '+JSON.stringify({backend,bias,index,mask,d}));
  }
  for(const mode of ['primitive','gltf'])for(let index=0;index<4;index++){const d=pixelDifference(images.get(`${backend}-bias0.12-light${index}-layers0-${mode}`),images.get(`${backend}-bias0.12-light${index}-layers3-${mode}`));if(d.changedPixels<50)throw Error('Missing shadow on receiver '+JSON.stringify({backend,mode,index,d}));}
  for(let index=1;index<4;index++){const d=pixelDifference(images.get(`${backend}-bias0.12-light0-layers3-both`),images.get(`${backend}-bias0.12-light${index}-layers3-both`));if(d.changedPixels<50)throw Error('Light indices do not select distinguishable maps '+index);}
  console.log(JSON.stringify({backend,cases:results.filter(r=>r.name.startsWith(backend+'-')).length,exact:results.filter(r=>r.name.startsWith(backend+'-')&&r.difference.exact).length}));
 }
}catch(e){errors.push(String(e.stack||e));process.exitCode=1;}
finally{if(browser)await browser.close();if(server){server.forceClose();await server.close();}const after=await identities(),changed=Object.keys(before).filter(f=>before[f]!==after[f]);if(changed.length)process.exitCode=1;await fs.writeFile(path.join(out,'report.json'),JSON.stringify({qualityMode,qualityGate:qualityMode==='pcf4'?'all probe pixels maxchannel<=8/255; separate shadow-edge RMSE gate required':'all probe pixels exactly equal',scope:'Same-renderer frozen-release pixel oracle: four distinct lights, masks0/1/2/3, normalbias0/.12, primitive/glTF receivers; simultaneous colored/alternating lights, shadowcount0/1/4, legacy/authored, dielectric/metallic. Not timing or Quest proof.',before,after,changed,errors,manifests,results},null,2)+'\n');}
