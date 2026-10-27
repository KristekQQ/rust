import pathlib,json,gzip,statistics
b=pathlib.Path('output/render-performance');d=pathlib.Path('docs/performance/2026-10-25');d.mkdir(parents=True,exist_ok=True)
def read(name):
 p=b/name/'report.json'
 return json.loads(p.read_text()) if p.exists() else None
def f(v):return '—' if v is None else f'{v:.2f}'
def med(a):return statistics.median(a) if a else None
def metric(r,k,stat='median'):return (r.get(k) or {}).get(stat)
def w(r,k):return ((r.get('work') or {}).get('gltf') or {}).get(k,{}).get('perApplicationRaf')
raw_cache={}
def sampled_upload(name,id):
 if name not in raw_cache:
  path=b/name/'raw.json.gz';raw_cache[name]=json.loads(gzip.decompress(path.read_bytes())) if path.exists() else []
 run=next((r for r in raw_cache[name] if r['id']==id),{})
 values=[v['renderer']['uploadBytes'] for v in run.get('samples',[]) if isinstance((v.get('renderer') or {}).get('uploadBytes'),(int,float))]
 return med(values)
def pairtxt(rows,k):return '/'.join(f(metric(r,k)) for r in rows)
def link(n):return f'[{n}]({n}/summary.json)'
lines=['# Zrychlení Rust/WASM rendereru — 25. 10. 2026','',
'Změna zrychluje přípravu velkých kompatibilních skupin glTF objektů. Zachovává počet objektů, rozlišení, shadery, stíny i pořadí kreslení. Neřeší celý renderer ani neprokazuje vyšší výkon na fyzickém Questu.','',
'## Identita a podmínky','',
'- Vlastní větev `codex/wasm-render-performance`, základ `64bfda680483e71f6aeb6bf902e63b29b3e90791`; změny této úlohy nejsou commitnuté ani pushnuté.',
'- Originální release WASM `c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a`; finální release `6f9a3bba80c0681fbf4908ac4dbda7fd5088ed72aa6c4f8523b3a2d5a68518b9`.',
'- Apple M1, Chrome155, hardwarové ANGLE/Metal, desktop headless, 960×640, DPR1, seed1337. Běhy a buildy agenta sériově; nepřítomnost jiné GPU aktivity nebyla potvrzena.',
'- Primární protokol: tři páry AB/BA/AB, 8s zahřátí +25s měření, profiler prokazatelně vypnutý. Doplňkové rodiny/kapacita mají kratší protokol uvedený v reportech; neslouží jako jistý důkaz malých zrychlení.',
'- Původní produkční balíček na8443 a reference na8878 zůstaly zachované. Tato úloha nenasadila nový build na8443.',
'- [Přesné spuštěné příkazy](commands.json), [hashovaný seznam archivu](archive-manifest.json), [protokol](../../RENDER_PERFORMANCE.md), [deník včetně zamítnutých pokusů](../EXPERIMENTS_2026-10-25.md).',
'', '## Primární výsledek — 6144 viditelných objektů','',
'Šest skutečných instancovaných příkazů a 12 288 trojúhelníků na aplikační snímek. U původního buildu se trojúhelníky určují z ověřené geometrie a skutečně odeslaných instancí; nový přímý čítač ještě neobsahuje. Před/po stejná geometrie, kamera, měřítko i animace. Porovnání AABB nyní používá omezený přesný strom; po osmi vyčerpáních rozpočtu v jedné skupině přepne na původní lineární dotaz.','',
'']
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 n='final-paired-off-'+suffix;x=read(n)
 if not x:continue
 lines+=['','### '+label,'','| Běh | CPU medián/p95/p99/max ms | RAF medián/p95/p99 ms | RAF překročení: počet / součet nad rozpočet ms |','|---|---:|---:|---:|']
 for r in x['results']:
  c=r['rafCallbackCpu'];a=r['rafInterval'];lines.append(f"| {label} {r['label']} pár{r['pair']} | {'/'.join(f(c.get(k)) for k in ['median','p95','p99','max'])} | {'/'.join(f(a.get(k)) for k in ['median','p95','p99'])} | {a['overruns']} / {f(a['overrunDurationMs'])} |")
 ratio=(x.get('comparisons') or [{}])[0].get('cpuRatio') or {};lines+=['',f"{label}: medián poměrů CPU {f(ratio.get('median'))}; úspora {f((1-ratio['median'])*100) if ratio.get('median') is not None else '—'} %. {link(n)}. Obraz: "+str([p['pixel'].get('exact') for c in x.get('comparisons',[]) for p in c['paired']])+'.','']
 for role in ['baseline','candidate']:
  cold=[r['coldReadyMs'] for r in x['results'] if r['label']==role];lines.append(f"Studená stránka {role}: ready {min(cold)}–{max(cold)}ms (n={len(cold)}, obsahuje setup/network, OS/driver cache nejsou vyčištěné).")
lines+=['Překročení používá předem určený rozpočet16,667ms a toleranci0,5ms. Číslo je součet času nad rozpočet u intervalů překračujících toleranci, ne doba všech pomalých snímků. RAF je aplikační kadence, ne přímé měření prezentace v headsetu. Celé distribuce včetně průměru a přesných překročení jsou v JSON; jednotlivé snímky v raw.json.gz.','',
'## Profil a prostředky','',
'Krátký profil1s/8s je samostatné měření se zapnutým profilerem. GPU WebGPU = asynchronní pojmenovaný pass; GL = EXT interval příkazů mezi RAF callbacky, který může zahrnovat mezery CPU/driveru. Tyto scope nejsou vzájemně zaměnitelné ani celé GPU frame.','',
'| Backend/verze | CPU ms | glTF queue ms | scene.gpuUpdates ms | GPU scope medián ms | skutečné glTF draws / instance / tris | glTF upload volání / uniform+palette B za snímek | nové stránky během měření | scratch B |',
'|---|---:|---:|---:|---:|---:|---:|---:|---:|']
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 x=read('final-profile-'+suffix)
 if not x:continue
 for r in x['results']:
  scopes=r.get('sampledCpuScopeMs') or {};gpu=(r.get('gpuPassMs') or {}).get('scene.main') if suffix=='wg' else r.get('glRafGpuMs');end=(r.get('work') or {}).get('gauges',{}).get('end') or {}
  lines.append(f"| {label} {r['label']} | {f(metric(r,'rafCallbackCpu'))} | {f((scopes.get('gltf.queue') or {}).get('median'))} | {f((scopes.get('scene.gpuUpdates') or {}).get('median'))} | {f((gpu or {}).get('median'))} | {f(w(r,'drawCalls'))} / {f(w(r,'instancesDrawn'))} / {f(w(r,'trianglesDrawn'))} | {f(w(r,'bufferUploadCalls'))} / {f(w(r,'uniformUploadBytes'))} | {f(((r.get('work') or {}).get('gltf') or {}).get('instancePagesCreated',{}).get('delta'))} | {end.get('queueScratchBytes','—')} |")
lines+=['','Starší baseline nové čítače neobsahuje: „—“ znamená nedostupné. `renderer_stats().drawCalls` je starší odhad počtu objektových batchů; zde se používají skutečné glTF/draw debug čítače. Aktivní/retinované stránky jsou poslední gauge na pass/view klíč; scratch je retinovaná kapacita, nikoli všechny alokace. GPU page pooling se v této změně neopravoval.','',
'Samostatná instrumentovaná stránka zaznamenává skutečná API vytvoření bufferů/textur/bind groups/pipeline, uploady, průchody, submissions a změny stavu. Jde o cold-init+1s, nikoli důkaz nulových warm alokací. Kompletní počty jsou `apiOperations` v každém summary. Profilerové fáze se mohou vnořovat: nesčítat `gltf.update` se `scene.gpuUpdates`.','',
'## Kapacita','',
'Krátký průzkumný párový sweep0,5s/2,5s, stejné původně kalibrované loady, profiler off. Nejvyšší úspěšný **testovaný** počet není globální maximum. Kritérium: RAF medián≤budget+0,5ms, p95≤2×budget+0,5ms, podíl vynechaných intervalů≤10%, CPU p95≤budget+0,5ms.','',
'| Backend | Původní nejvyšší úspěšný test | Finální nejvyšší úspěšný test | Nejvyšší zkoušený počet |','|---|---:|---:|---:|']
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 x=read('final-capacity-'+suffix)
 if not x:continue
 budget=x['settings']['budget'];passes=lambda r:r['rafInterval']['median']<=budget+.5 and r['rafInterval']['p95']<=2*budget+.5 and r['missedFrameFraction']<=.1 and r['rafCallbackCpu']['p95']<=budget+.5
 vals=[max([r['load'] for r in x['results'] if r['label']==v and passes(r)],default=0) for v in ['baseline','candidate']];lines.append(f"| {label} | {vals[0]} | {vals[1]} | {max([r['load'] for r in x['results']],default=0)} |")
lines+=['','## Ostatní workloady','',
'Pevný baseline load, jeden doplňkový pár; rigid/skinning/morph/fragment/effects/heterogeneous/dynamic2s/5s, skutečné hry2s/8s. Zapnutý profiler. Malé rozdíly jsou orientační. PBR/fragment overdraw je označená syntetická diagnostika. Aktivní hry mají časované vstupy, ale capture neresetuje nativní herní stav: pixelová shoda je nedostupná, ne PASS.','',
'| Backend / workload | Load | CPU medián před→po ms | GPU scope: medián před→po ms | RAF p95/p99 po ms | RAF překročení po počet/ms | Obraz |',
'|---|---:|---:|---:|---:|---:|---|']
familyrows=[]
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 for family in ['rigid','skin','morph','fragment','effects','batching-heterogeneous','dynamic','rim-static','arena-static','rim-active','arena-active']:
  n=('final-control-' if family.startswith(('rim','arena')) else 'final-family-')+family+'-'+suffix;x=read(n)
  if family=='skin' and suffix=='wg':
   fixed=read(n+'-timefix')
   if fixed:
    x=fixed;n+='-timefix'
  if not x or len(x['results'])<2:continue
  a=next(r for r in x['results'] if r['label']=='baseline');c=next(r for r in x['results'] if r['label']=='candidate');familyrows.append((label,family,n,a,c))
  scopes=a.get('gpuPassMs') or {}
  gpukey=max(scopes,key=lambda k:scopes[k]['median']) if suffix=='wg' and scopes else 'GL RAF interval'
  def gpu(r):return ((r.get('gpuPassMs') or {}).get(gpukey) if suffix=='wg' else r.get('glRafGpuMs')) or {}
  pix=[p['pixel'].get('pass') for comp in x.get('comparisons',[]) for p in comp['paired']];p='nedostupné' if None in pix else ('PASS' if pix and all(pix) else 'FAIL')
  lines.append(f"| {label} {family} | {a['load']} | {f(metric(a,'rafCallbackCpu'))}→{f(metric(c,'rafCallbackCpu'))} | `{gpukey}` {f(gpu(a).get('median'))}→{f(gpu(c).get('median'))} | {f(metric(c,'rafInterval','p95'))}/{f(metric(c,'rafInterval','p99'))} | {c['rafInterval']['overruns']}/{f(c['rafInterval']['overrunDurationMs'])} | {p} |")
lines+=['','| Backend/workload/verze | glTF draw / instances / triangles za snímek | glTF upload calls / uniform+palette B za snímek | celý renderer upload B, medián vzorků10Hz | nové stránky / buffery / bindgroups v měření |','|---|---:|---:|---:|---:|']
for label,family,name,a,c in familyrows:
 for r in [a,c]:
  v=(r.get('work') or {}).get('gltf') or {};delta=lambda k:v.get(k,{}).get('delta');lines.append(f"| {label} {family} {r['label']} | {f(w(r,'drawCalls'))}/{f(w(r,'instancesDrawn'))}/{f(w(r,'trianglesDrawn'))} | {f(w(r,'bufferUploadCalls'))}/{f(w(r,'uniformUploadBytes'))} | {f(sampled_upload(name,r['id']))} | {f(delta('instancePagesCreated'))}/{f(delta('instanceBuffersCreated'))}/{f(delta('instanceBindGroupsCreated'))} |")
lines+=['','GPU sloupec vybírá nejdelší jednotlivý pojmenovaný scope z baseline, nikoli celý frame ani součet passů; všechny passy včetně postprocess jsou zachované v JSON. Uniform+palette byte sloupec používá uniformUploadBytes; palette je jeho podmnožina a nesčítá se znovu. Nezahrnuje průběžný CPU morph vertex upload. Proto je vedle uveden skutečný per-frame renderer.uploadBytes, medián vzorků přibližně10Hz (požadované uploady, nikoli fyzický přenos po sběrnici). Morph zde deformuje384vertexů×96B; nulový uniform čítač neznamená nulové uploady. glTF čítače nepokrývají primitivní/neon/SSR kreslení. Celé API počty jsou samostatně v JSON. Tvrzení o animaci nebo GPU geometrii vycházejí z reálně vykreslených assetů a čítačů; u skin/morph se nevydává deklarovaný počet rootů za skutečný počet trojúhelníků.','',
'## Delší dynamický běh a řezy','',
'Dynamický soak:2s zahřátí+120s, původní kalibrace stejného harnessu, finální release. WebGPU běh má profiler zapnutý; WebGL2 dlouhý běh jej výslovně vypíná. Tyto dva běhy nejsou párovým srovnáním backendů. Dvanáctitrojúhelníkové uzavřené řezané testovací těleso slouží k opakovanému spawn/cut/despawn; samostatný herní cut benchmark níže ověřuje skutečné půlky nepřítele.','',
'| Backend | Load | CPU medián/p99/max ms | RAF p95/p99/max ms | cuts / spawns / explicit removes | root culling min/max |','|---|---:|---:|---:|---:|---:|']
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 n='final-soak-dynamic-'+suffix;x=read(n)
 if not x or not x.get('results'):continue
 r=x['results'][0];vals=[]
 raw=b/n/'raw.json.gz'
 if raw.exists():
  for run in json.loads(gzip.decompress(raw.read_bytes())):
   vals.extend(s.get('renderer',{}).get('culled') for s in run.get('samples',[]) if isinstance(s.get('renderer',{}).get('culled'),(int,float)))
 lines.append(f"| {label} | {r['load']} | {'/'.join(f(metric(r,'rafCallbackCpu',k)) for k in ['median','p99','max'])} | {'/'.join(f(metric(r,'rafInterval',k)) for k in ['p95','p99','max'])} | {r['commands'].get('cuts')}/{r['commands'].get('spawns')}/{r['commands'].get('removes')} | {min(vals) if vals else '—'}/{max(vals) if vals else '—'} |")
lines+=['','Top-level culling může předcházet glTF primitive testu; nulová delta glTF cull neznamená nulové root culling. `removes` počítá explicitní odstranění cutteru+dvou půlek; původní target odstraňuje nativní cut a počítá jej událost cuts.','',
'Každá následující sada má tři skutečné běhy simulovaného XR. Latence je od prvního neprázdného požadavku na řez po konec prvního simulačního XR snímku s kompletními půlkami; nezahrnuje potvrzenou prezentaci na fyzickém displeji. Měří se handler onmessage na hlavním vlákně, serializace požadavku a nativní instalace. Doba samotného výpočtu uvnitř workeru není samostatně instrumentovaná; nesmí se zaměnit za krátký onmessage handler.','',
'| Backend/verze | request→publish medián/p95/max ms | hlavní onmessage max ms | transport prepare max ms | stav |','|---|---:|---:|---:|---|']
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 for version in ['baseline','candidate']:
  p=pathlib.Path('output/review-optimization')/(version+'-cuts-'+suffix+'-cuts')/'report.json'
  if not p.exists():continue
  x=json.loads(p.read_text());s=x.get('summary') or {};lat=s.get('requestToPublishMs') or {};lines.append(f"| {label} {version} | {'/'.join(f(lat.get(k)) for k in ['median','p95','max'])} | {f((s.get('workerHandlerMaxMs') or {}).get('max'))} | {f((s.get('transportPrepareMaxMs') or {}).get('max'))} | {x.get('status')} |")
lines+=['','## Three.js r170','',
'Samostatná geometrická reference: shodné vektory kamery/FOV (nativní LH a Three RH konvence se liší),4vertexy/6indexů na quad, +Z normály, počet objektů, elapsed-time rotace ve shodném pořadíYXZ, viewport a AAfalse. PBR/osvětlení, materiálový systém a část přípravy se liší. Pixel gate je výslovně vypnutý. Není to důkaz výkonu shodné celé hry. Starší `three-geometry-gl` běh používal odlišné pořadí Euler rotací a je pouze historický, nikoli finální odpovídající pohyb.','',
'| Reprezentace / verze | CPU mediány 3 párů ms | skutečné draw commands / tris |','|---|---:|---:|']
for mode,n in [('instanced','final-three-geometry-gl'),('objects','final-three-objects-gl')]:
 x=read(n)
 if not x:continue
 for version in ['baseline','candidate']:
  rows=sorted([r for r in x['results'] if r['label']==version],key=lambda r:r['pair']);rr=rows[-1].get('renderer',{}) if rows else {};draw=rr.get('drawCalls') if version=='candidate' else w(rows[-1],'drawCalls') if rows else None;tris=rr.get('triangles') if version=='candidate' else w(rows[-1],'trianglesDrawn') if rows else None;lines.append(f"| {mode} {'Rust final' if version=='baseline' else 'Three.js'} | {pairtxt(rows,'rafCallbackCpu')} | {f(draw)}/{f(tris)} |")
lines+=['','Instanced varianta má předem vytvořené skupiny1024 objektů a šest příkazů, zatímco objects vytváří6144 nezávislých root/mesh dvojic a6144 příkazů. První vynechává dynamické hledání skupin; druhá zvyšuje náklad odesílání. Obě hranice jsou užitečné pro profilování, žádná sama neurčuje univerzálního vítěze. GPU čas v těchto profiler-off kontrolách je nedostupný.','',
'## Korektnost, zamítnuté varianty a další krok','',
'- 7 nativních testů nezávislého pairwise oracle;14 skutečných udržovaných browser/backend testů včetně unbatched obrazové reference, overlap/pořadí/mirror/nonuniform, skin/culling, viditelných půlek/evaporace a simulovaného stereoXR. Strict WASM Clippy a SDK typecheck PASS. Nativní testy samy WASM render neověřují.',
'- První spatial výhra zakryla regresi nepříznivého pořadí: diagnostická varianta f69de1cf měla asi28→55ms. Zachována a odmítnuta. Samostatné counterfix/purefallback varianty32,9/30,9ms proti28,3ms také nebyly přijaty. Finální adaptivní fallback prošel trojím adversarial párem; nepříznivé malé delty až~3% jsou otevřeně zachované.',
'- Úplně překryté drobné quady měly pouze4viditelnépixely a oba buildy selhaly foreground gate. Nejde o platný výkonový výsledek;97%částečný překryv kontrolu obsahu splnil.',
'- Stride96B, CPU morph fallback, per-primitive inverse/determinant/bounds a GPU page churn zůstávají. WebGL XR druhý kontext/upload/blit, foveace/eye targets a absent-normal shader jsou auditované, ale neoptimalizované samostatné kandidáty. Předchozí zamítnutá uniform arena se neopakovala.',
'- Další experimenty podle důkazů: dynamický WebGPU soak vytvořil2648stránek/5296bufferů/2648bindgroups za120s při stabilním scratch a skutečném root cullingu0–153. Ověřit oddělení aktivního počtu od omezeného retinovaného poolu při zachování pass/view vlastnictví. Pro CPU throughput v nyní dominantním `scene.gpuUpdates` oddělit přechod SDK→WASM, world transforms a opakované per-node inverse/bounds; ověřit per-node cache a dávkový vstup na víceprimitivových animovaných assetech. Zachovat stejný pohyb, data a neporušit rollback. Nezačínat shaderovým přepisem, když hlavní pass zde stojí řádově0,25ms.',
'- Fyzický Quest3: výkon, teplotní drift, eye rozměry/Hz/foveace a input-to-display latence **neověřené**. Desktop/simulované XR tyto závěry nenahrazuje.',
'', 'Úplná data a nepříznivé běhy zůstávají v archivu; počet úspěšných testů není důkaz „vše dokonale optimalizováno“.']
lines+=['','## Dodatečné kontroly nejasných výsledků','']
for suffix,label in [('wg','WebGPU'),('gl','WebGL2')]:
 n='final-dynamic-paired-off-'+suffix;x=read(n)
 if not x:continue
 ratio=x.get('comparisons',[{}])[0].get('cpuRatio',{}).get('median');pix=[p['pixel'].get('exact') for c in x.get('comparisons',[]) for p in c['paired']]
 a=sorted([r for r in x['results'] if r['label']=='baseline'],key=lambda r:r['pair']);c=sorted([r for r in x['results'] if r['label']=='candidate'],key=lambda r:r['pair'])
 lines.append(f"- {label} dynamická zátěž, tři páry2s/10s bez profileru: CPU mediány původní {pairtxt(a,'rafCallbackCpu')}ms, finální {pairtxt(c,'rafCallbackCpu')}ms; medián poměrů {f(ratio)}. Přesný obraz: {pix}. {link(n)}. Předem podmíněná kontrola následuje po krátkém nepříznivém rozdílu nad5%.")
x=read('final-effects-off-compare-wg')
if x and len(x['results'])==2:
 a=x['results'][0];c=x['results'][1];lines.append(f"- Efekty64 bez profileru, jeden2s/5s pár: CPU {f(metric(a,'rafCallbackCpu'))}→{f(metric(c,'rafCallbackCpu'))}ms; RAFp99 {f(metric(a,'rafInterval','p99'))}→{f(metric(c,'rafInterval','p99'))}ms. GPU timing nedostupný záměrně; výsledek odděluje reálné renderování od neúspěšného native timestamp readbacku. {link('final-effects-off-compare-wg')}.")
x=read('fresh-skill-smoke')
if x:lines.append('- Čerstvý proces přes benchmark-core.mjs spustil skutečný smoke finálního release,64 viditelných objektů, WebGPU/profiler off; manifest a skutečné příkazy uložené. Hlavní kalibrace a třípárová regression kontrola jsou rovněž samostatné nové Node/browser procesy. Schema skillu validováno dostupným quick_validate.py. Viz fresh-workflow.json.gz a validation-commands.json.gz.')
(d/'REPORT.md').write_text('\n'.join(lines)+'\n')
print(str(d/'REPORT.md'))
