import {reportDiagnostic,probeWebGpuXR} from '../shared/diagnostics.js';
import { initEngine } from '../../js/engine.js';
import { createScene, launchApple, previewSlice, FRUIT_OPACITY, FRUIT_OPACITY_VARIATION, CUT_EFFECT_SIZE, NEON_GLOW, FLOOR_SSR, FLOOR_SSR_BOUNDS } from './scene.js';

const canvas = document.getElementById('gpu-canvas');
const status = document.getElementById('status');
const enter = document.getElementById('enter');
const exit = document.getElementById('exit');
if (!(canvas instanceof HTMLCanvasElement) || !status || !(enter instanceof HTMLButtonElement) || !(exit instanceof HTMLButtonElement)) throw new Error('Chybí UI.');

const parameters=new URLSearchParams(location.search);
const backend = parameters.get('backend') === 'webgl' ? 'webgl' : 'auto';
let enableSSR=parameters.get('ssr')!=='0';
const enableGlow=parameters.get('glow')!=='0';
const revision='cyber-neon-shader-25';
let solidFruit=true;
export const engine = await initEngine(canvas, {backend,profiling:parameters.get('profile')==='1'});
document.documentElement.dataset.rendererBackend = engine.renderer_backend();
reportDiagnostic('engine-ready',undefined,{ssr:enableSSR,glow:enableGlow,revision,profiling:parameters.get('profile')==='1'});
engine.xr.addEventListener('gpuerror',event=>{const detail=/** @type {CustomEvent} */(event).detail;reportDiagnostic('gpu-validation',new Error(detail.message),detail);});
engine.xr.addEventListener('gpulost',event=>{const detail=/** @type {CustomEvent} */(event).detail;reportDiagnostic('gpu-lost',new Error(detail.message),detail);});
engine.xr.addEventListener('performance',event=>{
    const stats=engine.debugStats();
    // Static graph metadata is already logged at firstframe. Periodic reports
    // carry compact live counters instead of repeatedly serializing the graph.
    reportDiagnostic('xr-performance',undefined,{.../** @type {CustomEvent} */(event).detail,revision,ssr:enableSSR,glow:enableGlow,
        postprocessing:engine.postprocessing.stats(),profile:engine.frameProfile(),
        render:{objects:stats.total,visible:stats.visible,drawCalls:stats.drawCalls,mainDrawCalls:stats.mainDrawCalls,
            uploadPages:stats.instanceUploadPages,sharedMeshes:stats.sharedMeshes,numericFont:stats.numericFont,translucency:stats.translucency,neonGlow:stats.neonGlow}});
});
engine.set_render_effects(false, false);
engine.set_grid_visible(false);
engine.setBackgroundColor([0,0,0]);
engine.setNeonGlow(enableGlow?NEON_GLOW:{strength:0});
engine.postprocessing.configureSSR({resolutionScale:.5,receiverBounds:FLOOR_SSR_BOUNDS});
if(enableSSR)engine.postprocessing.enable('ssr',FLOOR_SSR);
const ssrToggle=document.getElementById('ssr');
const build=document.getElementById('build');
const showBuild=()=>{if(build)build.textContent=`Verze ${revision} · ${engine.renderer_backend()==='BrowserWebGpu'?'WebGPU':'WebGL'} · SSR ${enableSSR?'zapnuto':'vypnuto'}`;};
showBuild();
if(ssrToggle instanceof HTMLInputElement){
    ssrToggle.checked=enableSSR;
    ssrToggle.addEventListener('change',()=>{
        enableSSR=ssrToggle.checked;
        reportDiagnostic('ssr-changed',undefined,{enabled:enableSSR,revision,atMs:performance.now()});
        if(enableSSR)engine.postprocessing.enable('ssr',FLOOR_SSR);
        else engine.postprocessing.disable('ssr');
        showBuild();
    });
}

engine.set_light_helpers_visible(false);
engine.setDebugOverlay(false);
const home = () => engine.setOrbitView([0,1.75,1.2],[0,1.45,-1.6]);
home();
export const world = await createScene(engine.scene);
engine.xr.configureComboDisplay(world.comboDisplay);
const fruitStyle=document.getElementById('solid-fruit');
const applyFruitStyle=()=>world.galleryIcons.forEach((icon,index)=>icon.setColorCutout(solidFruit?0:.25).setOpacity(solidFruit?FRUIT_OPACITY:1,{opaqueAbove:.25,variation:solidFruit?FRUIT_OPACITY_VARIATION:0,coverage:world.galleryPatterns[index].coverage??.5}));
applyFruitStyle();
if(fruitStyle instanceof HTMLInputElement){
    fruitStyle.checked=solidFruit;
    fruitStyle.addEventListener('change',()=>{
        solidFruit=fruitStyle.checked;
        applyFruitStyle();
        // Only new projectiles use the new style; selection is locked inside VR.
        for(const apple of flying)if(apple.exists)apple.remove();
        flying=[];
        engine.scene.clearCutFeedback();
        for(const piece of previewPieces)if(piece.exists)piece.remove();
        previewPieces=[];
    });
}
// Desktop orbit and immersive eye views use opposite horizontal handedness.
// Keep this preview correction local to the example's world-space text.
const setPanelFacing = (/** @type {boolean} */ immersive) => {
    for (const panel of [world.infoPanel,world.timerPanel,world.scoreLabel,world.scorePanel,world.currentLabel,...world.awardPanels,world.comboRules,...world.galleryLabels,world.restartButton,world.credits,world.logo])
        panel.setScale([immersive ? 1 : -1,1,1]);

};
setPanelFacing(false);
document.querySelector('#reset')?.addEventListener('click',home);
const verify = document.querySelector('#verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    const result=document.querySelector('#result');
    try{
        if(result)result.textContent='RUNNING: pixelové kontroly';
        const {verifyArena}=await import('./verify.js');
        const evidence=await verifyArena(engine,world,canvas,previewCut);
        const shared=evidence.sharedMeshes;
        if(result)result.textContent=`PASS: ovoce, řezy, SSR, sdílení ${shared.liveInstances} → ${shared.gpuMeshes}; geometrie ${(shared.geometryBytes/1024).toFixed(1)} KiB místo ${(shared.instanceGeometryBytes/1024).toFixed(1)} KiB, paleta ${(shared.colorTableBytes/1024).toFixed(1)} KiB`;
    }catch(error){if(result)result.textContent=`FAIL: ${error instanceof Error?error.stack:error}`;}
};
export const player = engine.xr.player;
player.equip('right', world.sword, {cutting: true, requireTrigger: false, haptics:true});
player.equip('left', world.leftSword, {cutting: true, requireTrigger: false, haptics:true});
engine.xr.bindButton(world.restartButton);
engine.xr.addEventListener('buttonhover', event => {
    const {handedness,point}=/** @type {CustomEvent<{handedness:'left'|'right',point:import('../../js/engine.d.ts').Vector3|null}>} */(event).detail;
    world.pointers[handedness==='left'?0:1].setPosition(point ?? [100,100,100]);
});
let shot = 0, cuts = 0;
/** @type {ReturnType<typeof engine.scene.playCutFeedback>|null} */
let feedback=null;
/** @type {import('../../js/engine.js').SceneObject[]} */
let previewPieces=[];
/** @param {number} [size] @param {boolean} [solid] */
export function previewCut(size=CUT_EFFECT_SIZE,solid=solidFruit) {
    engine.scene.clearCutFeedback();
    for(const piece of previewPieces)if(piece.exists)piece.remove();
    const result=previewSlice(engine.scene,world.fruits.find(f=>f.name==='watermelon')??world.fruits[0],size,solid);
    previewPieces=result.report.pieces;
    feedback=result.feedback;return result;
}
document.getElementById('slice-preview')?.addEventListener('click',()=>{if(!engine.xr.isPresenting)previewCut();});
/** @type {import('../../js/engine.js').SceneObject[]} */
let flying = [];
let launchTimer = 0;
export function shootApple() {
    flying = flying.filter(apple => apple.exists);
    if (engine.xr.roundState?.finished || flying.length >= 3) return null;
    const index = shot++;
    const fruit=world.nextFruitVariant(world.playFruits[index % world.playFruits.length]);
    const apple = launchApple(engine.scene, fruit, index % 2 === 0 ? -1 : 1,solidFruit);
    flying.push(apple);
    return apple;
}
export const gameState = () => ({shots: shot, cuts, round: engine.xr.roundState, liveApples: flying.filter(apple => apple.exists), launching: launchTimer !== 0, feedback});
export const stopLaunches = () => { window.clearInterval(launchTimer); launchTimer = 0; };
const startRound = () => {
    stopLaunches();
    for(const piece of previewPieces)if(piece.exists)piece.remove();
    previewPieces=[];
    engine.scene.clearCutFeedback();feedback=null;
    for (const apple of flying) if (apple.exists) apple.remove();
    flying = []; shot = 0; cuts = 0;
    engine.xr.startRound(30);
    const count = document.getElementById('cuts'); if (count) count.textContent = '0';
    status.textContent = 'Kolo 30 s. Tref 3 stejné plody za sebou pro body. Jiný plod začne novou sérii.';
    shootApple(); launchTimer = window.setInterval(shootApple, 550);
};
document.getElementById('shoot')?.addEventListener('click', shootApple);
engine.xr.addEventListener('select', event => {
    const {handedness,target}=/** @type {CustomEvent<{handedness:string,target:import('../../js/engine.js').SceneObject|null}>} */(event).detail;
    if (target?.id===world.restartButton.id) startRound();
    else if (handedness==='left' && !engine.xr.roundState?.finished) shootApple();
});
engine.xr.addEventListener('buttonpress', () => startRound());
engine.xr.addEventListener('cut', event => {
    const report = /** @type {CustomEvent<import('../../js/engine.d.ts').XRCutReport>} */(event).detail;
    for (const piece of report.pieces) piece.setEmission(1.1);
    feedback=engine.scene.playCutFeedback(report.pieces,{mirrorText:!engine.xr.isPresenting,size:CUT_EFFECT_SIZE});
    cuts += report.removed.length;
    const count = document.getElementById('cuts'); if (count) count.textContent = String(engine.xr.roundState?.score ?? 0);
});
const fit = () => { if (!engine.xr.isPresenting) engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight)); };
fit();
window.addEventListener('resize', fit);
const ready = async () => {
    try {
        enter.disabled = !(await engine.xr.isSupported());
        const missingGpuBinding = engine.renderer_backend() === 'BrowserWebGpu' && typeof Reflect.get(globalThis,'XRGPUBinding') !== 'function';
        status.textContent = enter.disabled
            ? (!isSecureContext ? 'Pro VR je potřeba důvěryhodné HTTPS nebo localhost přes USB/ADB.'
                : missingGpuBinding ? 'WebGPU náhled běží, ale prohlížeč neposkytuje WebGPU pro VR (XRGPUBinding). V chrome://flags zkontroluj WebXR Experiments a potom restartuj prohlížeč. Pokud volba není dostupná, použij WebGL / WebXR.'
                : 'VR není pro tuto konfiguraci dostupné. Použij odkaz WebGL / WebXR.')
            : 'Připraveno. Sekej katanami v obou rukou. Neonové ovoce vystřeluje samo, informace o scéně vidíš i ve VR.';
    } catch (error) { status.textContent = String(error); }
};
enter.onclick = () => {
    enter.disabled = true;
    void engine.xr.enter().catch(async error => { status.textContent = String(error); reportDiagnostic('xr-enter',error); enter.disabled = !(await engine.xr.isSupported().catch(()=>false)); });
};
exit.onclick = () => { void engine.xr.exit().catch(error => { status.textContent = String(error); }); };
engine.xr.addEventListener('firstframe', event => reportDiagnostic('xr-first-frame',undefined,{.../** @type {CustomEvent} */(event).detail,postprocessing:engine.postprocessing.stats(),profile:engine.frameProfile(),graph:engine.postprocessing.graph()}));
engine.xr.addEventListener('start', () => { reportDiagnostic('xr-start'); if(fruitStyle instanceof HTMLInputElement)fruitStyle.disabled=true; setPanelFacing(true); exit.hidden = false; startRound(); });
engine.xr.addEventListener('roundend', () => {
    stopLaunches();
    world.apples.forEach((fruit,i)=>fruit.setPosition(world.previewPositions[i]));
    status.textContent = `Konec kola — skóre ${engine.xr.roundState?.score ?? 0}, trojic ${engine.xr.roundState?.triples ?? 0}. Namiř na DALSI KOLO a stiskni trigger.`;
});
engine.xr.addEventListener('end', () => { if(fruitStyle instanceof HTMLInputElement)fruitStyle.disabled=false; setPanelFacing(false); stopLaunches(); engine.scene.clearCutFeedback(); world.pointers.forEach(p=>p.setPosition([100,100,100])); exit.hidden = true; void ready(); home(); fit(); status.textContent = 'VR ukončeno. Můžete vstoupit znovu.'; });
engine.xr.addEventListener('error', () => { status.textContent = `VR se zastavilo: ${engine.xr.lastError}`; reportDiagnostic('xr-frame',engine.xr.lastError); });
window.addEventListener('pagehide', () => { stopLaunches(); window.removeEventListener('resize', fit); engine.dispose(); }, { once: true });
await ready();

const diagnostics=document.getElementById('xr-diagnostics');
if(diagnostics instanceof HTMLButtonElement)diagnostics.onclick=()=>{
    if(engine.xr.isPresenting)return;
    diagnostics.disabled=true;
    status.textContent='Testuji nativní WebGPU VR relaci…';
    void probeWebGpuXR().then(message=>{status.textContent=message;}).catch(error=>reportDiagnostic('xr-probe-request',error)).finally(()=>{diagnostics.disabled=false;});
};
const xrPlatform=Reflect.get(navigator,'xr');
void Promise.resolve(xrPlatform?.isSessionSupported('immersive-vr')).then(supported=>{
    reportDiagnostic('xr-capabilities',undefined,{immersiveVR:!!supported,interfaces:Object.getOwnPropertyNames(globalThis).filter(name=>/^XR.*(Binding|Layer)$/.test(name)),
        gpuBindingVisible:typeof Reflect.get(globalThis,'XRGPUBinding')==='function',visibility:document.visibilityState});
}).catch(error=>reportDiagnostic('xr-capabilities',error));
