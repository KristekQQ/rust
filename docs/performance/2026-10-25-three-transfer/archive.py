import pathlib,json,gzip,hashlib,shutil,tarfile,subprocess
b=pathlib.Path('output/render-performance-three-transfer');d=pathlib.Path('docs/performance/2026-10-25-three-transfer');d.mkdir(exist_ok=True)
commands=json.loads((b/'commands.json').read_text());index=[]
for cmd in commands:
 n=cmd['name'];p=b/n;r=p/'report.json'
 if not r.exists():continue
 x=json.loads(r.read_text());out=d/n;out.mkdir(exist_ok=True)
 (out/'report.json.gz').write_bytes(gzip.compress(r.read_bytes(),mtime=0))
 for name in ['raw.json.gz','manifest-baseline.json','manifest-candidate.json','config.json']:
  if (p/name).exists():shutil.copyfile(p/name,out/name)
 for row in x.get('results',[]):
  file=(row.get('capture') or {}).get('file')
  if file and (p/file).exists():shutil.copyfile(p/file,out/file)
 compact={k:x.get(k) for k in ['version','date','command','settings','driverIdentity','browser','hardware','baselineBuildFingerprint','candidateBuildFingerprint','calibration','comparisons','failures','error','driverInputsChangedDuringRun','serverErrors','durationSeconds','measurementProtocol','profilerOffControl','comparisonKind','threeReference','cleanup','cleanupFailure','calibrationValidation','results']}
 (out/'summary.json').write_text(json.dumps(compact,separators=(',',':'))+'\n');index.append({'name':n,'exitCode':cmd.get('exitCode'),'results':len(x.get('results',[]))})
for name in ['commands.json','plan.json','validation.json','THREE-LICENSE.txt']:
 if (b/name).exists():shutil.copyfile(b/name,d/name)
for name in ['run-series.py','archive.py','write-report.py']:
 if (b/name).exists():shutil.copyfile(b/name,d/name)
for label in ['candidate-adaptive','candidate-compose','candidate-cache','three-reference']:
 p=pathlib.Path('output/render-performance')/label
 for name in ['render-build.json','identity.json']:
  if (p/name).exists():(d/(label+'-'+name+'.gz')).write_bytes(gzip.compress((p/name).read_bytes(),mtime=0))
with tarfile.open(d/'final-source.tar.gz','w:gz') as t:
 for name in ['src/scene.rs','src/visibility.rs','src/render/renderer.rs','src/render/gltf_gpu.rs','src/render_overlap.rs','Cargo.toml','Cargo.lock','rust-toolchain.toml','scripts/render-performance.mjs','scripts/render-performance-lib.mjs','scripts/render-performance-api.mjs','scripts/render-performance-gl-timer.mjs','scripts/render-performance-order.mjs','tests/render-performance.js','tests/render-performance-three.js','tests/render-performance.html','tests/render-performance-three.html']:
  t.add(name,arcname=name,recursive=False)
(d/'source.patch.gz').write_bytes(gzip.compress(subprocess.check_output(['git','diff','HEAD','--','src/scene.rs','src/visibility.rs','src/render/renderer.rs']),mtime=0))
three=pathlib.Path('output/render-performance/three-reference/vendor/three.module.js');(d/'three-r170.module.js.gz').write_bytes(gzip.compress(three.read_bytes(),mtime=0))
(d/'THREE-NOTICE.txt').write_text('Frozen Three.js r170 source included for reproducible implementation audit and diagnostic comparisons. Original copyright2010-2024 Three.js Authors / SPDX MIT header retained. No Three runtime dependency was added to the Rust engine. See https://github.com/mrdoob/three.js/blob/r170/LICENSE .\n')
(d/'index.json').write_text(json.dumps(index,indent=2)+'\n')
manifest={str(p.relative_to(d)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in sorted(d.rglob('*')) if p.is_file() and p.name!='archive-manifest.json'}
(d/'archive-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({'files':len(manifest),'bytes':sum(v['bytes'] for v in manifest.values()),'reports':len(index)}))
