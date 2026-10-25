"""Original procedural outdoor city alley. Run Blender --background --python this.py."""
import bpy, math, json, random, pathlib, sys
from mathutils import Vector
ROOT=pathlib.Path(__file__).resolve().parent
OUT=ROOT.parents[2]/'output'/'cyberpunk-alley'
OUT.mkdir(parents=True,exist_ok=True)
bpy.context.preferences.filepaths.save_version=0
random.seed(27)
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
M={}
def mat(n,c,metal=0,rough=.65,emit=0):
 m=bpy.data.materials.new(n); m.diffuse_color=(*c,1); m.use_nodes=True
 p=m.node_tree.nodes.get('Principled BSDF'); p.inputs['Base Color'].default_value=(*c,1);p.inputs['Metallic'].default_value=metal;p.inputs['Roughness'].default_value=rough
 if emit:p.inputs['Emission Color'].default_value=(*c,1);p.inputs['Emission Strength'].default_value=emit
 M[n]=m
for args in [('asphalt',(.065,.09,.12),.15,.65),('concrete',(.19,.25,.32),.1,.8),('blue_facade',(.12,.21,.29),.35,.5),('warm_facade',(.27,.24,.25),.2,.7),('metal',(.085,.12,.16),.7,.38),('silver',(.3,.4,.45),.7,.38),('window',(.025,.055,.08),.6,.24),('lit_window',(.64,.43,.21),.3,.4,1.25),('cyan',(.04,.75,.85),.25,.35,3),('pink',(.9,.075,.32),.2,.4,3),('amber',(.95,.4,.055),.2,.4,2),('paint',(.65,.68,.62),0,.8)]:mat(*args)
def coord(p):return(p[0],-p[2],p[1])
def box(n,p,d,m,bevel=0):
 bpy.ops.mesh.primitive_cube_add(size=1,location=coord(p));o=bpy.context.object;o.name=n;o.dimensions=(d[0],d[2],d[1]);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);o.data.materials.append(M[m])
 if bevel:
  mod=o.modifiers.new('Edge highlights','BEVEL');mod.width=bevel;mod.segments=1;bpy.ops.object.modifier_apply(modifier=mod.name)
 return o
def pipe(n,points,r,m,sides=8):
 for a,b in zip(points,points[1:]):
  a,b=Vector(coord(a)),Vector(coord(b));v=b-a
  bpy.ops.mesh.primitive_cylinder_add(vertices=sides,radius=r,depth=v.length,location=(a+b)/2);o=bpy.context.object;o.name=n;o.rotation_euler=v.to_track_quat('Z','Y').to_euler();o.data.materials.append(M[m])
def text(s,p,size,m,face=0):
 bpy.ops.object.text_add(location=coord(p),rotation=(math.pi/2,0,face));o=bpy.context.object;o.name='Mesh lettering '+s;o.data.body=s;o.data.size=size;o.data.align_x='CENTER';o.data.extrude=.012;o.data.bevel_depth=.003;o.data.bevel_resolution=0;o.data.materials.append(M[m]);bpy.ops.object.convert(target='MESH')
obstacles=[]
def obstacle(n,p,d,m='metal',b=.04):
 o=box(n,p,d,m,b);obstacles.append(dict(id=n,center=list(p),halfExtents=[v/2 for v in d]));return o
# Continuous pavement at exact navigation y=0, with inlaid gutters and road markings.
box('Continuous planar street',(0,-.15,-12),(32,.3,44),'asphalt')
for x in [-10.8,10.8]:
 box('Inlaid drainage channel',(x,.002,-12),(.24,.005,43),'metal')
 for z in range(-32,9,2):box('Drain slot',(x,.006,z),(.19,.007,.055),'silver')
for z in range(-30,8,4):
 for x in [-3.9,3.9]:box('Worn lane stripe',(x,.004,z),(.07,.008,1.2),'paint')
# Asymmetric skyline. Facades occupy outer strips; stepped wings form alcoves.
for side in [-1,1]:
 for i,z in enumerate([4,-5,-15,-26]):
  depth=[7,8,9,10][i];h=([18,24,21,30] if side<0 else [26,17,28,22])[i]
  x=side*(13.6+([.8,1.8,0,.6][i] if side<0 else [0,.6,1.7,0][i]))
  obstacle('Building %s %s'%(side,i),(x,h/2,z),(4.4,h,depth),'blue_facade' if (i+side)%2 else 'warm_facade',.09)
  inner=x-side*2.2
  box('Stone footing',(inner-side*.12,.45,z),(.28,.9,depth),'concrete',.025)
  for floor in range(1,int(h/3.3)):
   y=floor*3.3
   box('Floor cornice',(inner-side*.13,y,z),(.3,.18,depth+.25),'silver')
   for dz in [-depth*.32,0,depth*.32]:
    box('Recessed window',(inner-side*.035,y+1.3,z+dz),(.10,1.65,1.45),'window')
    if random.random()<.32:box('Occupied window',(inner-side*.095,y+1.3,z+dz),(.025,1.36,1.16),'lit_window')
    box('Window mullion',(inner-side*.115,y+1.3,z+dz),(.04,1.7,.065),'metal')
   if floor in [2,4] and i%2==0:
    box('Cantilever balcony',(inner-side*.75,y-.15,z),(1.65,.22,depth*.7),'metal',.035)
    pipe('Balcony rail',[(inner-side*1.5,y+.8,z-depth*.35),(inner-side*1.5,y+.8,z+depth*.35)],.045,'silver')
    for dz in [-depth*.35,0,depth*.35]:pipe('Rail upright',[(inner-side*1.5,y,z+dz),(inner-side*1.5,y+.8,z+dz)],.035,'silver')
  box('Roof parapet',(x,h+.35,z),(4.6,.7,depth+.2),'metal',.04)
  box('Roof mechanical penthouse',(x,h+1.6,z+.8),(2,2.5,2.6),'concrete',.08)
  pipe('Roof antenna',[(x,h+2,z),(x,h+5,z)],.055,'silver')
  pipe('Facade utility pipe',[(inner-side*.28,.6,z-depth*.38),(inner-side*.28,h+.8,z-depth*.38),(x,h+.8,z-depth*.38)],.09,'silver')
  # Shop frames and real shutter slats at street level.
  for dz in [-depth*.23,depth*.23]:
   box('Shopfront recess',(inner-side*.12,1.8,z+dz),(.17,2.6,depth*.36),'window')
   for k in range(12):box('Roller shutter',(inner-side*.23,.6+k*.19,z+dz),(.11,.12,depth*.34),'silver')
   box('Shop lintel',(inner-side*.3,3.35,z+dz),(.28,.38,depth*.4),'metal',.03)
  if i in [0,2]:
   box('Shop canopy',(inner-side*.9,3.6,z),(1.8,.18,depth*.9),'metal',.06)
   box('Canopy edge',(inner-side*1.8,3.55,z),(.05,.06,depth*.86),'cyan' if side<0 else 'amber')
  # Vent blades at reachable visual height (outside walk bounds).
  box('Vent housing',(inner-side*.4,4.6,z+1),(.7,1.2,1.3),'metal',.05)
  for k in range(6):box('Vent louvers',(inner-side*.78,4.17+k*.15,z+1),(.05,.07,1.1),'silver')
# Far city closes view while remaining 1m beyond navigation.
obstacle('End building',(-5,10,-34),(20,20,3),'warm_facade',.08)
obstacle('End service tower',(11,16,-35),(10,32,4),'blue_facade',.08)
for x in range(-13,14,3):
 for y in range(5,20,3):box('End windows',(x,y,-32.46),(1.3,1.5,.045),'lit_window' if (x+y)%4==0 else 'window')
box('Terminal sign panel',(-4,5.2,-32.35),(10,1.7,.22),'metal',.05)
text('KAIRO  //  09',(-4,4.65,-32.19),1.0,'cyan')
# High overhead bridge and slender support members leave open sky and ground routes.
box('Skybridge deck',(0,9.2,-20),(25,.5,2.3),'metal',.06)
for z in [-21.08,-18.92]:
 pipe('Bridge upper rail',[(-12,10.4,z),(12,10.4,z)],.06,'silver')
 for x in range(-12,13,2):pipe('Bridge baluster',[(x,9.4,z),(x,10.4,z)],.045,'silver')
box('Bridge amber strip',(0,9.01,-18.8),(22,.055,.055),'amber')
for z in [-2,-14,-27]:
 for i in range(12):
  x1=-12+i*2;x2=x1+2
  y1=12-2*math.sin(math.pi*i/12);y2=12-2*math.sin(math.pi*(i+1)/12)
  pipe('Sagging overhead cable',[(x1,y1,z),(x2,y2,z)],.025,'metal',6)
# Distinct projecting signs as mesh lettering. Central-facing facade placements.
for side,z,label,color in [(-1,1,'NOODLE','amber'),(1,-7,'NOVA','pink'),(-1,-16,'ION','cyan'),(1,-26,'09','amber')]:
 x=side*10.3
 box('Projecting sign casing',(x,6.4,z),(.38,3.6,2.1),'metal',.06)
 text(label,(x-side*.23,6.15,z),.46 if len(label)>3 else .85,color,math.pi/2 if side<0 else -math.pi/2)
 box('Sign vertical neon',(x-side*.25,6.5,z-1),(.035,3.2,.045),color)
# Ground obstacles always registered at the exact geometry footprint.
for x,z in [(-8.6,2.8),(9.2,-10),(-9.2,-24)]:
 obstacle('Vending kiosk %s'%z,(x,1.25,z),(1.2,2.5,.9),'metal',.07)
 box('Kiosk illuminated screen',(x,1.55,z+.465),(.86,.83,.035),'cyan')
 box('Kiosk lower hatch',(x,.48,z+.47),(.8,.35,.035),'silver')
 for xx in [-.25,0,.25]:box('Kiosk selector',(x+xx,.95,z+.49),(.09,.09,.035),'amber')
for j,(x,z) in enumerate([(-6,-9),(6.4,-19),(9.3,3),(-10,-28)]):
 obstacle('Utility cover %d'%j,(x,.65,z),(2.2,1.3,1.2),'concrete',.09)
 box('Utility lid',(x,1.32,z),(2.25,.10,1.24),'metal',.025)
 for dx in [-.7,.7]:box('Safety corner',(x+dx,1.38,z),(.13,.025,.95),'amber')
obstacle('Branch barricade',(0,.7,-25),(3.6,1.4,.9),'concrete',.08)
for x in [-1.4,-.7,0,.7,1.4]:box('Barricade hazard markers',(x,1.08,-24.53),(.25,.35,.025),'amber')
# Flat flush landing rings, no navigable raised platform.
pads=[(0,0),(-7,-5),(7,-5),(0,-10),(-8,-15),(8,-15),(-6,-23),(6,-23),(0,-30)]
for i,(x,z) in enumerate(pads):
 for k in range(20):
  a=2*math.pi*k/20;b=2*math.pi*(k+.72)/20
  pipe('Flush landing inlay',[(x+.86*math.cos(a),.006,z+.86*math.sin(a)),(x+.86*math.cos(b),.006,z+.86*math.sin(b))],.012,'cyan',5)
 box('Landing ordinal',(x,.005,z),(.12,.006,.45),'paint')
# Join by material for predictable GLB primitive count.
bpy.ops.object.select_all(action='DESELECT')
for name,m in M.items():
 objs=[o for o in bpy.context.scene.objects if o.type=='MESH' and len(o.data.materials)==1 and o.data.materials[0]==m]
 if not objs:continue
 for o in objs:o.select_set(True)
 bpy.context.view_layer.objects.active=objs[0];bpy.ops.object.join();o=bpy.context.object;o.name='Alley_'+name
 bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
 for poly in o.data.polygons:poly.use_smooth=False
 bpy.ops.object.select_all(action='DESELECT')
meshes=[o for o in bpy.context.scene.objects if o.type=='MESH']
triangles=sum(sum(len(p.vertices)-2 for p in o.data.polygons) for o in meshes)
layout=dict(version=1,units='meters',upAxis='Y',forwardAxis='-Z',walkableBounds=[-14.5,14.5,-31.4,7.4],spawn=[0,0,0],landingPads=[dict(id='landing-%d'%i,position=[x,0,z],radius=.85) for i,(x,z) in enumerate(pads)],obstacles=obstacles,modelSpawns=[dict(id='encounter-%d'%i,position=[x,0,z]) for i,(x,z) in enumerate([(-6,-11),(2,-18),(7,-9),(-5,-29),(8,-28)])],asset=dict(triangles=triangles,primitives=len(meshes),materials=len(M)))
(ROOT/'layout.json').write_text(json.dumps(layout,indent=2)+'\n')
bpy.ops.export_scene.gltf(filepath=str(ROOT/'cyberpunk-alley.glb'),export_format='GLB',export_yup=True,export_lights=False,export_cameras=False)
# Presentation-only lighting/cameras; not exported. Both cameras look into actual city geometry.
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=24
scene.world.color=(.19,.23,.3)
def area(n,p,energy,color,size,target):
 bpy.ops.object.light_add(type='AREA',location=coord(p));o=bpy.context.object;o.name=n;o.data.energy=energy;o.data.color=color;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector(coord(target))-o.location).to_track_quat('-Z','Y').to_euler()
area('Moon softbox',(0,24,-8),6500,(.55,.7,1),24,(0,0,-12))
area('Street fill',(0,9,8),3800,(.45,.65,1),15,(0,3,-15))
area('Warm shop bounce',(-8,6,-14),1700,(1,.3,.12),8,(0,2,-12))
area('Pink alley bounce',(9,6,-8),1600,(1,.08,.25),8,(0,2,-12))
scene.view_settings.view_transform='AgX';scene.render.resolution_x=1400;scene.render.resolution_y=1000;scene.render.resolution_percentage=100
for name,p,target,lens in [('hero',(0,17,14),(0,6,-14),23),('street',(0,1.75,6),(0,5,-18),20)]:
 bpy.ops.object.camera_add(location=coord(p));cam=bpy.context.object;cam.name=name;cam.rotation_euler=(Vector(coord(target))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.lens=lens;scene.camera=cam
 scene.render.filepath=str(OUT/(name+'.png'));bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'cyberpunk-alley.blend'))
(OUT/'asset-report.json').write_text(json.dumps(dict(triangles=triangles,primitives=len(meshes),materials=len(M),glbBytes=(ROOT/'cyberpunk-alley.glb').stat().st_size),indent=2)+'\n')
print('ALLEY_ASSET',triangles,len(meshes))
