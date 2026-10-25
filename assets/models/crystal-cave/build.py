"""Original seeded low-poly cave. Blender --background --python build.py.
Authoring helpers accept engine meters (Y up, -Z forward). No textures/downloads.
"""
import bpy, math, random, json, pathlib, struct
from mathutils import Vector
ROOT=pathlib.Path(__file__).resolve().parent
R=random.Random(10931)
bpy.context.preferences.filepaths.save_version=0
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
M={}; obstacles=[]
def mat(n,c,emit=0,metal=0):
 m=bpy.data.materials.new(n);m.diffuse_color=(*c,1);m.use_nodes=True
 p=m.node_tree.nodes.get('Principled BSDF');p.inputs['Base Color'].default_value=(*c,1);p.inputs['Roughness'].default_value=.85 if not emit else .32;p.inputs['Metallic'].default_value=metal
 if emit:p.inputs['Emission Color'].default_value=(*c,1);p.inputs['Emission Strength'].default_value=emit
 M[n]=m
for a in [('slate',(.27,.31,.34)),('ochre',(.32,.245,.17)),('shadow',(.17,.19,.23)),('path',(.30,.265,.205)),('cyan',(.055,.66,.8),1.8),('violet',(.38,.12,.75),1.5),('iron',(.12,.105,.085),0,.5),('amber',(1,.44,.07),2.2)]:mat(*a)
def C(p):return (p[0],-p[2],p[1])
def mesh(n,verts,faces,m):
 d=bpy.data.meshes.new(n);d.from_pydata([C(v) for v in verts],[],faces);d.update();o=bpy.data.objects.new(n,d);bpy.context.collection.objects.link(o);d.materials.append(M[m]);return o
def box(n,p,d,m):
 bpy.ops.mesh.primitive_cube_add(size=1,location=C(p));o=bpy.context.object;o.name=n;o.dimensions=(d[0],d[2],d[1]);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);o.data.materials.append(M[m]);return o
def rock(n,p,s,m='slate',sub=1):
 bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=sub,radius=1,location=C(p));o=bpy.context.object;o.name=n
 for v in o.data.vertices:v.co*=R.uniform(.84,1.16)
 o.scale=(s[0],s[2],s[1]);o.rotation_euler=(R.random()*.3,R.random()*.3,R.random()*6.28);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);o.data.materials.append(M[m]);return o
def rod(n,a,b,r,m,r2=None,sides=6):
 a,b=Vector(C(a)),Vector(C(b));v=b-a;bpy.ops.mesh.primitive_cone_add(vertices=sides,radius1=r,radius2=r if r2 is None else r2,depth=v.length,location=(a+b)/2);o=bpy.context.object;o.name=n;o.rotation_euler=v.to_track_quat('Z','Y').to_euler();o.data.materials.append(M[m]);return o
def crystal(n,p,h,r,m,lean=(0,0)):
 # Hexagonal shaft with asymmetric pointed termination; all real opaque facets.
 vs=[]
 for y,scale in [(0,.8),(h*.73,1)]:
  for i in range(6):
   a=2*math.pi*i/6;vs.append((p[0]+math.cos(a)*r*scale+lean[0]*y,p[1]+y,p[2]+math.sin(a)*r*scale+lean[1]*y))
 vs.append((p[0]+lean[0]*h+r*.15,p[1]+h,p[2]+lean[1]*h))
 faces=[tuple(reversed(range(6)))]+[(i,(i+1)%6,(i+1)%6+6,i+6) for i in range(6)]+[(i+6,(i+1)%6+6,12) for i in range(6)]
 return mesh(n,vs,[tuple(reversed(f)) for f in faces],m)
def obstacle(n,x,z,hx,hz,height):obstacles.append(dict(id=n,center=[x,height/2,z],halfExtents=[hx,height/2,hz]))
# Continuous dry floor. Full cave shell has no missing roof faces.
box('Dry continuous floor',(0,-.18,-15),(18,.36,38),'path')
# Cross-section from left ground to right ground, with an irregular broad barrel vault.
section=[(-8.9,0),(-8.8,2.5),(-8.5,5),(-7.1,7.0),(-4.5,8.6),(0,9.2),(4.5,8.5),(7.2,7),(8.6,4.9),(8.9,2.3),(8.9,0)]
rings=[]
for j in range(20):
 z=4-2*j;r=[]
 for i,(x,y) in enumerate(section):
  r.append((x+R.uniform(-.28,.28),y+R.uniform(-.26,.26) if y else 0,z+R.uniform(-.15,.15)))
 rings.append(r)
for j in range(19):
 for i in range(10):
  a,b,c,d=rings[j][i],rings[j+1][i],rings[j+1][i+1],rings[j][i+1]
  # Inward facing wall and roof surfaces (material remains single sided).
  mesh('Vault facet',[a,b,c,d],[(0,1,2),(0,2,3)],R.choices(['slate','ochre','shadow'],[6,2,2])[0])
# Back wall seals the chamber, textured with faceted large stones.
mesh('Sealed rear wall',rings[-1]+[(0,0,-34)],[(i+1,i,11) for i in range(10)],'shadow')
for side in [-1,1]:
 for j in range(19):
  z=2-j*1.9
  rock('Layered wall outcrop',(side*8.15,R.uniform(1.3,5),z),(R.uniform(.7,1.4),R.uniform(1.3,2.6),R.uniform(1.0,1.65)),R.choice(['slate','ochre','shadow']),2)
 # A few safe floor boulders, each fully included in an obstacle rectangle.
 for j,z in enumerate([-3,-11,-19,-27]):
  x=side*6.35;rock('Side boulder',(x,.6,z),(.8,.9,1.05),'ochre',2);obstacle('boulder-%s-%s'%(side,j),x,z,1.15,1.35,1.8)
# Rough low rims delineate side crystal shelves, comfortably outside central lane.
clusters=[(-5.1,-5,'violet'),(5.25,-8,'cyan'),(-5.15,-14,'cyan'),(5.2,-19,'violet'),(-5.1,-25,'violet'),(4.8,-27,'cyan')]
for k,(x,z,m) in enumerate(clusters):
 rock('Crystal socket',(x,.18,z),(1.1,.4,1.2),'shadow',2)
 for i in range(9):
  a=R.random()*6.28;rr=R.uniform(.1,.82);h=R.uniform(.5,1.8)
  crystal('Side crystal',(x+math.cos(a)*rr,.1,z+math.sin(a)*rr),h,R.uniform(.10,.25),m,(math.cos(a)*.20,math.sin(a)*.20))
 obstacle('crystal-shelf-%d'%k,x,z,1.5,1.5,2.4)
# Monumental cyan geode off the central lane: visible from entrance, end pad stays clear.
for i in range(16):
 a=R.random()*6.28;r=R.uniform(.1,1.25)
 crystal('Heart of the cave',(4.6+math.cos(a)*r,.05,-31+math.sin(a)*r),R.uniform(1.6,4.8),R.uniform(.20,.46),'cyan',(math.cos(a)*.12,math.sin(a)*.12))
rock('Heart socket',(4.6,.2,-31),(1.65,.45,1.65),'slate',2);obstacle('heart-formation',4.6,-31,2.0,2,5.2)
# Hanging stone teeth: lowest point above 4.8m. No head-height hazards.
for i in range(45):
 x=R.uniform(-6.6,6.6);z=R.uniform(-32,2);top=8.2-abs(x)*.16;length=R.uniform(.7,2.2)
 rod('Stalactite',(x,top-length,z),(x+.16,top+.5,z),R.uniform(.04,.10),'ochre',R.uniform(.25,.50))
for side in [-1,1]:
 for z in [-6,-16,-24,-30]:
  x=side*7.1;rod('Wall stalagmite',(x,0,z),(x+.13,R.uniform(1.0,2.8),z),.44,'slate',.03)
  # Conservative full-height footprint includes the tilted base and tip.
  obstacle('stalagmite-%s-%s'%('left' if side<0 else 'right',-z),x+.05,z,.55,.5,2.9)
# Warm entrance lamps and two deeper lanterns with geometric cages.
for k,(x,z) in enumerate([(-5,1),(5,1),(-6,-17),(6,-24)]):
 rod('Lamp post',(x,0,z),(x,2.8,z),.06,'iron')
 box('Lamp footing',(x,.12,z),(.35,.24,.35),'iron')
 rod('Lantern glow',(x,2.25,z),(x,2.70,z),.14,'amber',sides=6)
 for i in range(6):
  a=i*math.pi/3;dx,dz=.18*math.cos(a),.18*math.sin(a)
  rod('Lantern cage',(x+dx,2.20,z+dz),(x+dx,2.78,z+dz),.018,'iron',sides=4)
 rod('Lantern roof',(x,2.76,z),(x,2.92,z),.27,'iron',.04)
 obstacle('lantern-%d'%k,x,z,.3,.3,3)
# Flush mineral trail markers sit outside spawn/pad centers; no raised path geometry.
pads=[(0,0),(0,-6),(0,-12),(0,-18),(0,-24),(0,-30),(-3.2,-9),(3.1,-14),(-3.1,-21)]
for x,z in pads:
 for side in [-1,1]:
  box('Flush trail mineral',(x+side*.82,.003,z),(.10,.006,.42),'cyan')
# Join geometry by material: exactly eight exported material primitives.
for n,m in M.items():
 objs=[o for o in bpy.context.scene.objects if o.type=='MESH' and o.data.materials[0]==m]
 bpy.ops.object.select_all(action='DESELECT')
 for o in objs:o.select_set(True)
 bpy.context.view_layer.objects.active=objs[0];bpy.ops.object.join();o=bpy.context.object;o.name='Cave_'+n
 bpy.ops.object.transform_apply(location=True,rotation=True,scale=True)
 for p in o.data.polygons:p.use_smooth=False
bpy.ops.object.select_all(action='DESELECT')
meshes=[o for o in bpy.context.scene.objects if o.type=='MESH'];triangles=sum(len(p.vertices)-2 for o in meshes for p in o.data.polygons)
layout=dict(version=1,units='meters',upAxis='Y',forwardAxis='-Z',walkableBounds=[-7,7,-31,2],spawn=[0,0,0],obstacles=obstacles,landingPads=[dict(id='landing-%d'%i,position=[x,0,z],radius=.6) for i,(x,z) in enumerate(pads)])
(ROOT/'layout.json').write_text(json.dumps(layout,indent=2)+'\n')
bpy.ops.export_scene.gltf(filepath=str(ROOT/'crystal-cave.glb'),export_format='GLB',export_yup=True,export_lights=False,export_cameras=False,export_animations=False)
# Presentation lighting only; main engine scene supplies its own modest lights.
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=24
scene.world.color=(.11,.13,.17)
def area(n,p,e,c,size,target):
 bpy.ops.object.light_add(type='AREA',location=C(p));o=bpy.context.object;o.name=n;o.data.energy=e;o.data.color=c;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector(C(target))-o.location).to_track_quat('-Z','Y').to_euler()
area('Entrance daylight',(0,5,3),1200,(.65,.73,1),8,(0,2,-10))
for z in [-6,-17,-28]:area('Interior soft fill',(0,6,z),420,(.32,.65,.8),8,(0,1,z))
for x,z,m in clusters:area('Crystal bounce',(x,1.8,z),95,(.15,.8,1) if m=='cyan' else (.52,.22,1),2,(0,1,z))
for x,z in [(-5,1),(5,1),(-6,-17),(6,-24)]:area('Warm lantern bounce',(x,2.6,z),85,(1,.43,.12),2,(0,1,z-2))
area('Heart cyan bounce',(4,3,-30),180,(.1,.8,1),3,(0,2,-28))
bpy.ops.object.camera_add(location=C((0,1.72,1.2)));cam=bpy.context.object;cam.name='Interior preview';cam.rotation_euler=(Vector(C((.4,3.0,-18)))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.lens=18;scene.camera=cam
scene.view_settings.view_transform='AgX';scene.render.resolution_x=1400;scene.render.resolution_y=1000;scene.render.resolution_percentage=100
scene.render.filepath=str(ROOT/'interior-preview.png');bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'crystal-cave.blend'));bpy.ops.render.render(write_still=True)
report=dict(triangles=triangles,meshes=len(meshes),materials=len(M),glbBytes=(ROOT/'crystal-cave.glb').stat().st_size,seed=10931)
(ROOT/'asset-report.json').write_text(json.dumps(report,indent=2)+'\n');print('CAVE_ASSET',json.dumps(report))
