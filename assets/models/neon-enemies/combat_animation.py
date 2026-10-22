"""Portable, native glTF locomotion and combat clips. No JS frame animation.
Projectiles are visual choreography for an asset preview; collision/game damage
belongs to the host game. Independent pivots let clips reset every transient.
"""
import math
from mathutils import Vector

DURATIONS={'Run':1.4,'Attack2':3.0,'Attack3':3.6,'Attack4':4.0}

def add_effects(api):
    if api['model']=='blade-wraith':return
    tank=api['model']=='core-tank'
    for i in range(6):
        x=(-.58 if i%2==0 else .58) if tank else (-.08 if i%2==0 else .08)
        z=2.17 if tank else 1.35;y=-.84 if tank else -.75
        p=api['pivot']('Plasma projectile %d'%i,(x,y,z));p['projectile']=i
        api['box']('Plasma bolt body',(x,y,z),(.065,.30,.065),'neon',p,.009)
        api['box']('Incandescent bolt core',(x,y-.035,z),(.027,.39,.027),'hot',p,.005)
        p.scale=(.001,)*3
    if not tank:
        p=api['pivot']('Reactor muzzle flash',(0,-.74,1.35))
        api['ball']('Reactor discharge flash',(0,-.74,1.35),(.105,.23,.105),'hot',p)
        p.scale=(.001,)*3

def launches(clip,slot,tank):
    if clip=='Attack2':return [.36] if slot in ([0,1] if tank else [0]) else []
    if clip=='Attack3':return [[.28,.45,.62][slot//2]] if slot<6 else []
    if clip=='Attack4':return [.52+.04*(slot//2)]
    if clip=='Attack' and tank:return [.335] if slot==0 else [.515] if slot==1 else []
    return []

def pose(api,c,clip,t,d,angles,sc):
    """Modify an already neutral pose. Returns scale; effects also support old Attack."""
    model=api['model'];n=c.name.lower();env=api['envelope'];tank=model=='core-tank'
    side=1 if 'left' in n else -1
    wraith=model=='blade-wraith'
    if clip=='Run':
        a=math.sin(t*math.tau);b=1-math.cos(t*math.tau*2)
        if n=='motion pivot':
            d.z=(.045 if wraith else .055)*b
            angles[0]=(.12 if wraith else .17)*math.sin(t*math.pi)**2
            angles[1]=(.035 if wraith else .11)*a
        if wraith:
            if 'thigh'in n:angles[0]=side*.72*a
            if 'shin'in n:angles[0]=-.95*max(0,side*a)
            if 'shoulder'in n:angles[0]=-side*.58*a;angles[1]=side*.06*b
            if 'forearm'in n:angles[0]=-.18*b
            if 'torso'in n:angles[2]=.10*a
            if 'diamond'in n:angles[2]=-.07*a
        elif 'wing'in n:angles[1]=.16*a;angles[2]=.08*b
        elif 'shield'in n:angles[1]=side*.09*b
        elif 'cannon'in n:angles[0]=-.04*b
    elif clip in ['Attack2','Attack3','Attack4']:
        charge=env(t,[(0,0),(.22,1),(.49,1),(.78,.2),(1,0)])
        if wraith:
            if clip=='Attack2':
                swing=env(t,[(0,0),(.26,-1.8),(.39,-1.8),(.53,.95),(.69,.35),(1,0)])
                if 'shoulder'in n:angles[0]=swing;angles[1]=side*.16*charge
                if 'forearm'in n:angles[0]=-.55*charge
                if 'torso'in n:angles[0]=env(t,[(0,0),(.30,-.22),(.55,.40),(1,0)])
                if n=='motion pivot':d.y=-.45*math.sin(math.pi*t)**2;d.z=.18*math.sin(math.pi*t)**2
                if 'thigh'in n:angles[0]=side*.20*charge
                if 'shin'in n:angles[0]=-.35*charge
            elif clip=='Attack3':
                thrust=env(t,[(0,0),(.24,-.55),(.40,-.55),(.51,1),(.65,1),(1,0)])
                if n=='motion pivot':d.y=-.8*max(0,thrust)
                if 'torso'in n:angles[2]=.35*thrust;angles[0]=.14*charge
                if 'shoulder'in n:angles[0]=(-1.35 if side==1 else .30)*thrust;angles[1]=side*.18*charge
                if 'forearm'in n:angles[0]=-.65*charge*(1-max(0,thrust))
                if 'blade wrist'in n:angles[1]=side*.20*thrust
                if 'thigh'in n:angles[0]=side*.38*charge
                if 'shin'in n:angles[0]=-.25*charge
            else:
                spread=env(t,[(0,0),(.22,1),(.68,1),(.86,.25),(1,0)])
                spin=env(t,[(0,0),(.20,-.22),(.44,2.4),(.68,5.7),(.85,math.tau),(1,math.tau)])
                if n=='motion pivot':angles[2]=spin;d.z=.20*spread
                if 'shoulder'in n:angles[1]=side*1.05*spread;angles[0]=-.15*spread
                if 'forearm'in n:angles[0]=-.15*spread
                if 'blade wrist'in n:angles[1]=-side*.30*spread
                if 'thigh'in n:angles[0]=side*.20*spread
                if 'shin'in n:angles[0]=-.35*spread
        else:
            # Brace, aim, recoil. Different action rhythms are visible beyond bolts.
            times=sorted(set(x for slot in range(6)for x in launches(clip,slot,tank)))
            recoil=sum(env(t,[(0,0),(max(0,x-.015),0),(x+.015,1),(x+.10,0),(1,0)])for x in times)
            if n=='motion pivot':d.z=-.07*charge;d.y=.10*recoil;angles[0]=-.07*charge
            if 'cannon'in n:
                d.y=.22*recoil;angles[0]=-.04*charge+.10*recoil
                angles[2]=side*(.035 if clip!='Attack3' else .12)*charge
            if 'shield'in n:angles[1]=side*.22*charge
            if 'wing'in n:angles[1]=.22*charge;angles[2]=.11*charge
            if ('reactor'in n and 'flash'not in n):
                angles[2]=(.09 if tank else .18)*math.sin(t*math.tau)*charge
                if clip=='Attack4':sc*=1+.06*charge
    # Bolts travel forward in the model frame, independently of recoiling guns.
    if 'projectile'in c:
        slot=c['projectile'];sc=Vector((.001,)*3)
        for start in launches(clip,slot,tank):
            u=(t-start)/.22
            if 0<u<1:
                d.y=-3.4*u
                flare=min(1,u/.07,(1-u)/.12)
                sc=Vector((max(.001,flare*(1.65 if clip=='Attack4'else 1)),)*3)
    if 'muzzle flash'in n and clip in ['Attack2','Attack3','Attack4']:
        slots=[i for i in range(6) if not tank or i%2==(0 if 'left'in n else 1)]
        flash=0
        for start in sorted(set(x for i in slots for x in launches(clip,i,tank))):
            flash=max(flash,env(t,[(0,0),(max(0,start-.025),0),(start+.01,1),(start+.07,0),(1,0)]))
        sc=Vector((max(.001,flash*(1.5 if clip=='Attack4'else 1)),)*3)
    return sc
