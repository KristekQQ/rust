import json,pathlib,struct,math,hashlib
r=pathlib.Path(__file__).resolve().parents[3];a=r/'assets/models/cyberpunk-alley';l=json.loads((a/'layout.json').read_text());b=(a/'cyberpunk-alley.glb').read_bytes();n,t=struct.unpack_from('<II',b,12);g=json.loads(b[20:20+n]);p=l['landingPads'];seen={0}
while True:
 s=seen|{j for i in seen for j,q in enumerate(p) if math.dist(p[i]['position'],q['position'])<=10}
 if s==seen:break
 seen=s
assert len(seen)==len(p)
for q in [dict(position=l['spawn'])]+p+l['modelSpawns']:
 for o in l['obstacles']:assert not all(abs(q['position'][k]-o['center'][k])<o['halfExtents'][k]+.4 for k in [0,2]),(q,o)
for o in l['obstacles']:
 x,y,z=o['center'];hx,hy,hz=o['halfExtents'];assert not(x+hx>-2 and x-hx<2 and z+hz>-20 and z-hz<1)
pr=sum(len(m['primitives']) for m in g['meshes']);tri=sum(g['accessors'][p['indices']]['count']//3 for m in g['meshes'] for p in m['primitives']);assert tri<100000 and pr<35
assert all('uri' not in v for v in g.get('buffers',[])) and not g.get('images')
assert not g.get('extensions',{}).get('KHR_lights_punctual')
report=dict(checks=['pad graph connected with hops <=10m','spawn/pads/model positions clear with 0.4m radius','central lane clear','triangle and primitive budgets','self-contained GLB; no external buffers or images; no GLB lights'],triangles=tri,primitives=pr,pads=len(p),groundObstacles=len(l['obstacles']),sha256={f:hashlib.sha256((a/f).read_bytes()).hexdigest() for f in ['cyberpunk-alley.glb','layout.json']})
(r/'output/cyberpunk-alley').mkdir(parents=True,exist_ok=True)
(r/'output/cyberpunk-alley/layout-checks.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
