"""Seeded original woodland arena. Blender --background --python build_blender.py.
Coordinates supplied to helpers are engine meters: Y up, -Z forward.
"""
import bpy, math, random, json, pathlib, struct
from mathutils import Vector
ROOT=pathlib.Path(__file__).resolve().parent
OUT=ROOT.parents[2]/'output'/'forest-arena-assets'; OUT.mkdir(parents=True,exist_ok=True)
R=random.Random(7319)
bpy.context.preferences.filepaths.save_version=0
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
M={}; obstacles=[]
def mat(n,c,emit=0):
 m=bpy.data.materials.new(n);m.diffuse_color=(*c,1);m.use_nodes=True;p=m.node_tree.nodes.get('Principled BSDF');p.inputs['Base Color'].default_value=(*c,1);p.inputs['Roughness'].default_value=.86
 if emit:p.inputs['Emission Color'].default_value=(*c,1);p.inputs['Emission Strength'].default_value=emit
 M[n]=m
for a in [('earth',(.15,.205,.12)),('path',(.34,.32,.19)),('bark',(.19,.12,.075)),('bark_light',(.3,.22,.12)),('leaves_dark',(.075,.22,.115)),('leaves',(.16,.35,.16)),('leaves_sun',(.32,.46,.16)),('grass',(.24,.39,.13)),('fern',(.12,.31,.18)),('stone',(.31,.36,.32)),('stone_light',(.46,.49,.4)),('moss',(.24,.34,.115)),('gold',(.72,.48,.13)),('mushroom',(.68,.26,.12)),('cream',(.76,.68,.43)),('eyes',(.19,.83,.59),2)]:mat(*a)
def C(p):return(p[0],-p[2],p[1])
def assign(o,n,m):o.name=n;o.data.materials.append(M[m]);return o
def box(n,p,d,m):
 bpy.ops.mesh.primitive_cube_add(size=1,location=C(p));o=bpy.context.object;o.dimensions=(d[0],d[2],d[1]);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);return assign(o,n,m)
def ico(n,p,s,m,sub=1):
 bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=sub,radius=1,location=C(p));o=bpy.context.object;o.scale=(s[0],s[2],s[1]);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);return assign(o,n,m)
def rod(n,a,b,r,m,r2=None,sides=7):
 a,b=Vector(C(a)),Vector(C(b));v=b-a;bpy.ops.mesh.primitive_cone_add(vertices=sides,radius1=r,radius2=r if r2 is None else r2,depth=v.length,location=(a+b)*.5);o=bpy.context.object;o.rotation_euler=v.to_track_quat('Z','Y').to_euler();return assign(o,n,m)
def footprint(n,objs):
 vs=[o.matrix_world@v.co for o in objs for v in o.data.vertices];lo=[min(v[i] for v in vs) for i in range(3)];hi=[max(v[i] for v in vs) for i in range(3)];obstacles.append(dict(id=n,center=[(lo[0]+hi[0])/2,(lo[2]+hi[2])/2,-(lo[1]+hi[1])/2],halfExtents=[(hi[0]-lo[0])/2,(hi[2]-lo[2])/2,(hi[1]-lo[1])/2]))
def join_material():
 for m in M.values():
  objs=[o for o in bpy.context.scene.objects if o.type=='MESH' and len(o.data.materials)==1 and o.data.materials[0]==m]
  if not objs:continue
  bpy.ops.object.select_all(action='DESELECT')
  for o in objs:o.select_set(True)
  bpy.context.view_layer.objects.active=objs[0];bpy.ops.object.join();bpy.context.object.name='Woodland_'+m.name
 bpy.ops.object.select_all(action='DESELECT')
def export(name):
 meshes=[o for o in bpy.context.scene.objects if o.type=='MESH'];tr=sum(len(p.vertices)-2 for o in meshes for p in o.data.polygons)
 bpy.ops.export_scene.gltf(filepath=str(ROOT/(name+'.glb')),export_format='GLB',export_yup=True,export_lights=False,export_cameras=False,export_animations=True)
 # Blender creates one action per object. Merge channels into one complete clip.
 path=ROOT/(name+'.glb');raw=path.read_bytes();json_len=struct.unpack_from('<I',raw,12)[0];doc=json.loads(raw[20:20+json_len]);rest=raw[20+json_len:]
 if doc.get('animations'):
  merged=dict(name='Idle',samplers=[],channels=[])
  for animation in doc['animations']:
   offset=len(merged['samplers']);merged['samplers'].extend(animation['samplers'])
   for channel in animation['channels']:channel['sampler']+=offset;merged['channels'].append(channel)
  doc['animations']=[merged];encoded=json.dumps(doc,separators=(',',':')).encode();encoded+=b' '*((-len(encoded))%4);path.write_bytes(struct.pack('<4sII',b'glTF',2,20+len(encoded)+len(rest))+struct.pack('<I4s',len(encoded),b'JSON')+encoded+rest)
 return dict(triangles=tr,primitives=sum(len(o.data.materials) for o in meshes),meshes=len(meshes),glbBytes=(ROOT/(name+'.glb')).stat().st_size)
# Flat continuous floor and broad branching ochre paths: all top surfaces exactly y=0.
box('Continuous walkable woodland floor',(0,-.16,-40),(110,.32,130),'earth')
# Connected waypoints. Main spine, two side loops, cross routes.
pads=[];path_count=0
def route(points):
 global path_count
 for a,b in zip(points,points[1:]):
  length=math.dist(a,b);steps=math.ceil(length/5.8)
  for i in range(steps+1):
   t=i/steps;p=(round(a[0]+(b[0]-a[0])*t,3),round(a[1]+(b[1]-a[1])*t,3))
   if p not in pads:pads.append(p)
  # Flat true geometry path ribbons.
  dx,dz=b[0]-a[0],b[1]-a[1];l=math.hypot(dx,dz);w=3.6;nx,nz=-dz/l*w,dx/l*w
  path_count+=1;path_y=.012+path_count*.001
  verts=[C((a[0]+nx,path_y,a[1]+nz)),C((a[0]-nx,path_y,a[1]-nz)),C((b[0]-nx,path_y,b[1]-nz)),C((b[0]+nx,path_y,b[1]+nz))];mesh=bpy.data.meshes.new('Path ribbon');mesh.from_pydata(verts,[],[(3,2,1,0)]);o=bpy.data.objects.new('Broad woodland trail',mesh);bpy.context.collection.objects.link(o);assign(o,'Broad woodland trail','path')
route([(0,0),(0,-18),(0,-40),(0,-64),(0,-88)])
route([(0,-12),(-22,-20),(-32,-38),(-30,-62),(-20,-80),(0,-88)])
route([(0,-12),(24,-24),(32,-44),(28,-67),(16,-82),(0,-88)])
route([(-32,-38),(0,-40),(32,-44)])
route([(-30,-62),(0,-64),(28,-67)])
# Clearing has irregular layered ground contours flush with floor.
for x,z,s in [(0,-27,18),(0,-49,17),(-28,-39,8),(29,-47,8),(0,-86,11)]:
 ico('Flat clearing earth',(x,-.055,z),(s,.055,s*.8),'path',2)
def near_route(x,z,margin=6):return min(math.hypot(x-a,z-b) for a,b in pads)<margin
# Trees: each has flared root buttresses, tapered trunk, spreading branches and layered crown.
def tree(x,z,h,index):
 r=h*.044;low=[rod('Ancient tapered trunk',(x,0,z),(x+.25,h*.72,z-.2),r,'bark',r*.43,9)]
 for k in range(5):
  a=k*math.tau/5+.3;low.append(rod('Flared buttress root',(x+math.cos(a)*r*1.8,.08,z+math.sin(a)*r*1.8),(x,.95,z),r*.32,'bark',r*.25))
 footprint('tree-%d'%index,low)
 for k in range(5):
  a=k*2.4+index;start=(x,h*(.43+k*.055),z);end=(x+math.cos(a)*h*.26,h*(.66+k*.035),z+math.sin(a)*h*.26)
  rod('Visible spreading branch',start,end,r*.36,'bark_light',r*.1)
  ico('Layered leafy canopy',end,(h*.25,h*.17,h*.24),['leaves_dark','leaves','leaves_sun'][k%3],1)
 ico('Crown canopy',(x,h*.91,z),(h*.26,h*.21,h*.25),'leaves',2)
for i in range(110):
 x=R.uniform(-49,49);z=R.uniform(-98,18)
 if near_route(x,z,7) or (abs(x)<19 and -65<z<4):continue
 tree(x,z,R.uniform(10,18),i)
# Tall outer silhouette trees outside walk bounds.
for i in range(30):
 x,z=(R.choice([-54,54]),R.uniform(-104,23)) if i<20 else (R.uniform(-52,52),-105)
 tree(x,z,R.uniform(15,23),200+i)
# Perimeter rocks/hills, conservatively registered including complete geometry.
for i in range(40):
 x,z=(R.choice([-49,49])+R.uniform(-2,2),R.uniform(-99,19)) if i<28 else (R.uniform(-49,49),R.choice([-99,19]))
 if near_route(x,z,5):continue
 s=R.uniform(2.2,4.2);o=ico('Mossy boundary outcrop',(x,s*.36,z),(s,s*.62,s*1.3),'stone',1);footprint('rock-%d'%i,[o]);ico('Outcrop moss',(x,s*.78,z),(s*.8,.25,s),'moss',1)
# Memorial arches: open central passage; only lower stone pillars block navigation.
def arch(cx,cz,width,height,name):
 for side in [-1,1]:
  x=cx+side*width/2;parts=[]
  for j in range(4):parts.append(box(name+' pillar block',(x,.6+j*1.13,cz),(1.55,1.1,2),'stone_light' if j%2 else 'stone'))
  footprint(name+'-pillar-'+str(side),parts)
  ico('Moss growing on ancient masonry',(x,4.52,cz),(1,.22,1.2),'moss')
 for i in range(9):
  a=math.pi*i/8;o=box(name+' arch voussoir',(cx+width/2*math.cos(a),4.5+width*.4*math.sin(a),cz),(1.12,1,2),'stone_light');o.rotation_euler[1]=a-math.pi/2
 for side in [-1,1]:
  rod('Climbing vine',[(cx+side*width/2)-.3,.2,cz+1.03],[(cx+side*width/2)+.2,4.5,cz+1.03],.08,'moss')
arch(0,-82,8,7,'Sanctuary gate');arch(-30,-48,7,7,'West pilgrim arch');arch(30,-57,7,7,'East moon arch')
# Sanctuary moonstone and broken surrounding steles create a distant destination.
for i in range(8):
 a=i*math.tau/8;x=math.cos(a)*9;z=-94+math.sin(a)*5
 if abs(x)<4:continue
 o=box('Ruined sanctuary stele',(x,1.5,z),(1.4,3,1.4),'stone');footprint('stele-'+str(i),[o]);ico('Stele moss',(x,3.02,z),(.9,.18,.9),'moss')
# Low nonblocking grass tufts and ferns off the walking ribbons.
verts=[];faces=[]
for i in range(3100):
 x,z=R.uniform(-48,48),R.uniform(-98,18)
 if near_route(x,z,4):continue
 h=R.uniform(.18,.6)
 for a in [R.random()*math.tau,R.random()*math.tau]:
  dx,dz=math.cos(a)*.13,math.sin(a)*.13;n=len(verts);verts.extend([C((x-dx,.01,z-dz)),C((x+dx,.01,z+dz)),C((x+dx*.3,h,z+dz*.3))]);faces.append((n,n+1,n+2))
mesh=bpy.data.meshes.new('Meadow blade geometry');mesh.from_pydata(verts,[],faces);o=bpy.data.objects.new('Thousands of real grass blades',mesh);bpy.context.collection.objects.link(o);assign(o,o.name,'grass')
for i in range(80):
 x,z=R.uniform(-44,44),R.uniform(-94,14)
 if near_route(x,z,4.7):continue
 for k in range(5):
  a=k*math.tau/5;rod('Fern stem',(x,.02,z),(x+math.cos(a)*.7,.5,z+math.sin(a)*.7),.035,'fern',.005,4)
 for j in range(3):
  dx,dz=R.uniform(-.7,.7),R.uniform(-.7,.7);h=R.uniform(.2,.5);rod('Wild mushroom stem',(x+dx,0,z+dz),(x+dx,h,z+dz),.055,'cream',sides=6);ico('Wild mushroom cap',(x+dx,h,z+dz),(.22,.1,.22),'mushroom')
# Gold route markers are ground flush, readable but natural.
for i,(x,z) in enumerate(pads):
 for k in range(8):
  a=k*math.tau/8;ico('Flush route pebble',(x+math.cos(a)*.9,.018,z+math.sin(a)*.9),(.09,.018,.09),'gold')
join_material();metrics=export('forest-arena')
layout=dict(version=1,units='meters',upAxis='Y',forwardAxis='-Z',walkableBounds=[-50,50,-100,20],spawn=[0,0,0],landingPads=[dict(id='landing-%d'%i,position=[x,0,z],radius=.85) for i,(x,z) in enumerate(pads)],obstacles=obstacles,modelSpawns=[dict(id='encounter-%d'%i,slug=s,position=[x,0,z]) for i,(s,x,z) in enumerate([('golem',-9,-26),('treant',10,-43),('mushroom',-29,-38),('golem',27,-65),('treant',0,-73)])],asset=metrics)
(ROOT/'layout.json').write_text(json.dumps(layout,indent=2)+'\n')
# Presentation cameras and natural lighting live in editable .blend, not GLB.
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=20;scene.world.color=(.34,.42,.3);scene.world.use_nodes=True;bg=scene.world.node_tree.nodes.get('Background');bg.inputs['Color'].default_value=(.36,.52,.65,1);bg.inputs['Strength'].default_value=.45
bpy.ops.object.light_add(type='SUN',location=(0,0,30));sun=bpy.context.object;sun.name='Late afternoon sun';sun.data.energy=2.6;sun.data.angle=.2;sun.rotation_euler=(.42,-.5,-.5)
bpy.ops.object.light_add(type='AREA',location=C((0,28,-30)));o=bpy.context.object;o.data.energy=16000;o.data.shape='DISK';o.data.size=65
scene.view_settings.view_transform='AgX';scene.render.resolution_x=1400;scene.render.resolution_y=900;scene.render.resolution_percentage=100
for name,p,t,lens in [('scenic',(0,2.1,8),(0,5,-32),22),('overview',(72,85,72),(0,0,-40),38)]:
 bpy.ops.object.camera_add(location=C(p));cam=bpy.context.object;cam.name=name;cam.rotation_euler=(Vector(C(t))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.lens=lens;scene.camera=cam;scene.render.filepath=str(OUT/(name+'.png'));bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'forest-arena.blend'))
# Distinct original articulated creatures. Every moving limb has a cyclic Idle channel.
creature_metrics={}
for slug in ['golem','treant','mushroom']:
 bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
 parts=[]
 def orb(n,p,s,m,sub=1):o=ico(n,p,s,m,sub);parts.append(o);return o
 def limb(n,a,b,r,m):o=rod(n,a,b,r,m,r*.7);parts.append(o);return o
 if slug=='golem':
  orb('Riven stone torso',(0,1.7,0),(.85,1,.48),'stone',2);orb('Moss shoulder mantle',(0,2.25,0),(1,.35,.5),'moss');orb('Ancient square stone head',(0,2.9,0),(.52,.55,.44),'stone_light')
  for side in [-1,1]:
   orb('Heavy stone foot',(side*.43,.24,.04),(.4,.25,.52),'stone');limb('Stone shin',(side*.43,.35,0),(side*.43,1.2,0),.29,'stone_light');orb('Boulder fist',(side*1.15,1.15,.04),(.35,.45,.34),'stone');limb('Rock arm',(side*.73,2.25,0),(side*1.15,1.18,0),.32,'stone');orb('Jade eye',(side*.2,2.98,.4),(.09,.06,.045),'eyes')
  for x,y in [(-.35,1.9),(.25,1.55),(0,2.1)]:orb('Living moss fissure',(x,y,.44),(.12,.22,.045),'moss')
 elif slug=='treant':
  limb('Gnarled heartwood trunk',(0,.7,0),(0,2.7,0),.56,'bark');orb('Crown face',(0,2.75,0),(.53,.65,.42),'bark_light');orb('Leaf beard',(0,2.1,.22),(.4,.6,.35),'leaves')
  for side in [-1,1]:
   limb('Root leg',(side*.24,1,0),(side*.53,.1,.17),.22,'bark');limb('Branch arm',(side*.43,2.2,0),(side*1.1,1.4,.05),.18,'bark_light');orb('Leaf shoulder',(side*.6,2.3,0),(.45,.3,.4),'leaves_sun');orb('Amber eye',(side*.2,2.85,.39),(.085,.07,.04),'gold')
   limb('Great antler',(side*.3,3.05,0),(side*.9,4,.0),.12,'bark_light');limb('Antler fork',(side*.65,3.6,0),(side*1.2,3.8,.1),.08,'bark_light');limb('Antler crown',(side*.73,3.75,0),(side*.65,4.22,0),.06,'bark_light')
 elif slug=='mushroom':
  orb('Cream stalk body',(0,.8,0),(.43,.8,.38),'cream',2);orb('Wide russet cap',(0,1.72,0),(1,.46,.82),'mushroom',2);orb('Golden cap underside',(0,1.53,0),(.88,.11,.72),'gold')
  for side in [-1,1]:
   limb('Little stem arm',(side*.3,1,0),(side*.65,.7,.12),.12,'cream');orb('Root boot',(side*.28,.14,.13),(.25,.15,.33),'bark');orb('Luminous friendly eye',(side*.16,1.11,.35),(.065,.09,.035),'eyes')
  for i in range(9):
   a=i*2.4;r=.7*math.sqrt((i+.5)/9);orb('Ivory cap fleck',(math.cos(a)*r,1.85+.25*(1-r),math.sin(a)*r),(.12,.045,.1),'cream')
 # Animate actual authored geometry individually, without a static rest duplicate.
 for i,o in enumerate(parts):
  base=o.location.copy();base_rotation=o.rotation_euler.copy()
  for frame,factor in [(1,0),(31,1),(61,0)]:
   o.location=base+Vector((0,0,.035*factor*(1 if slug!='treant' else .6)));o.rotation_euler=base_rotation.copy();o.rotation_euler[1]=base_rotation[1]+(.035*factor if 'arm' in o.name.lower() else 0);o.keyframe_insert(data_path='location',frame=frame);o.keyframe_insert(data_path='rotation_euler',frame=frame)
  if o.animation_data and o.animation_data.action:o.animation_data.action.name='Idle'
 scene.frame_start=1;scene.frame_end=61;scene.render.fps=30;scene.frame_set(1)
 creature_metrics[slug]=export(slug)
 bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/(slug+'.blend')))
(OUT/'asset-report.json').write_text(json.dumps(dict(environment=metrics,creatures=creature_metrics,pads=len(pads),obstacles=len(obstacles),seed=7319),indent=2)+'\n')
print('FOREST_ASSET',json.dumps(metrics),len(pads),len(obstacles))
