from pathlib import Path
import json,gzip,hashlib,shutil,statistics,subprocess
b=Path('output/render-performance-rim-results');d=Path('docs/performance/2026-10-26-rim')
plan=json.loads((b/'plan.json').read_text());commands=json.loads((b/'commands.json').read_text());v=json.loads((d/'view-data.json').read_text())
limit='Tento přehled porovnává původní a současný Rust renderer. Neobsahuje plně shodný herní test Říma proti Three.js.'
if limit not in v['limits']:v['limits'].append(limit)
(d/'view-data.json').write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
subprocess.run(['node',str(d/'build-view.mjs'),str(d/'view-data.json'),str(d/'index.html')],check=True)
lines=['# Actual Rome renderer progress — 2026-10-26','','The real authored `examples/rim` does not yet demonstrate a substantial improvement. Small CPU changes are mixed across repetitions. The large improvements on thousands of synthetic instances do not transfer automatically to this scene.','','## Matched primary measurements','','| Backend / workload | CPU medians before (ms) | CPU medians after (ms) | Median paired after/before ratio |','|---|---|---|---|']
for s in v['scenes']:lines.append('| '+s['backend']+' / '+('static' if 'static' in s['id'] else 'moving enemies')+' | '+' / '.join(f'{n:.2f}' for n in s['cpuBefore'])+' | '+' / '.join(f'{n:.2f}' for n in s['cpuAfter'])+f" | {s['cpuRatio']:.4f} |")
lines+=['','Three AB/BA/AB pairs per backend/workload; 8 s warmup and 25 s measurement, profiler off. Identical 960 × 640 DPR 1 viewport, 15 runtime/asset hashes, authored appearance and native gameplay source. CPU covers application RAF work, including simulation and render preparation; it excludes between-RAF timers/workers. The capped 60 Hz cadence stayed unchanged. No capacity or physical Quest gain is established.','','Predeclared meaningful-improvement criterion: at least 5% and 0.2 ms CPU median reduction, with all three pairs improving. **None of the four scenarios passes all criteria.** Preserve the adverse WebGPU static pair and WebGL2 moving-enemy pair.','','## Actual workload and image evidence','','Static comparisons: all six pixel gates pass. All three WebGPU pairs and one WebGL2 pair are exact; the other two WebGL2 pairs differ by at most 1/255 in a channel, with no pixels exceeding the original tolerance of 8.','','Active comparisons: native frame counters advance and six enemies remain in motion. Recorded fired-projectile deltas are zero; beam activity is not separately counted. This is a movement/animation trace, not a firefight, sword or cut benchmark. Identical time-based input does not guarantee identical simulation/capture state, so active image parity is unavailable. Real images are shown without an invented parity result.','','The fixture imports actual `scene.js`, `appearance.js` and native gameplay, excluding the desktop DOM controls in `main.js`. Resolution, assets and quality were preserved. The old glTF triangle counter is unavailable, not zero.','','## Separate GPU diagnostics','','| Backend / workload | Before (ms) | After (ms) | Scope |','|---|---:|---:|---|']
for s in v['scenes']:lines.append('| '+s['backend']+' / '+('static' if 'static' in s['id'] else 'moving enemies')+f" | {s['gpuBefore']:.2f} | {s['gpuAfter']:.2f} | "+('WebGPU scene.main pass' if s['backend']=='WebGPU' else 'WebGL EXT command interval')+' |')
lines+=['','One separate profiled 2 s/8 s run per build/workload/backend, with separate API diagnostics. WebGPU values cover only `scene.main`; GL command intervals can include CPU/driver gaps and are not pure GPU busy time. These are different scopes, not whole GPU-frame time; do not add CPU and GPU numbers or claim a GPU improvement. Last pending GL queries may be explicitly discarded at the measurement boundary; invalid/disjoint samples are excluded.','','## Identity, limits and delivery','','Original release WASM: `'+plan['baselineWasm']+'`.','','Retained release WASM: `'+plan['candidateWasm']+'`.','','No Rust, SDK, gameplay or appearance changes in this measurement task. Boundary diagnostics run outside the timed window. Live ports 8443/8878 were preserved; no commit, push or deployment. External GPU load/temperatures were not verified. These are desktop measurements, not physical Quest evidence. This report is old/new Rust, not full-game Rome parity against Three.js.','','## Browse and reproduce','','[Interactive results and actual image slider](index.html) · [Structured results, per-pair tails and native state](view-data.json) · [Predeclared plan and file identities](plan.json) · [Exact commands](commands.json) · [UI checks](view-verification.json) · [Archive validation](validation.json).','','Each named command directory retains the full compressed report, unchanged raw samples, actual served manifests and every capture. Measurement source is in `measurement-source.tar.gz`; `run-series.py` replays command construction. Three.js reference-control evidence remains in [the earlier technique-transfer report](../2026-10-25-three-transfer/REPORT.md); those controls are not matched whole-game Rome comparisons.','','Next useful experiment: profile this actual scene at representative headset resolution and isolate the limiting glTF/shader/preparation scope before another renderer change. Preserve quality and recheck the same paired workloads. Physical Quest measurement remains separate.']
(d/'REPORT.md').write_text('\n'.join(lines)+'\n')
static=[];active=0;rows=0;profiles=0;pending=[]
for cmd in commands:
 n=cmd['name'];x=json.loads((b/n/'report.json').read_text());assert cmd['exitCode']==0 and not x.get('error') and not x.get('driverInputsChangedDuringRun') and all(not e for e in x['serverErrors'].values())
 assert gzip.decompress((d/n/'report.json.gz').read_bytes())==(b/n/'report.json').read_bytes()
 assert (d/n/'raw.json.gz').read_bytes()==(b/n/'raw.json.gz').read_bytes();json.loads(gzip.decompress((d/n/'raw.json.gz').read_bytes()))
 for r in x['results']:
  assert not r['gpuError']
  for source in ['manifest-baseline.json','manifest-candidate.json','config.json']:
   if (b/n/source).exists():assert (b/n/source).read_bytes()==(d/n/source).read_bytes()
  f=r['capture']['file'];assert (b/n/f).read_bytes()==(d/n/f).read_bytes()
  if n.startswith('paired-'):
   rows+=1;assert r['profiling'] is False and r['glRafGpu']['queriesCreated']==0
   expected=plan['baselineWasm'] if r['label']=='baseline' else plan['candidateWasm'];assert x[r['label']+'BuildFingerprint']['wasm']==expected
   assert r['backend']==('Gl' if n.endswith('-gl') else 'BrowserWebGpu')
   if 'active' in n:
    p=r['phaseEvidence'];assert p['end']['native']['frame']['simulation']>p['start']['native']['frame']['simulation'];assert p['end']['native']['active'];assert len(p['end']['native']['enemies'])==6
  if n.startswith('profile-'):
   profiles+=1;assert r['profiling'] is True
   if n.endswith('-gl'):
    g=r['glRafGpu'];assert g['supported'] and not g['invalid'] and g['disjointDiscarded']==0 and g['queriesCreated']==g['queriesDeleted'];pending.append({'name':n,'incomplete':g['incomplete']})
 if n.startswith('paired-'):
  for p in x['comparisons'][0]['paired']:
   if 'static' in n:assert p['pixel']['pass'];static.append(p['pixel'])
   else:assert p['pixel']['pass'] is None;active+=1
assert len(commands)==16 and rows==24 and profiles==8 and len(static)==6 and active==6
ui=json.loads((d/'view-verification.json').read_text());assert len(ui['checks'])==4 and not ui['pageErrors'] and ui['mobileOverflow'] is False
validation={'scope':'actual authored Rome original/current desktop; no new renderer implementation/deployment','commandsExitZero':16,'primaryMeasuredRuns':24,'profileRuns':8,'staticPixelPairs':6,'staticExactPairs':sum(p['exact'] for p in static),'staticMaxChannelDelta':max(p['maxChannelDelta'] for p in static),'activePixelParityUnavailablePairs':active,'archivedReportsAndRawEqualToOriginal':16,'uiModesPass':4,'mobileNoOverflow':True,'imageOverlayGeometryMatches':True,'mainProfilingOff':True,'toolingTestsPreviouslyPassed':26,'glProfileBoundaryDiscards':pending,'physicalQuestMeasured':False,'threeWholeGameParity':False,'noNewBuildOrDeployment':True}
(d/'validation.json').write_text(json.dumps(validation,indent=2)+'\n')
for n in ['verify-view.mjs','finalize-results.py']:shutil.copyfile(b/n,d/n)
manifest={str(p.relative_to(d)):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(d.rglob('*')) if p.is_file() and p.name!='archive-manifest.json'}
(d/'archive-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
assert all((d/n).stat().st_size==v['bytes'] and hashlib.sha256((d/n).read_bytes()).hexdigest()==v['sha256'] for n,v in manifest.items())
print(json.dumps({'validation':'PASS','files':len(manifest),'bytes':sum(v['bytes'] for v in manifest.values()),'primaryRuns':rows,'profiles':profiles,'staticExactPairs':sum(p['exact'] for p in static),'uiModes':4}))
