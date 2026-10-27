"""Collect existing serialized samples; no renderer execution or build."""
from pathlib import Path
import gzip, hashlib, json, shutil, statistics
root=Path('output/rim-instanced-three');out=Path('docs/performance/2026-10-26-rim-instanced-three');evidence=out/'evidence'
evidence.mkdir(parents=True,exist_ok=True)
identity=json.loads((root/'driver/identity.json').read_text())
for name,sha in identity.items():assert hashlib.sha256((root/'driver'/name).read_bytes()).hexdigest()==sha,name
shutil.copytree(root/'driver',evidence/'driver',dirs_exist_ok=True)
hashes={};rows=[]
def archive(file,name):
    data=file.read_bytes();dest=evidence/name;dest.parent.mkdir(parents=True,exist_ok=True)
    if not name.endswith('.gz'):dest=Path(str(dest)+'.gz');data=gzip.compress(data,mtime=0)
    dest.write_bytes(data);hashes[str(dest.relative_to(out))]=hashlib.sha256(data).hexdigest()
    gzip.decompress(data)
for directory in ['gl-three-512','wg-512-0','wg-512-1','wg-512-2']:
    report=json.loads((root/directory/'report.json').read_text())
    assert not report['driverInputsChangedDuringRun']
    assert all(not x for x in report['serverErrors'].values())
    assert all(x['complete'] for x in report['cleanup'])
    for name,sha in report['driverIdentity']['files'].items():assert identity[name]==sha,(directory,name)
    for file in (root/directory).iterdir():
        if file.suffix=='.json' or file.name.endswith('.json.gz'):archive(file,directory+'/'+file.name)
    for x in report['results']:
        assert x['load']==512 and not x['profiling']
        if x['label']=='candidate':assert x['metadata']['stress']['representation']=='instanced'
        rows.append({'directory':directory,'id':x['id'],'renderer':'Three.js InstancedMesh WebGL2' if x['label']=='candidate' else 'native '+report['settings']['backend'],
            'fps':1000/x['rafInterval']['mean'],'cpuMedianMs':x['rafCallbackCpu']['median'],
            'actualCalls':x['debug']['drawCalls'],'triangles':x['renderer'].get('triangles') if x['label']=='candidate' else x['work']['gltf']['trianglesDrawn']['perApplicationRaf'],
            'stress':x['metadata'].get('stress'),'warmupMs':report['measurementProtocol']['warmup'],'sampleMs':report['measurementProtocol']['sample']})
verification=json.loads((root/'checks/browser-production-final.json').read_text())
assert all(x['status']=='PASS' for x in verification['results'])
archive(root/'checks/browser-production-final.json','browser-production-final.json')
for file in (root/'checks').glob('*.log'):shutil.copy2(file,evidence/file.name)
summary={'scope':'Apple M1 desktop 2880x1920, same static authored GLBs/count/positions; AA/shader/culling policies remain unmatched; no physical Quest evidence',
    'load':512,'records':rows,'medians':{name:statistics.median(x['fps'] for x in rows if x['renderer']==name)for name in sorted({x['renderer']for x in rows})},
    'appRelease':verification['production']['releaseId'],'build':verification['build'],
    'verification':[{'suite':x['suite'],'backend':x['backend'],'status':x['status']}for x in verification['results']],
    'threeOracle':next(x['threeStressEvidence']for x in verification['results']if x['suite']=='rim-three-stress')}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');(out/'archive-sha256.json').write_text(json.dumps(hashes,indent=2)+'\n')
print(json.dumps({'records':len(rows),'medians':summary['medians'],'archives':len(hashes),'app':summary['appRelease']}))
