"""V3 hard-surface redesign, evaluated inside the native Blender builder.
Geometry, not a render overlay: closed shell patches, machined hollow collars,
faceted armor and separately modeled luminous blade cores. Existing rigs retained.
"""
import bpy, bmesh, math
from mathutils import Vector

def redesign(api):
    global A, M, parts
    A=api;M=A['M'];parts=A['parts'];slug=A['model']
    M['armor']=A['material']('Gunmetal | satin armor',(.31,.35,.44),.48,.30)
    M['rim']=A['material']('Silver | milled edge',(.60,.67,.77),.55,.26)
    color=(1,.18,.008) if slug=='core-tank' else (1,.008,.24)
    M['filament']=A['material']('Neon | white filament',tuple(.65+.35*x for x in color),.08,.22,3)
    by={c.name.removesuffix(' pivot'):c for c in A['controls']}
    if slug=='blade-wraith':wraith(by)
    else:orb(by,slug=='core-tank')
    A['parts']=parts

def remove(words,parent=None):
    for o in list(parts):
        if (parent is None or o.parent==parent) and any(w in o.name for w in words):
            parts.remove(o);bpy.data.objects.remove(o,do_unlink=True)

def mesh(name,v,f,mat,p,bevel=.008,smooth=False):
    data=bpy.data.meshes.new(name);data.from_pydata(v,[],f);data.update()
    bm=bmesh.new();bm.from_mesh(data);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(data);bm.free()
    obj=bpy.data.objects.new(name,data);bpy.context.collection.objects.link(obj)
    return A['finish'](obj,name,mat,p,bevel,smooth=smooth)

def ridge(name,pts,y,d,mat,p,crest=.06):return A['ridge'](name,pts,y,d,mat,p,crest)
def plate(name,pts,y,d,mat,p):return A['plate'](name,pts,y,d,mat,p,.006)
def ring(name,c,r,t,mat,p):return A['ring'](name,c,r,t,mat,p)
def cyl(name,c,r,d,mat,p,axis=(0,-1,0),verts=16):return A['cyl'](name,c,r,d,mat,p,axis,verts)
def ball(name,c,s,mat,p):return A['ball'](name,c,s,mat,p)
def beam(name,a,b,w,d,mat,p):return A['beam'](name,a,b,w,d,mat,p)
def box(name,c,s,mat,p):return A['box'](name,c,s,mat,p,.009)

def collar(name,profile,z,mat,p,segments=48):
    # Closed section revolved about Y; profiles can taper and step the optical throat.
    v=[(r*math.sin(i*math.tau/segments),y,z+r*math.cos(i*math.tau/segments)) for r,y in profile for i in range(segments)]
    f=[];n=len(profile)
    for j in range(n):
        for i in range(segments):f.append((j*segments+i,j*segments+(i+1)%segments,((j+1)%n)*segments+(i+1)%segments,((j+1)%n)*segments+i))
    return mesh(name,v,f,mat,p,.004,smooth=True)

def shell(name,z,rx,ry,rz,theta0,theta1,phi0,phi1,mat,p):
    # A thick section of an ellipsoid, rather than planar radial armor tiles.
    n=8;m=6;v=[]
    for layer in [1,.92]:
        for j in range(n+1):
            t=math.radians(theta0+(theta1-theta0)*j/n)
            for i in range(m+1):
                ph=math.radians(phi0+(phi1-phi0)*i/m)
                v.append((rx*layer*math.sin(t)*math.cos(ph),-ry*layer*math.cos(t),z+rz*layer*math.sin(t)*math.sin(ph)))
    stride=m+1;offset=(n+1)*(m+1);f=[]
    for j in range(n):
        for i in range(m):
            k=j*stride+i;f.extend([(k,k+1,k+1+stride,k+stride),(k+offset,k+stride+offset,k+stride+1+offset,k+1+offset)])
    edge=list(range(m+1))+[j*stride+m for j in range(1,n+1)]+[n*stride+i for i in range(m-1,-1,-1)]+[j*stride for j in range(n-1,0,-1)]
    for i,k in enumerate(edge):q=edge[(i+1)%len(edge)];f.append((k,q,q+offset,k+offset))
    return mesh(name,v,f,mat,p,0,smooth=True)

def loft(name,sections,mat,p):
    # Variable cross sections shape the side silhouette as well as the front.
    # No constant-depth extrusion: both ends taper into the mechanical joints.
    segments=8;v=[]
    for x,y,z,rx,ry in sections:
        for i in range(segments):
            a=math.tau*i/segments
            v.append((x+rx*math.cos(a),y+ry*math.sin(a),z))
    f=[tuple(reversed(range(segments)))]
    for j in range(len(sections)-1):
        for i in range(segments):
            k=j*segments+i;q=j*segments+(i+1)%segments
            f.append((k,q,q+segments,k+segments))
    f.append(tuple(range(len(v)-segments,len(v))))
    return mesh(name,v,f,mat,p,.006)

def radial(pts,a,z):return [(x*math.cos(a)+r*math.sin(a),z-x*math.sin(a)+r*math.cos(a))for x,r in pts]

def orb(by,tank):
    body=by['Heavy reactor'if tank else'Reactor'];z=1.3 if tank else 1.35
    remove(['front core','front iris','Concentric reactor','Central energy','Front reactor','Orange reactor','Core dark','Tank concentric','Central sun','optical throat','iris','Iris radial','Bezel fastener','Deep aperture pupil'],body)
    r=.47 if tank else .43;y=-.73 if tank else-.61
    collar('V3 hollow reactor shoulder',[(r+.065,y+.16),(r+.055,y-.015),(r-.005,y-.065),(r-.035,y-.05),(r-.035,y+.14)],z,'armor',body)
    collar('V3 polished outer chamfer',[(r+.052,y-.012),(r+.04,y-.04),(r+.017,y-.055),(r+.003,y-.042),(r+.02,y-.02)],z,'rim',body)
    collar('V3 deep black optical throat',[(r-.026,y+.08),(r-.026,y-.058),(.235,y-.022),(.195,y+.05),(.195,y+.10)],z,'black',body)
    ring('V3 continuous energy annulus',(0,y-.062,z),r-.032,.025,'neon',body)
    ring('V3 thin incandescent ring',(0,y-.079,z),r-.032,.007,'filament',body)
    ring('V3 internal floating rotor',(0,y-.032,z),.236,.017,'edge',body)
    ring('V3 inner energy ring',(0,y-.042,z),.211,.010,'neon',body)
    ball('V3 convex glass energy lens',(0,y+.024,z),(.158,.08,.158),'neon',body)
    ball('V3 brilliant pupil',(0,y-.049,z),(.065,.027,.065),'filament',body)
    for i in range(9 if tank else 7):
        a=i*math.tau/(9 if tank else 7)
        # Swept iris vanes clearly separated from the luminous rings.
        pts=radial([(-.027,.245),(.025,.238),(.081,.333),(.03,.355)],a,z)
        ridge('V3 overlapping iris shutter',pts,y-.01,.026,'edge',body,.015)
    count=10 if tank else 8
    for i in range(count):
        key=('Tank shell sector %02d'if tank else'Armor sector %02d')%i;p=by[key]
        remove(['carapace','segmented front armor','Rear sealed shell','Armor captive screw','Radial orange core vent'],p)
        ph=90-i*360/count;span=360/count-7
        rx,ry,rz=(.81,.63,.77)if tank else(.68,.53,.69)
        shell('V3 curved separated carapace',z,rx,ry,rz,44 if tank else 45,111,ph-span*.5,ph+span*.5,'armor'if i%3==1 else'white',p)
        shell('V3 layered rear armor',z,rx*.99,ry,rz*.99,119,157,ph-span*.48,ph+span*.48,'edge',p)
        # Narrow ring seating, bolts and tiny edge apertures follow the curvature.
        a=i*math.tau/count;rr=.57 if tank else .49
        cyl('V3 panel recessed hex socket',(rr*math.sin(a),-.39 if tank else-.34,z+rr*math.cos(a)),.024,.024,'black',p,verts=6)
    if tank:heavy(by)
    else:drone_wings(by)

def drone_wings(by):
    z=1.35
    for i,a in enumerate([0,math.tau/3,2*math.tau/3]):
        p=by['Dagger wing %d'%i]
        remove(['Swept dagger','Wing ivory','Magenta inset','central armor ridge','Wing root hex'],p)
        ridge('V3 swept dagger chassis',radial([(-.15,.61),(.20,.64),(.27,.88),(.14,1.66),(-.065,1.38),(-.24,.89)],a,z),.015,.22,'dark',p,.085)
        ridge('V3 sculpted dagger dorsal shell',radial([(-.17,.69),(.14,.72),(.20,.87),(.15,1.50),(.01,1.26),(-.17,.94)],a,z),-.13,.075,'armor',p,.08)
        # Blade has a colored body, a white-hot sharp edge and a separate spine.
        plate('V3 blade energy body',radial([(-.17,.80),(-.07,.80),(.125,1.62),(-.115,1.20),(-.22,.94)],a,z),-.218,.026,'neon',p)
        plate('V3 white hot cutting edge',radial([(-.205,.91),(-.186,.91),(.125,1.62),(-.135,1.17)],a,z),-.237,.010,'filament',p)
        ridge('V3 root articulated silver clamp',radial([(-.18,.65),(.18,.66),(.21,.80),(-.18,.83)],a,z),-.19,.095,'rim',p,.025)
        for r in [.68,.76]:cyl('V3 wing root dark screw',(r*math.sin(a),-.28,z+r*math.cos(a)),.022,.016,'black',p,verts=6)
        # Energy cable is visible from the side; it connects the rotor to the blade.
        beam('V3 wing energy feed',(.50*math.sin(a),-.13,z+.50*math.cos(a)),(.68*math.sin(a),-.13,z+.68*math.cos(a)),.035,.035,'neon',p)

def wraith(by):
    body=by['Torso'];head=by['Diamond helmet']
    remove(['helmet','Helmet','face slit','crown facet'],head)
    cyl('V3 articulated neck spindle',(0,.045,2.27),.075,.22,'dark',head,axis=(0,0,1))
    cyl('V3 neck bearing collar',(0,.045,2.23),.09,.035,'edge',head,axis=(0,0,1))
    # An elongated sculpted diamond helmet with side planes, not a flat diamond slab.
    loft('Sculpted helmet with tapered occipital shell',[(0,-.14,2.29,.02,.13),(0,-.055,2.43,.105,.245),(0,-.01,2.65,.16,.31),(0,.015,2.82,.095,.17),(0,-.01,3.015,.004,.006)],'dark',head)
    for s in [-1,1]:
        ridge('V3 swept silver temple',[(0,3.005),(s*.21,2.67),(s*.13,2.46),(s*.07,2.32),(s*.08,2.65)],-.16,.06,'armor',head,.055)
        ridge('V3 bright crown facet',[(0,3.005),(s*.155,2.71),(s*.075,2.65)],-.205,.035,'rim',head,.04)
        ridge('V3 angular jaw guard',[(s*.115,2.62),(s*.15,2.54),(s*.065,2.31),(s*.045,2.45)],-.22,.035,'edge',head,.025)
    plate('V3 deep visor inset',[(0,2.80),(.086,2.66),(.047,2.42),(0,2.30),(-.047,2.42),(-.086,2.66)],-.278,.028,'black',head)
    ridge('V3 magenta crystalline visor',[(0,2.77),(.052,2.65),(.024,2.43),(0,2.34),(-.024,2.43),(-.052,2.65)],-.305,.022,'neon',head,.022)
    plate('V3 white visor filament',[(0,2.74),(.012,2.64),(0,2.37),(-.012,2.64)],-.342,.007,'filament',head)
    remove(['Narrow tapered torso'],body)
    loft('Contoured thorax and tapered waist',[(0,.015,1.63,.095,.09),(0,.035,1.80,.19,.145),(0,.025,2.05,.30,.20),(0,.02,2.18,.29,.14),(0,.02,2.23,.10,.085)],'dark',body)
    pelvis=by['Pelvis'];remove(['Pointed pelvis'],pelvis)
    loft('Contoured pelvic joint shell',[(0,0,1.25,.04,.045),(0,.015,1.39,.19,.13),(0,.025,1.57,.23,.15),(0,.025,1.62,.17,.11)],'dark',pelvis)
    remove(['Pectoral armor','Chest light rib','Chest micro reactor','clavicle crest','Deep swept chest half','Oblique chest vent'],body)
    for s in [-1,1]:
        ridge('V3 curved ribcage outer armor',[(s*.04,2.18),(s*.26,2.27),(s*.36,2.13),(s*.25,1.93),(s*.105,1.76)],-.12,.23,'dark',body,.12)
        ridge('V3 layered silver clavicle',[(s*.035,2.19),(s*.24,2.26),(s*.32,2.18),(s*.18,2.06),(s*.075,2.10)],-.24,.07,'armor',body,.045)
        ridge('V3 split tapered breastplate',[(s*.08,2.11),(s*.24,2.14),(s*.20,1.95),(s*.11,1.81)],-.28,.06,'edge',body,.06)
        beam('V3 chest energy seam',(s*.05,-.325,2.17),(s*.095,-.35,1.96),.018,.012,'neon',body)
        for j in range(3):beam('V3 inset rib vent',(s*.22,-.27,2.03-j*.047),(s*.29,-.18,2.07-j*.047),.025,.018,'black',body)
    ridge('V3 sternum reactor bezel',[(-.062,2.08),(0,2.16),(.062,2.08),(0,1.94)],-.33,.045,'black',body,.025)
    ridge('V3 sternum energy shard',[(-.027,2.08),(0,2.12),(.027,2.08),(0,1.98)],-.375,.012,'neon',body,.012)
    for side,s in [('Left',-1),('Right',1)]:
        shoulder=by[side+' shoulder'];fore=by[side+' blade forearm'];wrist=by[side+' blade wrist'];thigh=by[side+' thigh'];shin=by[side+' shin']
        remove(['Layered pointed pauldron','Pauldrons lower','Shoulder peaked'],shoulder)
        for j in range(3):
            dx=j*.065;dz=-j*.105
            pts=[(s*(.30+dx),2.28+dz),(s*(.42+dx),2.41+dz),(s*(.63+dx),2.25+dz),(s*(.70+dx),2.07+dz),(s*(.48+dx),2.14+dz)]
            ridge('V3 overlapping swept shoulder scale',pts,.02+j*.025,.22-j*.04,'white'if j==0 else'armor'if j==1 else'edge',shoulder,.075)
        cyl('V3 visible lateral shoulder bearing',(s*.48,.06,2.15),.115,.12,'edge',shoulder,axis=(1,0,0))
        remove(['Forearm angular gauntlet','Forearm ivory edge','Forearm peaked'],fore)
        loft('Tapered forearm chassis',[(s*.90,.015,1.41,.055,.055),(s*.85,.025,1.54,.115,.13),(s*.75,.015,1.72,.105,.135),(s*.68,0,1.85,.05,.07)],'dark',fore)
        ridge('V3 vambrace silver upper shell',[(s*.66,1.83),(s*.79,1.79),(s*.90,1.55),(s*.78,1.58)],-.15,.05,'armor',fore,.05)
        ridge('V3 pointed wrist shield',[(s*.83,1.64),(s*.96,1.57),(s*1.04,1.35),(s*.86,1.44)],-.13,.055,'edge',fore,.04)
        remove(['Long forearm energy sword','Sword incandescent spine','Blade black tang','Wrist energy emitter'],wrist)
        cyl('V3 recessed wrist power socket',(s*.865,-.145,1.53),.09,.065,'black',wrist)
        ring('V3 wrist energy annulus',(s*.865,-.186,1.53),.055,.012,'neon',wrist)
        tip=(s*1.53,.35)
        ridge('V3 tapered blade housing',[(s*.86,1.49),(s*1.01,1.44),(s*1.53,.35),(s*1.16,1.12)],-.06,.09,'dark',wrist,.022)
        plate('V3 full energy blade',[(s*.88,1.46),(s*.98,1.42),tip,(s*1.13,1.08)],-.136,.024,'neon',wrist)
        plate('V3 incandescent sword edge',[(s*.89,1.44),(s*.91,1.43),tip,(s*1.145,1.04)],-.154,.009,'filament',wrist)
        remove(['Flared hip armor','Thigh front ivory'],thigh)
        ridge('V3 swept hip blade',[(s*.19,1.59),(s*.37,1.50),(s*.49,1.15),(s*.35,1.24),(s*.22,1.40)],.01,.24,'armor',thigh,.065)
        ridge('V3 separated thigh armor',[(s*.22,1.34),(s*.34,1.31),(s*.42,1.08),(s*.30,1.01),(s*.24,1.13)],-.13,.075,'white',thigh,.045)
        remove(['Segmented shin armor','Knife edge shin','Pointed armored foot','Sharp pointed toe'],shin)
        ridge('V3 upper knee floating shield',[(s*.27,.97),(s*.38,1.02),(s*.46,.87),(s*.39,.73),(s*.31,.82)],-.14,.11,'armor',shin,.06)
        loft('Tapered spear greave side profile',[(s*.49,-.09,.04,.007,.012),(s*.445,-.04,.25,.042,.065),(s*.41,-.015,.52,.075,.11),(s*.37,-.015,.78,.07,.09)],'edge',shin)
        ridge('V3 silver shin outer facet',[(s*.39,.77),(s*.45,.71),(s*.49,.41),(s*.47,.21),(s*.42,.43)],-.17,.045,'white',shin,.025)
        plate('V3 narrow shin energy inset',[(s*.37,.68),(s*.39,.66),(s*.445,.22)],-.224,.009,'neon',shin)
        for j in range(3):box('V3 hip rear cooling fin',(s*.30,.18,1.36-j*.09),(.11,.10,.025),'edge',thigh)

def heavy(by):
    for side,s in [('Left',-1),('Right',1)]:
        p=by[side+' hanging shield'];g=by[side+' top cannon']
        remove(['Broad shield graphite','Broad ivory shield','Shield graphite lower','loadbearing ridge','Long orange shield vent','Raised shield neon'],p)
        ridge('V3 heavy tapered shield housing',[(s*.80,1.98),(s*1.18,1.91),(s*1.43,.35),(s*1.22,.10),(s*.98,.70),(s*.75,1.51)],.02,.48,'dark',p,.09)
        ridge('V3 upper beveled shield cap',[(s*.82,1.95),(s*1.14,1.89),(s*1.18,1.57),(s*.99,1.36),(s*.83,1.55)],-.25,.13,'white',p,.08)
        ridge('V3 long diagonal shield armor',[(s*.99,1.49),(s*1.19,1.62),(s*1.35,.55),(s*1.22,.24),(s*1.10,.74)],-.25,.11,'armor',p,.06)
        plate('V3 armored energy recess',[(s*.94,1.55),(s*.99,1.56),(s*1.27,.40),(s*1.23,.26)],-.38,.02,'black',p)
        plate('V3 long shield energy blade',[(s*.95,1.52),(s*.98,1.53),(s*1.26,.42),(s*1.23,.29)],-.399,.014,'neon',p)
        beam('V3 shield incandescent spine',(s*.965,-.415,1.48),(s*1.24,-.415,.39),.009,.009,'filament',p)
        remove(['Cannon ivory upper armor','Cannon rail'],g)
        ridge('V3 cannon slanted armor roof',[(s*.35,2.35),(s*.40,2.48),(s*.69,2.43),(s*.81,2.29),(s*.59,2.27)],-.08,.78,'armor',g,.06)
        box('V3 rail gun double guide',(s*.51,-.13,2.48),(.045,.63,.038),'edge',g)
        box('V3 rail gun double guide',(s*.64,-.13,2.465),(.045,.63,.038),'edge',g)
        for j in range(3):
            box('V3 cannon front bevel vent',(s*.58+(j-1)*.09,-.686,2.28),(.045,.018,.03),'black',g)
        box('V3 muzzle incandescent strip',(s*.58,-.712,2.17),(.15,.008,.012),'filament',g)
