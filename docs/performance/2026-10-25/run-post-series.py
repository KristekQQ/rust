import subprocess,json,time,pathlib
b=pathlib.Path('output/render-performance');jobs=[]
for backend,suffix in [('webgpu','wg'),('webgl','gl')]:
 for family in ['rim-active','arena-active']:
  cal='final-cal-'+family+'-'+suffix
  jobs.append((cal,['node','scripts/render-performance.mjs','calibrate','--baseline-root',str(b/'baseline'),'--family',family,'--backend',backend,'--max-load','1','--warmup','1000','--sample','2500','--out',str(b/cal)]))
  name='final-control-'+family+'-'+suffix
  jobs.append((name,['node','scripts/render-performance.mjs','compare','--baseline-root',str(b/'baseline'),'--candidate-root',str(b/'candidate-adaptive'),'--config',str(b/cal/'config.json'),'--max-load','1','--pairs','1','--warmup','2000','--sample','8000','--diagnostics','--out',str(b/name)]))
 for label,root in [('baseline','baseline'),('candidate','candidate-adaptive')]:
  name=label+'-cuts-'+suffix
  jobs.append((name,['node','scripts/benchmark-rim-cuts.mjs',name,str(b/root/'pkg'),backend]))
# Isolate profiler readback failure from actual effects rendering, without discarding it.
cal='final-effects-off-calibration-wg'
jobs.append((cal,['node','scripts/render-performance.mjs','calibrate','--baseline-root',str(b/'baseline'),'--family','effects','--backend','webgpu','--max-load','64','--warmup','500','--sample','2500','--no-profile','--out',str(b/cal)]))
name='final-effects-off-compare-wg'
jobs.append((name,['node','scripts/render-performance.mjs','compare','--baseline-root',str(b/'baseline'),'--candidate-root',str(b/'candidate-adaptive'),'--config',str(b/cal/'config.json'),'--max-load','64','--load','64','--pairs','1','--warmup','2000','--sample','5000','--no-profile','--out',str(b/name)]))
# Investigate any >5% short dynamic CPU delta with three unprofiled repetitions.
for backend,suffix in [('webgpu','wg'),('webgl','gl')]:
 source=b/('final-family-dynamic-'+suffix)/'report.json'
 if source.exists():
  evidence=json.loads(source.read_text());ratio=((evidence.get('comparisons') or [{}])[0].get('cpuRatio') or {}).get('median',0)
  if ratio>1.05:
   name='final-dynamic-paired-off-'+suffix
   jobs.append((name,['node','scripts/render-performance.mjs','compare','--baseline-root',str(b/'baseline'),'--candidate-root',str(b/'candidate-adaptive'),'--config',str(b/('final-stress-dynamic-'+suffix)/'config.json'),'--max-load','512','--pairs','3','--warmup','2000','--sample','10000','--no-profile','--out',str(b/name)]))
name='fresh-skill-smoke'
jobs.append((name,['node','scripts/benchmark-core.mjs','smoke','--baseline-root',str(b/'candidate-adaptive'),'--family','batching','--backend','webgpu','--load','64','--max-load','64','--warmup','500','--sample','2500','--no-profile','--out',str(b/name)]))
meta=json.loads((b/'post-series.json').read_text()) if (b/'post-series.json').exists() else []
for name,command in jobs:
 if any(v['name']==name for v in meta):continue
 entry={'name':name,'command':command,'started':time.strftime('%Y-%m-%dT%H:%M:%S%z')};meta.append(entry);(b/'post-series.json').write_text(json.dumps(meta,indent=2));print('START '+name,flush=True)
 with (b/(name+'.log')).open('w') as log:r=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
 entry.update(exitCode=r.returncode,finished=time.strftime('%Y-%m-%dT%H:%M:%S%z'));(b/'post-series.json').write_text(json.dumps(meta,indent=2));print('END '+name+' exit='+str(r.returncode),flush=True)
 if r.returncode:raise SystemExit(r.returncode)
