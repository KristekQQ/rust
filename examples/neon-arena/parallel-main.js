import {initEngine} from '../../js/engine.js';
import {createScene,NEON_GLOW,ARENA_REVISION} from './scene.js';
import {ParallelBridge} from './parallel-bridge.js';
import {installRuntimeInfo} from '../shared/runtime-info.js';
import {installScoreboard,loadScoreboard} from './scoreboard.js';
import {reportDiagnostic} from '../shared/diagnostics.js';
const canvas=document.querySelector('#gpu-canvas'),status=document.querySelector('#status'),error=document.querySelector('#error'),enter=document.querySelector('#enter'),exit=document.querySelector('#exit');
if(!(canvas instanceof HTMLCanvasElement)||!(enter instanceof HTMLButtonElement)||!(exit instanceof HTMLButtonElement)||!status||!error)throw new Error('Chybí ovládání');
window.addEventListener('error',event=>{error.textContent=event.message;});
window.addEventListener('unhandledrejection',event=>{error.textContent=String(event.reason);});
const parameters=new URLSearchParams(location.search),backend=parameters.get('backend')==='webgl'?'webgl':'auto';
export const engine=await initEngine(canvas,{backend});
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
engine.setDebugOverlay(false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_render_effects(false,false);engine.postprocessing.disable('ssr');engine.postprocessing.disable('bloom');engine.postprocessing.disable('fxaa');engine.setBackgroundColor([0,0,0]);engine.setNeonGlow(NEON_GLOW);engine.setOrbitView([0,1.75,1.2],[0,1.45,-1.6]);
engine.setSceneAuthoringPaused(true);
export const world=await createScene(engine.scene);await world.loadDecorations();
if(world.arenaFont.selected==='cynatar')engine.prewarmFontGlowAtlas('0123456789+-');
let scores=[];
try{scores=await loadScoreboard();}catch{try{scores=JSON.parse(localStorage.getItem('neon-parallel-top5')??'[]');}catch{}}
if(!Array.isArray(scores))scores=[];scores=scores.filter(value=>Number.isSafeInteger(value)&&value>=0).slice(0,100);
const mode=parameters.get('simulation')==='inline'?'inline':'worker';
export const bridge=await ParallelBridge.create(engine,world,{mode,scores,observerUrl:parameters.get('observer')});
engine.setSceneAuthoringPaused(false);
engine.xr.player.equip('left',world.leftSword,{cutting:false,requireTrigger:false});
engine.xr.player.equip('right',world.sword,{cutting:false,requireTrigger:false});
const infoToggle=document.querySelector('#vr-info');
const runtimeInfo=installRuntimeInfo(engine,{panel:world.infoPanel,toggle:infoToggle instanceof HTMLInputElement?infoToggle:null});runtimeInfo.setVisible(true);
if(infoToggle instanceof HTMLInputElement)infoToggle.checked=runtimeInfo.visible;
let y=false,saved='';
bridge.addEventListener('state',()=>{const value=JSON.stringify(bridge.state?.top5??[]);if(value!==saved){saved=value;try{localStorage.setItem('neon-parallel-top5',value);}catch{}}});
const removeScoreboard=installScoreboard(bridge,{onScores:values=>bridge.restoreScores(values)});
engine.xr.setSimulationFrameHandler(sample=>{
    bridge.step(sample);
    if(sample[7]&&!y){runtimeInfo.setVisible(!runtimeInfo.visible);if(infoToggle instanceof HTMLInputElement)infoToggle.checked=runtimeInfo.visible;}
    y=!!sample[7];
});
engine.xr.addEventListener('start',()=>{bridge.setImmersive(true);bridge.newSession();document.body.classList.add('xr');enter.hidden=true;exit.hidden=false;status.textContent='Hra běží v brýlích.';});
engine.xr.addEventListener('end',()=>{
    document.body.classList.remove('xr');enter.hidden=false;exit.hidden=true;y=false;status.textContent='Aréna je připravená pro další kolo.';
    try{bridge.setImmersive(false);bridge.resetTracking();bridge.step(new Float32Array(88));void bridge.settle().catch(reason=>{error.textContent=String(reason);});}
    catch(reason){error.textContent=String(reason);}
});
engine.xr.addEventListener('performance',event=>reportDiagnostic('neon-native-parity',undefined,{...event.detail,simulation:mode,transport:bridge.stats,state:bridge.state}));
const fail=reason=>{error.textContent=String(reason);reportDiagnostic('neon-parity-error',reason);};
engine.xr.addEventListener('error',()=>{status.textContent='Spuštění hry se zastavilo.';fail(engine.xr.lastError??new Error('WebXR se přerušilo.'));});
enter.onclick=()=>{enter.disabled=true;void engine.xr.enter().catch(fail).finally(()=>{enter.disabled=false;});};exit.onclick=()=>void engine.xr.exit();
const build=document.querySelector('#build');if(build)build.textContent=`${ARENA_REVISION} · ${engine.renderer_backend()}`;
status.textContent='Aréna je připravená.';enter.disabled=!(await engine.xr.isSupported());
const backendLink=document.querySelector('#backend');if(backendLink instanceof HTMLAnchorElement)backendLink.textContent=backend==='webgl'?'WebGL2 ✓':'WebGL2';
window.addEventListener('pagehide',()=>{removeScoreboard();runtimeInfo.dispose();bridge.dispose();engine.dispose();},{once:true});
globalThis.__neonParallel={engine,world,bridge};

if(parameters.get('verify')==='1'){
    const result=document.createElement('pre');result.id='result';result.hidden=true;
    result.textContent=world.sword.exists&&world.leftSword.exists&&bridge.state?'PASS: complete native arena and game initialized':'FAIL: incomplete native arena';document.body.append(result);
}
