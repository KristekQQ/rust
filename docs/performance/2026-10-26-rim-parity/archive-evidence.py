"""Archive completed experiment evidence without copying heavyweight build roots."""
from pathlib import Path
import hashlib,json,tarfile
root=Path.cwd(); source=root/'output/render-performance-rim-parity'; out=root/'docs/performance/2026-10-26-rim-parity/evidence';out.mkdir(exist_ok=True)
files={}
def add(p,arc):
 if p.is_file():files[arc]=p
for p in source.iterdir():
 if p.is_file() and p.suffix in {'.json','.md','.log','.py','.gz','.rs','.wgsl'}:add(p,'experiments/'+p.name)
 elif p.is_dir() and not p.name.startswith('candidate-') and p.name!='three-original':
  for q in p.rglob('*'):
   if q.is_file() and (q.name in {'report.json','raw.json.gz','config.json','controls.json','result.json','direct-no-map-supplement.json'} or (('oracle' in p.name or 'pixels' in p.name or 'inspect' in p.name) and q.suffix in {'.json','.png','.txt','.wgsl','.glsl','.metal'})):
    add(q,'experiments/'+str(q.relative_to(source)))
for name in ['candidate-stream','candidate-single-static','candidate-neutral-normal','candidate-integer-powers']:
 p=source/name
 add(p/'render-build.json',name+'/render-build.json');add(p/'pkg/verification-build.json',name+'/pkg/verification-build.json')
 for folder in ['src','tests']:
  for q in (p/folder).rglob('*'):
   if q.is_file() and q.suffix in {'.rs','.wgsl','.js','.html','.json'}:add(q,name+'/'+str(q.relative_to(p)))
for name in ['benchmark-core.mjs','render-performance.mjs','render-performance-lib.mjs','render-performance-order.mjs','render-performance-api.mjs','render-performance-gl-timer.mjs','browser-options.mjs','verify-normal-map.mjs','verify-shadow-filter.mjs','check-shadow-controls.mjs','check-pcf-shadow-edges.mjs','freeze-render-build.mjs','build-fingerprint.mjs']:
 add(root/'scripts'/name,'current-driver/scripts/'+name)
for pattern in ['render-performance*','normal-map-probe*','shadow-filter-probe*','pixels.js']:
 for q in (root/'tests').glob(pattern):
  if q.is_file():add(q,'current-driver/tests/'+q.name)
for q in (source/'three-original').rglob('*'):
 if q.is_file():add(q,'three-original/'+str(q.relative_to(source/'three-original')))
# Keep every numeric/raw result, source and failure family, but avoid versioning
# thousands of redundant passing probe PNGs. All excluded PNG hashes remain listed.
all_capture_files={arc:p for arc,p in files.items() if arc.endswith('.png')}
groups={}
full_families={'single-sun-pixels','neutral-normal-oracle-v2','neutral-normal-no-uv'}
for arc,p in all_capture_files.items():
 family=arc.split('/')[1] if arc.startswith('experiments/') else ''
 if family in full_families:continue
 key=(str(Path(arc).parent),'-'.join(p.name.split('-')[:2]))
 groups.setdefault(key,[]).append(arc)
omitted={}
for names in groups.values():
 names.sort();keep={names[0],names[-1]}
 for arc in names:
  if arc not in keep:omitted[arc]=files.pop(arc)
archive=out/'experiment-archive.tar.gz'
with tarfile.open(archive,'w:gz',compresslevel=6) as tar:
 for arc,p in sorted(files.items()):tar.add(p,arcname=arc,recursive=False)
manifest={'archive':archive.name,'sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'files':{arc:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for arc,p in sorted(files.items())},'omittedPassingProbeImages':{arc:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for arc,p in sorted(omitted.items())},'scope':'Completed and rejected local experiments, source snapshots and original Three reference. WASM binary hashes are in frozen manifests; binaries remain local. All normal-correction and single-sun failure captures are included; other passing probe images are represented by first/last per backend/build and complete omitted-image hashes. Full original probe images remain locally under output/. Final timing raw data and selected images are also collected separately.'}
(out/'archive-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({'archive':str(archive),'files':len(files),'bytes':archive.stat().st_size,'sha256':manifest['sha256']}))
