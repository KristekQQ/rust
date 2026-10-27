import pathlib,json,subprocess,time,sys,os
b=pathlib.Path('output/render-performance-rim-results');ledger=b/'commands.json';records=json.loads(ledger.read_text()) if ledger.exists() else [];before='output/render-performance/baseline';after='output/render-performance/candidate-cache'
def run(name,args):
 cmd=['node','scripts/benchmark-core.mjs',*args,'--out',str(b/name)];r={'name':name,'argv':cmd,'startedUtc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())};records.append(r);ledger.write_text(json.dumps(records,indent=2)+'\n');print('START '+name,flush=True)
 with (b/(name+'.log')).open('w') as log:
  try:p=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,timeout=720,env={**os.environ,'VERIFY_WASM_PROFILE':'release'});r['exitCode']=p.returncode
  except subprocess.TimeoutExpired:r['exitCode']=124;r['timeoutSeconds']=720
 r['finishedUtc']=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime());ledger.write_text(json.dumps(records,indent=2)+'\n')
 report=b/name/'report.json'
 if report.exists():
  x=json.loads(report.read_text());print(json.dumps({'name':name,'exit':r['exitCode'],'runs':[{'label':v['label'],'pair':v['pair'],'cpu':v['rafCallbackCpu']['median'],'cpuP99':v['rafCallbackCpu']['p99'],'rafP99':v['rafInterval']['p99']} for v in x.get('results',[])],'comparisons':x.get('comparisons'),'error':x.get('error')},separators=(',',':')),flush=True)
 if r['exitCode']:raise SystemExit(r['exitCode'])
phase=sys.argv[1]
for backend,short in [('webgpu','wg'),('webgl','gl')]:
 for family in ['rim-static','rim-active']:
  name=family+'-'+short;common=['--backend',backend,'--family',family,'--max-load','1','--max-time','600'];cfg=str(b/('cal-'+name)/'config.json')
  if phase=='main':
   run('cal-'+name,['calibrate','--baseline-root',before,*common,'--warmup','1000','--sample','3000','--no-profile'])
   run('paired-'+name,['compare','--baseline-root',before,'--candidate-root',after,'--config',cfg,*common,'--warmup','8000','--sample','25000','--pairs','3','--no-profile'])
  elif phase=='profile':
   for label,root in [('before',before),('after',after)]:run('profile-'+label+'-'+name,['smoke','--baseline-root',root,*common,'--warmup','2000','--sample','8000','--diagnostics'])
