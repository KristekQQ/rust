#!/usr/bin/env node
// Independent nonblank controls over already captured baseline images; no GPU run.
import fs from 'node:fs/promises';
import path from 'node:path';
import {PNG} from 'pngjs';
import {pixelDifference,hash} from './render-performance-lib.mjs';
const root=path.resolve(process.argv[2]||''),input=JSON.parse(await fs.readFile(path.join(root,'report.json'),'utf8'));if(input.errors.length||input.changed.length)throw Error('Only a completed stable oracle can be checked');
const results=[],cache=new Map();
async function image(name){if(!cache.has(name)){const bytes=await fs.readFile(path.join(root,'baseline-'+name+'.png'));cache.set(name,{image:PNG.sync.read(bytes),sha256:hash(bytes)});}return cache.get(name);}
for(const backend of ['webgpu','webgl']){
 const name=(legacy,pattern,metallic,shadows)=>`${backend}-bias0.12-light0-layers3-both-legacy${legacy}-${pattern}-metal${metallic}-shadows${shadows}`;
 for(const legacy of [false,true])for(const metallic of [false,true])for(const [label,first,second] of [
  ['multiple colored vs alternating lights',name(legacy,'all',metallic,4),name(legacy,'alternating',metallic,4)],
  ['same four lights, shadows on vs off',name(legacy,'all',metallic,4),name(legacy,'all',metallic,0)],
  ['one active light vs one zero-color light',name(legacy,'all',metallic,1),name(legacy,'alternating',metallic,1)]
 ]){const a=await image(first),b=await image(second),difference=pixelDifference(a.image,b.image);results.push({backend,label,first,second,hashes:[a.sha256,b.sha256],difference});}
 for(const metallic of [false,true]){const first=name(false,'all',metallic,4),second=name(true,'all',metallic,4),a=await image(first),b=await image(second);results.push({backend,label:'authored sun vs legacy lighting',first,second,hashes:[a.sha256,b.sha256],difference:pixelDifference(a.image,b.image)});}
 for(const legacy of [false,true]){const first=name(legacy,'all',false,4),second=name(legacy,'all',true,4),a=await image(first),b=await image(second);results.push({backend,label:'dielectric vs metallic glTF receiver',first,second,hashes:[a.sha256,b.sha256],difference:pixelDifference(a.image,b.image)});}
}
const failures=results.filter(r=>r.difference.changedPixels<50),report={sourceReportSha256:hash(await fs.readFile(path.join(root,'report.json'))),scope:'Post-capture visible-work controls, NOT new timing or cross-engine parity. Shadowcount1 case has one total light; not four lights with only one shadow caster.',pass:failures.length===0,results};await fs.writeFile(path.join(root,'negative-controls.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({checks:results.length,failures:failures.map(r=>({backend:r.backend,label:r.label,changed:r.difference.changedPixels}))}));if(failures.length)process.exitCode=1;
