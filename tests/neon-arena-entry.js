// Reuse the actual production menu/CSS and main module; only the XR platform is simulated.
import {installXRSimulator} from './xr/simulator.js';
import {assertForeground,assertChanged} from './pixels.js';
import {VR_DEBUG_PANEL_OFFSET} from '../examples/shared/vr-debug-panel.js';
const sim=installXRSimulator({profile:{id:'arena-entry-72',hz:72,eyeWidth:320,eyeHeight:320},clockTimers:false});
const response=await fetch('../examples/neon-arena/');if(!response.ok)throw Error('Arena HTML unavailable');
const source=new DOMParser().parseFromString(await response.text(),'text/html');
for(const script of source.querySelectorAll('script'))script.remove();
for(const node of source.head.querySelectorAll('style,meta[name="build-profile"]'))document.head.append(node);
document.body.append(...source.body.childNodes);
const {engine,world}=await import('../examples/neon-arena/main.js');
const checks=[];
const assert=(condition,name)=>{checks.push({name,status:condition?'PASS':'FAIL'});if(!condition)throw Error(name);};
const near=(a,b)=>a.length===b.length&&a.every((v,i)=>Math.abs(v-b[i])<.003);
const menu=document.querySelector('#arena-menu'),enter=document.querySelector('#enter'),toggle=document.querySelector('#vr-info');
const yaw=-Math.atan2(.9,1),pitch=-Math.atan2(1.25,Math.hypot(.9,1));
const sp=Math.sin(pitch/2),cp=Math.cos(pitch/2),sy=Math.sin(yaw/2),cy=Math.cos(yaw/2);
const pose={position:[0,1.6,0],orientation:[cy*sp,sy*cp,-sy*sp,cy*cp]};
const frame=(head={})=>{sim.frame({head:{tracked:true,...pose,...head},controllers:{left:{position:[-.5,1.2,-.2]},right:{position:[.5,1.2,-.2]}}});if(engine.xr.lastError)throw Error(String(engine.xr.lastError));};
const api={
 ready:true,checks,
 get presenting(){return engine.xr.isPresenting;},
 before(){
  assert(!enter.disabled,'VR entry enabled by supported XR platform');
  assert(enter.querySelector('svg[aria-hidden="true"]')&&enter.textContent.includes('Vstoupit do VR'),'visible headset icon accompanies accessible VR entry label');
  const rect=enter.getBoundingClientRect();
  assert(rect.top>=0&&rect.bottom<=innerHeight&&rect.height>=44,'entry button visible above long scene description');
  const hit=document.elementFromPoint(rect.x+rect.width/2,rect.y+rect.height/2);
  assert(hit===enter||enter.contains(hit),'actual pointer hit reaches VR entry');
  assert(!document.querySelector('[data-runtime-info]'),'no desktop statistics panel before VR');
  assert(engine.xr.player.head.object===null&&world.infoPanel.meshInfo.position[0]>80,'native information hidden before VR');
 },
 inside(){
  frame();frame();
  assert(engine.xr.isPresenting,'real entry button starts simulated XR');
  assert(getComputedStyle(menu).display==='none','web menu hidden during VR');
  assert(!engine.debugStats().enabled,'screen-space debug overlay stays off');
  assert(engine.xr.player.head.object?.id===world.infoPanel.id,'native diagnostics bound inside VR');
  assert(world.infoPanel.meshInfo.scale.every(v=>v===1),'spatial panel keeps positive unmirrored scale independent of desktop labels');
  assert(near(world.infoPanel.meshInfo.position,VR_DEBUG_PANEL_OFFSET),'diagnostics use shared right-foot spatial offset');
  const left=sim.pixels('left'),right=sim.pixels('right');
  assertForeground(left,'VR information left eye',1000);assertForeground(right,'VR information right eye',1000);
  const stereo=sim.pixels(),canvas=document.createElement('canvas');canvas.width=stereo.width;canvas.height=stereo.height;canvas.getContext('2d').putImageData(stereo,0,0);
  api.stereoPng=canvas.toDataURL('image/png');
  toggle.checked=false;toggle.dispatchEvent(new Event('change'));frame();
  assert(engine.xr.player.head.object===null,'VR information toggle releases the native binding');
  assertChanged(left,sim.pixels('left'),'native diagnostics contribute left-eye pixels',30);
  assertChanged(right,sim.pixels('right'),'native diagnostics contribute right-eye pixels',30);
  toggle.checked=true;toggle.dispatchEvent(new Event('change'));frame();
  const anchored=world.infoPanel.meshInfo.position;
  frame({orientation:[0,0,0,1]});assert(near(world.infoPanel.meshInfo.position,anchored),'gaze rotation leaves panel at player floor anchor');
  frame({tracked:false});assert(world.infoPanel.meshInfo.position[1]<-9000,'tracking loss hides diagnostics');
  frame();assert(near(world.infoPanel.meshInfo.position,VR_DEBUG_PANEL_OFFSET),'tracking recovery restores panel');
 },
 async leave(){
  await engine.xr.exit();
  assert(!document.body.classList.contains('xr')&&getComputedStyle(menu).display!=='none','menu returns after VR exit');
  assert(engine.xr.player.head.object===null&&world.infoPanel.meshInfo.position[0]>80,'diagnostics hidden after VR exit');
 },
 finish(){
  frame();frame();
  assert(engine.xr.player.head.object?.id===world.infoPanel.id,'second real click reuses native information panel');
  document.querySelector('#entry-result').textContent='PASS: actual VR entry, spatial information in both eyes, toggle, tracking and reentry';
  return document.querySelector('#entry-result').textContent;
 }
};
window.arenaEntryTest=api;
