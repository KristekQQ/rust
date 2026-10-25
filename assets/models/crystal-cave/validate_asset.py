"""Dependency-free exported GLB/layout contract validation."""
import json, pathlib, struct
ROOT=pathlib.Path(__file__).resolve().parent
raw=(ROOT/'crystal-cave.glb').read_bytes()
magic,version,length=struct.unpack_from('<III',raw)
assert magic==0x46546c67 and version==2 and length==len(raw)
n=struct.unpack_from('<I',raw,12)[0];g=json.loads(raw[20:20+n]);layout=json.loads((ROOT/'layout.json').read_text())
primitives=[p for m in g['meshes'] for p in m['primitives']]
triangles=sum(g['accessors'][p['indices']]['count']//3 for p in primitives)
assert triangles<25000
assert len(primitives)<=8 and len(g['materials'])<=8
assert len(g.get('textures',[]))==0
assert all('rotation' not in n and 'translation' not in n and 'scale' not in n for n in g['nodes'])
a=[g['accessors'][p['attributes']['POSITION']] for p in primitives]
bounds=[[min(v['min'][i] for v in a) for i in range(3)],[max(v['max'][i] for v in a) for i in range(3)]]
assert bounds[0][1]<=0 and bounds[1][1]>=9
assert bounds[0][2]<-34 and bounds[1][2]>=4
assert layout['walkableBounds']==[-7,7,-31,2] and layout['spawn']==[0,0,0]
for o in layout['obstacles']:
 x,_,z=o['center'];hx,_,hz=o['halfExtents'];assert x+hx<=-2 or x-hx>=2,o['id']
# Every side stalagmite has a box, including the previously passable right z=-16.
by_id={o['id']:o for o in layout['obstacles']}
for side in ['left','right']:
 for z in [6,16,24,30]:
  o=by_id['stalagmite-%s-%s'%(side,z)]
  assert o['halfExtents']==[.55,1.45,.5]
  assert o['center']==[(-7.1 if side=='left' else 7.1)+.05,1.45,-z]
o=by_id['stalagmite-right-16']
assert abs(6.9-o['center'][0])<o['halfExtents'][0] and abs(-16-o['center'][2])<o['halfExtents'][2]
for p in layout['landingPads']:
 x,y,z=p['position'];r=p['radius'];assert y==0 and -7+r<=x<=7-r and -31+r<=z<=2-r
 for o in layout['obstacles']:
  ox,_,oz=o['center'];hx,_,hz=o['halfExtents'];dx=max(abs(x-ox)-hx,0);dz=max(abs(z-oz)-hz,0);assert dx*dx+dz*dz>=r*r,p['id']
report=dict(triangles=triangles,primitives=len(primitives),materials=len(g['materials']),bounds=bounds,landingPads=len(layout['landingPads']),obstacles=len(layout['obstacles']),glbBytes=len(raw))
(ROOT/'asset-report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
