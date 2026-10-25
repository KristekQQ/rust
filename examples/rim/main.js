import {installRimDiagnostics} from './diagnostics.js';
import {installDiagnostics,reportDiagnostic} from '../shared/diagnostics.js';
import {configureAppearance} from './appearance.js';
import {initEngine} from '../shared/runtime-info.js';
import {createScene,createGameplay} from './scene.js';
import {installCutTransport} from './cut-transport.js';
const canvas=document.getElementById('gpu-canvas');
if(!(canvas instanceof HTMLCanvasElement))throw Error('Canvas nebyl nalezen.');
const params=new URLSearchParams(location.search);
const buildId=document.querySelector('meta[name="engine-build"]')?.getAttribute('content')||'working';
const releaseId=document.querySelector('meta[name="app-build"]')?.getAttribute('content')||'working';
installDiagnostics();reportDiagnostic('rim-bootstrap',undefined,{build:releaseId,wasmBuild:buildId});
const profiling=params.get('profile')==='1'&&params.get('info')!=='0';
if(params.has('verify'))document.querySelector('details')?.setAttribute('open','');
export const engine=await initEngine(canvas,{backend:params.get('backend')==='webgl'?'webgl':'auto',profiling});
configureAppearance(engine);
const resize=()=>{if(!engine.xr.isPresenting)engine.resize(Math.max(1,Math.round(canvas.clientWidth*Math.min(devicePixelRatio,1.5))),Math.max(1,Math.round(canvas.clientHeight*Math.min(devicePixelRatio,1.5))));};resize();window.addEventListener('resize',resize);
export const content=await createScene(engine.scene);
export const game=await createGameplay(engine,content);
const stopTransport=installCutTransport(engine.scene,engine.xr);
const stopDiagnostics=installRimDiagnostics(engine,{build:releaseId,wasmBuild:buildId,getTransport:()=>Reflect.get(window,'rimCutTransportDiagnostics')});
export let activeCamera='gate';
/** @param {string} name */
export function view(name){
 const camera=content.layout.cameras.find(c=>c.name===name);if(!camera)throw Error('Neznámý pohled '+name);
 engine.scene.rimInput({pause:true,reset:true});content.environment.useCamera(camera.nodeIndex);activeCamera=name;
 for(const button of document.querySelectorAll('[data-view]'))button.setAttribute('aria-pressed',String(button.getAttribute('data-view')===name));
}
view(params.get('view')||'gate');
/** Start/resume actual native gameplay. */
export function play(){engine.scene.rimInput({start:true,pause:false});activeCamera='player';document.body.classList.add('playing');}
for(const button of document.querySelectorAll('[data-view]'))button.addEventListener('click',()=>{document.exitPointerLock();document.body.classList.remove('playing');view(button.getAttribute('data-view')||'gate');});
document.getElementById('play')?.addEventListener('click',()=>play());
/** @type {{x:number,y:number}|null} */let drag=null;
canvas.addEventListener('contextmenu',event=>event.preventDefault());
canvas.addEventListener('pointerdown',event=>{if(engine.xr.isPresenting)return;play();if(event.button===0||event.button===2)engine.scene.rimInput({slash:event.button===2?0:1});drag={x:event.clientX,y:event.clientY};canvas.setPointerCapture(event.pointerId);});
canvas.addEventListener('pointermove',event=>{if(!drag||engine.xr.isPresenting)return;engine.scene.rimInput({look:[event.clientX-drag.x,event.clientY-drag.y]});drag={x:event.clientX,y:event.clientY};});
for(const name of ['pointerup','pointercancel'])canvas.addEventListener(name,()=>{drag=null;});
const keys=new Set();
const movement=()=>engine.scene.rimInput({movement:[Number(keys.has('KeyD')||keys.has('ArrowRight'))-Number(keys.has('KeyA')||keys.has('ArrowLeft')),Number(keys.has('KeyW')||keys.has('ArrowUp'))-Number(keys.has('KeyS')||keys.has('ArrowDown'))],sprint:keys.has('ShiftLeft')||keys.has('ShiftRight')});
window.addEventListener('keydown',event=>{
 if(event.target instanceof HTMLSelectElement||event.target instanceof HTMLInputElement)return;
 if(['Space','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(event.code))event.preventDefault();keys.add(event.code);movement();
 if(event.repeat)return;
 if(event.code==='KeyQ')engine.scene.rimInput({slash:0});if(event.code==='KeyE')engine.scene.rimInput({slash:1});
 if(event.code==='KeyC')engine.scene.rimInput({dash:true});if(event.code==='Space')engine.scene.rimInput({aim:true});
 if(event.code==='Escape'){engine.scene.rimInput({reset:true});document.body.classList.remove('playing');}
});
window.addEventListener('keyup',event=>{keys.delete(event.code);movement();if(event.code==='Space')engine.scene.rimInput({aim:false});});
window.addEventListener('mousemove',event=>{if(document.pointerLockElement===canvas)engine.scene.rimInput({look:[event.movementX,event.movementY]});});
const reset=()=>{keys.clear();engine.scene.rimInput({reset:true});};window.addEventListener('blur',reset);document.addEventListener('visibilitychange',()=>{if(document.hidden)reset();});
for(const button of document.querySelectorAll('[data-key]')){
 button.addEventListener('pointerdown',event=>{if(event instanceof PointerEvent){event.preventDefault();button.setPointerCapture(event.pointerId);}keys.add(button.getAttribute('data-key'));movement();});
 for(const name of ['pointerup','pointercancel'])button.addEventListener(name,()=>{keys.delete(button.getAttribute('data-key'));movement();});
}
document.getElementById('attack')?.addEventListener('click',()=>engine.scene.rimInput({slash:1}));document.getElementById('dash')?.addEventListener('click',()=>engine.scene.rimInput({dash:true}));
const jump=document.getElementById('jump');jump?.addEventListener('pointerdown',event=>{if(event instanceof PointerEvent)jump.setPointerCapture(event.pointerId);engine.scene.rimInput({aim:true});});for(const name of ['pointerup','pointercancel'])jump?.addEventListener(name,()=>engine.scene.rimInput({aim:false}));
document.getElementById('death-mode')?.addEventListener('change',event=>{if(event.target instanceof HTMLSelectElement)engine.scene.rimInput({death_mode:/** @type {'cut'|'dissolve'} */(event.target.value)});});
const enter=document.getElementById('enter');
if(enter instanceof HTMLButtonElement){enter.disabled=!(await engine.xr.isSupported());enter.onclick=async()=>{try{play();await engine.xr.enter({resolutionScale:1});}catch(error){reportDiagnostic('rim-xr-enter',error,{build:releaseId,wasmBuild:buildId});const status=document.getElementById('status');if(status)status.textContent=String(error);}};}
engine.xr.addEventListener('start',()=>document.body.classList.add('xr'));engine.xr.addEventListener('end',()=>{document.body.classList.remove('xr');reset();resize();});
const hudTimer=setInterval(()=>{if(engine.xr.isPresenting)return;const state=engine.scene.rimState;if(!state)return;const status=document.getElementById('status');if(status)status.textContent=state.active?(state.dead?'PADL JSI · nová šance za 3 s':`Život ${Math.ceil(state.health)} % · Vlna ${state.wave} · poraženo ${state.kills} · odraženo ${state.projectiles.reflected} · stamina ${state.player.stamina.value.toFixed(1)}/4`):'Procházet a hrát · WASD / myš · Q/E meče · C dash · drž mezerník, pusť pro skok';},150);
window.addEventListener('pagehide',()=>{clearInterval(hudTimer);stopDiagnostics();stopTransport();window.removeEventListener('resize',resize);engine.dispose();},{once:true});
export function describe(){return {renderer:'Rust/WASM',build:releaseId,wasmBuild:buildId,backend:engine.renderer_backend(),camera:activeCamera,gameplay:engine.scene.rimState,city:content.layout.city,render:engine.renderer_stats()};}
Reflect.set(window,'rimDemo',{engine,content,game,view,play,describe});Reflect.set(window,'render_game_to_text',()=>JSON.stringify(describe()));
document.documentElement.dataset.rendererBackend=engine.renderer_backend();document.documentElement.dataset.ready='true';const backendLabel=document.getElementById('backend');if(backendLabel)backendLabel.textContent='Vlastní render · '+(engine.renderer_backend()==='Gl'?'WebGL2':'WebGPU')+' · '+(releaseId==='working'?'pracovní sestava':'sestava '+releaseId);
const profileToggle=document.getElementById('profile-toggle');if(profileToggle instanceof HTMLAnchorElement){const url=new URL(location.href);url.searchParams.set('profile',profiling?'0':'1');if(!profiling)url.searchParams.delete('info');profileToggle.href=url.href;profileToggle.textContent=profiling?'Vypnout podrobné měření CPU/GPU':'Zapnout podrobné měření CPU/GPU';}document.documentElement.dataset.buildId=releaseId;document.documentElement.dataset.engineBuildId=buildId;
const verifyButton=document.getElementById('verify');if(verifyButton instanceof HTMLButtonElement)verifyButton.onclick=async()=>{const {verify}=await import('./verify.js');await verify();};
// Start combat explicitly from desktop/VR controls, matching the reference welcome state.
