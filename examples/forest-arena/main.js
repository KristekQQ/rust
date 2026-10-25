import {initEngine} from '../../js/engine.js';
import {installRuntimeInfo} from '../shared/runtime-info.js';
import {createScene} from './scene.js';
import {installDiagnostics,reportDiagnostic} from '../shared/diagnostics.js';
installDiagnostics();
const revision='forest-arena-spatial-fps-2';
const canvas=document.getElementById('gpu-canvas');
const enter=document.getElementById('enter'),exit=document.getElementById('exit'),status=document.getElementById('status');
if(!(canvas instanceof HTMLCanvasElement)||!(enter instanceof HTMLButtonElement)||!(exit instanceof HTMLButtonElement)||!status)throw new Error('Chybí UI arény.');
const query=new URLSearchParams(location.search);
export const engine=await initEngine(canvas,{backend:query.get('backend')==='webgl'?'webgl':'auto',profiling:query.get('profile')==='1'&&query.get('info')!=='0'});
engine.set_render_effects(true,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
engine.setBackgroundColor([.38,.55,.63]);engine.postprocessing.enable('bloom',{strength:.12});
export const streetView=()=>engine.setCameraView([0,2.1,4],[0,5,-30]);
export const overview=()=>engine.setOrbitView([72,85,92],[0,0,-20]);
streetView();
// VR diagnostics are a real floor-anchored scene node, not a corner-of-eye overlay.
const fpsToggle=document.getElementById('show-fps');
if(!(fpsToggle instanceof HTMLInputElement))throw new Error('Chybí přepínač FPS.');
engine.setDebugOverlay(false);

export const world=await createScene(engine.scene).catch(error=>{
    status.textContent=`Arénu se nepodařilo načíst: ${error}`;
    const result=document.getElementById('result');if(result)result.textContent=`FAIL: ${error}`;
    throw error;
});
engine.xr.player.configureLocomotion({...world.locomotion,marker:world.marker});
installRuntimeInfo(engine,{panel:world.performancePanel,toggle:fpsToggle});
for(const hand of /** @type {const} */(['left','right'])){
    const item=engine.scene.createCube([0,-10000,0]).setScale([.045,.07,.10]).setEmission(.4);
    engine.xr.player.equip(hand,item);
}
const fit=()=>{if(!engine.xr.isPresenting)engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));};
fit();window.addEventListener('resize',fit);
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
const ready=async()=>{enter.disabled=!(await engine.xr.isSupported());status.textContent=enter.disabled?'Náhled na PC. Pro VR otevři tuto stránku na Questu přes důvěryhodné HTTPS.':'Připraveno. Levý analog: chůze · pravý: otočení · A: teleport.';};
enter.onclick=async()=>{enter.disabled=true;try{await engine.xr.enter();}catch(error){status.textContent=String(error);enter.disabled=false;}};
exit.onclick=()=>{void engine.xr.exit();};
engine.xr.addEventListener('start',()=>{document.body.classList.add('xr');exit.hidden=false;engine.setDebugOverlay(false);});
engine.xr.addEventListener('end',()=>{document.body.classList.remove('xr');exit.hidden=true;engine.setDebugOverlay(false);fit();void ready();});
engine.xr.addEventListener('error',()=>{status.textContent=`VR: ${engine.xr.lastError}`;reportDiagnostic('xr-frame',engine.xr.lastError);});
window.addEventListener('pagehide',()=>{window.removeEventListener('resize',fit);engine.dispose();},{once:true});
const verify=document.getElementById('verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    const result=document.getElementById('result');if(!result)return;
    result.textContent='RUNNING: ověřuji modely a vykreslenou lesní arénu…';
    try{const {verifyScene}=await import('./verify.js');await verifyScene(engine,world,canvas);result.textContent='PASS: lesní GLB, průchozí teleportové trasy, tři pohledy a skutečné pixely.';}
    catch(error){result.textContent=`FAIL: ${error}`;console.error(error);}
};
if(query.has('verify'))document.querySelector('#verification')?.setAttribute('open','');
for(const [id,view] of /** @type {const} */([['street',streetView],['overview',overview]])){
    document.getElementById(id)?.addEventListener('click',view);
}
document.getElementById('ruins')?.addEventListener('click',()=>engine.setCameraView([8,2,-52],[0,4,-62]));
await ready();

reportDiagnostic('engine-ready',undefined,{revision,shadows:true,bloom:.12});
engine.xr.addEventListener('firstframe',event=>reportDiagnostic('xr-first-frame',undefined,{.../** @type {CustomEvent} */(event).detail,revision}));
engine.xr.addEventListener('performance',event=>{
    const sample=/** @type {CustomEvent} */(event).detail;
    const stats=engine.debugStats(),player=engine.xr.player.state;
    reportDiagnostic('xr-performance',undefined,{...sample,revision,spatialPanel:fpsToggle.checked,screenOverlay:stats.enabled,
        effects:engine.effects_stats(),postprocessing:engine.postprocessing.stats(),profile:engine.frameProfile(),
        tracking:{head:player.head.tracked,left:player.leftHand.tracked,right:player.rightHand.tracked},
        render:{objects:stats.total,visible:stats.visible,drawCalls:stats.drawCalls,mainDrawCalls:stats.mainDrawCalls}});
    const last=document.getElementById('last-fps');
    if(last)last.textContent=`Poslední VR měření: ${sample.fps.toFixed(1)} FPS · snímek ${sample.frameMs.toFixed(1)} ms · CPU odeslání ${sample.cpuSubmitMs.toFixed(1)} ms.`;
});
