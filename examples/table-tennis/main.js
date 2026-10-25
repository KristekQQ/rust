import {initEngine} from '../../js/engine.js';
import {installRuntimeInfo} from '../shared/runtime-info.js';
import {createScene,DESKTOP_PADDLE_ROTATION} from './scene.js';
const canvas=document.querySelector('#gpu-canvas');
const status=document.querySelector('#status'),score=document.querySelector('#score');
const enter=document.querySelector('#enter'),exit=document.querySelector('#exit');
if(!(canvas instanceof HTMLCanvasElement)||!status||!score||!(enter instanceof HTMLButtonElement)||!(exit instanceof HTMLButtonElement))throw new Error('Chybí UI');
const params=new URLSearchParams(location.search);
if(params.has('verify')){const button=document.querySelector('#verify');if(button instanceof HTMLElement)button.style.display='inline-block';}
export const engine=await initEngine(canvas,{backend:params.get('backend')==='webgl'?'webgl':'auto',profiling:params.get('profile')==='1'&&params.get('info')!=='0'});
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
engine.set_render_effects(false,false);
engine.setBackgroundColor([.012,.02,.035]);
engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
engine.setCameraView([0,1.65,.7],[0,.9,-2.2]);
export const world=createScene(engine.scene);
engine.setDebugOverlay(false);
world.infoPanel.setPosition([0,-10000,0]);
const fpsToggle=document.querySelector('#fps-toggle');
const runtimeInfo=installRuntimeInfo(engine,{panel:world.infoPanel,toggle:fpsToggle instanceof HTMLInputElement?fpsToggle:null});
const bindInfo=()=>runtimeInfo.setVisible(!(fpsToggle instanceof HTMLInputElement)||fpsToggle.checked);
const textScale=(xr=false)=>{for(const panel of [world.scorePanel,world.titlePanel])panel.setScale([xr?1:-1,1,1]);};
textScale();
engine.scene.configureTableTennis(world);
engine.scene.tableTennisCommand('practice');
engine.scene.tableTennisCommand('move',0,1);
engine.xr.player.equip('right',world.player,{cutting:false,requireTrigger:false});
const serve=()=>engine.scene.tableTennisCommand('serve');
const reset=()=>engine.scene.tableTennisCommand('reset');
const modeButton=document.querySelector('#mode');
const toggleMode=()=>{engine.scene.tableTennisCommand(engine.scene.tableTennisState?.practice?'match':'practice');if(!engine.xr.isPresenting)engine.scene.tableTennisCommand('move',0,1);};
modeButton?.addEventListener('click',toggleMode);
document.querySelector('#serve')?.addEventListener('click',serve);
document.querySelector('#reset')?.addEventListener('click',reset);
/** @param {PointerEvent} event */
const move=(event)=>{
    if(engine.xr.isPresenting)return;
    const rect=canvas.getBoundingClientRect();
    const x=-((event.clientX-rect.left)/rect.width*2-1)*1.05;
    const y=.78+(1-(event.clientY-rect.top)/rect.height)*.8;
    engine.scene.tableTennisCommand('move',x,y);
}
canvas.addEventListener('pointermove',move);
canvas.addEventListener('pointerdown',event=>{
    event.preventDefault();event.stopImmediatePropagation();
    if(event.isTrusted)canvas.setPointerCapture(event.pointerId);
    move(event);
},{capture:true});
/** @param {KeyboardEvent} event */
function key(event){if(event.repeat)return;if(event.code==='F3'){event.preventDefault();if(fpsToggle instanceof HTMLInputElement){fpsToggle.checked=!fpsToggle.checked;if(engine.xr.isPresenting)bindInfo();}return;}if(event.code==='Space'){event.preventDefault();serve();}if(event.code==='KeyR')reset();}
window.addEventListener('keydown',key);
const fit=()=>{if(!engine.xr.isPresenting)engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));};
fit();window.addEventListener('resize',fit);
// Presentation-only snapshots; no JS physics, rule evaluation or RAF loop.
const labels={ready:'Pohybuj pálkou myší. Stiskni Podat a rozehraj výměnu.',rally:'Vrať míček po jednom odrazu na své půlce stolu.',point:'Bod. Další podání se připravuje…',finished:'Konec zápasu. Nový zápas spustíš tlačítkem nebo R.'};
const timer=window.setInterval(()=>{
    const s=engine.scene.tableTennisState;if(!s)return;
    score.textContent=s.practice?`${s.rallyHits} odrazů`:`${s.score[0]} : ${s.score[1]}`;
    if(modeButton)modeButton.textContent=s.practice?'Přepnout na zápas':'Volný trénink';
    const caption=document.querySelector('#mode-caption');if(caption)caption.textContent=s.practice?'VOLNÝ TRÉNINK · FYZICKÉ ODRAZY':'TY / SOUPEŘ · DO 11';
    const serveButton=document.querySelector('#serve');if(serveButton)serveButton.textContent=s.practice?'Pustit míček':'Podat';
    status.textContent=s.practice?(s.phase==='ready'?'Nastav pálku vodorovně. Pusť míček a zkoušej odrazy.':'Míček odráží pohyb a sklon pálky. Další pustíš pravým triggerem.'):labels[/** @type {keyof typeof labels} */(s.phase)]??s.phase;
},150);
enter.onclick=()=>{enter.disabled=true;void engine.xr.enter().catch(e=>{status.textContent=String(e);enter.disabled=false;});};
exit.onclick=()=>{void engine.xr.exit().catch(e=>{status.textContent=String(e);});};
engine.xr.addEventListener('select',event=>{const {handedness}=/** @type {CustomEvent<{handedness:string}>} */(event).detail;if(handedness==='left')toggleMode();else serve();});
engine.xr.addEventListener('start',()=>{exit.hidden=false;textScale(true);bindInfo();reset();});
engine.xr.addEventListener('end',()=>{engine.xr.player.bindHead(null);exit.hidden=true;enter.disabled=false;textScale();world.player.setRotation(DESKTOP_PADDLE_ROTATION);engine.scene.tableTennisCommand('move',0,1);fit();});
engine.xr.addEventListener('error',()=>{status.textContent=engine.xr.lastError?String(engine.xr.lastError):'Chyba VR';});
try{enter.disabled=!(await engine.xr.isSupported());}catch{enter.disabled=true;}
const verifyButton=document.querySelector('#verify');
if(verifyButton instanceof HTMLButtonElement)verifyButton.onclick=()=>{void import('./verify.js').then(m=>m.verify(engine,world,canvas));};
window.addEventListener('pagehide',()=>{window.clearInterval(timer);window.removeEventListener('keydown',key);window.removeEventListener('resize',fit);engine.dispose();},{once:true});
