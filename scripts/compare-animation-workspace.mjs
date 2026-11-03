#!/usr/bin/env node
// Exact diagnostic captures, outside timing. Reuses maintained frozen server/browser helpers.
import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import puppeteer from 'puppeteer';
import {PNG} from 'pngjs';
import {serveFrozen,pixelDifference,boundedCleanup} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
const [baseline,candidate,outArg]=process.argv.slice(2);if(!baseline||!candidate||!outArg)throw Error('Usage: node scripts/compare-animation-workspace.mjs BASELINE CANDIDATE NEW_OUTPUT');
const root=process.cwd(),out=path.resolve(outArg);await fs.mkdir(out);const report={status:'RUNNING',scope:'exact fixed-step hot animation pixels; desktop/simulated XR, no Quest proof',runs:[],pairs:[],cleanup:[]};
let browser;
try{
 const executablePath=process.env.BROWSER_EXECUTABLE||execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();
 browser=await puppeteer.launch({executablePath,headless:true,args:browserArgs()});report.browser=await browser.version();
 for(const backend of ['webgpu','webgl']){
  const evidence=[];
  for(const [label,source] of [['baseline',baseline],['candidate',candidate]]){
   const server=await serveFrozen(source,root);let page;
   try{
    page=await browser.newPage();const errors=[];page.on('pageerror',error=>errors.push(String(error)));
    page.on('console',message=>{if(message.type()==='error')errors.push(message.text());});
    await page.goto(server.url+'/tests/animation-workspace.html?backend='+backend,{waitUntil:'networkidle0',timeout:120000});
    await page.waitForFunction(()=>Boolean(window.animationWorkspaceEvidence),{timeout:120000});
    const data=await page.evaluate(()=>({evidence:window.animationWorkspaceEvidence,status:document.querySelector('#result').textContent,backend:document.documentElement.dataset.rendererBackend}));
    if(errors.length||!data.status.startsWith('PASS'))throw Error(JSON.stringify({errors,...data}));
    const expected=backend==='webgpu'?'BrowserWebGpu':'Gl';if(data.backend!==expected)throw Error('Backend fallback');
    for(const capture of data.evidence.captures)for(const [eye,png] of capture.images.entries())await fs.writeFile(path.join(out,`${backend}-${label}-${capture.name}-${capture.frame}-${eye}.png`),Buffer.from(png.split(',')[1],'base64'));
    evidence.push(data.evidence);report.runs.push({label,backend,manifest:server.manifest(),status:data.status});
   }finally{if(page)await page.close();await server.close();}
  }
  for(let i=0;i<evidence[0].captures.length;i++){
   const a=evidence[0].captures[i],b=evidence[1].captures[i];if(JSON.stringify(a.playback)!==JSON.stringify(b.playback))throw Error('Capture time/state mismatch');
   for(let eye=0;eye<2;eye++){
    const before=PNG.sync.read(Buffer.from(a.images[eye].split(',')[1],'base64')),after=PNG.sync.read(Buffer.from(b.images[eye].split(',')[1],'base64'));
    const diff=pixelDifference(before,after,{channelTolerance:0,pixelFractionTolerance:0});report.pairs.push({backend,name:a.name,frame:a.frame,eye,...diff});
    if(!diff.exact)throw Error('Strict animation pixels differ');
   }
  }
 }
 report.status='PASS';
}catch(error){report.status='FAIL';report.error=String(error.stack||error);process.exitCode=1;}
finally{if(browser)report.cleanup.push(await boundedCleanup(()=>browser.close(),{label:'browser',force:()=>browser.process()?.kill('SIGKILL')}));await fs.writeFile(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({status:report.status,pairs:report.pairs.length,error:report.error}));}
