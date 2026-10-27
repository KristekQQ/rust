"""Archive existing measurements; never starts a browser or rebuilds a package."""
from pathlib import Path
import gzip, hashlib, json, shutil, statistics

source = Path('output/rim-stress')
base = Path('docs/performance/2026-10-26-rim-stress')
evidence = base / 'evidence'
evidence.mkdir(parents=True, exist_ok=True)
identity = json.loads((source / 'measurement-driver/identity.json').read_text())
for name, sha in identity['files'].items():
    assert hashlib.sha256((source / 'measurement-driver' / name).read_bytes()).hexdigest() == sha, name
shutil.copytree(source / 'measurement-driver', evidence / 'measurement-driver', dirs_exist_ok=True)
archive_hashes = {}
def archive(file, name):
    data = file.read_bytes()
    dest = evidence / (name + '.gz')
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(gzip.compress(data, mtime=0))
    assert gzip.decompress(dest.read_bytes()) == data
    archive_hashes[str(dest.relative_to(base))] = hashlib.sha256(dest.read_bytes()).hexdigest()

records = []
for run in ['smoke', 'cal-wg', 'wg-256', 'wg-512-0', 'wg-512-1', 'wg-512-2', 'wg-1024-pilot', 'cal-gl-fixed-512', 'gl-three-512', 'gpu-profile-512']:
    report = json.loads((source / run / 'report.json').read_text())
    assert not report['driverInputsChangedDuringRun'], run
    assert all(not errors for errors in report['serverErrors'].values()), run
    assert all(x['complete'] for x in report['cleanup']), run
    for name, sha in report['driverIdentity']['files'].items():
        if run in ['smoke', 'cal-wg']:
            original = source / 'native' / name
            assert hashlib.sha256(original.read_bytes()).hexdigest() == sha, (run, name)
            target = evidence / 'early-driver' / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, target)
        else:
            assert identity['files'][name] == sha, (run, name)
    for file in (source / run).glob('*.json'):
        archive(file, run + '/' + file.name)
    for x in report['results']:
        records.append(dict(run=run, id=x['id'], load=x['load'], label=x['label'], backend=x['backend'],
            warmupMs=report['measurementProtocol']['warmup'], sampleMs=report['measurementProtocol']['sample'],
            profiling=x['profiling'], observedRafFps=1000 / x['rafInterval']['mean'],
            callbackCpuMedianMs=x['rafCallbackCpu']['median'],
            actualDrawCalls=x['debug']['drawCalls'],
            submittedTrianglesPerFrame=x.get('work', {}).get('gltf', {}).get('trianglesDrawn', {}).get('perApplicationRaf') or x['renderer'].get('triangles'),
            gpuSamples=x['gpuSamples'], gpuPassMs=x['gpuPassMs']))
for name in ['browser-production-final.json', 'stereo-production-last-webgl.json', 'browser-production-rejected.json']:
    archive(source / 'checks' / name, 'checks/' + name)
for name in ['native-rim.log', 'wasm-clippy.log', 'wasm-build.log', 'types-final.log', 'units-complete.log', 'browser-production-final.log', 'button-hit-diagnostic.log']:
    shutil.copy2(source / 'checks' / name, evidence / name)
archive(source / 'native/render-build.json', 'native-render-build.json')
archive(source / 'three-reference/identity.json', 'three-reference-identity.json')
verification = json.loads((source / 'checks/browser-production-final.json').read_text())
assert len(verification['results']) == 8 and all(x['status'] == 'PASS' for x in verification['results'])
summary = dict(scope='Desktop Apple M1, static gate camera, 2880x1920, observed RAF cadence; no physical Quest claim',
    build=verification['build'], appRelease=verification['production']['releaseId'], records=records,
    medianFps={
        'webgpu512': statistics.median(x['observedRafFps'] for x in records if x['run'].startswith('wg-512-')),
        'webgl512': statistics.median(x['observedRafFps'] for x in records if x['run'] == 'gl-three-512' and x['label'] == 'baseline'),
        'three512': statistics.median(x['observedRafFps'] for x in records if x['run'] == 'gl-three-512' and x['label'] == 'candidate')},
    verification=[{k: x[k] for k in ['suite', 'backend', 'status']} for x in verification['results']],
    xrEvidence=[x['rimStressEvidence'] for x in verification['results'] if x['suite'] == 'rim-stress-xr'])
(base / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(base / 'archive-sha256.json').write_text(json.dumps(archive_hashes, indent=2) + '\n')
print(json.dumps(dict(records=len(records), medianFps=summary['medianFps'], archived=len(archive_hashes), appRelease=summary['appRelease'])))
