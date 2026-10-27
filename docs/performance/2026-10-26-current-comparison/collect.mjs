import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {assertFreshBuild} from '../../../scripts/build-fingerprint.mjs';
import {assertProductionBuild} from '../../../scripts/build-production.mjs';
const root=process.cwd(),source=path.join(root,'output/render-performance-current-vs-three'),out=path.join(root,'docs/performance/2026-10-26-current-comparison');
const names=['gl-vs-three','webgpu-0','webgpu-1','webgpu-2','normal-gl-three','normal-webgpu'];
const reports={};await fs.mkdir(path.join(out,'evidence'),{recursive:true});
for(const name of names){
 const dir=path.join(source,name),r=JSON.parse(await fs.readFile(path.join(dir,'report.json'),'utf8'));reports[name]=r;
 assert.deepEqual(r.driverInputsChangedDuringRun,[]);
 assert.deepEqual(r.serverErrors.baseline,[]);
 if(r.candidateRoot)assert.deepEqual(r.serverErrors.candidate,[]);else assert.equal(r.serverErrors.candidate,null);
 assert(r.cleanup.every(item=>item.complete));
 for(const row of r.results){assert.equal(row.profiling,false);assert(row.rafInterval.count>100);assert(row.renderer.drawCalls>0);}
 const dest=path.join(out,'evidence',name);await fs.mkdir(dest,{recursive:true});
 for(const file of await fs.readdir(dir))if(/\.json(?:\.gz)?$/.test(file))await fs.copyFile(path.join(dir,file),path.join(dest,file));
}
const median=a=>a.slice().sort((x,y)=>x-y)[Math.floor(a.length/2)];
const summary=rows=>{const fps=rows.map(r=>1000/r.rafInterval.mean);return {runs:rows.length,fpsEach:fps,medianFps:median(fps),minFps:Math.min(...fps),maxFps:Math.max(...fps),cpuMedianMs:median(rows.map(r=>r.rafCallbackCpu.median)),frameP95EachMs:rows.map(r=>r.rafInterval.p95)};};
const high=reports['gl-vs-three'];assert.equal(high.results.length,6);assert.equal(high.settings.width,4096);assert.equal(high.settings.height,2730);
const highRows={rustWebGL:summary(high.results.filter(r=>r.label==='baseline')),threeWebGL:summary(high.results.filter(r=>r.label==='candidate')),rustWebGPU:summary([0,1,2].flatMap(i=>reports['webgpu-'+i].results))};
for(const r of [0,1,2].map(i=>reports['webgpu-'+i])){assert.equal(r.settings.width,4096);assert.equal(r.settings.height,2730);assert.equal(r.settings.warmup,8000);assert.equal(r.settings.sample,25000);}
const normal=reports['normal-gl-three'];assert.equal(normal.results.length,2);assert.equal(normal.settings.width,2880);assert.equal(normal.settings.height,1920);
const normalRows={rustWebGL:summary(normal.results.filter(r=>r.label==='baseline')),threeWebGL:summary(normal.results.filter(r=>r.label==='candidate')),rustWebGPU:summary(reports['normal-webgpu'].results)};
const build=await assertFreshBuild(),production=await assertProductionBuild(path.join(root,'output/production'),{checkCurrentSource:true});assert.equal(build.wasm,'6c4bc5dd9e4e98c0ad42ebbc52aec25034511fb7ba75068c07fcc414b597219e');assert.equal(production.build.wasm,build.wasm);assert.equal(production.releaseId,'de4fef8a1a');
const result={date:new Date().toISOString(),build,productionApp:production.releaseId,scope:'Apple M1 desktop headless real Rust renderer and original Three r170; static actual Rome gate view. Native release WASM equals deployed production. Benchmark runtime source is unminified; production minifies those same source modules. Not active combat or Quest evidence.',fpsDefinition:'Per-run FPS = 1000 / mean RAF interval; table uses median of those per-run FPS. Observed ~60Hz ceiling does not reveal additional throughput headroom.',protocol:{high:'3 GL/Three AB/BA/AB pairs,8s warmup/25s samples,profiler off;3 separate native WebGPU runs with same8s/25s protocol; WebGPU is sequential after GL/Three, not a three-way interleaving.',normal:'1 short control per implementation,3s warmup/8s sample; capped, not a ranking.'},high:{width:4096,height:2730,...highRows,webGLFpsDeficitPercent:(1-highRows.rustWebGL.medianFps/highRows.threeWebGL.medianFps)*100},normal:{width:2880,height:1920,...normalRows},limits:['Same authored city/camera/resolution, but image equivalence is not established: Three MSAA4 vs native FXAA, PBR/shadow policies and runtime differ.','Static gate-view diagnostic does not exercise current moving NPCs, their new blobs, combat or cuts.','Desktop RAF cadence is not physical Quest or compositor presentation measurement.','Native lower callback CPU does not identify a specific GPU bottleneck.','No new source optimization was introduced during this measurement; do not attribute run-to-run changes against older a54 results to NPC blobs.']};
await fs.writeFile(path.join(out,'summary.json'),JSON.stringify(result,null,2)+'\n');
await fs.copyFile(path.join(source,'reference-current.json'),path.join(out,'evidence/reference-current.json'));
await fs.copyFile('/tmp/rim-current-followups.sh',path.join(out,'followups.sh'));
console.log(JSON.stringify({high:result.high,normal:result.normal},null,2));
