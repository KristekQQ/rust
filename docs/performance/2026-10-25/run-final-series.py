import subprocess,json,time,pathlib,sys
b=pathlib.Path('output/render-performance');driver=['node','scripts/render-performance.mjs'];jobs=[]
def add(name,args,role=None): jobs.append({'name':name,'command':driver+args+['--out',str(b/name)],'role':role})
def args(*v):return list(v)
# Secondary fixed-load adversarial controls. Main long-pair protocol follows.
for backend,suffix,order,overlap,load in [('webgpu','wg','shuffled',0,6144),('webgpu','wg','alternating',0,6144),('webgpu','wg','spatial',.97,512),('webgl','gl','shuffled',0,6144),('webgl','gl','spatial',.97,512)]:
 group='fixed-adversarial-'+suffix+'-'+order+('-dense97' if overlap else '')
 for pair in range(3):
  for role in (['before','after'] if pair%2==0 else ['after','before']):
   add(group+'-p'+str(pair)+'-'+role,args('smoke','--baseline-root',str(b/('baseline' if role=='before' else 'candidate-adaptive')),'--family','batching','--backend',backend,'--order',order,'--overlap',str(overlap),'--load',str(load),'--max-load','6144','--no-profile','--warmup','1000','--sample','5000'),{'group':group,'pair':pair,'role':role})
for backend,suffix in [('webgpu','wg'),('webgl','gl')]:
 cal='final-calibration-'+suffix
 add(cal,args('calibrate','--baseline-root',str(b/'baseline'),'--family','batching','--backend',backend,'--max-load','6144','--warmup','1000','--sample','2500'))
 common=args('--baseline-root',str(b/'baseline'),'--candidate-root',str(b/'candidate-adaptive'),'--config',str(b/cal/'config.json'),'--level','over','--max-load','6144')
 add('final-paired-off-'+suffix,args('regression')+common+args('--pairs','3','--no-profile','--warmup','8000','--sample','25000'))
 add('final-profile-'+suffix,args('compare')+common+args('--pairs','1','--warmup','1000','--sample','8000','--diagnostics'))
 add('final-capacity-'+suffix,args('capacity')+common+args('--pairs','1','--no-profile','--warmup','500','--sample','2500','--max-time','300'))
 add('final-smoke-matrix-'+suffix,args('smoke','--baseline-root',str(b/'candidate-adaptive'),'--smoke-matrix','--backend',backend,'--warmup','500','--sample','2500','--max-time','240'))
 for family,maximum in [('rigid',64),('skin',32),('morph',32),('fragment',64),('effects',64),('batching-heterogeneous',256),('dynamic',512)]:
  familycal='final-stress-'+family+'-'+suffix+('-timefix' if suffix=='wg' and family in ['skin','fragment'] else '')
  familycompare='final-family-'+family+'-'+suffix+('-timefix' if suffix=='wg' and family=='skin' else '')
  add(familycal,args('calibrate','--baseline-root',str(b/'baseline'),'--family',family,'--backend',backend,'--max-load',str(maximum),'--warmup','500','--sample','2500','--max-time','180'))
  add(familycompare,args('compare','--baseline-root',str(b/'baseline'),'--candidate-root',str(b/'candidate-adaptive'),'--config',str(b/familycal/'config.json'),'--level','near','--max-load',str(maximum),'--pairs','1','--warmup','2000','--sample','5000','--diagnostics'))
  jobs[-1]['autoOver']=True
 add('final-soak-dynamic-'+suffix,args('soak','--baseline-root',str(b/'candidate-adaptive'),'--config',str(b/('final-stress-dynamic-'+suffix)/'config.json'),'--max-load','512','--warmup','2000','--sample','120000','--max-time','200','--no-profile','--diagnostics'))
 for family in ['rim-static','arena-static']:
  cal='final-cal-'+family+'-'+suffix
  add(cal,args('calibrate','--baseline-root',str(b/'baseline'),'--family',family,'--backend',backend,'--max-load','1','--warmup','1000','--sample','2500'))
  add('final-control-'+family+'-'+suffix,args('compare','--baseline-root',str(b/'baseline'),'--candidate-root',str(b/'candidate-adaptive'),'--config',str(b/cal/'config.json'),'--max-load','1','--pairs','1','--warmup','2000','--sample','8000'))
# Three gets a fresh calibration whose baseline is our final release.
add('final-three-calibration-gl',args('calibrate','--baseline-root',str(b/'candidate-adaptive'),'--family','batching','--backend','webgl','--max-load','6144','--warmup','500','--sample','1500'))
add('final-three-geometry-gl',args('compare','--baseline-root',str(b/'candidate-adaptive'),'--three-root',str(b/'three-reference'),'--config',str(b/'final-three-calibration-gl'/'config.json'),'--load','6144','--max-load','6144','--pairs','3','--no-profile','--warmup','4000','--sample','10000','--diagnostics'))
add('final-three-objects-gl',args('compare','--baseline-root',str(b/'candidate-adaptive'),'--three-root',str(b/'three-reference'),'--config',str(b/'final-three-calibration-gl'/'config.json'),'--load','6144','--max-load','6144','--three-representation','objects','--pairs','3','--no-profile','--warmup','4000','--sample','10000','--diagnostics'))
meta=json.loads((b/'extra-series.json').read_text()) if '--resume' in sys.argv and (b/'extra-series.json').exists() else []
for entry in jobs:
 if any(old['name']==entry['name'] for old in meta):continue
 if entry.get('autoOver'):
  cfg=json.loads(pathlib.Path(entry['command'][entry['command'].index('--config')+1]).read_text())
  entry['command'][entry['command'].index('--level')+1]='over' if cfg['levels'].get('over') is not None else 'near'
 entry['started']=time.strftime('%Y-%m-%dT%H:%M:%S%z');meta.append(entry);(b/'extra-series.json').write_text(json.dumps(meta,indent=2));print('START '+entry['name'],flush=True)
 with (b/(entry['name']+'.log')).open('w') as log:result=subprocess.run(entry['command'],stdout=log,stderr=subprocess.STDOUT)
 entry.update(exitCode=result.returncode,finished=time.strftime('%Y-%m-%dT%H:%M:%S%z'));(b/'extra-series.json').write_text(json.dumps(meta,indent=2));print('END '+entry['name']+' exit='+str(result.returncode),flush=True)
 if entry.get('role') and entry['role'].get('pair')==2 and entry['role']['role']=='after':
  subprocess.run(['node','output/render-performance/check-adversarial.mjs'],check=True)
 # The first six before/after controls need inspection before accepting the final change.
 # A file gate allows the owner to halt safely between independent runs.
 while (b/'hold-final-series').exists():time.sleep(1)
