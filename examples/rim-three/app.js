import * as THREE from 'three';
import {installThreeRuntimeInfo} from './runtime-info-three.js';
import {PlayerStamina} from './player-stamina.js?v=stamina-1';
import {DashPhysics} from './dash-physics.js?v=stamina-1';
import {createPlayerHUD} from './player-hud.js?v=stamina-1';
import {createVRDebugPanel} from './vr-debug-panel.js';
import {createCity} from './city.js';
import {CollisionWorld,boxShape} from './collision-world.js';
import {JumpPhysics,SLOW_TIME} from './jump-physics.js';
import {createCharacters} from './characters.js?v=cut-1';
import {createCombat} from './combat.js?v=stamina-1';
import {GLTFExporter} from './vendor/GLTFExporter.js';
const $=id=>document.getElementById(id);
const renderer=new THREE.WebGLRenderer({canvas:$('world'),antialias:true,powerPreference:'high-performance'});
renderer.setPixelRatio(Math.min(devicePixelRatio,1.5));renderer.setSize(innerWidth,innerHeight);
renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.25;
renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.PCFSoftShadowMap;renderer.shadowMap.autoUpdate=false;renderer.shadowMap.needsUpdate=true;
renderer.xr.enabled=true;renderer.xr.setReferenceSpaceType('local-floor');renderer.xr.setFramebufferScaleFactor(1);
const scene=new THREE.Scene();scene.background=new THREE.Color('#b8d0ca');scene.fog=new THREE.Fog('#b8d0ca',90,245);
scene.add(new THREE.HemisphereLight('#e5f4ff','#b79868',2.5));const sun=new THREE.DirectionalLight('#ffe0ab',3.1);sun.position.set(-45,75,35);sun.castShadow=true;sun.shadow.mapSize.set(2048,2048);Object.assign(sun.shadow.camera,{left:-95,right:95,top:95,bottom:-95,near:1,far:220});sun.shadow.normalBias=.12;sun.shadow.bias=-.00015;sun.target.position.set(0,0,-15);scene.add(sun.target);scene.add(sun);
const city=createCity(THREE);scene.add(city.group);city.group.traverse(o=>{if(o.isMesh){o.castShadow=true;o.receiveShadow=true;}});
const characters=createCharacters(city,renderer);city.landmarks.push({name:'Naše postavy',x:0,z:8});
const camera=new THREE.PerspectiveCamera(65,innerWidth/innerHeight,.08,350);camera.position.set(0,1.7,0);camera.rotation.order='YXZ';
const rig=new THREE.Group();rig.position.set(0,0,42);rig.add(camera);scene.add(rig);
const debugPanel=createVRDebugPanel(scene,renderer);
const playerHUD=createPlayerHUD(scene),stamina=new PlayerStamina(),dash=new DashPhysics();
let dashButtonWasDown=true,wasDead=false;
const hudQuaternion=new THREE.Quaternion();
let manualTime=false,walking=false,session=null,last=0,snapReady=true,drag=null,errors=[],teleports=0;
const jump=new JumpPhysics();
let collisionWorld=new CollisionWorld(city.collisionShapes);
characters.ready.then(()=>{
 for(const entry of characters.state.entries.filter(e=>e.loaded&&e.slug==='madmechanic'))
  city.collisionShapes.push(boxShape(entry.x,(entry.height+.13)/2,entry.z,1.6,entry.height+.13,1.6));
 collisionWorld=new CollisionWorld(city.collisionShapes);
});
const ARC_SPEED=22;
let previousPhysicalHead=null;
const xrPosition=new THREE.Vector3(0,1.7,0),xrOrientation=new THREE.Quaternion();
const keys=new Set(), v=new THREE.Vector3(),head=new THREE.Vector3(),direction=new THREE.Vector3();
const boundaries={minX:-78,maxX:78,minZ:-82,maxZ:60};
function bodyHeight(){return renderer.xr.isPresenting?THREE.MathUtils.clamp(xrPosition.y,.6,2.5):1.7}
function inBounds(x,z){return x>boundaries.minX&&x<boundaries.maxX&&z>boundaries.minZ&&z<boundaries.maxZ}
function canStand(x,z,y=rig.position.y){return inBounds(x,z)&&collisionWorld.clear({x,y,z},bodyHeight())}
function getHead(){rig.updateMatrixWorld(true);if(renderer.xr.isPresenting){head.copy(xrPosition).applyMatrix4(rig.matrixWorld)}else camera.getWorldPosition(head);return head}
function move(dx,dz){
 const p=getHead().clone(),feet={x:p.x,y:rig.position.y,z:p.z};
 const hit=collisionWorld.move(feet,{x:dx,y:0,z:dz},bodyHeight());
 if(inBounds(hit.position.x,hit.position.z)){
  rig.position.x+=hit.position.x-p.x;rig.position.z+=hit.position.z-p.z;
  rig.position.y=hit.position.y;jump.height=rig.position.y;
 }
 rig.updateMatrixWorld(true);
}
function teleport(x,z,y=0){if(!canStand(x,z,y))return false;dash.reset();jump.reset();jump.height=y;rig.position.y=y;getHead();rig.position.x+=x-head.x;rig.position.z+=z-head.z;rig.updateMatrixWorld(true);previousPhysicalHead=null;teleports++;combat.resetTracking();return true}
function turn(angle){const before=getHead().clone();rig.rotation.y+=angle;rig.updateMatrixWorld(true);const after=getHead().clone();rig.position.x+=before.x-after.x;rig.position.z+=before.z-after.z;rig.updateMatrixWorld(true);combat.resetTracking()}
const ring=new THREE.Mesh(new THREE.RingGeometry(.30,.44,40),new THREE.MeshBasicMaterial({color:'#b8ffbe',side:THREE.DoubleSide,depthTest:false}));ring.rotation.x=-Math.PI/2;ring.renderOrder=100;ring.visible=false;scene.add(ring);
const controllers=[];
function controllerHelp(){const canvas=document.createElement('canvas');canvas.width=768;canvas.height=384;const ctx=canvas.getContext('2d');ctx.fillStyle='#17392f';ctx.fillRect(0,0,768,384);ctx.strokeStyle='#eac98a';ctx.lineWidth=6;ctx.strokeRect(5,5,758,374);ctx.fillStyle='#f8e7c5';ctx.font='bold 40px Arial';ctx.fillText('AURELIA · SOUBOJ',28,63);ctx.font='29px Arial';['A: dash · čtyři dílky staminy','Spoušť: zpomal · pusť: skoč','Skok i dash stojí jeden dílek','Levá: chůze · pravá: otočení'].forEach((line,i)=>ctx.fillText(line,28,130+i*62));const texture=new THREE.CanvasTexture(canvas);texture.colorSpace=THREE.SRGBColorSpace;const mesh=new THREE.Mesh(new THREE.PlaneGeometry(.44,.22),new THREE.MeshBasicMaterial({map:texture,side:THREE.DoubleSide}));mesh.position.set(0,.16,-.2);mesh.rotation.x=-.35;return mesh}
for(let i=0;i<2;i++){
 const c=renderer.xr.getController(i);rig.add(c);c.userData.aiming=false;
 const ray=new THREE.Line(new THREE.BufferGeometry().setAttribute('position',new THREE.BufferAttribute(new Float32Array(33*3),3)),new THREE.LineBasicMaterial({color:'#d6f9cc'}));scene.add(ray);ray.visible=false;
 const grip=renderer.xr.getControllerGrip(i);rig.add(grip);
 const help=controllerHelp();c.add(help);help.visible=false;
 c.addEventListener('connected',e=>{c.userData.hand=e.data.handedness;help.visible=e.data.handedness==='left'});
 c.addEventListener('squeezestart',()=>{if(c.userData.hand==='left')help.visible=!help.visible});
 c.addEventListener('selectstart',()=>beginAim(c));c.addEventListener('selectend',()=>confirmAim(c));c.addEventListener('disconnected',()=>{c.userData.aiming=false;ray.visible=false;help.visible=false;dash.reset();dashButtonWasDown=true;combat.resetTracking()});controllers.push({c,ray,grip,help});
}
const desktopAim=new THREE.Object3D();desktopAim.position.set(0,-.25,-.2);camera.add(desktopAim);desktopAim.userData.aiming=false;
const desktopRay=new THREE.Line(new THREE.BufferGeometry().setAttribute('position',new THREE.BufferAttribute(new Float32Array(33*3),3)),new THREE.LineBasicMaterial({color:'#d6f9cc'}));desktopRay.visible=false;scene.add(desktopRay);
const aimSources=[...controllers,{c:desktopAim,ray:desktopRay}];
const combat=createCombat({scene,camera,renderer,controllers,characters,city,castWorld:(a,b)=>collisionWorld.castSegment(a,b),getHead,getForward:()=>session?new THREE.Vector3(0,0,-1).applyQuaternion(xrOrientation).applyQuaternion(rig.quaternion):camera.getWorldDirection(new THREE.Vector3())});
function timeScale(){return aimSources.some(({c})=>c.userData.aiming)?SLOW_TIME:1}
function clearAiming(){for(const {c,ray} of aimSources){c.userData.aiming=false;ray.visible=false}ring.visible=false}
function beginAim(c){if(combat.getPlayerState().dead||!stamina.canSpend())return false;clearAiming();c.userData.aiming=true;return true}
function canJumpTo(target){getHead();return canStand(target.x,target.z,target.y)&&Math.hypot(target.x-head.x,target.z-head.z)<=35}
function launchJump(target){if(combat.getPlayerState().dead||!canJumpTo(target)||!stamina.spend())return false;dash.reset();const p=getHead().clone();jump.launch({x:p.x,z:p.z,height:rig.position.y},target);return true}
function dashDirection(){
 const forward=session?new THREE.Vector3(0,0,-1).applyQuaternion(xrOrientation).applyQuaternion(rig.quaternion):camera.getWorldDirection(new THREE.Vector3());forward.y=0;
 if(forward.lengthSq()<1e-6)forward.set(0,0,-1).applyQuaternion(rig.quaternion);forward.normalize();
 let side=0,ahead=0;
 if(session){const source=[...session.inputSources].find(s=>s.handedness==='left'&&s.gamepad);if(source){const a=source.gamepad.axes,offset=a.length>=4?2:0;side=a[offset]||0;ahead=-(a[offset+1]||0);}}
 else{side=Number(keys.has('KeyD')||keys.has('ArrowRight'))-Number(keys.has('KeyA')||keys.has('ArrowLeft'));ahead=Number(keys.has('KeyW')||keys.has('ArrowUp'))-Number(keys.has('KeyS')||keys.has('ArrowDown'));}
 return Math.hypot(side,ahead)>.2?new THREE.Vector3(forward.x*ahead-forward.z*side,0,forward.z*ahead+forward.x*side).normalize():forward;
}
function startDash(){
 if(combat.getPlayerState().dead||dash.active||!stamina.canSpend())return false;
 if(!dash.start(dashDirection()))return false;
 stamina.spend();clearAiming();jump.vx=jump.vz=0;jump.target=null;combat.resetTracking();return true;
}
function pollDashButton(){
 if(!session)return;
 const pressed=[...session.inputSources].some(s=>s.handedness==='right'&&s.gamepad&&(s.gamepad.buttons[4]?.pressed||s.gamepad.buttons[4]?.value>.5));
 if(pressed&&!dashButtonWasDown)startDash();dashButtonWasDown=pressed;
}
function cancelMovementInput(){clearAiming();dash.reset();dashButtonWasDown=true}
function confirmAim(c){if(!c.userData.aiming)return false;const target=aim(c);clearAiming();return target.valid&&launchJump(target.point)}
function aim(c){
 const origin=c.getWorldPosition(new THREE.Vector3()),velocity=new THREE.Vector3(0,0,-1).applyQuaternion(c.getWorldQuaternion(new THREE.Quaternion())).multiplyScalar(ARC_SPEED);
 let previous=origin.clone(),point=origin.clone(),normal=new THREE.Vector3(0,1,0),surfacePoint=point.clone(),time=0,valid=false;
 if(origin.y<=rig.position.y+.15)return {origin,velocity,time,point,normal,surfacePoint,valid};
 const p=getHead().clone();if(collisionWorld.castSegment(p,origin))return {origin,velocity,time,point,normal,surfacePoint,valid};
 for(let i=1;i<=720;i++){
  const t=i/120,next=origin.clone().addScaledVector(velocity,t);next.y-=4.9*t*t;
  const hit=collisionWorld.castSegment(previous,next);
  if(hit){
   time=(i-1+hit.fraction)/120;surfacePoint.set(hit.point.x,hit.point.y,hit.point.z);normal.set(hit.normal.x,hit.normal.y,hit.normal.z);point.copy(surfacePoint);
   if(normal.y>.55&&velocity.y-9.8*time<0){
    point.y+=.32*(Math.abs(normal.x)+Math.abs(normal.z))/normal.y+1e-5;
    valid=stamina.canSpend()&&!combat.getPlayerState().dead&&canJumpTo(point);
   }
   return {origin,velocity,time,point,normal,surfacePoint,valid};
  }
  previous=next;point=next;time=t;
  if(next.y<-.5)break;
 }
 return {origin,velocity,time,point,normal,surfacePoint,valid};
}
function update(dt){const realDt=dt;dt=Math.min(dt,.05);pollDashButton();const scale=timeScale(),worldDt=dt*scale;characters.update(worldDt);document.body.classList.toggle('slow-time',scale<1);$('timeState').hidden=scale===1;let forward=0,side=0;if(walking&&!session){forward=(keys.has('KeyW')||keys.has('ArrowUp')?1:0)-(keys.has('KeyS')||keys.has('ArrowDown')?1:0);side=(keys.has('KeyD')||keys.has('ArrowRight')?1:0)-(keys.has('KeyA')||keys.has('ArrowLeft')?1:0)}
if(session){for(const source of session.inputSources){const gp=source.gamepad;if(!gp)continue;const ax=gp.axes.length>=4?gp.axes.slice(2,4):gp.axes.slice(0,2);if(source.handedness==='left'){side=Math.abs(ax[0])>.15?ax[0]:0;forward=Math.abs(ax[1])>.15?-ax[1]:0}if(source.handedness==='right'){if(Math.abs(ax[0])<.3)snapReady=true;if(Math.abs(ax[0])>.65&&snapReady){turn(-Math.sign(ax[0])*Math.PI/6);snapReady=false}}}}
if(!jump.active&&!dash.active&&(forward||side)){if(renderer.xr.isPresenting){direction.set(0,0,-1).applyQuaternion(xrOrientation).applyQuaternion(rig.quaternion)}else camera.getWorldDirection(direction);direction.y=0;direction.normalize();const speed=(session?1.8:keys.has('ShiftLeft')?6:2.5)*worldDt/Math.max(1,Math.hypot(side,forward));move((direction.x*forward-direction.z*side)*speed,(direction.z*forward+direction.x*side)*speed)}
// Room-scale head motion uses the same body sweep; tracking stays realtime.
if(renderer.xr.isPresenting){
 if(previousPhysicalHead){
  const localDelta=xrPosition.clone().sub(previousPhysicalHead);localDelta.y=0;localDelta.applyQuaternion(rig.quaternion);
  const p=getHead().clone(),feet={x:p.x-localDelta.x,y:rig.position.y,z:p.z-localDelta.z};
  const hit=collisionWorld.move(feet,localDelta,bodyHeight());
  if(inBounds(hit.position.x,hit.position.z)){rig.position.x+=hit.position.x-p.x;rig.position.z+=hit.position.z-p.z;rig.position.y=hit.position.y;jump.height=rig.position.y;}
  else {rig.position.x-=localDelta.x;rig.position.z-=localDelta.z;}
 }
 previousPhysicalHead=xrPosition.clone();
}else previousPhysicalHead=null;
if(dash.active){const p=getHead().clone(),step=dash.step(worldDt);move(step.dx,step.dz);const travelled=p.distanceTo(getHead());if(travelled<Math.hypot(step.dx,step.dz)*.1)dash.reset();}
{const p=getHead().clone(),step=jump.stepWorld(worldDt,p,collisionWorld,bodyHeight());rig.position.x+=step.dx;rig.position.z+=step.dz;rig.position.y=step.height;rig.updateMatrixWorld(true)}
combat.update(worldDt,realDt,{presenting:!!session});
const player=combat.getPlayerState();
if(player.dead){cancelMovementInput();jump.vx=jump.vz=0;}else{if(wasDead)stamina.reset();stamina.update(realDt);}wasDead=player.dead;

ring.visible=false;for(const {c,ray} of aimSources){ray.visible=c.userData.aiming;if(ray.visible){const target=aim(c);const pos=ray.geometry.attributes.position;for(let i=0;i<33;i++){const t=target.time*i/32;pos.setXYZ(i,target.origin.x+target.velocity.x*t,target.origin.y+target.velocity.y*t-4.9*t*t,target.origin.z+target.velocity.z*t);}pos.needsUpdate=true;ray.geometry.computeBoundingSphere();ray.material.color.set(target.valid?'#c9f6b4':'#ff997d');if(target.valid){ring.position.copy(target.surfacePoint).addScaledVector(target.normal,.015);ring.quaternion.setFromUnitVectors(new THREE.Vector3(0,0,1),target.normal);ring.visible=true;}}}
getHead();if(renderer.xr.isPresenting)hudQuaternion.copy(rig.quaternion).multiply(xrOrientation);else camera.getWorldQuaternion(hudQuaternion);playerHUD.sync({head,quaternion:hudQuaternion,hp:player.health,stamina:stamina.value,visible:walking||!!session,timeScale:scale});debugPanel.sync(head,rig,renderer.xr.isPresenting&&runtimeInfo.visible,collisionWorld);$('coords').textContent=`${Math.round(head.x)} / ${Math.round(head.z)} m`;const near=city.landmarks.reduce((best,l)=>Math.hypot(head.x-l.x,head.z-l.z)<best.distance?{name:l.name,distance:Math.hypot(head.x-l.x,head.z-l.z)}:best,{name:'Ulice města',distance:18});$('place').textContent=near.name.toUpperCase();}
const runtimeInfo=installThreeRuntimeInfo(renderer,{panel:debugPanel,readWorld:()=>({nodes:collisionWorld.stats.nodes})});
renderer.setAnimationLoop((time,frame)=>{const runtimeStart=runtimeInfo.begin(time);const dt=last?(time-last)/1000:1/60;last=time;let tracked=true;if(frame){const pose=frame.getViewerPose(renderer.xr.getReferenceSpace());tracked=!!pose;if(pose){xrPosition.copy(pose.transform.position);xrOrientation.copy(pose.transform.orientation);}}if(!manualTime){if(tracked)update(dt);else{cancelMovementInput();combat.resetTracking()}}if(!tracked){debugPanel.hide();playerHUD.hide();}if(!manualTime&&tracked&&renderer.xr.isPresenting)debugPanel.sample(dt);renderer.render(scene,camera);runtimeInfo.end(runtimeStart)});
function startWalk(){walking=true;combat.start();document.body.classList.add('walking');$('crosshair').hidden=false;$('world').focus()}
$('walk').onclick=startWalk;$('help').onclick=()=>{$('guide').hidden=!$('guide').hidden};$('closeHelp').onclick=()=>{$('guide').hidden=true};
$('world').addEventListener('pointerdown',e=>{if(!walking)startWalk();if(!session&&[0,2].includes(e.button))combat.slash(e.button===2?0:1);drag={x:e.clientX,y:e.clientY};$('world').setPointerCapture(e.pointerId)});
$('world').addEventListener('pointermove',e=>{if(!drag||session)return;camera.rotation.y-=(e.clientX-drag.x)*.003;camera.rotation.x=THREE.MathUtils.clamp(camera.rotation.x-(e.clientY-drag.y)*.003,-1.35,1.35);drag={x:e.clientX,y:e.clientY}});
$('world').addEventListener('contextmenu',e=>e.preventDefault());
$('dash').onclick=()=>{startWalk();startDash()};
$('attack').onclick=()=>{startWalk();combat.slash(1)};
for(const event of ['pointerup','pointercancel'])$('world').addEventListener(event,()=>drag=null);
addEventListener('keydown',e=>{if(['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space'].includes(e.code))e.preventDefault();keys.add(e.code);if(e.code==='KeyC'&&!e.repeat&&walking&&!session)startDash();if(!e.repeat&&walking&&!session&&['KeyQ','KeyE'].includes(e.code))combat.slash(e.code==='KeyQ'?0:1);if(e.code==='Space'&&!e.repeat&&walking&&!session)beginAim(desktopAim);if(e.code==='KeyR'){teleport(0,42);camera.rotation.set(0,0,0);rig.rotation.y=0}if(e.code==='KeyF'){if(!document.fullscreenElement)document.documentElement.requestFullscreen?.();else document.exitFullscreen?.()}});addEventListener('keyup',e=>{keys.delete(e.code);if(e.code==='Space')confirmAim(desktopAim)});addEventListener('blur',()=>{keys.clear();cancelMovementInput();combat.resetTracking();drag=null});document.addEventListener('visibilitychange',()=>{if(document.hidden){keys.clear();cancelMovementInput();combat.resetTracking()}});
document.querySelectorAll('[data-key]').forEach(b=>{b.onpointerdown=e=>{e.preventDefault();keys.add(b.dataset.key);b.setPointerCapture(e.pointerId)};b.onpointerup=b.onpointercancel=()=>keys.delete(b.dataset.key)});
city.landmarks.forEach(l=>{const b=document.createElement('button');b.textContent=l.name;b.onclick=()=>{teleport(l.x,l.z);startWalk()};$('destinations').append(b)});
async function checkVR(){if(!window.isSecureContext){$('vr').textContent='VR vyžaduje HTTPS';$('vr').disabled=true;return}try{const supported=!!navigator.xr&&await navigator.xr.isSessionSupported('immersive-vr');$('vr').disabled=!supported;$('vr').textContent=supported?'Vstoupit do VR':'Otevřete v Quest Browseru';$('status').textContent=supported?'Drž spoušť: zpomal čas. Pusť: skoč. Funguje i ve vzduchu.':'Na počítači použijte Procházet. Pro VR otevřete stejný HTTPS odkaz v headsetu.'}catch(e){$('status').textContent=e.message}}
$('vr').onclick=async()=>{if(session)return;$('vr').disabled=true;try{session=await navigator.xr.requestSession('immersive-vr',{requiredFeatures:['local-floor'],optionalFeatures:['bounded-floor']});session.addEventListener('visibilitychange',()=>{if(session&&session.visibilityState!=='visible'){cancelMovementInput();combat.resetTracking()}});session.addEventListener('end',()=>{session=null;debugPanel.hide();playerHUD.hide();cancelMovementInput();combat.setVR(false);combat.resetTracking();clearAiming();jump.reset();previousPhysicalHead=null;rig.position.y=0;ring.visible=false;controllers.forEach(({c,ray})=>{c.userData.aiming=false;ray.visible=false});camera.position.set(0,1.7,0);camera.rotation.set(0,0,0);camera.fov=65;camera.aspect=innerWidth/innerHeight;camera.updateProjectionMatrix();document.body.classList.remove('xr');checkVR()},{once:true});camera.position.set(0,0,0);camera.rotation.set(0,0,0);await renderer.xr.setSession(session);dashButtonWasDown=true;combat.setVR(true);combat.start();document.body.classList.add('xr');keys.clear()}catch(e){session?.end().catch(()=>{});session=null;camera.position.set(0,1.7,0);$('vr').disabled=false;$('status').textContent=`VR nelze spustit: ${e.message}`}};
window.exportCity=async()=>{await characters.ready;if(characters.state.failed.length)throw new Error('Některé postavy se nenačetly. Obnovte stránku.');return new GLTFExporter().parseAsync(city.group,{binary:true,onlyVisible:false})};
$('download').onclick=async()=>{const b=$('download');b.disabled=true;b.textContent='Připravuji…';try{const buffer=await window.exportCity();const url=URL.createObjectURL(new Blob([buffer],{type:'model/gltf-binary'}));const a=document.createElement('a');a.href=url;a.download='aurelia-anticke-mesto.glb';a.click();setTimeout(()=>URL.revokeObjectURL(url),30000)}catch(e){$('status').textContent=e.message}finally{b.disabled=false;b.textContent='↓ Model GLB'}};
window.render_game_to_text=()=>JSON.stringify({ready:true,mode:session?'VR':walking?'walk':'welcome',position:{x:head.x,y:head.y,z:head.z},coordinates:'metres, Y up, north -Z',teleports,timeScale:timeScale(),jump:jump.describe(),dash:dash.describe(),stamina:stamina.describe(),playerHUD:playerHUD.describe(),landmarks:city.landmarks,colliders:city.collisionShapes.length,collision:collisionWorld.stats,stats:city.stats,characters:characters.describe(),combat:combat.describe(),debugPanel:debugPanel.describe(),drawCalls:renderer.info.render.calls,triangles:renderer.info.render.triangles,secure:window.isSecureContext,errors});
window.advanceTime=ms=>{manualTime=true;for(let i=0;i<Math.ceil(ms/16.667);i++)update(1/60);renderer.render(scene,camera)};
window.cityDebug={runtimeInfo,playerHUD,stamina,dash,startDash,pollDashButton,dashDirection,debugPanel,get collisionWorld(){return collisionWorld},bodyHeight,canStand,teleport,move,turn,getHead,rig,camera,scene,city,renderer,xrPosition,xrOrientation,aim,controllers,characters,combat,setManualTime:value=>{manualTime=!!value;last=0},startWalk,jump,launchJump,beginAim,confirmAim,clearAiming,desktopAim,timeScale,update};
addEventListener('error',e=>errors.push(e.message));addEventListener('resize',()=>{camera.aspect=innerWidth/innerHeight;camera.updateProjectionMatrix();renderer.setSize(innerWidth,innerHeight)});
checkVR();

if(new URLSearchParams(location.search).has('postavy'))characters.ready.then(()=>{teleport(0,8);camera.rotation.set(0,0,0);startWalk()});
