import {initEngine} from '../shared/runtime-info.js';
import {createScene} from './scene.js';
import {appleGeometry} from '../cutting/scene.js';

const canvas=document.querySelector('#gpu-canvas');
if(!(canvas instanceof HTMLCanvasElement))throw new Error('Chybí canvas');
const backend=new URLSearchParams(location.search).get('backend')==='webgl'?'webgl':'auto';
export const engine=await initEngine(canvas,{backend});
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
engine.setOrbitView([0,4.2,20],[0,4.2,0]);
engine.set_render_effects(false,false);
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
createScene(engine.scene);

// Center the cutting mesh so rotation does not swing around its base.
const sourceGeometry=appleGeometry(20,12);
const geometry={...sourceGeometry,positions:Float32Array.from(sourceGeometry.positions)};
for(let i=1;i<geometry.positions.length;i+=3)geometry.positions[i]-=1.16;
const POOL_SIZE=48;
export const pool=Array.from({length:POOL_SIZE},()=>({
  apple:engine.scene.createMesh(geometry).setPosition([100,0,0]).setScale([.38,.38,.38]),
  timer:0,
}));
const status=document.querySelector('#status');
let shot=0,debugOverlay=false,burstTimer=0,remaining=0,side=-1;
/** @param {(typeof pool)[number]} slot */
function park(slot){
  window.clearTimeout(slot.timer);
  slot.timer=0;
  slot.apple.projectile([0,0,0],{gravity:0,bounce:0}).spin([0,0,0]);
  slot.apple.setPosition([100,0,0]);
}
function shoot(launchSide=Math.random()<.5?-1:1){
  const slot=pool.find(entry=>entry.timer===0);
  if(!slot)return;
  const lift=10.2+Math.random()*2.1;
  const gravity=12;
  slot.apple.setPosition([launchSide*(2.8+Math.random()*.45),.55,(Math.random()-.5)*.14]);
  slot.apple.setRotation([0,Math.random()*Math.PI*2,0]);
  slot.apple.projectile([-launchSide*(1.8+Math.random()*.4),lift,0],{gravity,bounce:0});
  slot.apple.spin([(Math.random()-.5)*2.4,(Math.random()-.5)*2.4,(Math.random()-.5)*2.4]);
  // Return after the first arc, before fruit can collect on the floor.
  slot.timer=window.setTimeout(()=>park(slot),2*lift/gravity*1000);
  shot++;
  if(status)status.textContent=`${shot} jablek · dvě odpalovací místa · F3 informace`;
}
function nextShot(){
  if(remaining===0){
    remaining=1+Math.floor(Math.random()*10);
    side=Math.random()<.5?-1:1;
  }
  shoot(side);
  side=-side;
  remaining--;
  burstTimer=window.setTimeout(nextShot,remaining?155+Math.random()*150:450+Math.random()*800);
}
/** @param {KeyboardEvent} event */
const keydown=(event)=>{
  if(event.code!=='F3')return;
  event.preventDefault();
  if(event.repeat)return;
  debugOverlay=!debugOverlay;
  engine.setDebugOverlay(debugOverlay);
  document.documentElement.dataset.debugOverlay=String(debugOverlay);
};
window.addEventListener('keydown',keydown);
document.querySelector('#shoot')?.addEventListener('click',()=>shoot());
document.querySelector('#collision-test')?.addEventListener('click',()=>{
  window.clearTimeout(burstTimer);
  remaining=0;
  for(const slot of pool)park(slot);
  for(let i=0;i<2;i++){
    const direction=i===0?1:-1;
    const slot=pool[i];
    slot.apple.setPosition([-direction*1.3,3,0]);
    slot.apple.projectile([direction*3,3,0],{gravity:12,bounce:0});
    slot.timer=window.setTimeout(()=>park(slot),2200);
  }
  burstTimer=window.setTimeout(nextShot,2500);
});
const contactsTimer=window.setInterval(()=>{
  const label=document.querySelector('#contacts');
  if(label)label.textContent=String(engine.projectileContacts());
},200);
document.querySelector('#reset')?.addEventListener('click',()=>{
  window.clearTimeout(burstTimer);
  for(const slot of pool)park(slot);
  shot=0;remaining=0;
  engine.setOrbitView([0,4.2,20],[0,4.2,0]);
  nextShot();
});
const fit=()=>engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
fit();window.addEventListener('resize',fit);
nextShot();
window.addEventListener('pagehide',()=>{
  window.clearTimeout(burstTimer);
  window.clearInterval(contactsTimer);
  for(const slot of pool)window.clearTimeout(slot.timer);
  window.removeEventListener('keydown',keydown);
  window.removeEventListener('resize',fit);
  engine.dispose();
},{once:true});
