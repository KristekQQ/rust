"""Bake flat ink boundaries into the can's cylindrical mesh using Blender's constrained triangulation."""
import json, math, sys
from pathlib import Path
from mathutils import Vector
from mathutils.geometry import delaunay_2d_cdt

paths = []
def paint(ink, points):
    paths.append((ink, points))

def text(word,x,y,w,h,ink,slant=.10):
    # Rectilinear screen-print letterforms, expressed as merged horizontal strokes.
    glyphs={
        'F':['111','100','110','100','100'], 'R':['110','101','110','101','101'],
        'O':['111','101','101','101','111'], 'S':['111','100','111','001','111'],
        'T':['111','010','010','010','010'], 'K':['101','101','110','101','101'],
        'I':['111','010','010','010','111'], 'C':['111','100','100','100','111'],
        '0':['111','101','101','101','111'], '%':['101','001','010','100','101'],
    }
    dx=w/(4*len(word)-1);dy=h/5
    for n,c in enumerate(word):
        if c==' ':continue
        for row,line in enumerate(glyphs[c]):
            for col in range(3):
                if line[col]!='1' or col and line[col-1]=='1':continue
                end=col+1
                while end<3 and line[end]=='1':end+=1
                yy=y+(4-row)*dy
                l=x+(n*4+col)*dx+(yy-y)*slant
                rr=l+(end-col)*dx
                paint(ink,[(l,yy),(rr,yy),(rr+dy*slant,yy+dy),(l+dy*slant,yy+dy)])

# Frost Kick product reference: cyan lettering on pearl white, deep dark offset,
# thin lime keyline, ink drips, diagonal magenta slash and two-line flavour mark.
paint('pink',[(26.4,7),(30.2,31.4),(30.4,31.8),(26.6,8.1)])
for x,y,w,h in [(13.5,19,.17,3),(14.1,14,.18,4),(16.0,11,.1,2),(19.1,15,.19,5),(21.6,13,.14,4),(22,13,.17,4.2),(23.2,16,.09,2.8)]:
    paint('dark',[(x,y),(x+w,y-0.3),(x+w*.8,y-h),(x+w*.32,y-h-.4)])
x_shape=[(13,34),(15.65,34.15),(17.58,27.55),(19.35,33.85),(21.65,34.05),(18.62,23.15),(20.89,15.15),(18.05,15.55),(16.73,20.6),(15.07,14.7),(12.48,15.35),(15.6,24.3)]
p_shape=[(21.7,34.05),(26.15,34.6),(27.48,32.8),(26.85,25.05),(25.42,22.1),(23.24,21.75),(22.59,15.5),(20.58,14.95)]
hole=[(23.72,30.55),(25.21,30.7),(24.88,26.6),(23.36,26.55)]
for name,ox,oy,k in [('dark',.24,-1.05,1.045),('lime',0,.12,1.025),('cyan',0,0,1)]:
    for poly in [x_shape,p_shape]:
        cx=sum(x for x,y in poly)/len(poly);cy=sum(y for x,y in poly)/len(poly)
        paint(name,[(cx+(x-cx)*k+ox,cy+(y-cy)*k+oy) for x,y in poly])
paint('dark',hole)
paint('ice',[(14.4,32.65),(14.86,32.53),(15.30,31),(14.99,31.24)])
paint('ice',[(24.53,33.1),(26.03,33.4),(25.85,32.95)])
text('0%',17.3,10.2,4.6,2.7,'dark',0)
text('FROST',15.05,6.0,10.0,2.4,'gray')
text('KICK',15.1,1.75,9.75,3.55,'cyan')
# Side information plate and a compact barcode.
for j in range(7):
    x=30.7; yy=26.0+j*.61
    paint('gray',[(x,yy),(x+3.1-(j%3)*.2,yy),(x+3.1-(j%3)*.2,yy+.16),(x,yy+.16)])
for j,w in enumerate([.11,.16,.07,.20,.08,.14,.09,.15,.18,.08,.11]):
    x=30.8+j*.25
    paint('dark',[(x,13.4),(x+w,13.4),(x+w,19.1),(x,19.1)])

vertices=[];edges=[];lookup={}
def vid(x,y):
    key=(round(x,6),round(y,6))
    if key not in lookup:
        lookup[key]=len(vertices);vertices.append(Vector(key))
    return lookup[key]
def edge(a,b):
    if a!=b:edges.append((a,b))
for i in range(41):
    edge(vid(i,0),vid(i,40))
    if i:edge(vid(i-1,0),vid(i,0));edge(vid(i-1,40),vid(i,40))
for ink,path in paths:
    ids=[vid(*p) for p in path]
    for a,b in zip(ids,ids[1:]+ids[:1]):edge(a,b)
verts,_,faces,*_=delaunay_2d_cdt(vertices,edges,[],0,0.000003)
def inside(x,y,poly):
    odd=False
    for (x1,y1),(x2,y2) in zip(poly,poly[1:]+poly[:1]):
        if (y1>y)!=(y2>y) and x<(x2-x1)*(y-y1)/(y2-y1)+x1: odd=not odd
    return odd
palette=['pearl','dark','cyan','lime','pink','gray','ice']
packed=[]
for a,b,c in faces:
    center=(verts[a]+verts[b]+verts[c])/3
    tint=next((ink for ink,poly in reversed(paths) if inside(*center,poly)),'pearl')
    packed.extend([a,b,c,palette.index(tint)])
uv=[round(float(axis),6) for v in verts for axis in v]
dest=Path(sys.argv[-1])
dest.write_text('// Generated from the wrapped screen-print artwork. UV units span 0..40.\nexport const uv='+json.dumps(uv,separators=(',',':'))+';\nexport const faces='+json.dumps(packed,separators=(',',':'))+';\n')
print(f'{len(faces)} print triangles, {len(verts)} vertices')
