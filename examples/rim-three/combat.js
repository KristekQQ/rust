import {WAVES,POOL_COUNTS,SWORD_DAMAGE,REFLECT_DAMAGE,healthFor,tickHealth,bladeDamageAllowed,hurt} from './combat-rules.js';
import * as THREE from 'three';
import {createSplitDeathSystem} from './split-death.js?v=cut-sync-1';
import {createProjectileSystem} from './projectiles.js?v=air-1';
import {createPlasmaAttack} from './plasma.js?v=plasma-1';

const BASE=new THREE.Vector3(0,0,-.14),TIP=new THREE.Vector3(0,0,-1.02);
const horizontal=(a,b)=>Math.hypot(a.x-b.x,a.z-b.z);

export function createSword(){
  const sword=new THREE.Group();sword.name='Gladius · hráčův meč';
  const steel=new THREE.MeshStandardMaterial({color:'#e7eef2',metalness:.8,roughness:.25});
  const brass=new THREE.MeshStandardMaterial({color:'#dcb363',metalness:.65,roughness:.34});
  const leather=new THREE.MeshStandardMaterial({color:'#4a2520',roughness:.85});
  const vertices=[-.036,0,-.14, .036,0,-.14, 0,.009,-.14, 0,-.009,-.14, -.031,0,-.83, .031,0,-.83, 0,.009,-.83, 0,-.009,-.83, 0,0,-1.02];
  const geometry=new THREE.BufferGeometry();geometry.setAttribute('position',new THREE.Float32BufferAttribute(vertices,3));
  geometry.setIndex([0,2,6,0,6,4,2,1,5,2,5,6,0,4,7,0,7,3,3,7,5,3,5,1,4,6,8,6,5,8,7,4,8,5,7,8,0,3,1,0,1,2]);geometry.computeVertexNormals();
  sword.add(new THREE.Mesh(geometry,steel));
  const guard=new THREE.Mesh(new THREE.BoxGeometry(.21,.032,.048),brass);guard.position.z=-.115;sword.add(guard);
  const handle=new THREE.Mesh(new THREE.CylinderGeometry(.021,.024,.14,10),leather);handle.rotation.x=Math.PI/2;handle.position.z=-.018;sword.add(handle);
  const pommel=new THREE.Mesh(new THREE.SphereGeometry(.031,10,6),brass);pommel.scale.set(1,1,.7);pommel.position.z=.064;sword.add(pommel);
  return sword;
}

function inside(p,b){return p.x>=b.minX&&p.x<=b.maxX&&p.z>=b.minZ&&p.z<=b.maxZ}
function blocked(a,b,obstacles){
  return obstacles.some(rect=>{
    let lo=0,hi=1;
    for(const [key,min,max]of [['x',rect.minX,rect.maxX],['z',rect.minZ,rect.maxZ]]){
      const d=b[key]-a[key];if(Math.abs(d)<1e-8){if(a[key]<min||a[key]>max)return false;}
      else{const x=(min-a[key])/d,y=(max-a[key])/d;lo=Math.max(lo,Math.min(x,y));hi=Math.min(hi,Math.max(x,y));if(lo>hi)return false;}
    }
    return true;
  });
}
function navigator(city){
  const obstacles=city.colliders.map(b=>({minX:b.minX-.55,maxX:b.maxX+.55,minZ:b.minZ-.55,maxZ:b.maxZ+.55}));
  const free=p=>p.x>-77&&p.x<77&&p.z>-81&&p.z<59&&!obstacles.some(b=>inside(p,b));
  const corners=[];
  for(const b of obstacles)for(const x of [b.minX-.04,b.maxX+.04])for(const z of [b.minZ-.04,b.maxZ+.04])if(free({x,z}))corners.push({x,z});
  const edges=corners.map((a,i)=>corners.map((b,j)=>i!==j&&!blocked(a,b,obstacles)?horizontal(a,b):Infinity));
  function waypoint(start,goal){
    if(!blocked(start,goal,obstacles))return {...goal};
    const nodes=[start,goal,...corners],dist=nodes.map(()=>Infinity),prev=nodes.map(()=>-1),seen=nodes.map(()=>false);dist[0]=0;
    for(let k=0;k<nodes.length;k++){
      let i=-1;for(let n=0;n<nodes.length;n++)if(!seen[n]&&(i<0||dist[n]<dist[i]))i=n;
      if(i<0||!Number.isFinite(dist[i]))return null;
      if(i===1){let j=1;while(prev[j]>0)j=prev[j];return {...nodes[j]};}
      seen[i]=true;
      for(let j=1;j<nodes.length;j++)if(!seen[j]){
        const cost=i>1&&j>1?edges[i-2][j-2]:blocked(nodes[i],nodes[j],obstacles)?Infinity:horizontal(nodes[i],nodes[j]);
        if(dist[i]+cost<dist[j]){dist[j]=dist[i]+cost;prev[j]=i;}
      }
    }
    return null;
  }
  return {free,waypoint,clear:(a,b)=>!blocked(a,b,obstacles)};
}
function pointSegment(p,a,b){const d=b.clone().sub(a);return p.distanceTo(a.clone().addScaledVector(d,THREE.MathUtils.clamp(p.clone().sub(a).dot(d)/Math.max(1e-10,d.lengthSq()),0,1)))}
function continuous(previous,current,dt){
  return Number.isFinite(dt)&&dt>0&&dt<=.15&&previous&&
    previous.grip.distanceTo(current.grip)<=Math.min(1,15*dt+.025)&&
    previous.rotation.angleTo(current.rotation)<=Math.min(1.5,45*dt+.03)&&
    Math.max(previous.a.distanceTo(current.a),previous.b.distanceTo(current.b))<=1.5;
}
function hitSweep(previous,current,unit,dt){
  const travel=Math.max(previous.a.distanceTo(current.a),previous.b.distanceTo(current.b));
  if(!continuous(previous,current,dt))return false;
  const p=unit.entry.holder.position,height=unit.entry.height,radius=unit.flying?height*.5:.47;
  const low=unit.flying?height*.25:.35,high=unit.flying?height*.75:height*.75;
  for(let i=0,n=Math.max(1,Math.ceil(travel/.035));i<=n;i++){
    const a=previous.a.clone().lerp(current.a,i/n),b=previous.b.clone().lerp(current.b,i/n);
    for(let y=low;y<=high;y+=.08)if(pointSegment(new THREE.Vector3(p.x,p.y+y,p.z),a,b)<radius)return true;
  }
  return false;
}


export function createCombat({scene,camera,renderer,controllers,characters,city,getHead,getForward,castWorld=()=>null}){
  const desktop=new THREE.Group();desktop.name='Meče na obrazovce';camera.add(desktop);
  const desktopSwords=[createSword(),createSword()];desktopSwords.forEach(s=>desktop.add(s));
  const vrSwords=controllers.map(({grip})=>{const s=createSword();grip.add(s);return s;});
  const healthBackGeometry=new THREE.BoxGeometry(.74,.075,.025),healthFillGeometry=new THREE.BoxGeometry(.68,.045,.03);
  const healthBackMaterial=new THREE.MeshBasicMaterial({color:0x182321});
  const frameState={simulation:0,visual:0,lastHit:null};
  const state={phase:'loading',active:0,kills:0,started:false,hitsTaken:0,health:100,deaths:0};
  let units=[],current=[],nav=null,vr=false,lastHead=null,age=0,waitingTime=0,waveForward=new THREE.Vector3(0,0,-1);
  let previous=[null,null],swing=[-1,-1],swingIds=[0,0],strikeSerial=0;
  const splitDeaths=createSplitDeathSystem(scene,{renderer,camera}),deathModeOverrides=new Map();let defaultDeathMode='cut';
  const deathMode=unit=>deathModeOverrides.get(unit.entry.slug)||defaultDeathMode;
  function setDeathMode(mode,slug=null){
    if(!['dissolve','cut'].includes(mode))throw new Error('Unknown enemy death mode: '+mode);
    if(slug!==null&&!['drone-orbiter','core-tank','blade-wraith'].includes(slug))throw new Error('Unknown enemy type: '+slug);
    if(slug===null){defaultDeathMode=mode;deathModeOverrides.clear();}else deathModeOverrides.set(slug,mode);
    for(const unit of units)if(slug===null||unit.entry.slug===slug)splitDeaths.prepare(unit,deathMode(unit));
    return {default:defaultDeathMode,overrides:Object.fromEntries(deathModeOverrides)};
  }
  const status=document.getElementById('combatStatus');
  const projectiles=createProjectileSystem({scene,castWorld,onEnemyHit:id=>damageEnemy(units.find(u=>u.id===id),REFLECT_DAMAGE),onPlayerHit:()=>damage(25)});
  let deathTime=0;
  const wound=new THREE.Mesh(new THREE.TorusGeometry(.32,.014,5,40),new THREE.MeshBasicMaterial({color:0xff3045,transparent:true,opacity:0,depthTest:false,depthWrite:false}));wound.position.set(0,0,-.65);camera.add(wound);
  function damage(amount){
    if(state.phase==='dead'||amount<=0)return;state.hitsTaken++;state.health=Math.max(0,state.health-amount);
    if(state.health===0){state.phase='dead';state.deaths++;deathTime=0;projectiles.clear();for(const u of units){u.plasma.cancel();u.charge.visible=false;}resetTracking();}
    announce();
  }
  function announce(){if(status)status.textContent=state.started?(state.phase==='dead'?'PADL JSI · nová šance za 3 s':`Život ${Math.ceil(state.health)} % · Vlna ${state.active+1} · zbývá ${current.filter(u=>u.phase!=='dying'&&u.phase!=='dead').length} · poraženo ${state.kills} · odraženo ${projectiles.describe().reflected}`):'';}
  function resetTracking(){previous=[null,null];lastHead=null;}
  function play(unit,name,loop=true){
    const entry=unit.entry,clip=entry.animations.find(c=>c.name.toLowerCase()===name.toLowerCase())||entry.animations.find(c=>c.name==='Idle')||entry.animations[0];
    entry.mixer.stopAllAction();if(!clip)return 0;
    const action=entry.mixer.clipAction(clip);action.reset().setLoop(loop?THREE.LoopRepeat:THREE.LoopOnce,loop?Infinity:1);action.clampWhenFinished=!loop;action.play();return clip.duration;
  }
  function approach(head){
    if(nav.free(head))return head;
    for(const radius of [.6,.9,1.15])for(let i=0;i<12;i++){const angle=i*Math.PI/6,p={x:head.x+Math.cos(angle)*radius,z:head.z+Math.sin(angle)*radius};if(nav.free(p))return p;}
    return head;
  }
  function center(unit){const p=unit.entry.holder.position;return new THREE.Vector3(p.x,p.y+unit.entry.height*.5,p.z);}
  function spawn(){
    if(!nav||!units.length)return;
    const [slug,count]=WAVES[state.active%WAVES.length],candidates=units.filter(u=>u.entry.slug===slug).slice(0,count);
    const head=getHead().clone(),goal=approach(head);waveForward=getForward().clone();waveForward.y=0;waveForward.normalize();
    const locations=[];
    for(let slot=0;slot<candidates.length;slot++){
      const slotAngle=(slot-(candidates.length-1)/2)*.32;let chosen=null;
      for(const distance of [8,6,10,4,12]){
        for(const extra of [0,.25,-.25,.65,-.65,1.2,-1.2,Math.PI]){
          const d=waveForward.clone().applyAxisAngle(new THREE.Vector3(0,1,0),slotAngle+extra);
          const p={x:head.x+d.x*distance,z:head.z+d.z*distance};
          if(nav.free(p)&&nav.waypoint(p,goal)&&locations.every(q=>horizontal(p,q)>1.2)){chosen=p;break;}
        }
        if(chosen)break;
      }
      if(!chosen){state.phase='waiting';waitingTime=0;return;}
      locations.push(chosen);
    }
    projectiles.clear();for(const u of units){u.entry.holder.visible=false;u.charge.visible=false;u.plasma.cancel();if(u.phase==='dying')splitDeaths.cancel(u);u.phase='dead';u.healthBar.visible=false;}
    current=candidates;
    current.forEach((u,slot)=>{
      const p=locations[slot];u.entry.holder.position.set(p.x,u.flying?Math.max(.6,head.y-.2-u.entry.height*.5):.13,p.z);
      u.entry.holder.rotation.set(0,0,0);u.entry.holder.visible=true;u.phase='chase';u.health=healthFor(u.entry.slug);u.hitFlash=0;u.healthBar.visible=true;u.death=0;u.pathAge=0;u.waypoint=null;u.lastGoal=null;
      u.slotAngle=(slot-(current.length-1)/2)*.32;u.shotTimer=1.8+slot*.8;u.flash=0;u.charging=false;u.salvos=0;
      play(u,u.flying?'Idle':'Run');
    });
    state.phase='wave';resetTracking();announce();
  }
  function damageEnemy(unit,amount,blade=null){
    if(!unit||!current.includes(unit)||['dying','dead'].includes(unit.phase)||!hurt(unit.health,amount))return false;
    unit.hitFlash=.12;frameState.lastHit=frameState.simulation;
    if(unit.health.hp===0)defeat(unit,blade);
    return true;
  }
  function defeat(unit,blade=null){
    if(!unit||!current.includes(unit)||unit.phase==='dying'||unit.phase==='dead')return;
    unit.health.hp=0;unit.healthBar.visible=false;unit.plasma.cancel();unit.phase='dying';unit.death=0;state.kills++;unit.charge.visible=false;
    const mode=deathMode(unit);let plane=null;
    if(mode==='cut'){
      const point=center(unit),normal=new THREE.Vector3(0,1,0);
      if(blade){
        const edge=blade.current.b.clone().sub(blade.current.a);
        const motion=blade.current.a.clone().add(blade.current.b).sub(blade.previous.a).sub(blade.previous.b);
        normal.crossVectors(edge,motion);if(normal.lengthSq()<1e-8)normal.set(0,1,0);else normal.normalize();
      }
      plane=new THREE.Plane().setFromNormalAndCoplanarPoint(normal,point);
    }
    unit.deathDuration=splitDeaths.start(unit,plane,mode,frameState.simulation);frameState.lastHit=frameState.simulation;
    unit.entry.mixer.stopAllAction();announce();
  }
  const ready=characters.ready.then(async()=>{
    const originals=characters.state.entries.filter(e=>e.loaded&&e.slug!=='madmechanic');
    const slugs=new Set(originals.map(e=>e.slug));
    for(let i=city.colliders.length-1;i>=0;i--)if(slugs.has(city.colliders[i].actor))city.colliders.splice(i,1);
    const pool=[];
    for(const original of originals){
      pool.push(original);const count=POOL_COUNTS[original.slug];
      for(let i=1;i<count;i++)pool.push(characters.createInstance(original.slug));
    }
    units=pool.map((entry,i)=>{
      entry.holder.visible=false;
      for(const child of entry.holder.children)if(child.name.startsWith('Jmenovka')||child.geometry?.type==='CircleGeometry')child.visible=false;
      const charge=new THREE.Mesh(new THREE.TorusGeometry(entry.height*.22,.017,5,24),new THREE.MeshBasicMaterial({color:entry.slug==='core-tank'?'#ffb53c':'#ff713e',transparent:true,opacity:.85}));
      charge.position.set(0,entry.height*.55,entry.height*.46);charge.visible=false;entry.holder.add(charge);
      const healthBar=new THREE.Group();healthBar.name='HP '+entry.slug;healthBar.visible=false;
      const back=new THREE.Mesh(healthBackGeometry,healthBackMaterial);
      const fill=new THREE.Mesh(healthFillGeometry,new THREE.MeshBasicMaterial({color:0x5ee87d}));
      fill.position.z=.008;healthBar.add(back,fill);scene.add(healthBar);
      return {health:healthFor(entry.slug),healthBar,healthFill:fill,hitFlash:0,plasma:createPlasmaAttack({scene,castWorld,heavy:entry.slug==='core-tank'}),salvos:0,id:i+1,entry,flying:entry.slug!=='blade-wraith',phase:'dead',charge,death:0,deathDuration:3,waypoint:null,pathAge:0,lastGoal:null,slotAngle:0,shotTimer:1.8,flash:0};
    });
    units.forEach(u=>splitDeaths.prepare(u,deathMode(u)));
    nav=navigator(city);await splitDeaths.ready();state.phase=units.length?'ready':'unavailable';if(state.started)spawn();return describe();
  });
  function start(){if(state.started)return;state.started=true;if(units.length)spawn();announce();}
  function setVR(value){if(vr!==value){vr=value;resetTracking();swing=[-1,-1];}desktop.visible=state.started&&!vr;}
  function slash(hand=1){if(vr||!state.started||swing[hand]>=0)return false;swing[hand]=0;swingIds[hand]++;strikeSerial++;return true;}
  function blades(realDt){
    scene.updateMatrixWorld(true);const samples=[];
    for(let i=0;i<2;i++){
      if(vr&&(!controllers[i].grip.visible||!controllers[i].c.userData.hand)){previous[i]=null;continue;}
      const sword=(vr?vrSwords:desktopSwords)[i],grip=vr?controllers[i].grip:sword;
      const sample={a:sword.localToWorld(BASE.clone()),b:sword.localToWorld(TIP.clone()),grip:grip.getWorldPosition(new THREE.Vector3()),rotation:grip.getWorldQuaternion(new THREE.Quaternion())};
      const old=previous[i];previous[i]=sample;
      if(continuous(old,sample,realDt))samples.push({hand:i,a:sample.a,b:sample.b,previousA:old.a,previousB:old.b,current:sample,previous:old,canCut:vr||swing[i]>=0});
    }
    return samples;
  }
  function moveUnit(unit,head,worldDt){
    const p=unit.entry.holder.position,distance=horizontal(p,head),near=distance<1.25;
    unit.pathAge+=worldDt;
    let goal=head,speed=1.35;
    if(unit.flying){
      const d=waveForward.clone().applyAxisAngle(new THREE.Vector3(0,1,0),unit.slotAngle);
      const desired={x:head.x+d.x*5.3,z:head.z+d.z*5.3};goal=nav.free(desired)?desired:approach(head);speed=2.2;
      const desiredY=Math.max(.6,head.y-.2-unit.entry.height*.5)+Math.sin(age*1.7+unit.id)*.13;
      const oldCenter=center(unit),vertical=THREE.MathUtils.clamp(desiredY-p.y,-worldDt,worldDt);
      const nextCenter=oldCenter.clone();nextCenter.y+=vertical;
      if(!castWorld(oldCenter,nextCenter))p.y+=vertical;
    }else{
      const next=near?'attack':'chase';if(next!==unit.phase){unit.phase=next;play(unit,near?'Attack':'Run');}
      if(near)return;
      goal=approach(head);
    }
    const goalChanged=!unit.lastGoal||horizontal(goal,unit.lastGoal)>.5;
    if(!unit.waypoint||horizontal(p,unit.waypoint)<.08||!nav.clear(p,unit.waypoint)||(unit.pathAge>.3&&goalChanged)){
      unit.waypoint=nav.waypoint(p,goal);unit.lastGoal={x:goal.x,z:goal.z};unit.pathAge=0;
    }
    if(unit.waypoint){
      const d=horizontal(p,unit.waypoint),step=Math.min(worldDt*speed,d,unit.flying?Infinity:Math.max(0,distance-1.2));
      if(d>1e-5){
        const from=center(unit),to=from.clone();to.x+=(unit.waypoint.x-p.x)/d*step;to.z+=(unit.waypoint.z-p.z)/d*step;
        if(!castWorld(from,to)){p.x=to.x;p.z=to.z;}
      }
    }
  }
  function syncVisualState(){
    for(const unit of units){
      const visible=state.started&&state.phase!=='dead'&&!['dying','dead'].includes(unit.phase);
      unit.healthBar.visible=visible;if(!visible)continue;
      const p=unit.entry.holder.position,h=getHead(),ratio=unit.health.hp/unit.health.maxHp;
      unit.healthBar.position.set(p.x,p.y+unit.entry.height+.22,p.z);
      unit.healthBar.lookAt(h.x,unit.healthBar.position.y,h.z);
      unit.healthFill.scale.x=ratio;unit.healthFill.position.x=-.34*(1-ratio);
      unit.healthFill.material.color.setHex(unit.hitFlash>0?0xffffff:ratio>.5?0x5ee87d:ratio>.25?0xffc24a:0xff5151);
    }

    const dead=state.phase==='dead';
    wound.visible=state.health<100;wound.material.opacity=dead?.65:(1-state.health/100)*.4;
    desktop.visible=state.started&&!vr&&!dead;for(const sword of vrSwords)sword.visible=!dead;
    frameState.visual=frameState.simulation;
  }
  function update(worldDt,realDt,{presenting=false}={}){
    frameState.simulation++;
    try{
    setVR(presenting);desktop.visible=state.started&&!vr;
    if(!Number.isFinite(worldDt)||!Number.isFinite(realDt)||worldDt<=0||realDt<=0){resetTracking();return;}
    for(let i=0;i<2;i++){
      if(swing[i]>=0){swing[i]+=realDt;if(swing[i]>.38)swing[i]=-1;}
      const t=swing[i]<0?0:swing[i]/.38,w=swing[i]<0?0:Math.sin(Math.PI*t),side=i===0?-1:1;
      desktopSwords[i].position.set(side*.31-side*.32*w,-.29+.13*w,-.36-.18*w);
      desktopSwords[i].rotation.set(-.08-.28*w,side*(.07+.95*w),side*.12*w);
    }
    if(!state.started||!units.length)return;
    if(state.phase==='dead'){deathTime+=realDt;if(deathTime>=3){state.health=100;spawn();}return;}
    age+=worldDt;const head=getHead().clone();if(lastHead&&head.distanceTo(lastHead)>1)resetTracking();lastHead=head.clone();
    if(state.phase==='waiting'){waitingTime+=worldDt;if(waitingTime>.5)spawn();return;}
    for(const unit of current){
      tickHealth(unit.health,realDt);unit.hitFlash=Math.max(0,unit.hitFlash-realDt);
      if(unit.phase==='dead')continue;
      if(unit.phase==='dying'){
        unit.death+=worldDt;const complete=splitDeaths.update(unit,unit.death,worldDt,frameState.simulation);if(complete){unit.entry.holder.visible=false;unit.phase='dead';}continue;
      }
      if(!unit.plasma.active)moveUnit(unit,head,worldDt);const p=unit.entry.holder.position;if(!unit.plasma.active)unit.entry.holder.rotation.y=Math.atan2(head.x-p.x,head.z-p.z);
      if(unit.flying){
        if(unit.plasma.active){const amount=unit.plasma.update(worldDt,head);if(amount)damage(amount);if(state.phase==='dead')return;if(!unit.plasma.active)play(unit,'Idle');unit.charge.visible=false;continue;}
        unit.shotTimer-=worldDt;unit.flash=Math.max(0,unit.flash-worldDt);unit.charging=unit.shotTimer<.4;
        unit.charge.visible=unit.charging||unit.flash>0;unit.charge.scale.setScalar(unit.charging?.75+(1-unit.shotTimer/.4)*.6:1.2);
        if(unit.shotTimer<=0){
          const from=center(unit),target=head.clone();target.y-=.25;
          const d=target.clone().sub(from).normalize();from.addScaledVector(d,unit.entry.height*.48);
          if(horizontal(p,head)<12&&!castWorld(from,target)){
            const fired=unit.salvos%2===0?unit.plasma.begin(from,target):projectiles.shoot(from,target,unit.id);
            if(fired){unit.salvos++;play(unit,'Attack4',false);unit.flash=.12;unit.shotTimer=(unit.entry.slug==='core-tank'?2.8:3.2)+((unit.id%3)*.25);}
          }else unit.shotTimer=.45;
        }
      }
    }
    const validBlades=blades(realDt);
    for(const unit of current)if(!['dying','dead'].includes(unit.phase))for(let hand=0;hand<2;hand++){
      const blade=validBlades.find(b=>b.hand===hand);
      const contact=!!blade&&hitSweep(blade.previous,blade.current,unit,realDt);
      const travel=blade?Math.max(blade.previous.a.distanceTo(blade.current.a),blade.previous.b.distanceTo(blade.current.b)):0;
      const eligible=!!blade&&blade.canCut&&travel>=.018&&travel/realDt>=.3;
      if(bladeDamageAllowed(unit.health,hand,contact,eligible,vr?null:swingIds[hand]))damageEnemy(unit,SWORD_DAMAGE,blade);
    }
    const targets=current.map(u=>({id:u.id,position:center(u),radius:u.flying?u.entry.height*.48:.65,active:!['dying','dead'].includes(u.phase)}));
    const beforeReflections=projectiles.describe().reflected;
    projectiles.update(worldDt,{blades:validBlades,targets,player:{head,radius:.3}});
    if(state.phase==='dead')return;
    if(projectiles.describe().reflected!==beforeReflections)announce();
    if(current.length&&current.every(u=>u.phase==='dead')){state.active++;spawn();}
    }finally{syncVisualState();}
  }
  function describe(){
    const entries=current.map(u=>({id:u.id,deathMode:deathMode(u),name:u.entry.name,slug:u.entry.slug,height:u.entry.height,flying:u.flying,phase:u.phase,position:u.entry.holder.position.toArray(),center:center(u).toArray(),hp:u.health.hp,maxHp:u.health.maxHp,shotTimer:u.shotTimer,charging:u.charging,plasma:u.plasma.describe()}));
    const e=entries.find(e=>e.hp>0)||entries[0];
    return {...state,frame:{...frameState},deathMode:defaultDeathMode,deathModeOverrides:Object.fromEntries(deathModeOverrides),wave:state.active+1,enemy:e||null,enemies:entries,visibleEnemies:current.filter(u=>u.entry.holder.visible).map(u=>u.entry.slug),swords:{mode:vr?'tracked-grips':'desktop',equipped:2,strikes:strikeSerial},projectiles:projectiles.describe(),splitDeaths:splitDeaths.describe()};
  }
  return {getPlayerState:()=>({health:state.health,maxHealth:100,dead:state.phase==='dead',deaths:state.deaths}),ready,start,update,slash,setDeathMode,setVR,resetTracking,describe,desktopSwords,vrSwords,projectiles};
}
