import pathlib,json,gzip,hashlib,statistics,shutil,tarfile,subprocess
b=pathlib.Path('output/render-performance-rim-results');d=pathlib.Path('docs/performance/2026-10-26-rim');d.mkdir(exist_ok=True)
plan=json.loads((b/'plan.json').read_text());commands=json.loads((b/'commands.json').read_text());scenes=[];checks=[]
def read(name):return json.loads((b/name/'report.json').read_text())
def median(values):
 a=[v for v in values if isinstance(v,(int,float))];return statistics.median(a) if a else None
def w(r,k):return (((r.get('work') or {}).get('gltf') or {}).get(k) or {}).get('perApplicationRaf')
def f(v):return '—' if v is None else f'{v:.2f}'
for backend,short in [('WebGPU','wg'),('WebGL2','gl')]:
 for family,mode in [('rim-static','Klidová scéna'),('rim-active','Pohyb nepřátel')]:
  n=family+'-'+short;x=read('paired-'+n);assert len(x['results'])==6 and not x.get('error')
  a=sorted([r for r in x['results'] if r['label']=='baseline'],key=lambda r:r['pair']);c=sorted([r for r in x['results'] if r['label']=='candidate'],key=lambda r:r['pair']);compare=x['comparisons'][0];pixels=[p['pixel'] for p in compare['paired']]
  for label,run in [('before',a[0]),('after',c[0])]:shutil.copyfile(b/('paired-'+n)/run['capture']['file'],d/(n+'-'+label+'.png'))
  profiles=[read('profile-'+label+'-'+n)['results'][0] for label in ['before','after']]
  gpu=[(r.get('gpuPassMs') or {}).get('scene.main',{}).get('median') if short=='wg' else (r.get('glRafGpuMs') or {}).get('median') for r in profiles]
  native=lambda r,edge:((r.get('phaseEvidence') or {}).get(edge) or {}).get('native') or {}
  shot=lambda r:(native(r,'end').get('projectiles') or {}).get('fired',0)-(native(r,'start').get('projectiles') or {}).get('fired',0)
  work=[{'label':label,'before':median([w(r,k) for r in a]),'after':median([w(r,k) for r in c])} for label,k in [('glTF příkazy / snímek','drawCalls'),('glTF instance / snímek','instancesDrawn'),('glTF trojúhelníky / snímek','trianglesDrawn'),('glTF uniform+palette upload B / snímek','uniformUploadBytes')]]
  if family=='rim-active':
   for row in a+c:
    start=native(row,'start');end=native(row,'end');assert start.get('active') and end.get('active')
    assert end['frame']['simulation']>start['frame']['simulation']
   work+=[{'label':'Nepřátelské střely za25s','before':median([shot(r) for r in a]),'after':median([shot(r) for r in c])},{'label':'Nepřátelé na konci','before':median([len(native(r,'end').get('enemies',[])) for r in a]),'after':median([len(native(r,'end').get('enemies',[])) for r in c])}]
   pixel={'pass':None,'exact':None,'maxChannelDelta':None,'reason':'Běžící nativní hra nemá resetovaný identický capture stav. Snímky ilustrují tutéž scénu; pixelovou regresi ověřuje klidový režim.'}
  else:
   assert all(p['pass'] for p in pixels);pixel={'pass':True,'exact':all(p['exact'] for p in pixels),'maxChannelDelta':max(p['maxChannelDelta'] for p in pixels)}
  before=[r['rafCallbackCpu']['median'] for r in a];after=[r['rafCallbackCpu']['median'] for r in c];ratio=compare['cpuRatio']['median'];absolute=median(before)-median(after);consistent=all(p['cpuRatio']<1 for p in compare['paired']);meaningful=(1-ratio)>=.05 and absolute>=.2 and consistent
  s={'id':n,'backend':backend,'mode':mode,'caption':'Skutečné město, modely, materiály a nativní logika examples/rim. Stejná kamera a vizuální profil v obou buildech.' if family=='rim-static' else 'Nativní hra se šesti pohybujícími se nepřáteli a shodnými časovanými vstupy. Záznam počtu projektilů je níže; tento scénář neověřuje šerm ani řezy a snímky nejsou přesně synchronní.','cpuBefore':before,'cpuAfter':after,'cpuRatio':ratio,'gpuBefore':gpu[0],'gpuAfter':gpu[1],'gpuScope':'scene.main: asynchronní GPU čas hlavního passu, nikoli celého frame; samostatný profil2s/8s' if short=='wg' else 'EXT command interval mezi RAF callbacky; může obsahovat CPU/driver mezery, nikoli čistý GPU busy time; samostatný profil2s/8s','pairs':[{'pair':i+1,'beforeCpu':u['rafCallbackCpu']['median'],'afterCpu':v['rafCallbackCpu']['median'],'beforeCpuP95':u['rafCallbackCpu']['p95'],'afterCpuP95':v['rafCallbackCpu']['p95'],'beforeFrameP99':u['rafInterval']['p99'],'afterFrameP99':v['rafInterval']['p99']} for i,(u,v) in enumerate(zip(a,c))],'images':{'before':n+'-before.png','after':n+'-after.png'},'pixel':pixel,'work':work,'caveats':['Hlavní CPU měření má profiler vypnutý a zahrnuje aplikační RAF práci, ne timer/worker handlery ani input-to-display.','CPU a GPU časy se nesčítají; GPU profil je samostatná kratší diagnostika.','Chybějící starší glTF triangle čítač je nedostupný, nikoli0.','Tento trace je pohyb/animace nepřátel: v počátečních bězích0 vypuštěných projektilů. Aktivitu paprsků samostatně nepočítá. Neprovádí šerm/řezy ani přesný fixed-step záznam boje.'],'meaningfulCpuImprovement':meaningful,'nativeBoundaryEvidence':[{'label':r['label'],'pair':r['pair'],'start':native(r,'start'),'end':native(r,'end')} for r in a+c]}
  scenes.append(s)
valid=[s for s in scenes if s['meaningfulCpuImprovement']]
verdict='Na tomto Římě zatím měření nedokládá výrazné zrychlení. Dosavadní velké zisky z tisíců instancí se do této scény automaticky nepřenesly.' if not valid else 'Opakované měření ukazuje konkrétní zrychlení v části scénářů. Výsledek a špičky jsou níže pro každý backend zvlášť.'
view={'title':'Řím: původní a současný renderer','subtitle':'Skutečný examples/rim · desktop Apple M1 / Chrome ·960×640,DPR1 · fyzický Quest neověřený','verdict':verdict,'protocol':'3 páry AB/BA/AB;8s zahřátí +25s měření na běh, profiler prokazatelně vypnutý. Oddělený GPU profil2s/8s. Základní renderovací kvalita, assety, nativní gameplay a viewport jsou shodné.','builds':{'before':plan['baselineWasm'],'after':plan['candidateWasm']},'scenes':scenes,'limits':['Toto jsou desktopová měření, nikoli FPS na fyzickém Questu.','960×640 při DPR1; výsledky nelze převést na jiné rozlišení či dvě VR oči.','Zachovaná vzhledová konfigurace, shodných15 asset/runtime souborů a nativní gameplay zdroj.','Běží skutečné scene.js/appearance.js a nativní hra, bez desktopových DOM ovládacích prvků main.js.','U klidové scény platí pixelová kontrola; aktivní capture nemá deterministicky resetovanou simulaci.','CPU rozdíly0,1ms jsou zde blízko časové přesnosti a nejsou jistý přínos.','Pro výrazný CPU pokrok jsme předem požadovali alespoň5% i0,2ms a stejný směr ve všech3 párech.','Vnější GPU aktivita a teploty nebyly potvrzené; veškeré běhy agenta byly sériové.','Nový build nebyl tímto měřením nasazen na8443 ani pushnut.'],'evidence':[{'label':'Úplný report','href':'./REPORT.md'},{'label':'Přesné příkazy','href':'./commands.json'},{'label':'Protokol a identity','href':'./plan.json'},{'label':'Datový souhrn','href':'./view-data.json'}]}
(d/'view-data.json').write_text(json.dumps(view,indent=2,ensure_ascii=False)+'\n')
lines=['# Skutečný Řím: renderer před/po —2026-10-26','',''+verdict,'','Zdrojová identita: původní release`'+plan['baselineWasm']+'`, současný`'+plan['candidateWasm']+'`. Same15 runtime/asset hashes and native gameplay source; exact plan and commands retained. No Rust/SDK/gameplay/appearance change in this measurement task.','','## Výsledky skutečné scény','','| Backend / režim | CPU mediány před ms | CPU mediány po ms | Medián párového poměru | GPU scope před/po ms |','|---|---:|---:|---:|---:|']
for s in scenes:lines.append('| '+s['backend']+' / '+s['mode']+' | '+' / '.join(f(v) for v in s['cpuBefore'])+' | '+' / '.join(f(v) for v in s['cpuAfter'])+' | '+f(s['cpuRatio'])+' | '+f(s['gpuBefore'])+' / '+f(s['gpuAfter'])+' |')
lines+=['','CPU:3pairs8s/25s profiler off; GPU separate1pair2s/8s, scopes differ: native WebGPU scene.main versus GL EXT command interval, not whole GPU frame. Frame tails, actual work and phase boundaries are retained in full reports/raw.60Hz RAF cap is not a capacity/speedup proof. Physical Quest unverified.','','## Jednotlivé páry a obraz','']
for s in scenes:
 lines+=['### '+s['backend']+' · '+s['mode'],'','| Pár | CPU medián před/po | CPU p95 před/po | RAF p99 před/po |','|---|---:|---:|---:|']
 for p in s['pairs']:lines.append(f"| {p['pair']} | {f(p['beforeCpu'])}/{f(p['afterCpu'])} | {f(p['beforeCpuP95'])}/{f(p['afterCpuP95'])} | {f(p['beforeFrameP99'])}/{f(p['afterFrameP99'])} |")
 lines+=['',json.dumps(s['pixel'],ensure_ascii=False),'',f"[Původní snímek]({s['images']['before']}) · [Současný snímek]({s['images']['after']})",'']
lines+=['## Omezení','']+['- '+v for v in view['limits']]+['','## Reprodukce a zdroje','','[Interaktivní přehled](index.html) · [Plán](plan.json) · [Skutečné příkazy](commands.json) · [Souhrn](view-data.json). Original/final binary identities are pinned; compressed raw reports, actual served manifests and every capture below. Do not replace small real-Rome results with the previous synthetic6144-object speedups.']
(d/'REPORT.md').write_text('\n'.join(lines)+'\n')
for name in ['plan.json','commands.json']:shutil.copyfile(b/name,d/name)
for cmd in commands:
 n=cmd['name'];p=b/n;out=d/n;out.mkdir(exist_ok=True);x=read(n);assert cmd['exitCode']==0 and not x.get('driverInputsChangedDuringRun') and not x.get('error');assert all(not v for v in x.get('serverErrors',{}).values())
 (out/'report.json.gz').write_bytes(gzip.compress((p/'report.json').read_bytes(),mtime=0))
 for name in ['raw.json.gz','manifest-baseline.json','manifest-candidate.json','config.json']:
  if (p/name).exists():shutil.copyfile(p/name,out/name)
 for r in x.get('results',[]):
  f=r.get('capture',{}).get('file')
  if f:shutil.copyfile(p/f,out/f)
for name in ['run-series.py','build-results.py']:shutil.copyfile(b/name,d/name)
with tarfile.open(d/'measurement-source.tar.gz','w:gz') as t:
 for name in ['tests/render-performance.js','tests/render-performance.html','tests/pixels.js','scripts/render-performance-lib.mjs','scripts/render-performance.mjs','scripts/render-performance-api.mjs','scripts/render-performance-gl-timer.mjs','scripts/render-performance-order.mjs','scripts/browser-options.mjs']:
  t.add(name,arcname=name,recursive=False)
subprocess.run(['node',str(d/'build-view.mjs'),str(d/'view-data.json'),str(d/'index.html')],check=True)
manifest={str(p.relative_to(d)):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(d.rglob('*')) if p.is_file() and p.name!='archive-manifest.json'};(d/'archive-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'scenes':len(scenes),'reports':len(commands),'archiveFiles':len(manifest),'archiveBytes':sum(v['bytes'] for v in manifest.values()),'meaningfulCpuScenarios':len(valid)}))
