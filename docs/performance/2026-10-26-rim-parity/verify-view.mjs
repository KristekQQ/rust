import fs from 'node:fs/promises';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {serveFrozen} from '../../../scripts/render-performance-lib.mjs';
import {browserArgs} from '../../../scripts/browser-options.mjs';
import puppeteer from 'puppeteer';
const root=process.cwd(),folder=path.join(root,'docs/performance/2026-10-26-rim-parity'),server=await serveFrozen(folder,root),errors=[];
let executablePath;try{executablePath=execFileSync('git',['config','--local','--get','verification.browser'],{encoding:'utf8'}).trim();}catch{}
const browser=await puppeteer.launch({headless:true,executablePath,args:browserArgs()});
try{
 const page=await browser.newPage();await page.setViewport({width:1280,height:1050,deviceScaleFactor:1});page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error')errors.push(m.text())});
 await page.goto(server.url+'/',{waitUntil:'networkidle0'});
 const model=JSON.parse(await fs.readFile(path.join(folder,'view-data.json'),'utf8'));const checks=[];
 for(const s of model.scenes){
  await page.select('#backend',s.backend);await page.select('#mode',s.mode);await page.waitForFunction(()=>[...document.querySelectorAll('#images img')].every(i=>i.complete&&i.naturalWidth>0));
  const actual=await page.evaluate(()=>({rows:document.querySelectorAll('#pairs tr').length,pixel:document.querySelector('#images .status')?.textContent,caption:document.querySelector('#caption').textContent,images:[...document.querySelectorAll('#images img')].map(i=>[i.naturalWidth,i.naturalHeight]),bounds:[...document.querySelectorAll('#images img')].map(i=>{const r=i.getBoundingClientRect();return [r.x,r.y,r.width,r.height]})}));
  if(actual.bounds[0].some((v,i)=>Math.abs(v-actual.bounds[1][i])>1))throw Error('Before/after screenshot geometry differs '+s.id);
  if(s.pixel.pass===true&&!actual.pixel.includes('nad tolerancí'))throw Error('Pixel count lacks tolerance qualification '+s.id);
  if(actual.rows!==s.pairs.length||actual.images.length!==2)throw Error('Incomplete scene UI '+s.id);
  if(s.pixel.pass===null&&!actual.pixel.includes('není vyhodnocená'))throw Error('Unavailable pixel parity presented as failure '+s.id);
  const before=await page.$eval('#images .overlay',e=>getComputedStyle(e).clipPath);await page.$eval('#images input',e=>{e.value='90';e.dispatchEvent(new Event('input',{bubbles:true}))});const after=await page.$eval('#images .overlay',e=>getComputedStyle(e).clipPath);if(before===after)throw Error('Image slider did not update');checks.push({id:s.id,...actual,sliderChanged:true});
 }
 await page.select('#backend',model.scenes[0].backend);await page.select('#mode',model.scenes[0].mode);await page.waitForFunction(()=>[...document.querySelectorAll('#images img')].every(i=>i.complete&&i.naturalWidth>0));
 await page.screenshot({path:path.join(folder,'report-desktop.png'),fullPage:true});
 await page.screenshot({path:path.join(folder,'report-summary.png')});
 await page.setViewport({width:390,height:844,deviceScaleFactor:1});const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1);await page.screenshot({path:path.join(folder,'report-mobile.png'),fullPage:true});if(overflow)throw Error('Report overflows mobile viewport: '+JSON.stringify(await page.evaluate(()=>[...document.querySelectorAll('body *')].map(e=>({tag:e.tagName,id:e.id,width:e.getBoundingClientRect().width,right:e.getBoundingClientRect().right})).filter(r=>r.right>innerWidth+1).slice(0,8))));
 if(errors.length||server.errors().length)throw Error(JSON.stringify({errors,serverErrors:server.errors()}));
 await fs.writeFile(path.join(folder,'view-verification.json'),JSON.stringify({checks,mobileOverflow:false,pageErrors:errors,scope:'actual standalone results UI, not renderer/headset performance'},null,2)+'\n');
 console.log(JSON.stringify({uiChecks:'PASS',scenes:checks.length,mobileOverflow:false,url:server.url}));
}finally{await browser.close();}
console.log('REPORT_URL '+server.url+'/');
// The local report stays available for the user; no game/runtime is served here.
