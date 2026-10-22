#!/usr/bin/env python3
"""Standard-library GLB geometry/animation and variant integrity checks."""
import json, math, struct, hashlib
from pathlib import Path
HERE=Path(__file__).resolve().parent

def load(p):
    raw=p.read_bytes();magic,version,length=struct.unpack_from('<4sII',raw)
    assert magic==b'glTF'and version==2 and length==len(raw)
    n=struct.unpack_from('<I',raw,12)[0];return json.loads(raw[20:20+n]),raw[28+n:]

def values(d,b,i):
    a=d['accessors'][i];v=d['bufferViews'][a['bufferView']];n={'SCALAR':1,'VEC3':3,'VEC4':4}[a['type']]
    assert a['componentType']==5126
    start=v.get('byteOffset',0)+a.get('byteOffset',0);stride=v.get('byteStride',n*4)
    return [struct.unpack_from('<'+'f'*n,b,start+k*stride)for k in range(a['count'])]

def transform(n,p):
    if'matrix'in n:
        m=n['matrix'];return [sum(m[c*4+r]*([*p,1][c])for c in range(4))for r in range(3)]
    s=n.get('scale',[1]*3);v=[p[j]*s[j]for j in range(3)];x,y,z,w=n.get('rotation',[0,0,0,1]);qx,qy,qz=x,y,z
    uv=[qy*v[2]-qz*v[1],qz*v[0]-qx*v[2],qx*v[1]-qy*v[0]]
    uuv=[qy*uv[2]-qz*uv[1],qz*uv[0]-qx*uv[2],qx*uv[1]-qy*uv[0]]
    t=n.get('translation',[0]*3);return [v[j]+2*(w*uv[j]+uuv[j])+t[j]for j in range(3)]

reports=[]
for item in json.loads((HERE/'manifest.json').read_text()):
    slug=item['model'];d,b=load(HERE/(slug+'.glb'));parents={c:i for i,n in enumerate(d['nodes'])for c in n.get('children',[])};pts=[]
    for i,node in enumerate(d['nodes']):
        if'mesh'not in node:continue
        for pr in d['meshes'][node['mesh']]['primitives']:
            assert 'NORMAL' in pr['attributes'], (slug, 'missing normals')
            normals=values(d,b,pr['attributes']['NORMAL'])
            assert len(normals)==d['accessors'][pr['attributes']['POSITION']]['count']
            assert all(all(math.isfinite(x) for x in n) and abs(sum(x*x for x in n)-1)<.002 for n in normals), (slug,'invalid normals')
            for p in values(d,b,pr['attributes']['POSITION']):
                k=i
                while True:
                    p=transform(d['nodes'][k],p)
                    if k not in parents:break
                    k=parents[k]
                pts.append(p)
    bounds=[[min(p[j]for p in pts)for j in range(3)],[max(p[j]for p in pts)for j in range(3)]]
    assert max(abs(bounds[i][j]-item['bounds'][i][j])for i in range(2)for j in range(3))<.0001,(slug,bounds,item['bounds'])
    animated={i for i,n in enumerate(d['nodes'])if n.get('name','').endswith(' pivot')}
    def state(clip,t):
        import copy
        nodes=copy.deepcopy(d['nodes'])
        for ch in clip['channels']:
            sa=clip['samplers'][ch['sampler']];ts=[v[0]for v in values(d,b,sa['input'])];vs=values(d,b,sa['output']);time=t*ts[-1]
            k=next((i for i in range(len(ts)-1)if ts[i]<=time<=ts[i+1]),len(ts)-2);u=(time-ts[k])/(ts[k+1]-ts[k])
            a,z=vs[k],vs[k+1]
            if ch['target']['path']=='rotation'and sum(x*y for x,y in zip(a,z))<0:z=[-v for v in z]
            v=[x+(y-x)*u for x,y in zip(a,z)]
            if ch['target']['path']=='rotation':norm=math.sqrt(sum(x*x for x in v));v=[x/norm for x in v]
            nodes[ch['target']['node']][ch['target']['path']]=v
        return nodes
    def equivalent(a,z):
        for i in animated:
            for p,default in [('translation',[0,0,0]),('rotation',[0,0,0,1]),('scale',[1,1,1])]:
                x=a[i].get(p,default);y=z[i].get(p,default)
                if p=='rotation'and sum(m*n for m,n in zip(x,y))<0:y=[-v for v in y]
                if max(abs(m-n)for m,n in zip(x,y))>.00015:return False
        return True
    clips={a['name']:a for a in d['animations']};assert set(clips)=={'Idle','Run','Attack','Attack2','Attack3','Attack4','Explode','Assemble'}
    for name,clip in clips.items():
        covered={(c['target']['node'],c['target']['path'])for c in clip['channels']}
        assert all((i,p)in covered for i in animated for p in ['translation','rotation','scale'])
        changed=False
        for c in clip['channels']:
            sam=clip['samplers'][c['sampler']];ts=values(d,b,sam['input']);vs=values(d,b,sam['output'])
            assert ts[0][0]==0 and ts[-1][0]>1
            changed|=any(max(abs(x-y)for x,y in zip(v,vs[0]))>.001 for v in vs)
            if c['target']['path']=='rotation':assert all(abs(sum(x*x for x in v)-1)<.0001 for v in vs)
        assert changed,(slug,name,'static')
        if name not in ['Explode','Assemble']:assert equivalent(state(clip,0),state(clip,1)),(slug,name,'not recovered')
        if name!='Assemble':assert equivalent(state(clip,0),d['nodes']),(slug,name,'nonneutral start')
    assert equivalent(state(clips['Explode'],1),state(clips['Assemble'],0)),(slug,'assemble debris mismatch')
    assert equivalent(state(clips['Assemble'],1),d['nodes']),(slug,'assemble not neutral')
    assert not equivalent(state(clips['Explode'],0),state(clips['Explode'],1)),(slug,'death looped')
    assert equivalent(state(clips['Explode'],.8),state(clips['Explode'],1)),(slug,'debris not held')
    for a,z in [(.2,.5),(.5,.85),(.2,.85)]:
        assert not equivalent(state(clips['Attack'],a),state(clips['Attack'],z)),(slug,'attack phases identical')
    named={n.get('name'):i for i,n in enumerate(d['nodes'])}
    assert 'Motion pivot'in named
    if slug=='blade-wraith':
        for side in ['Left','Right']:
            assert parents[named[side+' blade forearm pivot']]==named[side+' shoulder pivot']
            assert parents[named[side+' blade wrist pivot']]==named[side+' blade forearm pivot']
            assert parents[named[side+' shin pivot']]==named[side+' thigh pivot']
            assert parents[named[side+' thigh pivot']]==named['Pelvis pivot']
            assert parents[named[side+' shoulder pivot']]==named['Torso pivot']
    else:
        body=named['Reactor pivot'if slug=='drone-orbiter'else'Heavy reactor pivot']
        assert parents[body]==named['Motion pivot']
    if slug!='blade-wraith':
        bolts=[i for i,n in enumerate(d['nodes'])if 'Plasma projectile' in n.get('name','') and n.get('name','').endswith(' pivot')]
        assert len(bolts)==6
        for name in ['Attack2','Attack3','Attack4']:
            shots=[state(clips[name],t/100) for t in range(101)]
            assert any(max(nodes[i]['scale'])>.7 for nodes in shots for i in bolts),(slug,name,'no visible shots')
            assert any(abs(nodes[i]['translation'][2]-d['nodes'][i]['translation'][2])>2 for nodes in shots for i in bolts),(slug,name,'shots do not travel')
        assert all(max(state(clips['Idle'],.5)[i]['scale'])<.002 for i in bolts)
    if slug=='core-tank':
        flashes=[i for i,n in enumerate(d['nodes'])if'muzzle flash pivot'in n.get('name','')]
        assert len(flashes)==2
        for i in flashes:
            ch=next(c for c in clips['Attack']['channels']if c['target']=={'node':i,'path':'scale'})
            assert max(v[0]for v in values(d,b,clips['Attack']['samplers'][ch['sampler']]['output']))>.7
            assert max(state(clips['Idle'],.5)[i]['scale'])<.002
    for color in ['pink','blue','green','amber']:
        vd,vb=load(HERE/(slug+'-'+color+'.glb'));assert vb==b
        assert vd['nodes']==d['nodes']and vd['animations']==d['animations']and vd['meshes']==d['meshes']
    reports.append({'model':slug,'bounds':bounds,'triangles':item['triangles'],'primitives':item['primitives'],'clips':item['clips'],'sha256':hashlib.sha256((HERE/(slug+'.glb')).read_bytes()).hexdigest()})
print(json.dumps(reports,indent=2));print('PASS: neutral FK bounds, complete clip resets, authored nonstatic quaternion clips, articulated hierarchy, held death and reverse Assemble, muzzle scale resets, four identical-geometry variants/model')
