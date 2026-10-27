import json, subprocess, time
from pathlib import Path
root=Path('/Users/Honza/.codex/worktrees/rim-publish/rust')
out=root/'output/opaque-batching/final-matrix';out.mkdir(exist_ok=True)
base='output/opaque-batching/baseline'; candidate='output/opaque-batching/candidate-v4-packed'
stages=[]
def job(name,command,**kw):
 args=['node','scripts/render-performance.mjs',command]
 for k,v in kw.items():
  key='--'+k.replace('_','-');args += [key] if v is True else [key,str(v)]
 args += ['--no-profile','--out','output/opaque-batching/final-matrix/'+name]
 stages.append((name,args))
shared={'family':'rim-stress','load':512,'max_load':2048,'width':2880,'height':1920}
job('pairs-wg','compare',baseline_root=base,candidate_root=candidate,config='output/opaque-batching/calibrate-wg/config.json',backend='webgpu',pairs=3,warmup=8000,sample=25000,**shared)
job('calibrate-gl','calibrate',baseline_root=base,backend='webgl',warmup=3000,sample=8000,**shared)
job('pairs-gl','compare',baseline_root=base,candidate_root=candidate,config='output/opaque-batching/final-matrix/calibrate-gl/config.json',backend='webgl',pairs=3,warmup=8000,sample=25000,**shared)
job('calibrate-v2-gl','calibrate',baseline_root=candidate,backend='webgl',warmup=3000,sample=8000,**shared)
job('three-gl','compare',baseline_root=candidate,three_root='output/rim-stress/three-reference',three_representation='instanced',config='output/opaque-batching/final-matrix/calibrate-v2-gl/config.json',backend='webgl',pairs=3,warmup=8000,sample=25000,**shared)
for family,load in [('dynamic',256),('rim-static',1)]:
 for backend in ['webgpu','webgl']:
  name=family+'-'+backend
  options=dict(family=family,load=load,max_load=max(load,1024),width=960,height=640,backend=backend)
  job('calibrate-'+name,'calibrate',baseline_root=base,warmup=2000,sample=6000,**options)
  job('control-'+name,'compare',baseline_root=base,candidate_root=candidate,config='output/opaque-batching/final-matrix/calibrate-'+name+'/config.json',pairs=1,warmup=2000,sample=8000,**options)
status={'started':time.time(),'completed':[],'active':None,'failed':None}
for name,args in stages:
 status['active']=name;status['command']=args;(out/'matrix-status.json').write_text(json.dumps(status,indent=2)+'\n');print('START '+name,flush=True)
 with (out/(name+'.log')).open('w') as f:result=subprocess.run(args,cwd=root,stdout=f,stderr=subprocess.STDOUT)
 status['completed'].append({'name':name,'exit':result.returncode,'time':time.time()})
 if result.returncode:
  status['failed']=name;status['active']=None;(out/'matrix-status.json').write_text(json.dumps(status,indent=2)+'\n');print('FAILED '+name,flush=True);raise SystemExit(result.returncode)
 print('PASS '+name,flush=True)
 if (out/'stop-after-stage').exists():
  status['active']=None;status['stoppedBetweenStages']=True;(out/'matrix-status.json').write_text(json.dumps(status,indent=2)+'\n');raise SystemExit(0)
status['active']=None;status['finished']=time.time();(out/'matrix-status.json').write_text(json.dumps(status,indent=2)+'\n')
