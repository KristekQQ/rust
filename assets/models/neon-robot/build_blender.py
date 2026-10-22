#!/usr/bin/env python3
"""Blender-built reference-inspired industrial neon robot; no old GLB imported.
Run: Blender --background --python assets/models/neon-robot/build_blender.py
Editable .blend, 1024px hero render and articulated three-clip GLB.
"""
import bpy, math, random, sys, json, struct
from pathlib import Path
from mathutils import Vector, Quaternion, Matrix, Euler
random.seed(41)
TAU=math.tau
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
OUT=ROOT/'output'/'neon-robot';OUT.mkdir(parents=True,exist_ok=True)

def preserve_neutral_clip_channels(path,neutral_trs,expected_bounds):
    """Blender drops neutral channels even with optimizer off. Restore explicit resets.
    This retains Blender's sampled moving tracks and adds constant glTF channels
    for the same authored controls, so switching clips cannot retain stale limbs.
    """
    raw=path.read_bytes();json_size=struct.unpack_from('<I',raw,12)[0]
    doc=json.loads(raw[20:20+json_size]);binary=bytearray(raw[28+json_size:])
    def accessor(values,kind):
        count={'SCALAR':1,'VEC3':3,'VEC4':4}[kind]
        binary.extend(b'\0'*((-len(binary))%4));offset=len(binary)
        flat=[v for row in values for v in row]
        binary.extend(struct.pack('<'+'f'*len(flat),*flat))
        view=len(doc['bufferViews']);doc['bufferViews'].append({'buffer':0,'byteOffset':offset,'byteLength':len(binary)-offset})
        a={'bufferView':view,'componentType':5126,'count':len(values),'type':kind}
        if kind=='SCALAR':a['min']=[min(flat)];a['max']=[max(flat)]
        doc['accessors'].append(a);return len(doc['accessors'])-1
    # The exporter also removes static TRS on animated empties: copy the true
    # Blender local neutral transforms into glTF coordinates before reset channels.
    for node in doc['nodes']:
        trs=neutral_trs.get(node.get('name'))
        if trs is None:continue
        node['translation'],node['rotation']=trs
    doc['extras']={'blenderNative':True,'neutralBounds':expected_bounds}
    pivots=[i for i,node in enumerate(doc['nodes'])if node.get('name','').endswith(' pivot')]
    for clip in doc['animations']:
        # Blender NLA starts at frame one; glTF loops start at time zero.
        remapped={}
        for sampler in clip['samplers']:
            old=sampler['input']
            if old not in remapped:
                a=doc['accessors'][old];v=doc['bufferViews'][a['bufferView']]
                start=v.get('byteOffset',0)+a.get('byteOffset',0)
                times=struct.unpack_from('<'+'f'*a['count'],binary,start)
                duration=round(times[-1]-times[0],5)
                shifted=[[t-times[0]]for t in times];shifted[0]=[0];shifted[-1]=[duration]
                remapped[old]=accessor(shifted,'SCALAR')
            sampler['input']=remapped[old]
        coverage={(c['target']['node'],c['target']['path'])for c in clip['channels']}
        duration=max(doc['accessors'][sampler['input']]['max'][0]for sampler in clip['samplers'])
        time=accessor([[0],[duration]],'SCALAR')
        for node in pivots:
            for prop,kind,default in [('translation','VEC3',[0,0,0]),('rotation','VEC4',[0,0,0,1])]:
                if (node,prop)in coverage:continue
                value=doc['nodes'][node].get(prop,default)
                sampler=len(clip['samplers']);clip['samplers'].append({'input':time,'output':accessor([value,value],kind),'interpolation':'LINEAR'})
                clip['channels'].append({'sampler':sampler,'target':{'node':node,'path':prop}})
    doc['buffers'][0]['byteLength']=len(binary)
    payload=json.dumps(doc,separators=(',',':')).encode();payload+=b' '*((-len(payload))%4)
    binary+=b'\0'*((-len(binary))%4)
    result=struct.pack('<4sII',b'glTF',2,28+len(payload)+len(binary))+struct.pack('<I4s',len(payload),b'JSON')+payload+struct.pack('<I4s',len(binary),b'BIN\0')+binary
    path.write_bytes(result)
    print('EXPLICIT_NEUTRAL_CHANNELS',[(a['name'],len(a['channels']))for a in doc['animations']],flush=True)

def export_ready(root,controls,merged):
    scene=bpy.context.scene;scene.frame_set(1);bpy.context.view_layer.update()
    lo=[float('inf')]*3;hi=[-float('inf')]*3
    for o in merged:
        for v in o.data.vertices:
            p=o.matrix_world@v.co
            for j in range(3):lo[j]=min(lo[j],p[j]);hi[j]=max(hi[j],p[j])
    lowest=lo[2]
    expected_bounds=[[lo[0],0,-hi[1]],[hi[0],hi[2]-lowest,-lo[1]]]
    neutral_trs={}
    for ctrl in controls.values():
        p=ctrl.location;q=ctrl.rotation_quaternion
        neutral_trs[ctrl.name]=([p.x,p.z,-p.y],[q.x,q.z,-q.y,q.w])
    root.location.z-=lowest
    bpy.context.view_layer.update();bpy.ops.object.select_all(action='DESELECT')
    for o in [root]+list(controls.values())+merged:o.select_set(True)
    bpy.context.view_layer.objects.active=merged[0]
    bpy.ops.export_scene.gltf(filepath=str(HERE/'neon-robot.glb'),export_format='GLB',use_selection=True,export_apply=True,export_animations=True,export_animation_mode='NLA_TRACKS',export_merge_animation='NLA_TRACK',export_force_sampling=True,export_optimize_animation_size=False,export_yup=True,export_materials='EXPORT')
    preserve_neutral_clip_channels(HERE/'neon-robot.glb',neutral_trs,expected_bounds)
    triangles=sum(sum(len(p.vertices)-2 for p in o.data.polygons)for o in merged)
    print('ANIMATED_GLB_EXPORTED',len(merged),'assemblies',triangles,'triangles','ground lift',round(-lowest,4),flush=True)

def export_character():
    """Apply modifiers on a temporary loaded scene, join to eleven rigid assemblies.
    The saved authoring .blend remains editable and is never overwritten here.
    """
    model_collection=bpy.data.collections.get('ROBOT | original Blender geometry')
    if bpy.data.objects.get('NeonRobot'):
        controls={o.name.removesuffix(' pivot'):o for o in model_collection.objects if o.name.endswith(' pivot') and o.animation_data}
        merged=[o for o in model_collection.objects if o.type=='MESH']
        return export_ready(bpy.data.objects['NeonRobot'],controls,merged)
    if model_collection is None:raise RuntimeError('Native robot collection missing')
    def assembly(o):
        chain=[];p=o
        while p:chain.append(p.name.lower());p=p.parent
        text=' '.join(chain)
        if 'head |' in text:return 'Head'
        if 'back reactor' in text:return 'Reactor'
        if 'foreground left' in text:return 'LeftLeg'
        if 'rear right' in text:return 'RightLeg'
        if 'energy blade and emitter' in text:return 'BladeWrist'
        if 'claw gauntlet' in text:return 'ClawWrist'
        if 'monumental blade cannon' in text or 'raised elbow' in text:return 'BladeForearm'
        if 'raised blade upper' in text or 'blade shoulder' in text:return 'BladeArm'
        if 'massive claw cannon' in text or 'claw elbow' in text:return 'ClawForearm'
        if 'claw upper-arm' in text or 'claw shoulder' in text:return 'ClawArm'
        return 'Body'
    # Preserve fully editable individual bevelled parts and curves in the .blend.
    archive=bpy.data.collections.new('AUTHORING | original editable meshes and curves')
    bpy.context.scene.collection.children.link(archive);archive.hide_render=True;archive.hide_viewport=True
    copies={}
    for o in list(model_collection.objects):
        duplicate=o.copy()
        if o.data is not None:duplicate.data=o.data.copy()
        duplicate.name='AUTHOR | '+o.name;archive.objects.link(duplicate);copies[o]=duplicate
    for source,duplicate in copies.items():
        if source.parent in copies:duplicate.parent=copies[source.parent]
    meshes=[o for o in model_collection.objects if o.type in {'MESH','CURVE'}]
    for o in meshes:o['rigidAssembly']=assembly(o)
    bpy.ops.object.select_all(action='DESELECT')
    for o in meshes:o.select_set(True)
    bpy.context.view_layer.objects.active=meshes[0]
    bpy.ops.object.convert(target='MESH')
    buckets={}
    for o in model_collection.objects:
        if o.type=='MESH':buckets.setdefault(o['rigidAssembly'],[]).append(o)
    merged=[]
    for name,objects in sorted(buckets.items()):
        bpy.ops.object.select_all(action='DESELECT')
        for o in objects:
            matrix=o.matrix_world.copy();o.parent=None;o.matrix_world=matrix;o.select_set(True)
        bpy.context.view_layer.objects.active=objects[0]
        bpy.ops.object.join();o=bpy.context.object;o.name=name;merged.append(o)
    # World-space articulated controls, each mesh attached with exact keep-world pose.
    root=bpy.data.objects.new('NeonRobot',None);model_collection.objects.link(root)
    pivots={'Body':(.28,0,.85),'Head':(.55,0,1.55),'Reactor':(.54,.31,1.40),
      'BladeArm':(-.05,0,1.95),'BladeForearm':(-.45,.1,2.9),'BladeWrist':(-1.25,-.15,1.7),
      'ClawArm':(1.05,0,1.7),'ClawForearm':(1.25,-.1,1.1),'ClawWrist':(1.65,-.35,.45),
      'LeftLeg':(-.15,0,.85),'RightLeg':(.35,.1,.8)}
    relationships={'Head':'Body','Reactor':'Body','BladeArm':'Body','ClawArm':'Body',
      'BladeForearm':'BladeArm','BladeWrist':'BladeForearm','ClawForearm':'ClawArm','ClawWrist':'ClawForearm'}
    controls={}
    for name,pivot in pivots.items():
        ctrl=bpy.data.objects.new(name+' pivot',None);model_collection.objects.link(ctrl);ctrl.location=pivot;ctrl.rotation_mode='QUATERNION';controls[name]=ctrl
    bpy.context.view_layer.update()
    for name,ctrl in controls.items():
        matrix=ctrl.matrix_world.copy();ctrl.parent=controls.get(relationships.get(name),root);ctrl.matrix_world=matrix
        bpy.context.view_layer.update()
    for o in merged:
        matrix=o.matrix_world.copy();o.parent=controls[o.name];o.matrix_world=matrix
    bpy.context.view_layer.update()
    neutral={name:(ctrl.location.copy(),ctrl.rotation_quaternion.copy())for name,ctrl in controls.items()}
    scene=bpy.context.scene;scene.render.fps=24;scene.frame_start=1;scene.frame_end=97
    for name,ctrl in controls.items():
        ctrl.animation_data_create()
        for clip,duration in [('Idle',4),('Attack',2),('Victory',3)]:
            action=bpy.data.actions.new(name+' | '+clip);ctrl.animation_data.action=action
            count=duration*24
            for i in range(count+1):
                u=i/count;s=math.sin(TAU*u);pulse=math.sin(math.pi*u)**2
                x=y=z=dx=dy=dz=0
                if clip=='Idle':
                    if name=='Body':dz=.012*(1-math.cos(TAU*u));y=.012*s
                    elif name=='Head':z=.035*s
                    elif name=='Reactor':y=.06*s
                    elif name in ['BladeArm','ClawArm']:x=.015*s
                elif clip=='Attack':
                    if name=='BladeArm':y=.65*pulse
                    elif name=='BladeForearm':y=-.20*pulse
                    elif name=='BladeWrist':y=.10*pulse
                    elif name=='Body':z=.10*s*pulse;dx=.04*pulse
                    elif name=='Head':z=-.12*s*pulse
                    elif name=='ClawArm':y=-.15*pulse
                else:
                    if name=='ClawArm':y=-1.8*pulse
                    elif name=='ClawForearm':x=.35*pulse
                    elif name=='ClawWrist':y=.12*s
                    elif name=='Head':y=.14*s
                    elif name=='Body':dz=.035*math.sin(TAU*u)**2
                    elif name=='BladeArm':y=.12*pulse
                    elif name=='Reactor':y=.12*s
                if i in (0,count):x=y=z=dx=dy=dz=0
                loc,rotation=neutral[name];ctrl.location=loc+Vector((dx,dy,dz));ctrl.rotation_quaternion=rotation@Euler((x,y,z),'XYZ').to_quaternion()
                ctrl.keyframe_insert(data_path='location',frame=i+1,group=name)
                ctrl.keyframe_insert(data_path='rotation_quaternion',frame=i+1,group=name)
            track=ctrl.animation_data.nla_tracks.new();track.name=clip
            strip=track.strips.new(clip,1,action);strip.blend_type='REPLACE';strip.extrapolation='NOTHING'
            ctrl.animation_data.action=None
    scene.frame_set(1)
    # The editable authoring archive and studio stay in the source file; export gets only eleven meshes + controls.
    bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'neon-robot.blend'))
    export_ready(root,controls,merged)

def blade_frame(direction):
    z=direction.normalized();x=Vector((0,-1,0)).cross(z).normalized();y=z.cross(x).normalized()
    return Matrix((x,y,z)).transposed().to_quaternion()

if '--refine-saved' in sys.argv:
    bpy.ops.wm.open_mainfile(filepath=str(HERE/'neon-robot.blend'))
    scene=bpy.context.scene
    bpy.data.lights['Soft neutral face key'].energy=350
    bpy.data.lights['Warm brass frontal kick'].energy=80
    cam=scene.camera;cam.location=(-.10,-6.2,.90);cam.rotation_euler=(Vector((-.25,0,1.30))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.lens=40
    blade=bpy.data.objects['Energy blade and emitter'];blade.rotation_quaternion=blade_frame(Vector((-1,-.1,-1.65)))
    for o in bpy.data.objects:
        if 'nested rounded hub' in o.name and o.type=='MESH' and o.data.materials[0].name.startswith('Exposed brushed'):
            o.data.materials[0]=bpy.data.materials['Carbon joints and vent recesses']
    scene.view_settings.exposure=-.3
    boot=bpy.data.objects['Foreground left enormous armored boot'];boot.scale=(1.7,1.7,1.7);boot.location.y=-1.0;boot.location.z=-.2835
    shin=bpy.data.objects['Foreground left shin piston barrel'];shin.scale=(1.15,1.15,1)
    for obj in bpy.data.objects:
        if 'cyan bearing' in obj.name and obj.type=='MESH':obj.data.materials[0]=bpy.data.materials['Warm antique brass machined straps']
    bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'neon-robot.blend'))
    bpy.ops.render.render(write_still=True)
    print('REFINED_HERO_READY',scene.render.filepath,flush=True)
    sys.exit(0)

if '--export-saved' in sys.argv:
    bpy.ops.wm.open_mainfile(filepath=str(HERE/'neon-robot.blend'))
    export_character()
    sys.exit(0)
bpy.ops.wm.read_factory_settings(use_empty=True)
for block in bpy.data.materials: bpy.data.materials.remove(block)
TAU=math.tau
robot=bpy.data.collections.new('ROBOT | original Blender geometry');bpy.context.scene.collection.children.link(robot)
stage=bpy.data.collections.new('STUDIO | lights camera floor');bpy.context.scene.collection.children.link(stage)
def collection(obj,col=robot):
    for c in list(obj.users_collection):c.objects.unlink(obj)
    col.objects.link(obj)
    return obj

def metal(name,color,metallic=.8,rough=.3):
    m=bpy.data.materials.new(name);m.use_nodes=True;n=m.node_tree.nodes;links=m.node_tree.links
    p=n.get('Principled BSDF');p.inputs['Base Color'].default_value=(*color,1);p.inputs['Metallic'].default_value=metallic;p.inputs['Roughness'].default_value=rough
    tex=n.new('ShaderNodeTexNoise');tex.inputs['Scale'].default_value=155;tex.inputs['Detail'].default_value=2.5;tex.inputs['Roughness'].default_value=.72
    bump=n.new('ShaderNodeBump');bump.inputs['Strength'].default_value=.22;bump.inputs['Distance'].default_value=.012
    links.new(tex.outputs['Fac'],bump.inputs['Height']);links.new(bump.outputs['Normal'],p.inputs['Normal'])
    # Brushed microscopic scratches: anisotropic 3D mapping, subtle roughness.
    coord=n.new('ShaderNodeTexCoord');mapping=n.new('ShaderNodeVectorMath');mapping.operation='MULTIPLY';mapping.inputs[1].default_value=(2,2,110)
    links.new(coord.outputs['Generated'],mapping.inputs[0]);wave=n.new('ShaderNodeTexNoise');wave.inputs['Scale'].default_value=35;wave.inputs['Detail'].default_value=1
    links.new(mapping.outputs['Vector'],wave.inputs['Vector']);ramp=n.new('ShaderNodeMapRange');ramp.inputs['From Min'].default_value=0;ramp.inputs['From Max'].default_value=1;ramp.inputs['To Min'].default_value=rough*.75;ramp.inputs['To Max'].default_value=min(.65,rough*1.4)
    links.new(wave.outputs['Fac'],ramp.inputs['Value']);links.new(ramp.outputs['Result'],p.inputs['Roughness'])
    m.diffuse_color=(*color,1)
    family='violet' if ('Blackberry' in name or 'Purple' in name) else 'brass' if ('brass' in name or 'golden' in name) else None
    if family:
        for suffix,socket in [('base','Base Color'),('rough','Roughness'),('normal',None)]:
            path=HERE/'textures'/f'{family}_{suffix}.png'
            img=bpy.data.images.load(str(path),check_existing=True)
            img.colorspace_settings.name='sRGB' if suffix=='base' else 'Non-Color'
            img.pack()
            image=n.new('ShaderNodeTexImage');image.image=img;image.label='Embedded worn metal '+suffix
            if socket:links.new(image.outputs['Color'],p.inputs[socket])
            else:
                normal=n.new('ShaderNodeNormalMap');normal.inputs['Strength'].default_value=.45
                links.new(image.outputs['Color'],normal.inputs['Color']);links.new(normal.outputs['Normal'],p.inputs['Normal'])
    return m

def neon(name,color,strength):
    m=bpy.data.materials.new(name);m.use_nodes=True;p=m.node_tree.nodes.get('Principled BSDF');p.inputs['Base Color'].default_value=(*color,1);p.inputs['Metallic'].default_value=.15;p.inputs['Roughness'].default_value=.2;p.inputs['Emission Color'].default_value=(*color,1);p.inputs['Emission Strength'].default_value=strength;m.diffuse_color=(*color,1);return m
armor=metal('Blackberry titanium | micro pitting + brushed wear',(.105,.027,.13),.83,.31)
lightarmor=metal('Purple gunmetal armor panel',(.18,.052,.19),.82,.28)
black=metal('Carbon joints and vent recesses',(.011,.008,.016),.65,.4)
brass=metal('Warm antique brass machined straps',(.52,.245,.065),.84,.28)
gold=metal('Polished golden fasteners',(.78,.43,.12),.85,.22)
chrome=metal('Exposed brushed piston chrome',(.5,.56,.63),.93,.2)
pink=neon('Saturated magenta neon', (1,.004,.20),5)
cyan=neon('Electric cyan neon',(.005,.64,1),5)
core=neon('Blade near white cyan core',(.42,.86,1),7)
amber=neon('Amber iris glass',(.9,.37,.008),.8)
blueglass=neon('Cyan lens optical glass',(.015,.5,.95),2.5)

created=[]
def setup(obj,name,pos,mat,parent=None):
    obj.name=name;collection(obj);obj.location=pos
    if mat:obj.data.materials.append(mat)
    if parent:obj.parent=parent
    created.append(obj);return obj

def empty(name,pos=(0,0,0),parent=None):
    o=bpy.data.objects.new(name,None);robot.objects.link(o);o.location=pos
    if parent:o.parent=parent
    return o

def smooth(o):
    if o.type=='MESH':
        for p in o.data.polygons:p.use_smooth=True
    return o

def bevel(o,width=.025,segments=3):
    mod=o.modifiers.new('Machined edge bevel','BEVEL');mod.width=width;mod.segments=segments
    mod=o.modifiers.new('Weighted surface normals','WEIGHTED_NORMAL');mod.keep_sharp=True
    return o

def sphere(name,pos,scale,mat,parent=None):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=40,ring_count=24,radius=1)
    o=setup(bpy.context.object,name,pos,mat,parent);o.scale=scale;smooth(o);return o

def box(name,pos,size,mat,parent=None,width=.015):
    bpy.ops.mesh.primitive_cube_add(size=1)
    o=setup(bpy.context.object,name,pos,mat,parent);o.scale=size
    # Apply dimensions before bevel so edges stay true-size.
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);bevel(o,width,3);return o

def cyl(name,pos,r,depth,mat,parent=None,direction=(0,0,1),verts=48,width=.012):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts,radius=r,depth=depth)
    o=setup(bpy.context.object,name,pos,mat,parent);o.rotation_mode='QUATERNION';o.rotation_quaternion=Vector(direction).to_track_quat('Z','Y');bevel(o,width,3);smooth(o);return o

def rod(name,a,b,r,mat,parent=None,verts=32,width=.008):
    a,b=Vector(a),Vector(b);return cyl(name,(a+b)/2,r,(b-a).length,mat,parent,b-a,verts,width)

def ring(name,pos,r,t,mat,parent=None,direction=(0,0,1)):
    bpy.ops.mesh.primitive_torus_add(major_segments=48,minor_segments=8,major_radius=r,minor_radius=t)
    o=setup(bpy.context.object,name,pos,mat,parent);o.rotation_mode='QUATERNION';o.rotation_quaternion=Vector(direction).to_track_quat('Z','Y');smooth(o);return o

def tube(name,points,r,mat,parent=None):
    cu=bpy.data.curves.new(name,'CURVE');cu.dimensions='3D';cu.resolution_u=12;cu.bevel_depth=r;cu.bevel_resolution=2
    sp=cu.splines.new('BEZIER');sp.bezier_points.add(len(points)-1)
    for b,p in zip(sp.bezier_points,points):b.co=p;b.handle_left_type='AUTO';b.handle_right_type='AUTO'
    obj=bpy.data.objects.new(name,cu);robot.objects.link(obj);obj.data.materials.append(mat)
    if parent:obj.parent=parent
    created.append(obj);return obj

# Shared hexagonal fastener mesh keeps hundreds of rivets cheap and editable.
boltmesh=None
def bolt(pos,r=.013,parent=None,direction=(0,-1,0),name='Recessed brass fastener'):
    global boltmesh
    if boltmesh is None:
        bpy.ops.mesh.primitive_cylinder_add(vertices=6,radius=1,depth=.50)
        o=bpy.context.object;boltmesh=o.data;bpy.data.objects.remove(o,do_unlink=True);boltmesh.materials.append(gold)
    o=bpy.data.objects.new(name,boltmesh);robot.objects.link(o);o.location=pos;o.scale=(r,r,r);o.rotation_mode='QUATERNION';o.rotation_quaternion=Vector(direction).to_track_quat('Z','Y')
    if parent:o.parent=parent
    created.append(o)
    return o

def bolt_circle(center,radius,count,parent=None,normal=(0,-1,0),size=.012):
    q=Vector(normal).to_track_quat('Z','Y');c=Vector(center)
    for k in range(count):
        a=k*TAU/count;p=c+q@Vector((radius*math.cos(a),radius*math.sin(a),0));bolt(p,size,parent,normal)

def segment(name,a,b,radius,parent=None):
    """Industrial cylinder in local Z coordinates with ribs, bands, pistons and vents."""
    a,b=Vector(a),Vector(b);length=(b-a).length
    g=empty(name,a,parent);g.rotation_mode='QUATERNION';g.rotation_quaternion=(b-a).to_track_quat('Z','Y')
    cyl(name+' inner carbon barrel',(0,0,length/2),radius*.89,length,black,g)
    # Three separate rolled shells leave visible black joint gaps.
    for i in range(3):
        z=length*(.18+i*.31)
        cyl(name+' curved armor sleeve',(0,0,z),radius,length*.27,armor,g,width=.016)
        # Fine circumferential grooves and rivet rows along shell edges.
        for edge in [-1,1]:
            zz=z+edge*length*.11
            ring('Black engraved panel seam',(0,0,zz),radius+.002,.004,black,g)
            for k in range(12):
                aa=k*TAU/12
                bolt(((radius+.006)*math.cos(aa),(radius+.006)*math.sin(aa),zz),.009,g,(math.cos(aa),math.sin(aa),0))
    for z in [length*.045,length*.36,length*.69,length*.955]:
        cyl('Heavy plated collar',(0,0,z),radius*1.085,.075,lightarmor,g,width=.01)
        ring('Brass collar rim',(0,0,z-.035),radius*1.09,.011,brass,g)
        ring('Continuous magenta power rail',(0,0,z+.038),radius*1.095,.014,pink,g)
        for k in range(10):
            aa=k*TAU/10;bolt(((radius*1.10)*math.cos(aa),(radius*1.10)*math.sin(aa),z),.012,g,(math.cos(aa),math.sin(aa),0))
    for k in range(4):
        aa=k*TAU/4+.33;x=radius*.90*math.cos(aa);y=radius*.90*math.sin(aa)
        rod('Exposed chromed actuator',(x,y,.08),(x,y,length-.08),.027,chrome,g)
        cyl('Actuator brass socket',(x,y,length*.64),.047,.15,brass,g)
        # raised axial strap with edge bevel and two square buckles
        xx=radius*1.01*math.cos(aa+.38);yy=radius*1.01*math.sin(aa+.38)
        strip=box('Heavy longitudinal armor strap',(xx,yy,length*.50),(.055,.035,length*.80),brass,g,.009);strip.rotation_euler.z=aa+.38-math.pi/2
        for zz in [length*.24,length*.76]:
            buckle=box('Plate strap clasp',(xx,yy,zz),(.083,.055,.065),lightarmor,g,.008);buckle.rotation_euler.z=aa+.38-math.pi/2
    # Cyan power pipe and curved auxiliary hydraulic line.
    x=radius*.85;y=-radius*.65
    tube('Cyan exposed conduit',[(x,y,.05),(x*1.15,y*1.15,length*.22),(x*1.15,y*1.15,length*.75),(x,y,length-.03)],.014,cyan,g)
    tube('Looped bronze hydraulic hose',[(-radius,0,.08),(-radius*1.32,0,length*.25),(-radius*1.32,0,length*.72),(-radius,0,length-.05)],.019,brass,g)
    # Actual dark vent slots and small diagonal machined scratches on front surface.
    for zz in [length*.16,length*.23,length*.47,length*.54,length*.78]:
        for xx in [-radius*.33,0,radius*.33]:
            vent=box('Recessed barrel cooling slot',(xx,-math.sqrt(max(.001,radius**2-xx**2))-.006,zz),(.027,.008,.014),black,g,.004)
    return g

def joint(name,pos,r,parent=None):
    sphere(name+' ball',pos,(r,r,r),black,parent)
    for offset,rad,mat in [(-.08,r*.95,brass),(-.105,r*.79,armor),(-.13,r*.64,black)]:
        cyl(name+' nested rounded hub',(pos[0],pos[1]-r*.70+offset,pos[2]),rad,.06,mat,parent,(0,-1,0),width=.018)
    ring(name+' brass bearing',(pos[0],pos[1]-r*.70-.165,pos[2]),r*.5,.013,brass,parent,(0,-1,0))
    cyl(name+' central hex axle',(pos[0],pos[1]-r*.70-.17,pos[2]),r*.18,.025,chrome,parent,(0,-1,0),verts=6,width=.005)
    bolt_circle((pos[0],pos[1]-r*.70-.14,pos[2]),r*.75,10,parent,size=.010)

# Central body leans forward and diagonally across the image; no separate chest.
body=empty('Body chassis | dynamic diagonal lunge',(.28,0,.85))
body.rotation_euler.y=math.radians(25)
sphere('Lower pressure vessel',(0,0,0),(.38,.32,.29),black,body)
for z,r in [(-.08,.32),(.02,.36),(.13,.39)]:
    cyl('Articulated waist armored segment',(0,0,z),r,.14,armor,body,width=.023)
    ring('Waist brass separation',(0,0,z-.06),r,.015,brass,body)
    ring('Waist bright magenta rail',(0,0,z+.045),r+.005,.012,pink,body)
head=empty('Head | enormous oval helmet',(.55,0,1.55));head.rotation_euler.y=math.radians(25)
sphere('Oval blackberry titanium helmet',(0,0,0),(.48,.38,.64),armor,head)
# Panel seams follow the true ellipsoid, with subtle front panel inset outline.
for angle in [-1.04,-.67,.65,1.04,2.25,3.8]:
    points=[]
    for j in range(17):
        t=-1.15+j*2.30/16
        points.append((.482*math.cos(t)*math.sin(angle),-.382*math.cos(t)*math.cos(angle),.642*math.sin(t)))
    tube('Etched meridian armor panel',points,.005,black,head)
for z in [-.45,-.3,.35,.5]:
    rr=math.sqrt(1-(z/.64)**2)
    pts=[(.484*rr*math.cos(a*TAU/36),.384*rr*math.sin(a*TAU/36),z)for a in range(37)]
    tube('Helmet circumferential dark engraving',pts,.004,black,head)
# Neon silhouette wraparound seams on either side of helmet.
for side in [-1,1]:
    tube('Helmet magenta edge rail',[(side*.07,.01,.636),(side*.36,-.07,.43),(side*.474,-.05,0),(side*.36,-.04,-.43),(side*.09,-.02,-.632)],.013,pink,head)
    tube('Helmet secondary cyan power loop',[(side*.30,-.18,.37),(side*.395,-.20,.22),(side*.40,-.20,-.06),(side*.30,-.23,-.29)],.012,cyan,head)
    for zz in [-.38,-.18,.04,.25,.43]:
        xx=side*.37;yy=-.38*math.sqrt(max(.04,1-(xx/.48)**2-(zz/.64)**2))-.006
        bolt((xx,yy,zz),.013,head)
# Brass median strap conforms to ellipsoid instead of standing as a flat bar.
verts=[];faces=[]
for j in range(25):
    z=-.50+j*1.1/24
    for x in [-.047,.047]:
        y=-.38*math.sqrt(max(.02,1-(z/.64)**2-(x/.48)**2))-.018
        verts.append((x,y,z))
for j in range(24):faces.append((j*2,j*2+1,j*2+3,j*2+2))
me=bpy.data.meshes.new('Curved brass forehead harness');me.from_pydata(verts,[],faces);me.update();o=bpy.data.objects.new('Curved riveted brass median forehead strap',me);robot.objects.link(o);o.parent=head;o.data.materials.append(brass);solid=o.modifiers.new('Brass strap thickness','SOLIDIFY');solid.thickness=.013;bevel(o,.009,3)
for z in [-.45,-.32,-.18,.02,.20,.37,.52]:
    y=-.38*math.sqrt(max(.02,1-(z/.64)**2))-.033;bolt((0,y,z),.013,head)
# Round inset lenses. Their housing geometry is fully machined, not flat discs.
for x,z,r,isamber in [(-.20,.10,.18,True),(.19,.045,.132,False)]:
    y=-.35
    cyl('Cast brass optical lens housing',(x,y,z),r+.032,.13,brass,head,(0,-1,0),width=.018)
    cyl('Black optical bezel',(x,y-.078,z),r+.008,.035,black,head,(0,-1,0),width=.008)
    ring('Polished circular goggle rim',(x,y-.105,z),r,.015,gold,head,(0,-1,0))
    sphere('Convex amber iris glass'if isamber else 'Convex cyan lens glass',(x,y-.112,z),(r*.88,.029,r*.88),amber if isamber else blueglass,head)
    if isamber:
        # Camera iris: six curved shutter lamellae inside one perfectly round aperture.
        for k in range(6):
            a=k*TAU/6; vv=[]
            for rr,aa in [(r*.84,a),(r*.84,a+.88),(r*.35,a+.64),(r*.35,a+.15)]:vv.append((x+rr*math.cos(aa),y-.146,z+rr*math.sin(aa)))
            mm=bpy.data.meshes.new('Curved shutter vane');mm.from_pydata(vv,[],[(0,1,2,3)]);mm.update();ob=bpy.data.objects.new('Golden camera iris lamella',mm);robot.objects.link(ob);ob.parent=head;ob.data.materials.append(brass);sol=ob.modifiers.new('Shutter thickness','SOLIDIFY');sol.thickness=.003;bevel(ob,.003,2)
        cyl('Round dark camera aperture',(x,y-.152,z),r*.33,.007,black,head,(0,-1,0),width=.003)
    else:
        ring('Blue lens inner optical halo',(x,y-.146,z),r*.57,.012,core,head,(0,-1,0))
        cyl('Central cyan optical pupil',(x,y-.159,z),r*.25,.01,cyan,head,(0,-1,0),width=.003)
        for k in range(3):
            aa=k*TAU/3;sphere('Optical internal highlight',(x+r*.39*math.cos(aa),y-.165,z+r*.39*math.sin(aa)),(.012,.009,.012),core,head)
    bolt_circle((x,y-.117,z),r+.015,12,head,size=.007)
# Brass mouth frame, recessed broad vent grille and cheek guards.
box('Lower face brass grille frame',(0,-.313,-.32),(.39,.070,.28),brass,head,.035)
box('Deep black mouth vent',(0,-.36,-.32),(.315,.035,.211),black,head,.025)
for zz in [-.398,-.357,-.316,-.275,-.234]:
    box('Horizontal brass breathing grille blade',(0,-.391,zz),(.30,.019,.012),gold,head,.005)
for side in [-1,1]:
    tube('Swept cheek brass brace',[(side*.26,-.34,-.05),(side*.22,-.38,-.22),(side*.20,-.34,-.43),(side*.12,-.30,-.48)],.025,brass,head)
    for zz in [-.21,-.41]:bolt((side*.18,-.405,zz),.014,head)
    # Fine chased brass scrollwork, following the upper face.
    for zz in [.25,.35]:
        pts=[]
        for j in range(15):
            t=j/14*TAU*1.1;rr=.035*(1-j/20);pts.append((side*(.075+rr*math.cos(t)),-.34+.06*(zz/.5),zz+rr*math.sin(t)))
        tube('Ornamental brass chased spiral',pts,.005,gold,head)
# Extra monocular on forehead, canted slightly up.
cyl('Forehead rangefinder body',(-.14,-.22,.49),.10,.13,armor,head,(0,-1,.5),width=.014)
ring('Forehead rangefinder silver rim',(-.14,-.30,.53),.085,.015,chrome,head,(0,-1,.5))
sphere('Rangefinder violet glass',(-.14,-.32,.54),(.061,.020,.061),pink,head)
# Two chimney crowns splay away from the head, with slots and bent glowing wires.
for side in [-1,1]:
    crown=empty('Splayed crown chimney',(side*.28,.025,.52),head);crown.rotation_euler.y=side*.28
    cyl('Crown slotted black barrel',(0,0,.10),.14,.24,armor,crown,width=.017)
    for z in [.015,.19]:
        cyl('Brass crown socket band',(0,0,z),.151,.04,brass,crown,width=.008)
        ring('Crown magenta rim',(0,0,z+.025),.154,.012,pink,crown)
    for k in range(12):
        a=k*TAU/12
        rod('Crown black inset vent',(.142*math.cos(a),.142*math.sin(a),.06),(.142*math.cos(a),.142*math.sin(a),.15),.010,black,crown,12,.003)
    for k in range(7):
        a=k*TAU/7;x=.09*math.cos(a);y=.09*math.sin(a)
        pts=[(x,y,.20),(x*1.3,y*1.3,.30),(x*2.3,y*2.3,.43),(x*2.8,y*2.8,.49+(k%3)*.025)]
        tube('Swept neon crown filament',pts,.010,pink if k%3 else cyan,crown)
        sphere('Wire tip terminal',pts[-1],(.011,.011,.011),core,crown)
# Back reactor, spine and pressure tanks complete all sides.
reactor=empty('Back reactor',(.54,.31,1.40));reactor.rotation_euler.y=math.radians(25)
cyl('Reactor graphite housing',(0,.08,0),.28,.19,black,reactor,(0,1,0),width=.025)
for y,r,mat in [(.17,.25,brass),(.20,.21,pink),(.22,.16,cyan)]:ring('Reactor concentric turbine',(0,y,0),r,.018,mat,reactor,(0,1,0))
for k in range(12):
    a=k*TAU/12;rod('Turbine radial blade',(.12*math.cos(a),.23,.12*math.sin(a)),(.23*math.cos(a+.22),.23,.23*math.sin(a+.22)),.020,armor,reactor)
for side in [-1,1]:
    cyl('Rear pressure canister',(side*.27,.25,0),.08,.57,brass,reactor,width=.018)
    for z in [-.23,0,.23]:ring('Pressure tank power strap',(side*.27,.25,z),.083,.011,cyan,reactor)
# Hero arm: upper arm raised high, elbow bends into a monumental cannon and down blade.
bs=(-.05,0,1.95);be=(-.45,.1,2.90);bw=(-1.25,-.15,1.70)
cs=(1.05,0,1.70);ce=(1.25,-.1,1.10);cw=(1.65,-.35,.45)
joint('Blade shoulder',bs,.22)
segment('Raised blade upper-arm hydraulic barrel',bs,be,.19)
joint('Raised elbow assembly',be,.25)
segment('Monumental blade cannon forearm',be,bw,.285)
joint('Claw shoulder',cs,.22)
segment('Claw upper-arm hydraulic barrel',cs,ce,.18)
joint('Claw elbow assembly',ce,.19)
segment('Massive claw cannon forearm',ce,cw,.255)
# Sword is not an upright accessory: it continues the descending industrial barrel.
blade=empty('Energy blade and emitter',bw);blade.rotation_mode='QUATERNION';blade.rotation_quaternion=blade_frame(Vector((-2.25,-.25,.05))-Vector(bw))
blade_length=(Vector((-2.25,-.25,.05))-Vector(bw)).length
for z,r in [(0,.23),(.18,.205),(.36,.17)]:
    cyl('Blade emitter armored cage',(0,0,z),r,.08,armor,blade,width=.016)
    ring('Blade emitter glowing collar',(0,0,z+.05),r,.016,pink,blade)
    for sign in [-1,1]:box('Emitter brass latch',(sign*r,0,z),(.065,.12,.11),brass,blade,.012)
for sign in [-1,1]:rod('Emitter exposed bright rail',(sign*.12,-.08,.04),(sign*.10,-.07,.44),.021,chrome,blade)
# Closed tapered diamond blade, bright cyan heart surrounded by saturated pink edges.
vs=[(-.135,0,.25),(0,-.028,.25),(.135,0,.25),(0,.028,.25),(0,0,blade_length)]
fs=[(0,2,1),(0,3,2),(0,1,4),(1,2,4),(2,3,4),(3,0,4)]
me=bpy.data.meshes.new('Closed diamond energy blade');me.from_pydata(vs,[],fs);me.update();ob=bpy.data.objects.new('Long tapered cyan energy blade',me);robot.objects.link(ob);ob.parent=blade;ob.data.materials.append(cyan)
for sign in [-1,1]:tube('Magenta outer blade edge',[(sign*.135,0,.25),(sign*.074,0,blade_length*.60),(0,0,blade_length)],.014,pink,blade)
tube('White hot blade spine',[(0,-.032,.25),(0,-.018,blade_length*.70),(0,0,blade_length)],.017,core,blade)
# Enormous segmented claw hangs down and forward, with curved plated fingers.
claw=empty('Claw gauntlet',cw);claw.rotation_euler.y=.24
sphere('Claw palm dark rounded armor',(0,-.02,-.12),(.225,.17,.24),armor,claw)
for x in [-.13,0,.13]:tube('Gauntlet magenta seams',[(x,-.17,.03),(x,-.19,-.12),(x,-.15,-.27)],.010,pink,claw)
for k in range(4):
    x=(k-1.5)*.115
    points=[(x,-.03,-.22),(x,-.10,-.36),(x,-.22,-.42),(x,-.31,-.33)]
    for j,(a,b) in enumerate(zip(points,points[1:])):
        rod('Segmented claw brass bone',a,b,.052,brass,claw)
        sphere('Black finger ball joint',a,(.057,.057,.057),black,claw)
        plate=box('Beveled curved finger armor',tuple((a[i]+b[i])/2 for i in range(3)),(.091,.098,(Vector(b)-Vector(a)).length*.80),lightarmor,claw,.015)
        plate.rotation_mode='QUATERNION';plate.rotation_quaternion=(Vector(b)-Vector(a)).to_track_quat('Z','Y')
    bolt((x,-.18,-.36),.017,claw)
tube('Massive articulated opposing thumb',[(-.20,0,-.06),(-.31,-.1,-.20),(-.28,-.25,-.31),(-.16,-.29,-.29)],.069,brass,claw)
# Staggered lunge: very large forward boot and receding support foot.
for name,hip,knee,ankle,foot,rthigh in [
 ('Foreground left',(-.15,0,.85),(-.70,-.15,.45),(-1.05,-.3,-.20),(-1.15,-.60,-.35),.235),
 ('Rear right',(.35,.1,.80),(.60,.25,.20),(.45,.35,-.25),(.45,.20,-.35),.185)]:
    joint(name+' hip',hip,rthigh*.88)
    segment(name+' heavy thigh',hip,knee,rthigh)
    joint(name+' round knee',knee,rthigh*.92)
    shin=segment(name+' shin piston barrel',knee,ankle,rthigh*.80)
    if name.startswith('Foreground'):shin.scale=(1.15,1.15,1)
    boot=empty(name+' enormous armored boot',foot)
    if name.startswith('Foreground'):
        boot.scale=(1.7,1.7,1.7);boot.location.y=-1.0;boot.location.z=-.2835
    sphere('Boot rounded main armor',(0,0,.04),(.31,.38,.19),armor,boot)
    box('Heavy layered black boot sole',(0,-.02,-.035),(.60,.77,.12),black,boot,.045)
    box('Toe broad rolled armor shell',(0,-.16,.08),(.56,.42,.21),lightarmor,boot,.075)
    for x in [-.19,0,.19]:
        tube('Boot brass axial armor strap',[(x,.12,.21),(x,-.20,.20),(x,-.36,.08)],.021,brass,boot)
        bolt((x,-.38,.09),.018,boot)
    tube('Boot toe magenta perimeter',[(-.27,-.26,.03),(-.22,-.38,.025),(0,-.405,.02),(.22,-.38,.025),(.27,-.26,.03)],.012,pink,boot)
# Studio is kept in its own collection and excluded from the GLB export.
scene=bpy.context.scene
scene.render.engine='CYCLES';scene.cycles.samples=24;scene.cycles.use_denoising=True
scene.render.resolution_x=1024;scene.render.resolution_y=1024;scene.render.resolution_percentage=100
if scene.world is None:scene.world=bpy.data.worlds.new('World')
scene.world.color=(.001,.001,.001);scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.003,.001,.006,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.1
scene.view_settings.view_transform='AgX';scene.view_settings.look='AgX - Medium High Contrast';scene.view_settings.exposure=-.3
floor=metal('Wet black studio floor',(.008,.006,.012),.65,.25)
bpy.ops.mesh.primitive_plane_add(size=200,location=(0,0,-.46));o=bpy.context.object;o.name='Studio dark reflective floor';collection(o,stage);o.data.materials.append(floor)
def light(name,pos,color,power,size,target):
    d=bpy.data.lights.new(name,'AREA');d.energy=power;d.color=color;d.shape='DISK';d.size=size;o=bpy.data.objects.new(name,d);stage.objects.link(o);o.location=pos;o.rotation_euler=(Vector(target)-o.location).to_track_quat('-Z','Y').to_euler()
light('Soft neutral face key',(-1,-4,5),(1,.70,.84),350,3,(.2,0,1.2))
light('Magenta left rim',(-4,1,3),(1,.008,.15),1500,2,(0,0,1.2))
light('Cyan right rim',(4,1,2),(.005,.40,1),1900,2,(0,0,1.2))
light('Warm brass frontal kick',(1,-4,.9),(1,.7,.4),80,2,(.5,0,1.2))
light('Pink floor spill',(-2,-1,.4),(1,.005,.12),100,2,(-1,0,-.4))
camdata=bpy.data.cameras.new('Hero camera');cam=bpy.data.objects.new('Hero camera',camdata);stage.objects.link(cam);cam.location=(-.10,-6.2,.90);target=Vector((-.25,0,1.30));cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();camdata.lens=40;scene.camera=cam
# Blender 5 compositor is a node-group; fall back to legacy tree on older Blender.
bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'neon-robot.blend'))
print('EDITABLE_GEOMETRY_SAVED',flush=True)
scene.use_nodes=True
if hasattr(scene,'compositing_node_group'):
    tree=bpy.data.node_groups.new('Hero render optical bloom','CompositorNodeTree');scene.compositing_node_group=tree
    tree.interface.new_socket(name='Image',in_out='OUTPUT',socket_type='NodeSocketColor');output=tree.nodes.new('NodeGroupOutput')
else:tree=scene.node_tree;tree.nodes.clear();output=tree.nodes.new('CompositorNodeComposite')
rl=tree.nodes.new('CompositorNodeRLayers');gl=tree.nodes.new('CompositorNodeGlare');gl.inputs['Type'].default_value='Fog Glow';gl.inputs['Quality'].default_value='High';gl.inputs['Size'].default_value=.5
# Older API threshold and modern sockets both accepted where available.
if 'Threshold' in gl.inputs:gl.inputs['Threshold'].default_value=1.1
if 'Strength' in gl.inputs:gl.inputs['Strength'].default_value=.6
if hasattr(gl,'threshold'):gl.threshold=1.1
if hasattr(gl,'size'):gl.size=8
tree.links.new(rl.outputs['Image'],gl.inputs['Image']);tree.links.new(gl.outputs['Image'],output.inputs[0])
scene.render.image_settings.file_format='PNG';scene.render.filepath=str(OUT/'blender-reference.png')
# This is a native authored Blender source, not a conversion of the old asset.
bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'neon-robot.blend'))
print('BLENDER_SOURCE_SAVED',flush=True)
bpy.ops.render.render(write_still=True)
print('HERO_RENDER_READY',scene.render.filepath,flush=True)
if '--render-only' in sys.argv:sys.exit(0)
# Static export uses joined assemblies; editable native source stays untouched.
export_character()
