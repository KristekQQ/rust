import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
test('benchmark refuses existing evidence before opening a browser or changing files',async()=>{
 const temporary=await fs.mkdtemp(path.join(os.tmpdir(),'render-evidence-'));
 try{
  const out=path.join(temporary,'previous');await fs.mkdir(out);await fs.writeFile(path.join(out,'report.json'),'previous unfavorable evidence');
  const result=spawnSync(process.execPath,['scripts/render-performance.mjs','smoke','--baseline-root',temporary,'--out',out],{encoding:'utf8',timeout:15000});
  assert.notEqual(result.status,0);assert.match(result.stderr,/Refusing existing benchmark output/);
  assert.equal(await fs.readFile(path.join(out,'report.json'),'utf8'),'previous unfavorable evidence');
  assert.deepEqual(await fs.readdir(out),['report.json']);
 }finally{await fs.rm(temporary,{recursive:true,force:true});}
});
