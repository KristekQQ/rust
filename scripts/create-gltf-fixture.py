#!/usr/bin/env python3
"""Deterministic core glTF fixture: external PNG, UV1, blend/mask, morph and cameras."""
import json, struct, zlib
from pathlib import Path
out=Path(__file__).resolve().parents[1]/'tests/fixtures/gltf/full'
out.mkdir(parents=True,exist_ok=True)
binary=bytearray(); views=[]; accessors=[]
def accessor(values,kind,components,typ=5126):
    while len(binary)%4: binary.append(0)
    start=len(binary); flat=[n for v in values for n in v] if components>1 else values
    binary.extend(struct.pack('<'+('f' if typ==5126 else 'H')*len(flat),*flat))
    view=len(views); views.append({'buffer':0,'byteOffset':start,'byteLength':len(binary)-start})
    item={'bufferView':view,'componentType':typ,'count':len(values),'type':kind}
    if typ==5126: item.update(min=[min(v[c] for v in values) for c in range(components)] if components>1 else [min(values)],max=[max(v[c] for v in values) for c in range(components)] if components>1 else [max(values)])
    index=len(accessors);accessors.append(item);return index
pos=accessor([[-1,0,0],[1,0,0],[1,2,0],[-1,2,0]],'VEC3',3)
norm=accessor([[0,0,1]]*4,'VEC3',3)
uv=accessor([[0,1],[1,1],[1,0],[0,0]],'VEC2',2)
indices=accessor([0,1,2,0,2,3],'SCALAR',1,5123)
morph=accessor([[0,0,0],[0,0,0],[0,1,0],[0,1,0]],'VEC3',3)
time=accessor([0,1,2],'SCALAR',1); weights=accessor([0,1,0],'SCALAR',1)
primitives=[{'attributes':{'POSITION':pos,'NORMAL':norm,'TEXCOORD_1':uv},'indices':indices,'material':i,'targets':[{'POSITION':morph}]} for i in range(3)]
doc={'asset':{'version':'2.0','generator':'rust engine core fixture'},'buffers':[{'uri':'full.bin','byteLength':len(binary)}],'bufferViews':views,'accessors':accessors,
'images':[{'uri':'checker.png'}],'samplers':[{'magFilter':9728,'minFilter':9984,'wrapS':33648,'wrapT':33071}],'textures':[{'source':0,'sampler':0}],
'materials':[{'pbrMetallicRoughness':{'baseColorTexture':{'index':0,'texCoord':1},'metallicFactor':0,'roughnessFactor':0.6},'doubleSided':True,'alphaMode':mode,'alphaCutoff':0.5} for mode in ['OPAQUE','MASK','BLEND']],
'meshes':[{'primitives':[p],'weights':[0]} for p in primitives],
'nodes':[{'name':'opaque','mesh':0,'translation':[-3,0,0]},{'name':'mask','mesh':1},{'name':'blend','mesh':2,'translation':[3,0,0]},{'name':'camera','camera':0,'translation':[0,3,12]}],
'cameras':[{'type':'perspective','perspective':{'yfov':0.65,'znear':0.1}}],
'animations':[{'name':'Morph','samplers':[{'input':time,'output':weights,'interpolation':'LINEAR'}],'channels':[{'sampler':0,'target':{'node':i,'path':'weights'}} for i in range(3)]}],
'scenes':[{'name':'All','nodes':[0,1,2,3]},{'name':'Single','nodes':[1,3]}],'scene':0}
(out/'full.bin').write_bytes(binary);(out/'full.gltf').write_text(json.dumps(doc,indent=2)+'\n')
def chunk(t,data):return struct.pack('>I',len(data))+t+data+struct.pack('>I',zlib.crc32(t+data)&0xffffffff)
rows=[]
for y in range(8):
    pixels=[]
    for x in range(8):pixels += [240,70,30,255] if (x//2+y//2)%2 else [30,180,240,50]
    rows.append(b'\0'+bytes(pixels))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',8,8,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows)))+chunk(b'IEND',b'')
(out/'checker.png').write_bytes(png)
