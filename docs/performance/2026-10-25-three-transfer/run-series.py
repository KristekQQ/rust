import pathlib,json,subprocess,time,sys,os
b=pathlib.Path('output/render-performance-three-transfer');ledger=b/'commands.json';records=json.loads(ledger.read_text()) if ledger.exists() else []
base='output/render-performance/candidate-adaptive';candidate=os.environ.get('PERF_CANDIDATE','output/render-performance/candidate-compose');three='output/render-performance/three-reference'
def run(name,args):
 out=b/name;cmd=['node','scripts/benchmark-core.mjs',*args,'--out',str(out)];record={'name':name,'argv':cmd,'startedUtc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())};records.append(record);ledger.write_text(json.dumps(records,indent=2)+'\n');print('START '+name,flush=True)
 with (b/(name+'.log')).open('w') as log:
  try:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,timeout=720,env={**os.environ,'VERIFY_WASM_PROFILE':'release'});record['exitCode']=r.returncode
  except subprocess.TimeoutExpired:record['timeoutSeconds']=720;record['exitCode']=124
 record['finishedUtc']=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime());ledger.write_text(json.dumps(records,indent=2)+'\n')
 p=out/'report.json'
 if p.exists():
  x=json.loads(p.read_text());print(json.dumps({'name':name,'exit':record['exitCode'],'runs':[{'label':r['label'],'load':r['load'],'cpu':r['rafCallbackCpu']['median'],'rafP99':r['rafInterval']['p99']} for r in x.get('results',[])],'ratios':[r.get('cpuRatio') for r in x.get('comparisons',[])],'error':x.get('error')},separators=(',',':')),flush=True)
 else:print('NO REPORT '+name,flush=True)
 if record['exitCode']!=0:raise SystemExit(record['exitCode'])
phase=sys.argv[1]
for backend,short in [('webgpu','wg'),('webgl','gl')]:
 cfg=str(b/('cal-'+short)/'config.json')
 common=['--backend',backend,'--family','batching','--max-load','6144','--max-time','600','--no-profile']
 if phase=='pilot':
  run('cal-'+short,['calibrate','--baseline-root',base,*common,'--warmup','500','--sample','2500'])
  run('pilot-'+short,['compare','--baseline-root',base,'--candidate-root',candidate,'--config',cfg,'--load','6144',*common,'--warmup','1000','--sample','5000','--pairs','1'])
 elif phase=='pilot-cache':
  run('pilot-cache-'+short,['compare','--baseline-root',base,'--candidate-root',candidate,'--config',cfg,'--load','6144',*common,'--warmup','1000','--sample','5000','--pairs','1'])
 elif phase=='primary':
  run('paired-'+short,['compare','--baseline-root',base,'--candidate-root',candidate,'--config',cfg,'--load','6144',*common,'--warmup','8000','--sample','25000','--pairs','3'])
 elif phase=='profile':
  for label,root in [('before',base),('after',candidate)]:run('profile-'+label+'-'+short,['smoke','--baseline-root',root,'--backend',backend,'--family','batching','--load','6144','--max-load','6144','--warmup','2000','--sample','8000','--max-time','90','--diagnostics'])
 elif phase=='controls':
  for family,maxload in [('dynamic','512'),('rim-static','1'),('arena-static','1')]:
   name=family+'-'+short;c=str(b/('cal-'+name)/'config.json')
   run('cal-'+name,['calibrate','--baseline-root',base,'--backend',backend,'--family',family,'--max-load',maxload,'--max-time','180','--no-profile','--warmup','500','--sample','2500'])
   run('paired-'+name,['compare','--baseline-root',base,'--candidate-root',candidate,'--config',c,'--backend',backend,'--family',family,'--max-load',maxload,'--load',maxload,'--max-time','180','--no-profile','--warmup','2000','--sample','8000','--pairs','1'])
if phase=='three':
 cfg=str(b/'cal-gl'/'config.json')
 # Calibrate against the retained candidate runtime, not the previous build.
 run('cal-three-gl',['calibrate','--baseline-root',candidate,'--backend','webgl','--family','batching','--max-load','6144','--max-time','180','--no-profile','--warmup','500','--sample','2500'])
 for representation in ['objects','instanced']:
  run('three-'+representation,['compare','--baseline-root',candidate,'--three-root',three,'--three-representation',representation,'--config',str(b/'cal-three-gl'/'config.json'),'--backend','webgl','--family','batching','--load','6144','--max-load','6144','--max-time','600','--no-profile','--warmup','4000','--sample','10000','--pairs','3'])
