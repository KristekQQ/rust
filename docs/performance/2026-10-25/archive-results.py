import pathlib,json,gzip,shutil,hashlib,tarfile,subprocess,statistics
b=pathlib.Path('output/render-performance');dest=pathlib.Path('docs/performance/2026-10-25');dest.mkdir(parents=True,exist_ok=True)
series=json.loads((b/'series.json').read_text())
extra=json.loads((b/'extra-series.json').read_text()) if (b/'extra-series.json').exists() else []
v2=json.loads((b/'v2-controls-series.json').read_text()) if (b/'v2-controls-series.json').exists() else []
post=json.loads((b/'post-series.json').read_text()) if (b/'post-series.json').exists() else []
commands=series+v2+extra+post
names=['calibration-final-wg']+[x['name'] for x in commands]+['candidate-functional-browser-report','smoke-gl-timer','baseline-calibrate-wg','final-calibrate-wg','smoke-dynamic-detail','v2-alternating-profile','v3-alternating-pilot','v3-alternating-baseline-control','v4-alternating-pilot','v5-alternating-pilot']
summary=[]
for name in dict.fromkeys(names):
 p=b/name
 if not (p/'report.json').exists(): continue
 data=json.loads((p/'report.json').read_text());out=dest/name;out.mkdir(exist_ok=True)
 (out/'report.json.gz').write_bytes(gzip.compress((p/'report.json').read_bytes(),mtime=0))
 for file in ['raw.json.gz','manifest-baseline.json','manifest-candidate.json','config.json']:
  if (p/file).exists():shutil.copyfile(p/file,out/file)
 rows=[]
 raw=json.loads(gzip.decompress((p/'raw.json.gz').read_bytes())) if (p/'raw.json.gz').exists() else []
 for r in data.get('results',[]):
  rows.append({k:r.get(k) for k in ['id','label','load','pair','coldReadyMs','coldTimingScope','metadata','startupProgress','rafInterval','rafCallbackCpu','glRafGpuMs','glRafGpu','gpuPassMs','gpuSamples','cpuScopes','sampledCpuScopeMs','work','commands','heapBytes','apiOperations','actualQualityState','backend','profiling','gpuSupported','gpuError','phaseEvidence','missedFrameFraction','renderer','debug','frames','elapsedMs','capture']})
 for row in rows:
  run=next((v for v in raw if v['id']==row['id']),{})
  values=[v['renderer']['uploadBytes'] for v in run.get('samples',[]) if isinstance((v.get('renderer') or {}).get('uploadBytes'),(int,float))]
  row['sampledRendererUploadBytes']={'median':statistics.median(values) if values else None,'samples':len(values),'scope':'engine per-frame uploadBytes gauge, sampled approximately10Hz; not physical bus traffic or all-frame distribution'}
 # Keep primary pair0 and highest-load stress images. All image hashes stay in reports.
 selected=[r for r in data.get('results',[]) if r.get('pair')==0]
 if name.startswith(('final-paired-off-','paired-off-','final-three-geometry','final-three-objects','final-dynamic-paired-off')):selected=data.get('results',[])
 elif len(selected)>2:selected=[selected[0],selected[-1]]
 for r in selected:
  file=r.get('capture',{}).get('file')
  if file and (p/file).exists():shutil.copyfile(p/file,out/file)
 compact={k:data.get(k) for k in ['version','date','command','settings','driverIdentity','browser','hardware','baselineBuildFingerprint','candidateBuildFingerprint','calibration','comparisons','failures','error','driverInputsChangedDuringRun','serverErrors','durationSeconds','measurementProtocol','profilerOffControl','comparisonKind','threeReference','cleanup','cleanupFailure','calibrationValidation']};compact['results']=rows;compact['processEvidence']=next((x for x in reversed(commands) if x['name']==name),None)
 (out/'summary.json').write_text(json.dumps(compact,separators=(',',':'))+'\n');summary.append({'name':name,'results':len(rows),'error':data.get('error'),'directory':name})
for file in ['candidate-functional-browser-report.json','final-functional-browser-report.json','barrier-diagnostic-failure.json','driver-v1.tar.gz','driver-v2.tar.gz','driver-v3.tar.gz','driver-v4.tar.gz','driver-v5.tar.gz','driver-v6.tar.gz','driver-v7.tar.gz','driver-v8.tar.gz','driver-phase-boundary.json','adversarial-summary.json','v2-adversarial-summary.json','live-build.json','fresh-workflow.json','validation-commands.json','three-pose-check.json','boundary-events.json','timer-window-audit.json','tool-versions.json']:
 if (b/file).exists():
  if file.endswith('.gz'):shutil.copyfile(b/file,dest/file)
  else:(dest/(file+'.gz')).write_bytes(gzip.compress((b/file).read_bytes(),mtime=0))
for label in ['baseline','baseline-instrumented','candidate','candidate-final','candidate-counterfix','candidate-purefallback','candidate-adaptive','three-reference']:
 for file in ['identity.json','render-build.json','source.patch','render_overlap.rs']:
  p=b/label/file
  if p.exists():(dest/(label+'-'+file+'.gz')).write_bytes(gzip.compress(p.read_bytes(),mtime=0))
for label in ['candidate-final','candidate-counterfix','candidate-purefallback','candidate-adaptive']:
 root=b/label
 if root.exists():
  with tarfile.open(dest/(label+'-core-source.tar.gz'),'w:gz') as a:
   for file in ['src/lib.rs','src/render/gltf_gpu.rs','src/render/renderer.rs','src/render_overlap.rs','js/engine.d.ts']:
    if (root/file).exists():a.add(root/file,arcname=file,recursive=False)
for label in ['baseline-cuts-wg','candidate-cuts-wg','baseline-cuts-gl','candidate-cuts-gl']:
 p=pathlib.Path('output/review-optimization')/(label+'-cuts')/'report.json'
 if p.exists():(dest/(label+'.json.gz')).write_bytes(gzip.compress(p.read_bytes(),mtime=0))
for file in ['run-final-series.py','run-post-series.py','check-adversarial.mjs','archive-results.py','write-report.py']:
 if (b/file).exists():shutil.copyfile(b/file,dest/file)
(dest/'commands.json').write_text(json.dumps(commands,indent=2)+'\n');(dest/'index.json').write_text(json.dumps(summary,indent=2)+'\n')
tracked=subprocess.check_output(['git','diff','--name-only','HEAD'],text=True).splitlines()
new=['src/render_overlap.rs','scripts/freeze-render-build.mjs','docs/performance/STATE.md','docs/performance/EXPERIMENTS_2026-10-25.md','docs/history/RENDER_PERFORMANCE-2026-10-25-legacy.md']
for pattern in ['scripts/render-performance*','tests/render-performance*','.agents/skills/wasm-render-performance/**/*']:
 new.extend(str(p) for p in pathlib.Path('.').glob(pattern) if p.is_file())
with tarfile.open(dest/'experiment-source.tar.gz','w:gz') as archive:
 for file in sorted(set(tracked+new)):
  p=pathlib.Path(file)
  if p.is_file() and not str(p).startswith(str(dest)):archive.add(p,arcname=file,recursive=False)
manifest={str(p.relative_to(dest)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in sorted(dest.rglob('*')) if p.is_file() and p.name!='archive-manifest.json'}
(dest/'archive-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'destination':str(dest),'files':len(manifest),'bytes':sum(x['bytes'] for x in manifest.values()),'reports':len(summary)}))
