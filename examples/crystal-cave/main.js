import {initEngine} from '../../js/engine.js';
import {installRuntimeInfo} from '../shared/runtime-info.js';
import {createScene} from './scene.js';
import {installDiagnostics,reportDiagnostic} from '../shared/diagnostics.js';
installDiagnostics();
const canvas=document.getElementById('gpu-canvas');
const enter=document.getElementById('enter'),exit=document.getElementById('exit'),status=document.getElementById('status');
if(!(canvas instanceof HTMLCanvasElement)||!(enter instanceof HTMLButtonElement)||!(exit instanceof HTMLButtonElement)||!status)throw new Error('Chybí UI jeskyně.');
const query=new URLSearchParams(location.search);
export const engine=await initEngine(canvas,{backend:query.get('backend')==='webgl'?'webgl':'auto',profiling:query.get('profile')==='1'&&query.get('info')!=='0'});
// Static cave lighting keeps the VR path free of dynamic shadow maps.
engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
engine.setBackgroundColor([.012,.020,.029]);engine.postprocessing.enable('bloom',{strength:.22});
export const entranceView=()=>engine.setCameraView([0,1.7,1],[0,3,-18]);
export const heartView=()=>engine.setCameraView([0,1.7,-20],[0,3,-31]);
entranceView();
const fpsToggle=document.getElementById('show-fps');
if(!(fpsToggle instanceof HTMLInputElement))throw new Error('Chybí přepínač FPS.');
engine.setDebugOverlay(false);
export const world=await createScene(engine.scene).catch(error=>{
    status.textContent=`Jeskyni se nepodařilo načíst: ${error}`;
    const result=document.getElementById('result');if(result)result.textContent=`FAIL: ${error}`;
    throw error;
});
engine.xr.player.configureLocomotion({...world.locomotion,marker:world.marker});
// One spatial scene node follows the native head pose; Rust refreshes its counters.
installRuntimeInfo(engine,{panel:world.performancePanel,toggle:fpsToggle});
for(const hand of /** @type {const} */(['left','right'])){
    const item=engine.scene.createCube([0,-10000,0]).setScale([.045,.07,.10]).setEmission(.4);
    engine.xr.player.equip(hand,item);
}
const fit=()=>{if(!engine.xr.isPresenting){const ratio=Math.min(devicePixelRatio||1,2);engine.resize(Math.max(1,Math.round(canvas.clientWidth*ratio)),Math.max(1,Math.round(canvas.clientHeight*ratio)));}};
fit();window.addEventListener('resize',fit);
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
const ready=async()=>{enter.disabled=!(await engine.xr.isSupported());status.textContent=enter.disabled?'Náhled na PC. Pro VR otevři tuto stránku na Questu přes důvěryhodné HTTPS.':'Připraveno. Levý analog: chůze · pravý: otočení · A: teleport.';};
enter.onclick=async()=>{enter.disabled=true;try{await engine.xr.enter({resolutionScale:1});}catch(error){status.textContent=String(error);enter.disabled=false;}};
exit.onclick=()=>{void engine.xr.exit();};
engine.xr.addEventListener('start',()=>{document.body.classList.add('xr');exit.hidden=false;});
engine.xr.addEventListener('end',()=>{document.body.classList.remove('xr');exit.hidden=true;fit();void ready();});
engine.xr.addEventListener('error',()=>{status.textContent=`VR: ${engine.xr.lastError}`;reportDiagnostic('xr-frame',engine.xr.lastError);});
window.addEventListener('pagehide',()=>{window.removeEventListener('resize',fit);engine.dispose();},{once:true});
const verify=document.getElementById('verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    const result=document.getElementById('result');if(!result)return;
    result.textContent='RUNNING: ověřuji modely a vykreslenou jeskyni…';
    try{const {verifyScene}=await import('./verify.js');await verifyScene(engine,world,canvas);result.textContent='PASS: jeskynní GLB, průchozí teleportové trasy, prostorové pohledy a skutečné pixely.';}
    catch(error){result.textContent=`FAIL: ${error}`;console.error(error);}
};
if(query.has('verify'))document.querySelector('#verification')?.setAttribute('open','');
for(const [id,view] of /** @type {const} */([['entrance',entranceView],['heart',heartView]])){
    document.getElementById(id)?.addEventListener('click',view);
}
await ready();

const revision='crystal-cave-1';
reportDiagnostic('engine-ready',undefined,{revision,shadows:false,resolutionScale:1});
engine.xr.addEventListener('firstframe',event=>reportDiagnostic('xr-first-frame',undefined,{.../** @type {CustomEvent} */(event).detail,revision,shadows:false}));
engine.xr.addEventListener('performance',event=>{
    const sample=/** @type {CustomEvent} */(event).detail;
    const stats=engine.debugStats();
    reportDiagnostic('xr-performance',undefined,{...sample,revision,shadows:false,spatialPanel:fpsToggle.checked,screenOverlay:stats.enabled,
        effects:engine.effects_stats(),postprocessing:engine.postprocessing.stats(),profile:engine.frameProfile(),
        render:{objects:stats.total,visible:stats.visible,drawCalls:stats.drawCalls,mainDrawCalls:stats.mainDrawCalls}});
    status.textContent=`Poslední VR měření: ${sample.fps.toFixed(1)} FPS · ${sample.eyeViewports.map((/** @type {{width:number,height:number}} */ eye)=>`${eye.width}×${eye.height}`).join(' / ')} px.`;
});
