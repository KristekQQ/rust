import {initEngine} from '../../js/engine.js';
import {installRuntimeInfo} from '../shared/runtime-info.js';
import {createScene,startCombat} from './scene.js';
import {installDiagnostics,reportDiagnostic} from '../shared/diagnostics.js';
installDiagnostics();
const canvas=document.getElementById('gpu-canvas');
const enter=document.getElementById('enter'),exit=document.getElementById('exit'),status=document.getElementById('status');
if(!(canvas instanceof HTMLCanvasElement)||!(enter instanceof HTMLButtonElement)||!(exit instanceof HTMLButtonElement)||!status)throw new Error('Chybí UI arény.');
const query=new URLSearchParams(location.search);
export const engine=await initEngine(canvas,{backend:query.get('backend')==='webgl'?'webgl':'auto',profiling:query.get('profile')==='1'&&query.get('info')!=='0'});
// Four animated shadow maps were too expensive for the headset scene.
engine.set_render_effects(false,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
engine.setBackgroundColor([.008,.014,.032]);engine.postprocessing.enable('bloom',{strength:.42});
export const streetView=()=>engine.setCameraView([0,1.85,6],[0,5,-18]);
export const overview=()=>engine.setOrbitView([0,23,18],[0,4,-15]);
streetView();
const fpsToggle=document.getElementById('show-fps');
if(!(fpsToggle instanceof HTMLInputElement))throw new Error('Chybí přepínač FPS.');
engine.setDebugOverlay(false);
export const world=await createScene(engine.scene).catch(error=>{
    status.textContent=`Arénu se nepodařilo načíst: ${error}`;
    const result=document.getElementById('result');if(result)result.textContent=`FAIL: ${error}`;
    throw error;
});
engine.xr.player.configureLocomotion({...world.locomotion,marker:world.marker});
// One spatial scene node follows the native head pose; Rust refreshes its counters.
installRuntimeInfo(engine,{panel:world.performancePanel,toggle:fpsToggle});
startCombat(engine,world);
const fit=()=>{if(!engine.xr.isPresenting){const ratio=Math.min(devicePixelRatio||1,2);engine.resize(Math.max(1,Math.round(canvas.clientWidth*ratio)),Math.max(1,Math.round(canvas.clientHeight*ratio)));}};
fit();window.addEventListener('resize',fit);
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
const ready=async()=>{enter.disabled=!(await engine.xr.isSupported());status.textContent=enter.disabled?'Náhled na PC. Pro VR otevři tuto stránku na Questu přes důvěryhodné HTTPS.':'Připraveno. Meč v každé ruce · sekni do nepřítele · další přijde až po jeho smrti.';};
enter.onclick=async()=>{enter.disabled=true;try{await engine.xr.enter({resolutionScale:1});}catch(error){status.textContent=String(error);enter.disabled=false;}};
exit.onclick=()=>{void engine.xr.exit();};
engine.xr.addEventListener('start',()=>{document.body.classList.add('xr');exit.hidden=false;});
engine.xr.addEventListener('end',()=>{document.body.classList.remove('xr');exit.hidden=true;fit();void ready();});
engine.xr.addEventListener('error',()=>{status.textContent=`VR: ${engine.xr.lastError}`;reportDiagnostic('xr-frame',engine.xr.lastError);});
window.addEventListener('pagehide',()=>{window.removeEventListener('resize',fit);engine.dispose();},{once:true});
const verify=document.getElementById('verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    const result=document.getElementById('result');if(!result)return;
    result.textContent='RUNNING: ověřuji modely a vykreslenou městskou arénu…';
    try{const {verifyScene}=await import('./verify.js');await verifyScene(engine,world,canvas);result.textContent='PASS: městský GLB, průchozí teleportové trasy, tři pohledy a skutečné pixely.';}
    catch(error){result.textContent=`FAIL: ${error}`;console.error(error);}
};
if(query.has('verify'))document.querySelector('#verification')?.setAttribute('open','');
for(const [id,view] of /** @type {const} */([['street',streetView],['overview',overview]])){
    document.getElementById(id)?.addEventListener('click',view);
}
await ready();

const revision='neon-district-5-melee';
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
