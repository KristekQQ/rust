"""Validate exported geometry, complete loop clips and teleport connectivity."""
import json,math,pathlib,struct
ROOT=pathlib.Path(__file__).resolve().parent
layout=json.loads((ROOT/'layout.json').read_text());pads=layout['landingPads'];obs=layout['obstacles']
assert pads[0]['position']==[0,0,0]
assert layout['walkableBounds']==[-50,50,-100,20]
def blocked(a,b,o):
 lo=0;hi=1
 for axis in [0,2]:
  c=o['center'][axis];h=o['halfExtents'][axis]+.42;d=b[axis]-a[axis]
  if abs(d)<1e-9:
   if not c-h<=a[axis]<=c+h:return False
  else:
   u,v=(c-h-a[axis])/d,(c+h-a[axis])/d;lo=max(lo,min(u,v));hi=min(hi,max(u,v))
   if lo>hi:return False
 return True
adj=[[] for _ in pads]
for i,p in enumerate(pads):
 assert p['position'][1]==0
 assert not any(blocked(p['position'],p['position'],o) for o in obs),('pad inside obstacle',i)
 for j,q in enumerate(pads[:i]):
  if math.dist(p['position'],q['position'])<=7 and not any(blocked(p['position'],q['position'],o) for o in obs):adj[i].append(j);adj[j].append(i)
seen={0};todo=[0]
while todo:
 for j in adj[todo.pop()]:
  if j not in seen:seen.add(j);todo.append(j)
assert len(seen)==len(pads),('disconnected pads',set(range(len(pads)))-seen)
for slug in ['forest-arena','golem','treant','mushroom']:
 raw=(ROOT/(slug+'.glb')).read_bytes();n=struct.unpack_from('<I',raw,12)[0];doc=json.loads(raw[20:20+n]);binary=raw[28+n:]
 primitives=[p for m in doc['meshes'] for p in m['primitives']];triangles=sum(doc['accessors'][p['indices']]['count']//3 for p in primitives)
 if slug=='forest-arena':assert len(primitives)<=40;assert triangles<=80000
 else:
  animations=doc['animations'];assert len(animations)==1 and animations[0]['name']=='Idle'
  anim=animations[0];animated=set(c['target']['node'] for c in anim['channels']);mesh_nodes={i for i,x in enumerate(doc['nodes']) if 'mesh' in x};assert mesh_nodes<=animated
  for s in anim['samplers']:
   a=doc['accessors'][s['output']];v=doc['bufferViews'][a['bufferView']];count={'VEC3':3,'VEC4':4}[a['type']];offset=v.get('byteOffset',0)+a.get('byteOffset',0);stride=v.get('byteStride',count*4);first=struct.unpack_from('<'+'f'*count,binary,offset);last=struct.unpack_from('<'+'f'*count,binary,offset+(a['count']-1)*stride);assert all(abs(x-y)<1e-5 for x,y in zip(first,last)),(slug,'loop seam')
 print(slug,triangles,'triangles',len(primitives),'primitives',len(raw),'bytes')
print('PASS:',len(pads),'connected pads;',len(obs),'geometry footprints; 3 complete seamless Idle clips')
