#!/usr/bin/env python3
"""Native Blender construction. Run Blender --background --python this-file.
Three original modular meshes inspired by the supplied enemy concept sheet.
"""
import bpy, bmesh, math, json, struct, random, sys
from pathlib import Path
from mathutils import Vector, Quaternion
HERE=Path(__file__).resolve().parent
OUT=HERE.parents[2]/'output'/'neon-enemies'; OUT.mkdir(parents=True,exist_ok=True)
M={}; controls=[]; parts=[]; model=''

def material(name,color,metal=.45,rough=.33,emission=0):
    m=bpy.data.materials.new(name);m.diffuse_color=(*color,1);m.use_nodes=True
    bs=m.node_tree.nodes.get('Principled BSDF');bs.inputs['Base Color'].default_value=(*color,1)
    bs.inputs['Metallic'].default_value=metal;bs.inputs['Roughness'].default_value=rough
    if emission:bs.inputs['Emission Color'].default_value=(*color,1);bs.inputs['Emission Strength'].default_value=emission
    return m

def start(slug,color):
    global M,controls,parts,model,root
    bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
    for a in list(bpy.data.actions):bpy.data.actions.remove(a)
    controls=[];parts=[];model=slug
    M={'dark':material('Graphite | ceramic mechanics',(.025,.032,.045),.7),
       'edge':material('Titanium | bevels',(.12,.15,.19),.8),
       'white':material('Ivory | angular armor',(.72,.77,.83),.4),
       'black':material('Obsidian | recesses',(.006,.009,.016),.35),
       'neon':material('Neon | configurable energy',color,.2,.25,3.2),
       'hot':material('Neon | bright core',tuple(.06+.94*c for c in color),.1,.2,4)}
    root=bpy.data.objects.new(slug,None);bpy.context.collection.objects.link(root)

def pivot(name,p):
    o=bpy.data.objects.new(name+' pivot',None);bpy.context.collection.objects.link(o);o.parent=root;o.location=p;o.rotation_mode='QUATERNION';controls.append(o);bpy.context.view_layer.update();return o

def finish(o,name,mat,p,bevel=0,smooth=False):
    o.name=name;o.data.materials.append(M[mat]);bpy.context.view_layer.update();wm=o.matrix_world.copy();o.parent=p;o.matrix_world=wm
    if bevel:
        mod=o.modifiers.new('Machined armor chamfer','BEVEL');mod.width=bevel;mod.segments=1
        bpy.context.view_layer.objects.active=o;bpy.ops.object.modifier_apply(modifier=mod.name)
    if smooth or bevel:
        for face in o.data.polygons:face.use_smooth=True
        o.data.set_sharp_from_angle(angle=math.radians(38))
    if bevel and not smooth:
        mod=o.modifiers.new('Weighted corner normals','WEIGHTED_NORMAL');mod.keep_sharp=True;bpy.ops.object.modifier_apply(modifier=mod.name)
    parts.append(o);return o

def box(name,c,s,mat,p,bevel=.025,rot=None):
    bpy.ops.mesh.primitive_cube_add(size=1,location=c);o=bpy.context.object;o.scale=s
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    if rot:o.rotation_euler=rot
    return finish(o,name,mat,p,bevel)

def ball(name,c,s,mat,p):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24,ring_count=12,location=c);o=bpy.context.object;o.scale=s
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    for f in o.data.polygons:f.use_smooth=True
    return finish(o,name,mat,p)

def cyl(name,c,r,depth,mat,p,axis=(0,-1,0),verts=24):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts,radius=r,depth=depth,location=c);o=bpy.context.object;o.rotation_mode='QUATERNION';o.rotation_quaternion=Vector((0,0,1)).rotation_difference(Vector(axis));return finish(o,name,mat,p,.012,smooth=verts>8)

def ring(name,c,r,t,mat,p):
    bpy.ops.mesh.primitive_torus_add(major_radius=r,minor_radius=t,major_segments=40,minor_segments=12,location=c,rotation=(math.pi/2,0,0));return finish(bpy.context.object,name,mat,p,smooth=True)

def plate(name,outline,y,depth,mat,p,bevel=.018):
    # outline in X/Z, true extruded volume with closed rear face.
    n=len(outline);v=[(x,y-depth/2,z)for x,z in outline]+[(x,y+depth/2,z)for x,z in outline]
    faces=[tuple(reversed(range(n))),tuple(range(n,2*n))]+[(i,(i+1)%n,(i+1)%n+n,i+n)for i in range(n)]
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(v,[],faces);mesh.update();bm=bmesh.new();bm.from_mesh(mesh);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(mesh);bm.free();o=bpy.data.objects.new(name,mesh);bpy.context.collection.objects.link(o);return finish(o,name,mat,p,bevel)

def beam(name,a,b,width,depth,mat,p):
    mid=(Vector(a)+Vector(b))/2;o=box(name,mid,(width,depth,(Vector(b)-Vector(a)).length),mat,p,.012)
    o.rotation_mode='QUATERNION';o.rotation_quaternion=Vector((0,0,1)).rotation_difference(Vector(b)-Vector(a));return o

def radial_plate(name,rz,y,depth,mat,p,ang,center):
    points=[(center[0]+x*math.cos(ang)+z*math.sin(ang),center[2]-x*math.sin(ang)+z*math.cos(ang))for x,z in rz]
    return plate(name,points,y,depth,mat,p)

def drone():
    start('drone-orbiter',(1,.008,.24));C=(0,0,1.35);body=pivot('Reactor',C)
    ball('Faceted spherical graphite hull',C,(.64,.47,.64),'dark',body)
    cyl('Recessed front core bezel',(0,-.46,1.35),.48,.15,'black',body)
    cyl('Layered front iris',(0,-.55,1.35),.37,.055,'dark',body)
    for r,t,ma in [(.44,.035,'edge'),(.365,.024,'neon'),(.30,.015,'hot'),(.13,.015,'neon')]:ring('Concentric reactor ring',(0,-.59,1.35),r,t,ma,body)
    ball('Central energy lens',(0,-.61,1.35),(.09,.035,.09),'hot',body)
    for i in range(8):
        a=i*math.tau/8;p=pivot('Armor sector %02d'%i,(.5*math.sin(a),0,1.35+.5*math.cos(a)))
        radial_plate('Ivory radial carapace',[(-.14,.47),(.14,.47),(.17,.56),(.09,.67),(-.10,.67),(-.17,.54)],-.18,.38,'white',p,a,C)
        radial_plate('Rear graphite carapace',[(-.17,.37),(.17,.37),(.21,.55),(.1,.63),(-.13,.63)],.29,.2,'edge',p,a,C)
    for i,a in enumerate([0,math.tau/3,2*math.tau/3]):
        p=pivot('Dagger wing %d'%i,(.56*math.sin(a),0,1.35+.56*math.cos(a)))
        q=(p.location.x,0,p.location.z);cyl('Wing rotating socket',q,.155,.23,'edge',p)
        radial_plate('Swept dagger obsidian backbone',[(-.15,.56),(.18,.61),(.25,.89),(.10,1.56),(-.14,1.32),(-.25,.83)],0,.22,'dark',p,a,C)
        radial_plate('Wing ivory armor',[(-.17,.69),(.12,.70),(.17,.90),(.10,1.51),(-.09,1.27),(-.20,.84)],-.13,.10,'white',p,a,C)
        radial_plate('Magenta inset wing strip',[(-.10,.75),(-.035,.78),(.085,1.48),(-.065,1.23),(-.155,.84)],-.193,.018,'neon',p,a,C)
    p=pivot('Lower keel',(0,.02,.76));plate('Ventral stabilizer',[(-.12,.84),(.12,.84),(.10,.53),(-.08,.24)],.04,.16,'white',p);plate('Keel energy',[(-.045,.72),(.05,.73),(-.065,.29)],-.06,.02,'neon',p)
    cyl('Rear service hub',(0,.47,1.35),.30,.13,'edge',body);ring('Rear exhaust annulus',(0,.55,1.35),.20,.02,'neon',body)

def wraith():
    start('blade-wraith',(1,.008,.24));body=pivot('Torso',(0,0,1.85))
    plate('Narrow tapered torso',[(-.35,2.18),(.35,2.18),(.25,1.83),(.10,1.62),(-.10,1.62),(-.25,1.83)],.02,.34,'dark',body)
    for s in [-1,1]:
        plate('Pectoral armor',[(s*.04,2.13),(s*.33,2.21),(s*.27,1.95),(s*.12,1.85)],-.205,.10,'white',body)
        beam('Chest light rib',(s*.09,-.27,2.1),(s*.18,-.26,1.94),.025,.018,'neon',body)
        beam('Rear spine brace',(s*.17,.24,2.14),(s*.09,.24,1.75),.075,.07,'edge',body)
    cyl('Chest micro reactor',(0,-.24,1.96),.067,.045,'neon',body)
    for z in [1.55,1.66,1.77]:box('Abdominal articulated vertebra',(0,.01,z),(.20,.22,.08),'edge',body,.02)
    head=pivot('Diamond helmet',(0,-.01,2.45))
    plate('Diamond helmet dark rear',[(-.19,2.63),(0,2.93),(.19,2.63),(.12,2.38),(0,2.26),(-.13,2.39)],.03,.30,'dark',head)
    plate('Helmet left ivory facet',[(-.19,2.63),(0,2.91),(-.025,2.64),(-.07,2.36),(-.15,2.48)],-.16,.075,'white',head)
    plate('Helmet right ivory facet',[(.19,2.63),(0,2.91),(.025,2.64),(.07,2.36),(.15,2.48)],-.16,.075,'white',head)
    plate('Long luminous face slit',[(0,2.70),(.053,2.61),(.025,2.39),(0,2.31),(-.025,2.39),(-.053,2.61)],-.215,.022,'hot',head)
    hips=pivot('Pelvis',(0,0,1.48));plate('Pointed pelvis',[(-.23,1.62),(.23,1.62),(.17,1.37),(0,1.24),(-.17,1.37)],0,.28,'dark',hips)
    for s in [-1,1]:
        arm=pivot(('Left'if s<0 else'Right')+' shoulder',(s*.37,0,2.15))
        ball('Shoulder ball',(s*.38,0,2.13),(.14,.14,.14),'edge',arm)
        plate('Layered pointed pauldron',[(s*.28,2.22),(s*.42,2.38),(s*.65,2.23),(s*.72,2.04),(s*.48,2.08)],-.025,.29,'white',arm)
        plate('Pauldrons lower segment',[(s*.44,2.08),(s*.69,2.03),(s*.64,1.91),(s*.47,1.97)],-.045,.24,'edge',arm)
        beam('Upper arm exposed piston',(s*.49,0,2.04),(s*.68,-.01,1.80),.115,.13,'dark',arm)
        fore=pivot(('Left'if s<0 else'Right')+' blade forearm',(s*.68,-.01,1.80))
        cyl('Elbow hinge',(s*.68,-.01,1.80),.11,.21,'edge',fore)
        plate('Forearm angular gauntlet',[(s*.65,1.84),(s*.79,1.78),(s*.98,1.42),(s*.80,1.44),(s*.69,1.61)],-.015,.24,'dark',fore)
        plate('Forearm ivory edge',[(s*.72,1.77),(s*.81,1.75),(s*.95,1.45),(s*.87,1.48)],-.16,.055,'white',fore)
        cyl('Wrist energy emitter',(s*.86,-.16,1.55),.075,.06,'neon',fore)
        plate('Blade black tang',[(s*.85,1.52),(s*.95,1.49),(s*1.42,.68),(s*1.12,1.09)],-.07,.09,'dark',fore)
        plate('Long forearm energy sword',[(s*.85,1.49),(s*1.01,1.45),(s*1.43,.64),(s*1.07,1.07)],-.14,.032,'neon',fore,.006)
        beam('Sword incandescent spine',(s*.92,-.162,1.44),(s*1.41,-.162,.67),.018,.018,'hot',fore)
        leg=pivot(('Left'if s<0 else'Right')+' thigh',(s*.18,.01,1.43))
        beam('Thigh mechanical skeleton',(s*.18,.01,1.43),(s*.34,.01,.94),.12,.15,'dark',leg)
        plate('Flared hip armor',[(s*.20,1.57),(s*.38,1.48),(s*.49,1.11),(s*.35,1.18),(s*.25,1.34)],-.02,.23,'white',leg)
        plate('Thigh front ivory',[(s*.21,1.33),(s*.32,1.37),(s*.43,1.03),(s*.30,1.04)],-.14,.08,'white',leg)
        shin=pivot(('Left'if s<0 else'Right')+' shin',(s*.34,0,.94))
        ball('Knee mechanical joint',(s*.34,.01,.94),(.10,.11,.10),'edge',shin)
        beam('Shin black strut',(s*.34,.02,.9),(s*.45,.04,.24),.10,.11,'dark',shin)
        plate('Segmented shin armor',[(s*.29,.88),(s*.43,.88),(s*.49,.54),(s*.44,.25),(s*.37,.49)],-.095,.15,'white',shin)
        plate('Pointed armored foot',[(s*.38,.30),(s*.50,.25),(s*.55,.05),(s*.37,.09)],-.12,.29,'edge',shin)
        beam('Shin pink accent',(s*.39,-.18,.78),(s*.43,-.18,.47),.018,.018,'neon',shin)
    box('Back power spine',(0,.24,1.99),(.16,.13,.42),'edge',body)


def tank():
    start('core-tank',(1,.18,.008));C=(0,0,1.3);body=pivot('Heavy reactor',C)
    ball('Heavy spherical reactor chassis',C,(.79,.57,.72),'dark',body)
    cyl('Front reactor black recess',(0,-.55,1.3),.49,.17,'black',body)
    cyl('Orange reactor plasma disk',(0,-.65,1.3),.37,.06,'neon',body)
    cyl('Core dark inner iris',(0,-.69,1.3),.27,.03,'dark',body)
    for r,t,ma in [(.47,.032,'edge'),(.365,.028,'hot'),(.27,.018,'neon')]:ring('Tank concentric core ring',(0,-.72,1.3),r,t,ma,body)
    ball('Central sun lens',(0,-.74,1.3),(.13,.04,.13),'hot',body)
    for i in range(10):
        a=i*math.tau/10;p=pivot('Tank shell sector %02d'%i,(.63*math.sin(a),0,1.3+.57*math.cos(a)))
        radial_plate('Heavy segmented front armor',[(-.16,.44),(.16,.44),(.22,.61),(.14,.74),(-.14,.74),(-.23,.60)],-.30,.35,'white'if i%3 else'edge',p,a,C)
        radial_plate('Rear sealed shell segment',[(-.15,.40),(.15,.40),(.20,.60),(.11,.71),(-.15,.69)],.39,.24,'edge',p,a,C)
        radial_plate('Radial orange core vent',[(-.025,.44),(.025,.44),(.027,.56),(-.027,.56)],-.495,.02,'neon',p,a,C)
    for s in [-1,1]:
        shield=pivot(('Left'if s<0 else'Right')+' hanging shield',(s*.81,0,1.69))
        cyl('Heavy shield axle',(s*.83,0,1.56),.20,.26,'edge',shield,axis=(1,0,0))
        plate('Broad shield graphite volume',[(s*.87,1.95),(s*1.15,1.89),(s*1.40,.39),(s*1.17,.19),(s*.90,.82),(s*.75,1.57)],.02,.53,'dark',shield)
        plate('Broad ivory shield front plate',[(s*.87,1.92),(s*1.12,1.84),(s*1.30,.70),(s*1.16,.40),(s*.97,.90),(s*.82,1.55)],-.275,.09,'white',shield)
        plate('Shield graphite lower blade',[(s*1.06,.95),(s*1.32,.69),(s*1.39,.39),(s*1.18,.21)],-.30,.12,'edge',shield)
        beam('Long orange shield vent',(s*1.06,-.34,1.55),(s*1.25,-.34,.62),.045,.025,'neon',shield)
        box('Shield rear hinge cover',(s*1.04,.34,1.45),(.20,.13,.57),'white',shield)
        gun=pivot(('Left'if s<0 else'Right')+' top cannon',(s*.54,.02,1.99))
        box('Top weapon support',(s*.54,.02,1.97),(.24,.29,.28),'dark',gun)
        box('Broad rectangular cannon body',(s*.58,-.16,2.18),(.42,.92,.27),'dark',gun,.04)
        box('Cannon ivory upper armor',(s*.58,-.12,2.34),(.44,.83,.11),'white',gun,.025)
        box('Cannon lower collar',(s*.58,-.58,2.16),(.46,.18,.29),'edge',gun,.018)
        box('Muzzle black opening',(s*.58,-.68,2.17),(.27,.025,.13),'black',gun,.005)
        box('Muzzle orange slot',(s*.58,-.698,2.17),(.18,.016,.045),'hot',gun,.004)
        box('Cannon flank energy strip',(s*.805,-.16,2.19),(.018,.58,.045),'neon',gun,.005)
    keel=pivot('Ventral armor',(0,0,.62));plate('Heavy lower pointed keel',[(-.27,.72),(.27,.72),(.19,.35),(0,.22),(-.19,.35)],.02,.40,'edge',keel)
    cyl('Rear reactor service plate',(0,.57,1.3),.35,.10,'white',body);ring('Rear orange exhaust',(0,.64,1.3),.22,.025,'neon',body)

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
        node['translation'],node['rotation'],node['scale']=trs
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
            for prop,kind,default in [('translation','VEC3',[0,0,0]),('rotation','VEC4',[0,0,0,1]),('scale','VEC3',[1,1,1])]:
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

def merge_and_ground():
    global parts
    merged=[]
    groups={ctrl: [o for o in parts if o.parent==ctrl] for ctrl in controls}
    for ctrl in controls:
        members=groups[ctrl]
        if not members:continue
        bpy.ops.object.select_all(action='DESELECT')
        for o in members:o.select_set(True)
        bpy.context.view_layer.objects.active=members[0];bpy.ops.object.join();o=bpy.context.object;o.name=ctrl.name.replace(' pivot',' | rigid assembly');merged.append(o)
    parts=merged;bpy.context.view_layer.update()
    coords=[o.matrix_world@v.co for o in parts for v in o.data.vertices]
    low=min(p.z for p in coords);root.location.z-=low;bpy.context.view_layer.update()
    coords=[o.matrix_world@v.co for o in parts for v in o.data.vertices]
    return [[min(p.x for p in coords),min(p.z for p in coords),-max(p.y for p in coords)],[max(p.x for p in coords),max(p.z for p in coords),-min(p.y for p in coords)]]


def ridge(name,outline,y,depth,mat,p,crest=.08):
    # Closed armor volume with raised faceted front ridge, not a flat extrusion.
    n=len(outline);cx=sum(x for x,z in outline)/n;cz=sum(z for x,z in outline)/n
    verts=[(x,y-depth/2,z)for x,z in outline]+[(x,y+depth/2,z)for x,z in outline]+[(cx,y-depth/2-crest,cz)]
    faces=[(i,(i+1)%n,2*n)for i in range(n)]+[tuple(range(n,2*n))]+[(i,i+n,(i+1)%n+n,(i+1)%n)for i in range(n)]
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(verts,[],faces);mesh.update();bm=bmesh.new();bm.from_mesh(mesh);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(mesh);bm.free();o=bpy.data.objects.new(name,mesh);bpy.context.collection.objects.link(o);return finish(o,name,mat,p,.009)

def polish():
    by={c.name.removesuffix(' pivot'):c for c in controls}
    reactor=by.get('Reactor',by.get('Heavy reactor'))
    if reactor:
        z=1.35 if model=='drone-orbiter'else 1.3;y=-.59 if model=='drone-orbiter'else-.73
        # A recessed iris: black lens, layered stepped rings, discrete radial shutters.
        cyl('Recessed optical throat',(0,y-.023,z),.265,.025,'black',reactor)
        ring('Machined iris inner bezel',(0,y-.045,z),.232,.018,'edge',reactor)
        ring('Thin luminous iris aperture',(0,y-.058,z),.176,.009,'neon',reactor)
        cyl('Deep aperture pupil',(0,y-.065,z),.084,.025,'hot',reactor)
        for i in range(12):
            a=i*math.tau/12;x=.29*math.sin(a);zz=z+.29*math.cos(a)
            o=box('Iris radial mechanical shutter',(x,y-.04,zz),(.035,.028,.086),'edge',reactor,.004);o.rotation_euler.y=a
            cyl('Bezel fastener',( .415*math.sin(a),y-.03,z+.415*math.cos(a)),.014,.018,'edge',reactor,verts=8)
        for s in [-1,1]:
            cyl('Rear thrust turbine housing',(s*.26,.50,z),.13,.21,'dark',reactor)
            ring('Rear turbine rim',(s*.26,.62,z),.095,.017,'edge',reactor)
            cyl('Rear thrust lens',(s*.26,.635,z),.063,.025,'neon',reactor)
            for j in range(4):box('Rear turbine grille',(s*.26+(j-1.5)*.035,.66,z),(.012,.018,.14),'black',reactor,.003)
    if model=='drone-orbiter':
        for i,a in enumerate([0,math.tau/3,2*math.tau/3]):
            p=by['Dagger wing %d'%i];C=(0,0,1.35)
            outline=[(-.13,.66),(.17,.72),(.22,.93),(.10,1.54),(-.08,1.26),(-.20,.84)]
            points=[(x*math.cos(a)+z*math.sin(a),1.35-x*math.sin(a)+z*math.cos(a))for x,z in outline]
            ridge('Wing sculpted central armor ridge',points,.025,.16,'dark',p,.14)
            for r in [.72,.87]:
                q=(r*math.sin(a),-.21,1.35+r*math.cos(a));cyl('Wing root hex bolt',q,.026,.025,'edge',p,verts=6)
            beam('Wing rear hydraulic spar',(.52*math.sin(a),.14,1.35+.52*math.cos(a)),(.87*math.sin(a),.14,1.35+.87*math.cos(a)),.055,.055,'edge',p)
        for i in range(8):
            p=by['Armor sector %02d'%i];a=i*math.tau/8
            for r in [.51,.57]:cyl('Armor captive screw',(r*math.sin(a),-.39,1.35+r*math.cos(a)),.016,.018,'dark',p,verts=6)
    elif model=='blade-wraith':
        body=by['Torso'];head=by['Diamond helmet']
        ridge('Three dimensional crown facet',[(-.19,2.63),(0,2.93),(.19,2.63),(0,2.48)],.07,.22,'edge',head,.12)
        for s in [-1,1]:
            ridge('Angular helmet cheek plane',[(s*.07,2.65),(s*.18,2.62),(s*.12,2.38),(s*.045,2.31)],-.17,.06,'edge',head,.055)
            ridge('Overlapping clavicle crest',[(s*.035,2.19),(s*.27,2.29),(s*.37,2.17),(s*.18,2.05)],-.14,.11,'white',body,.10)
            ridge('Deep swept chest half',[(s*.055,2.10),(s*.27,2.17),(s*.21,1.91),(s*.10,1.77)],-.23,.065,'edge',body,.10)
            for j in range(3):beam('Oblique chest vent',(s*.13,-.34,2.05-j*.045),(s*.23,-.31,2.10-j*.045),.017,.014,'black',body)
            arm=by[('Left'if s<0 else'Right')+' shoulder'];fore=by[('Left'if s<0 else'Right')+' blade forearm'];shin=by[('Left'if s<0 else'Right')+' shin'];thigh=by[('Left'if s<0 else'Right')+' thigh']
            ridge('Shoulder peaked upper shell',[(s*.33,2.26),(s*.43,2.41),(s*.62,2.25),(s*.70,2.08),(s*.48,2.14)],.05,.22,'white',arm,.12)
            beam('Upper arm chrome piston',(s*.48,-.095,2.02),(s*.66,-.095,1.84),.035,.035,'edge',arm)
            beam('Forearm hydraulic ram',(s*.71,.13,1.77),(s*.88,.13,1.48),.035,.035,'edge',fore)
            ridge('Forearm peaked vambrace',[(s*.68,1.82),(s*.8,1.77),(s*.96,1.43),(s*.82,1.49)],.01,.20,'edge',fore,.08)
            ridge('Knife edge shin greave',[(s*.29,.88),(s*.42,.88),(s*.50,.50),(s*.46,.19),(s*.37,.49)],-.11,.10,'edge',shin,.085)
            plate('Sharp pointed toe',[(s*.38,.22),(s*.48,.20),(s*.54,.015),(s*.36,.06)],-.21,.20,'dark',shin)
            for z,x in [(2.18,.52),(1.72,.75),(.78,.37)]:
                parent=arm if z>2 else fore if z>1 else shin;cyl('Armor hinge hex fastener',(s*x,-.22,z),.022,.023,'black',parent,verts=6)
            for j in range(3):box('Back power stack vent',(s*.06,.325,1.87+j*.09),(.05,.025,.045),'neon'if j==1 else'black',body,.004)
        for side,sign in [('Left',-1),('Right',1)]:
            fore=by[side+' blade forearm'];wrist=pivot(side+' blade wrist',(sign*.86,-.08,1.55));bpy.context.view_layer.update()
            for o in parts:
                if o.parent==fore and any(x in o.name for x in ['Long forearm energy sword','Sword incandescent spine','Blade black tang','Wrist energy emitter']):
                    wm=o.matrix_world.copy();o.parent=wrist;o.matrix_world=wm
        # Longer tapered energy edges with a thick mechanical spine.
        for o in parts:
            if 'Long forearm energy sword'in o.name or'Sword incandescent spine'in o.name or'Blade black tang'in o.name:
                wm=o.matrix_world.copy()
                for v in o.data.vertices:
                    p=wm@v.co
                    if p.z<1.45:p.z=1.45+(p.z-1.45)*1.18;p.x*=1.06
                    v.co=wm.inverted()@p
    else:
        body=by['Heavy reactor']
        for s in [-1,1]:
            shield=by[('Left'if s<0 else'Right')+' hanging shield'];gun=by[('Left'if s<0 else'Right')+' top cannon']
            ridge('Shield sculpted loadbearing ridge',[(s*.90,1.88),(s*1.11,1.80),(s*1.27,.76),(s*1.18,.40),(s*.99,.96)],-.22,.20,'edge',shield,.11)
            beam('Raised shield neon channel',(s*1.055,-.47,1.57),(s*1.245,-.46,.63),.035,.020,'neon',shield)
            beam('Exposed shield actuator',(s*.79,.12,1.76),(s*1.03,.12,1.10),.07,.07,'edge',shield)
            cyl('Shield actuator collar',(s*.87,.12,1.53),.065,.14,'dark',shield,axis=(0,0,1),verts=12)
            for j in range(4):box('Shield rear exhaust slot',(s*1.08,.425,1.25+j*.105),(.16,.018,.035),'black',shield,.004)
            for z in [.70,1.25,1.74]:cyl('Shield front captive bolt',(s*(1.2 if z<1 else 1.02),-.39,z),.022,.025,'black',shield,verts=6)
            box('Cannon rail',(s*.58,-.12,2.425),(.10,.65,.06),'edge',gun,.009)
            for j in range(5):box('Cannon heat sink fin',(s*.8,-.36+j*.105,2.25),(.025,.045,.10),'edge',gun,.004)
            cyl('Cannon elevation trunnion',(s*.54,.03,2.01),.13,.30,'edge',gun,axis=(1,0,0),verts=16)
            flash=pivot(('Left'if s<0 else'Right')+' muzzle flash',(s*.58,-.80,2.17));flash['gun']=gun.name
            ball('Muzzle flash plasma spindle',(s*.58,-.80,2.17),(.085,.22,.085),'hot',flash)
            flash.scale=(.001,)*3
        for i in range(5):box('Rear radiator slat',(0,.68,.99+i*.13),(.33,.06,.035),'black',body,.005)

def hierarchy():
    global controls
    originals=list(controls);by={c.name.removesuffix(' pivot'):c for c in originals}
    master=pivot('Motion',(0,0,1.35))
    bpy.context.view_layer.update()
    def parent_keep(c,parent):
        bpy.context.view_layer.update();wm=c.matrix_world.copy();c.parent=parent;c.matrix_parent_inverse.identity();c.matrix_world=wm;bpy.context.view_layer.update()
    for c in originals:parent_keep(c,master)
    if model=='blade-wraith':
        for key in ['Diamond helmet','Left shoulder','Right shoulder','Pelvis']:parent_keep(by[key],by['Torso'])
        for side in ['Left','Right']:
            parent_keep(by[side+' blade forearm'],by[side+' shoulder']);parent_keep(by[side+' blade wrist'],by[side+' blade forearm']);parent_keep(by[side+' thigh'],by['Pelvis']);parent_keep(by[side+' shin'],by[side+' thigh'])
    else:
        body=by['Reactor'if model=='drone-orbiter'else'Heavy reactor']
        for c in originals:
            if c!=body and 'projectile'not in c:parent_keep(c,body)
        for c in originals:
            if'gun'in c:parent_keep(c,bpy.data.objects[c['gun']])
    def depth(c):
        d=0;p=c.parent
        while p:d+=1;p=p.parent
        return d
    controls=sorted([master]+originals,key=depth)
    for c in controls:
        c['neutral_location']=list(c.location);c['neutral_rotation']=list(c.rotation_quaternion);c['neutral_scale']=list(c.scale);c['neutral_world']=[v for row in c.matrix_world for v in row]

# Authored phase envelopes, rather than one shared oscillation on every part.
def envelope(t,keys):
    for (a,x),(b,y)in zip(keys,keys[1:]):
        if a<=t<=b:
            u=(t-a)/(b-a);u=u*u*(3-2*u);return x+(y-x)*u
    return keys[0][1]if t<keys[0][0]else keys[-1][1]

def apply_pose(clip,t):
    from mathutils import Matrix,Euler
    if clip=='Assemble':clip='Explode';t=1-t
    for i,c in enumerate(controls):
        n=c.name.lower();loc=Vector(c['neutral_location']);q=Quaternion(c['neutral_rotation']);sc=Vector(c['neutral_scale']);d=Vector((0,0,0));angles=[0,0,0]
        if clip=='Idle':
            a=math.sin(t*math.tau);b=1-math.cos(t*math.tau)
            if n=='motion pivot':
                d.z=(.065 if model=='drone-orbiter'else .025 if model=='blade-wraith'else .012)*b
                angles=[.025*a,0,.06*a if model=='drone-orbiter'else .012*a]
            if model=='drone-orbiter'and'wing'in n:angles[1]=.07*math.sin(t*math.tau+i*.7)*math.sin(t*math.pi)**2
            if model=='blade-wraith':
                if'diamond'in n:angles[2]=.06*a
                if'shoulder'in n:angles[1]=(.035 if'left'in n else-.035)*b
                if'forearm'in n:angles[0]=-.025*b
            if model=='core-tank'and'cannon'in n:angles[0]=.025*a;angles[2]=.025*b*(1 if'left'in n else-1)
        elif clip=='Attack':
            if model=='drone-orbiter':
                bank=envelope(t,[(0,0),(.19,-.50),(.30,-.65),(.45,.85),(.62,.45),(.85,-.12),(1,0)])
                if n=='motion pivot':
                    d.y=envelope(t,[(0,0),(.22,.36),(.40,-1.12),(.58,-.60),(.82,.15),(1,0)])
                    d.z=envelope(t,[(0,0),(.22,.25),(.42,.02),(.65,.25),(1,0)])
                    angles=[envelope(t,[(0,0),(.22,-.25),(.40,.45),(.65,-.18),(1,0)]),bank,envelope(t,[(0,0),(.25,-.3),(.52,2.2),(.75,3.8),(1,math.tau)])]
                elif'wing'in n:angles[1]=bank*(.7 if i%2 else-.7);angles[2]=envelope(t,[(0,0),(.22,-.22),(.42,.35),(.68,.10),(1,0)])
            elif model=='blade-wraith':
                left='left'in n;sign=1 if left else-1
                twist=envelope(t,[(0,0),(.22,-.65),(.40,.85),(.58,-.75),(.76,.25),(1,0)])
                if n=='motion pivot':d.y=envelope(t,[(0,0),(.2,.18),(.4,-.45),(.62,-.25),(1,0)]);d.z=.04*math.sin(t*math.pi)**2
                if'torso'in n:angles=[.10*math.sin(t*math.pi),.12*twist,twist]
                if'diamond'in n:angles[2]=-.35*twist
                if'shoulder'in n:
                    keys=[(0,0),(.23,-.95),(.40,.85),(.60,.15),(.80,.20),(1,0)]if left else[(0,0),(.25,-.35),(.43,-1.0),(.60,.95),(.80,.18),(1,0)]
                    angles=[envelope(t,keys),sign*envelope(t,[(0,0),(.24,.7),(.42,-.5),(.63,.42),(1,0)]),sign*.18*math.sin(t*math.pi)]
                if'forearm'in n:angles=[envelope(t,[(0,0),(.24 if left else.40,-1.1),(.43 if left else.62,.50),(.8,-.15),(1,0)]),sign*.25*math.sin(t*math.pi),0]
                if'blade wrist'in n:angles=[0,sign*envelope(t,[(0,0),(.28 if left else .44,-.45),(.48 if left else .68,.75),(.85,-.12),(1,0)]),sign*.18*math.sin(t*math.pi)]
                if'thigh'in n:angles=[sign*.15*twist,sign*.1*math.sin(t*math.pi),-.20*twist]
                if'shin'in n:angles[0]=-.22*math.sin(t*math.pi)**2
            else:
                brace=envelope(t,[(0,0),(.20,1),(.72,1),(1,0)])
                if n=='motion pivot':d.z=-.06*brace;angles[0]=-.04*brace
                if'shield'in n:angles[1]=(.16 if'left'in n else-.16)*brace;d.y=-.10*brace
                if'cannon'in n:
                    left='left'in n;recoil=envelope(t,[(0,0),(.30 if left else.48,0),(.34 if left else.52,1),(.47 if left else.65,0),(1,0)])
                    d.y=.25*recoil;angles=[-.15*brace+.13*recoil,0,(.07 if left else-.07)*brace]
                if'muzzle flash'in n:
                    left='left'in n;flash=envelope(t,[(0,0),(.31 if left else.49,0),(.335 if left else.515,1),(.38 if left else.56,0),(1,0)]);sc=Vector((max(.001,flash),)*3)
                if'heavy reactor'in n:d.y=envelope(t,[(0,0),(.32,0),(.36,.08),(.46,0),(.50,0),(.54,.08),(.68,0),(1,0)])
        elif clip=='Explode':
            if n=='motion pivot':
                angles[2]=envelope(t,[(0,0),(.05,.08),(.09,-.07),(.14,0),(1,0)])
            elif'muzzle flash'not in n and 'projectile'not in c:
                from mathutils import Matrix
                rest=Matrix([c['neutral_world'][j:j+4]for j in range(0,16,4)]);origin=rest.translation.copy()
                delay=.12+(i%5)*.024;burst=envelope(t,[(0,0),(delay,0),(delay+.18,1.06),(.62,1),(1,1)])
                direction=origin-Vector((0,0,1.35));direction.y+=.30*math.sin(i*2.1)
                if direction.length<.2:direction=Vector((.3,.6,.4))
                direction.normalize();offset=direction*(.95+.15*(i%4))*burst;offset.z=max(offset.z,.25*burst)
                spin=Quaternion(Vector((.4+math.sin(i),.6,1)).normalized(),(1.0+i%3*.3)*burst)
                target=Matrix.LocRotScale(origin+offset,spin@rest.to_quaternion(),rest.to_scale());local=c.parent.matrix_world.inverted()@target
                c.location,c.rotation_quaternion,c.scale=local.decompose();bpy.context.view_layer.update();continue
        sc=combat_pose(globals(),c,clip,t,d,angles,sc)
        c.location=loc+d;c.rotation_quaternion=q@Euler(angles,'XYZ').to_quaternion();c.scale=sc;bpy.context.view_layer.update()

def animate():
    scene=bpy.context.scene;scene.render.fps=24;scene.frame_start=1;scene.frame_end=97
    durations={'Idle':4,**DURATIONS,'Attack':3.5 if model!='core-tank'else 4,'Explode':3,'Assemble':2.5}
    for clip,duration in durations.items():
        for c in controls:c.animation_data_create();c.animation_data.action=bpy.data.actions.new(model+' | '+clip+' | '+c.name)
        frames=round(duration*24)
        for f in range(frames+1):
            apply_pose(clip,f/frames)
            for c in controls:
                for path in ['location','rotation_quaternion','scale']:c.keyframe_insert(data_path=path,frame=f+1)
        for c in controls:
            action=c.animation_data.action;c.animation_data.action=None;track=c.animation_data.nla_tracks.new();track.name=clip;strip=track.strips.new(clip,1,action);strip.blend_type='REPLACE';strip.extrapolation='NOTHING';track.mute=True
    apply_pose('Idle',0);scene.frame_set(1);bpy.context.view_layer.update()

def write_doc(path,doc,binary):
    payload=json.dumps(doc,separators=(',',':')).encode();payload+=b' '*((-len(payload))%4);binary+=b'\0'*((-len(binary))%4)
    path.write_bytes(struct.pack('<4sII',b'glTF',2,28+len(payload)+len(binary))+struct.pack('<I4s',len(payload),b'JSON')+payload+struct.pack('<I4s',len(binary),b'BIN\0')+binary)

def variants(path):
    raw=path.read_bytes();size=struct.unpack_from('<I',raw,12)[0];doc=json.loads(raw[20:20+size]);binary=raw[28+size:]
    import copy
    for name,color in {'pink':[1,.008,.24],'blue':[.008,.28,1],'green':[.008,1,.22],'amber':[1,.18,.008]}.items():
        d=copy.deepcopy(doc)
        for mat in d['materials']:
            if mat['name'].startswith('Neon |'):
                c=([.65+.35*v for v in color]if'filament'in mat['name']else [.06+.94*v for v in color]if'bright core'in mat['name']else color)
                mat['pbrMetallicRoughness']['baseColorFactor']=[*c,1];mat['emissiveFactor']=c
        write_doc(HERE/(model+'-'+name+'.glb'),d,binary)

def studio():
    scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=16;scene.cycles.use_denoising=True
    scene.render.resolution_x=500;scene.render.resolution_y=540;scene.render.resolution_percentage=100
    scene.world.color=(.045,.045,.045)
    scene.view_settings.view_transform='Standard';scene.view_settings.exposure=-1.2
    tree=bpy.data.node_groups.new('Studio restrained neon bloom','CompositorNodeTree')
    scene.compositing_node_group=tree
    tree.interface.new_socket(name='Image',in_out='OUTPUT',socket_type='NodeSocketColor')
    rl=tree.nodes.new('CompositorNodeRLayers');gl=tree.nodes.new('CompositorNodeGlare');end=tree.nodes.new('NodeGroupOutput')
    gl.inputs['Type'].default_value='Fog Glow';gl.inputs['Threshold'].default_value=.8;gl.inputs['Strength'].default_value=.28;gl.inputs['Size'].default_value=.2
    tree.links.new(rl.outputs['Image'],gl.inputs['Image']);tree.links.new(gl.outputs['Image'],end.inputs['Image'])
    bpy.ops.mesh.primitive_plane_add(size=200,location=(0,0,-.025));floor=bpy.context.object;floor.name='STUDIO floor';floor.data.materials.append(material('STUDIO floor',(.018,.025,.04),.2,.5))
    for name,loc,power,size,col in [('Key',(-3,-4,6),700,4,(.8,.9,1)),('Rim',(3,2,5),1000,3,(.65,.78,1)),('Fill',(3,-4,2),300,3,(1,.65,.72))]:
        data=bpy.data.lights.new(name,'AREA');data.energy=power;data.shape='DISK';data.size=size;data.color=col;o=bpy.data.objects.new(name,data);bpy.context.collection.objects.link(o);o.location=loc;o.rotation_euler=(Vector((0,0,1.3))-o.location).to_track_quat('-Z','Y').to_euler()
    data=bpy.data.cameras.new('Inspection camera');cam=bpy.data.objects.new('Inspection camera',data);bpy.context.collection.objects.link(cam);data.type='ORTHO';data.ortho_scale=3.9;scene.camera=cam
    cam.location=(3,-7,3.4);cam.rotation_euler=(Vector((0,0,1.4))-cam.location).to_track_quat('-Z','Y').to_euler()
    return cam

def render_views(cam):
    scene=bpy.context.scene
    for label,loc,clip,t in [('front',(0,-7,3),'Idle',0),('side',(7,0,1.4),'Idle',0),('hero',(3,-7,3.4),'Idle',0),('rear',(-3,7,3.4),'Idle',0),('explode',(3,-7,3.4),'Explode',1),('attack',(3,-7,3.4),'Attack',.335 if model=='core-tank'else .42),('run',(3,-7,3.4),'Run',.25),('attack2',(5,-7,3.4),'Attack2',.47),('attack3',(5,-7,3.4),'Attack3',.56),('attack4',(5,-7,3.4),'Attack4',.66)]:
        apply_pose(clip,t)
        cam.location=loc;cam.rotation_euler=(Vector((0,0,1.4))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.ortho_scale=5.6 if label=='explode' or label.startswith('attack')else 3.9
        scene.render.filepath=str(OUT/(model+'-'+label+'.png'));bpy.ops.render.render(write_still=True)
    apply_pose('Idle',0)

sys.path.insert(0,str(HERE))
from design_v3 import redesign
from combat_animation import add_effects, pose as combat_pose, DURATIONS

manifest=[]
for build in [drone,wraith,tank]:
    build();polish();redesign(globals());add_effects(globals());bounds=merge_and_ground();hierarchy();animate()
    neutral={c.name:([c.location.x,c.location.z,-c.location.y],[c.rotation_quaternion.x,c.rotation_quaternion.z,-c.rotation_quaternion.y,c.rotation_quaternion.w],list(c.scale))for c in controls}
    # Exporter includes muted NLA tracks when export_animation_mode=NLA_TRACKS.
    for c in controls:
        for tr in c.animation_data.nla_tracks:tr.mute=False
    bpy.ops.object.select_all(action='DESELECT')
    for o in [root]+controls+parts:o.select_set(True)
    bpy.context.view_layer.objects.active=parts[0]
    path=HERE/(model+'.glb')
    bpy.ops.export_scene.gltf(filepath=str(path),export_format='GLB',use_selection=True,export_apply=True,export_animations=True,export_animation_mode='NLA_TRACKS',export_merge_animation='NLA_TRACK',export_force_sampling=True,export_optimize_animation_size=False,export_yup=True,export_materials='EXPORT')
    preserve_neutral_clip_channels(path,neutral,bounds);variants(path)
    for c in controls:
        for tr in c.animation_data.nla_tracks:tr.mute=True
    bpy.context.scene.frame_set(1)
    apply_pose('Idle',0)
    cam=studio()
    for c in controls:
        for tr in c.animation_data.nla_tracks:tr.mute=tr.name!='Idle'
    bpy.ops.wm.save_as_mainfile(filepath=str(HERE/(model+'.blend')))
    for c in controls:
        for tr in c.animation_data.nla_tracks:tr.mute=True
    raw=path.read_bytes();size=struct.unpack_from('<I',raw,12)[0];doc=json.loads(raw[20:20+size])
    tris=sum(sum(len(p.vertices)-2 for p in o.data.polygons)for o in parts)
    item={'model':model,'bounds':bounds,'triangles':tris,'assemblies':len(parts),'primitives':sum(len(m['primitives'])for m in doc['meshes']),'clips':{a['name']:max(doc['accessors'][s['input']]['max'][0]for s in a['samplers'])for a in doc['animations']}}
    assert set(item['clips'])=={'Idle','Attack','Explode','Assemble',*DURATIONS},item
    assert all(x>1 for x in item['clips'].values())
    assert item['primitives']<100 and tris<40000,item
    manifest.append(item);print('VERIFIED_MODEL',json.dumps(item),flush=True)
    render_views(cam)
(HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('ALL_ENEMIES_READY',flush=True)
