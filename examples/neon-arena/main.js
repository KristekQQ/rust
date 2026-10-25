import {reportDiagnostic,probeWebGpuXR} from '../shared/diagnostics.js';
import { initEngine } from '../../js/engine.js';
import {installRuntimeInfo} from '../shared/runtime-info.js';
import {VR_DEBUG_PANEL_OFFSET,VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from '../shared/vr-debug-panel.js';
import { ARENA_REVISION, createScene, createFruitMesh, launchApple, previewSlice, FRUIT_SCALE, RESTART_POSITION, FRUIT_OPACITY, FRUIT_OPACITY_VARIATION, CUT_EFFECT_SIZE, NEON_GLOW, FLOOR_SSR, FLOOR_SSR_BOUNDS } from './scene.js';
import {PerformanceTest,PERFORMANCE_PHASES} from './performance-test.js';
import {planWave} from './wave-plan.js';
import {validateRoundPlan,planCue} from './round-plan.js';
import {createArenaTools,installArenaPanel,installArenaBridge} from './arena-tools.js';
import {fetchPreset} from './arena-presets.js';

const canvas = document.getElementById('gpu-canvas');
const status = document.getElementById('status');
const enter = document.getElementById('enter');
const exit = document.getElementById('exit');
if (!(canvas instanceof HTMLCanvasElement) || !status || !(enter instanceof HTMLButtonElement) || !(exit instanceof HTMLButtonElement)) throw new Error('Chybí UI.');

const parameters=new URLSearchParams(location.search);
const backend = parameters.get('backend') === 'webgl' ? 'webgl' : 'auto';
let enableSSR=parameters.get('ssr')!=='0';
let enableGlow=parameters.get('glow')!=='0';
const revision=ARENA_REVISION;
let solidFruit=true;
export const engine = await initEngine(canvas, {backend,profiling:parameters.get('info')!=='0'&&parameters.get('profile')==='1'});
engine.setDebugOverlay(false);
/** @type {PerformanceTest|null} */
let performanceTest=null;
document.documentElement.dataset.rendererBackend = engine.renderer_backend();
reportDiagnostic('engine-ready',undefined,{ssr:enableSSR,glow:enableGlow,revision,profiling:engine.frameProfile().enabled});
engine.xr.addEventListener('gpuerror',event=>{const detail=/** @type {CustomEvent} */(event).detail;reportDiagnostic('gpu-validation',new Error(detail.message),detail);});
engine.xr.addEventListener('gpulost',event=>{performanceTest?.cancel('GPU ztraceno');const detail=/** @type {CustomEvent} */(event).detail;reportDiagnostic('gpu-lost',new Error(detail.message),detail);});
engine.xr.addEventListener('performance',event=>{
    const sample=/** @type {CustomEvent} */(event).detail;
    const stats=engine.debugStats();
    // Static graph metadata is already logged at firstframe. Periodic reports
    // carry compact live counters instead of repeatedly serializing the graph.
    reportDiagnostic('xr-performance',undefined,{.../** @type {CustomEvent} */(event).detail,revision,ssr:enableSSR,glow:enableGlow,
        postprocessing:engine.postprocessing.stats(),profile:engine.frameProfile(),
        render:{objects:stats.total,visible:stats.visible,drawCalls:stats.drawCalls,mainDrawCalls:stats.mainDrawCalls,
            uploadPages:stats.instanceUploadPages,sharedMeshes:stats.sharedMeshes,numericFont:stats.numericFont,translucency:stats.translucency,neonGlow:stats.neonGlow}});
    performanceTest?.addSample({...sample,renderer:engine.renderer_backend()});
});
engine.set_render_effects(false, false);
engine.set_grid_visible(false);
engine.setBackgroundColor([0,0,0]);
engine.setNeonGlow(enableGlow?NEON_GLOW:{strength:0});
engine.postprocessing.configureSSR({resolutionScale:.5,receiverBounds:FLOOR_SSR_BOUNDS});
if(enableSSR)engine.postprocessing.enable('ssr',FLOOR_SSR);
const ssrToggle=document.getElementById('ssr');
const build=document.getElementById('build');
const releaseBuild=document.querySelector('meta[name="build-profile"]')?.getAttribute('content')==='release';
const showBuild=()=>{if(build)build.textContent=`Verze ${revision}${releaseBuild?' · PRODUCTION RELEASE':''} · ${engine.renderer_backend()==='BrowserWebGpu'?'WebGPU':'WebGL'} · SSR ${enableSSR?'zapnuto':'vypnuto'}`;};
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
const fontSelect=document.getElementById('arena-font');
const initialFont=parameters.get('font')==='neon'?'neon':'cynatar';
if(initialFont!==world.arenaFont.selected)world.arenaFont.set(initialFont);
if(fontSelect instanceof HTMLSelectElement){
    fontSelect.disabled=false;fontSelect.value=world.arenaFont.selected;
    fontSelect.addEventListener('change',()=>{
        try{
            world.arenaFont.set(fontSelect.value==='neon'?'neon':'cynatar');
            reportDiagnostic('font-changed',undefined,{font:world.arenaFont.selected,revision});
        }catch(error){fontSelect.value=world.arenaFont.selected;reportDiagnostic('font-change',error);}
    });
}
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
    for (const panel of [...world.phasePanels.map(item=>item.object),...world.stockScorePanels,world.timerPanel,world.scoreLabel,world.scorePanel,world.currentLabel,...world.awardPanels,world.comboRules,...world.galleryLabels,world.restartButton,world.logo])
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
const vrInfoToggle=document.getElementById('vr-info');
const runtimeInfo=installRuntimeInfo(engine,{panel:world.infoPanel,toggle:vrInfoToggle instanceof HTMLInputElement?vrInfoToggle:null});
const syncVrInfo=()=>runtimeInfo.setVisible(vrInfoToggle instanceof HTMLInputElement?vrInfoToggle.checked:runtimeInfo.visible);

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
    if (engine.xr.roundState?.finished || flying.length >= 10) return null;
    const index = shot++;
    const fruit=world.nextFruitVariant(world.playFruits[index % world.playFruits.length]);
    const apple = launchApple(engine.scene, fruit, index % 2 === 0 ? -1 : 1,solidFruit);
    flying.push(apple);
    return apple;
}
/** @type {import('../../js/engine.d.ts').SceneObject[]} */
let launchGlints=[];
const clearLaunchGlints=()=>{for(const object of launchGlints)if(object.exists)object.remove();launchGlints=[];};
export const gameState = () => ({launchGlints:launchGlints.filter(object=>object.exists),shots: shot, cuts, round: engine.xr.roundState, liveApples: flying.filter(apple => apple.exists), launching: launchTimer !== 0, feedback});
export const stopLaunches = () => { window.clearInterval(launchTimer); launchTimer = 0;clearLaunchGlints(); };
let waveIndex=0,nextCueIndex=0,previewIndex=-1,nextPracticeAt=0;
let phaseLabel='';
/** @type {{index:number,at:number,label:string,spawned:number,live:number}[]} */
let cueHistory=[];
const roundResponse=await fetch(new URL('./minute-scene.json',import.meta.url),{cache:'no-store'});
if(!roundResponse.ok)throw new Error(`Nelze načíst minutovou scénu: HTTP ${roundResponse.status}`);
export const minutePlan=validateRoundPlan(await roundResponse.json());
/** @param {string} label @param {boolean} [preview] */
const showPhase=(label,preview=false)=>{
    phaseLabel=label;
    for(const panel of world.phasePanels)panel.object.setPosition(panel.title===label&&(engine.xr.isPresenting||preview)?[0,2.75,-2.15]:[100,100,100]);
};
/** @param {ReturnType<typeof planWave>} recipes @param {boolean} [showLaunch] */
const spawnRecipes=(recipes,showLaunch=false)=>{
    flying=flying.filter(object=>object.exists);
    if(recipes.length>10-flying.length)return {spawned:0,reason:'Předchozí ovoce je ještě ve hře.',live:flying.length};
    launchGlints=launchGlints.filter(object=>object.exists);
    for(const [i,recipe] of recipes.entries()){
        const source=world.fruits.find(fruit=>fruit.name===recipe.fruit);
        if(!source)throw new Error('Neznámé ovoce.');
        const object=launchApple(engine.scene,world.nextFruitVariant(source),0,solidFruit,recipe);
        flying.push(object);shot++;
        if(showLaunch)for(const side of [-1,1]){
            const glint=world.launchGlintAssets[i%3].createInstance()
                .setPosition([recipe.position[0]+side*.045,recipe.position[1]-.04,recipe.position[2]])
                .setScale([.006,.09,.006]).setRotation([0,0,side*.35])
                .setEmission(2.5).setRenderOptions({castsShadow:false,receivesShadow:false}).removeAfter(.24);
            launchGlints.push(glint);
        }
    }
    return {spawned:recipes.length,live:flying.length};
};
/** @param {import('./wave-plan.js').WaveConfig} config */
export function spawnWave(config=arenaTools.config) {
    if(performanceTest?.running)throw new Error('Probíhá srovnávací test.');
    if(engine.xr.roundState?.finished)throw new Error('Kolo skončilo. Spusť další kolo.');
    if(engine.xr.isPresenting&&launchTimer&&arenaTools.config.program==='minute')throw new Error('Minutová scéna právě řídí výstřely. Pro vlastní vlny zvol Volné vlny.');
    const report=spawnRecipes(planWave(config,waveIndex));
    if(report.spawned>0)waveIndex++;
    return {...report,formation:config.formation};
}
/** One authoring timer follows Rust's round clock. Hidden/paused XR never causes a wall-clock burst. */
const tickProgram=()=>{
    const round=engine.xr.roundState;
    if(!round||round.finished||performanceTest?.running)return;
    const elapsed=Math.max(0,round.duration-round.remaining);
    if(arenaTools.config.program==='minute'){
        while(nextCueIndex<minutePlan.cues.length&&minutePlan.cues[nextCueIndex].at<=elapsed+.001){
            const index=nextCueIndex++,cue=minutePlan.cues[index];
            if(elapsed-cue.at>.4)continue;
            showPhase(cue.label);
            const report=spawnRecipes(planCue(cue,arenaTools.config,index),!!cue.launch);
            cueHistory.push({index,at:elapsed,label:cue.label,spawned:report.spawned,live:report.live});
            status.textContent=`${cue.label} · ${Math.ceil(round.remaining)} s`;
        }
    }else if(elapsed>=nextPracticeAt){
        spawnWave();nextPracticeAt=elapsed+arenaTools.config.intervalMs/1000;
    }
};
/** @param {number|undefined} seconds */
export function previewMinute(seconds) {
    if(engine.xr.isPresenting||performanceTest?.running)throw new Error('Náhled je dostupný před vstupem do VR.');
    let index=seconds===undefined?(previewIndex+1)%minutePlan.cues.length:Math.max(0,minutePlan.cues.reduce((selected,cue,i)=>cue.at<=(seconds??0)?i:selected,0));
    if(seconds===undefined&&previewIndex>=0){
        const label=minutePlan.cues[previewIndex].label;
        while(minutePlan.cues[index].label===label){index=(index+1)%minutePlan.cues.length;if(index===previewIndex)break;}
    }
    previewIndex=index;
    for(const object of flying)if(object.exists)object.remove();flying=[];
    engine.scene.clearCutFeedback();clearLaunchGlints();
    const cue=minutePlan.cues[index];showPhase(cue.label,true);
    return {...spawnRecipes(planCue(cue,arenaTools.config,index),!!cue.launch),index,at:cue.at,label:cue.label};
}
export const programState=()=>({name:minutePlan.name,durationSeconds:minutePlan.durationSeconds,phase:phaseLabel,nextCueIndex,cues:cueHistory.map(cue=>({...cue})),totalCues:minutePlan.cues.length});
/** @param {import('./wave-plan.js').WaveConfig} previous */
const rescheduleWaves=previous=>{
    const wasRunning=launchTimer!==0;stopLaunches();
    if(previous.program!==arenaTools.config.program)showPhase('');
    const round=engine.xr.roundState;
    const resumed=!previous.automatic&&arenaTools.config.automatic;
    if((!wasRunning&&!resumed)||!round||round.finished||performanceTest?.running)return;
    const elapsed=round.duration-round.remaining;
    if(previous.program!==arenaTools.config.program||resumed){
        nextCueIndex=minutePlan.cues.findIndex(cue=>cue.at>=elapsed);
        if(nextCueIndex<0)nextCueIndex=minutePlan.cues.length;
        nextPracticeAt=elapsed;showPhase('');
    }
    if(arenaTools.config.automatic)launchTimer=window.setInterval(tickProgram,100);
};
const authoredPreset=await fetchPreset(new URL('./wave-preset.json',import.meta.url));
let presetStorage=null;try{presetStorage=window.localStorage;}catch{/* Configuration still works without browser storage. */}
export const arenaTools=createArenaTools({
    state:()=>({shots:shot,cuts,round:engine.xr.roundState,live:flying.filter(object=>object.exists).length,
        presenting:engine.xr.isPresenting,launching:launchTimer!==0,program:programState(),benchmark:performanceTest?.running??false}),
    bank:hand=>{const banked=engine.xr.bankChain(hand);const round=engine.xr.roundState;const count=document.getElementById('cuts');if(count)count.textContent=String(round?.score??0);return {banked,round};},
    spawn:config=>spawnWave(config),preview:seconds=>previewMinute(seconds),
    start:()=>{if(performanceTest?.running)throw new Error('Probíhá srovnávací test.');startRound();},
    stop:()=>stopLaunches(),
    changed:previous=>{rescheduleWaves(previous);syncArenaPanel();}
},{preset:authoredPreset,storage:presetStorage});
const syncArenaPanel=installArenaPanel(arenaTools);
const disconnectArenaBridge=installArenaBridge(arenaTools);
// Public authoring tool; optional UI and MCP use the same validated commands.
Reflect.set(window,'arenaTools',arenaTools);

const benchmarkToggle=document.getElementById('benchmark');
const benchmarkResults=document.getElementById('benchmark-results');
const benchmarkDownload=document.getElementById('benchmark-download');
if(benchmarkToggle instanceof HTMLInputElement)benchmarkToggle.checked=parameters.get('benchmark')==='1';
/** @type {import('../../js/engine.js').SceneObject[]} */
let benchmarkObjects=[];
/** @type {import('../../js/engine.js').SceneObject|null} */
let benchmarkPanel=null;
let benchmarkReport='';
let benchmarkTimeout=0;
export const benchmarkState=()=>performanceTest?.snapshot()??null;
function benchmarkLabel(/** @type {string} */ title,/** @type {string[]} */ lines) {
    if(benchmarkPanel?.exists)benchmarkPanel.remove();
    benchmarkPanel=engine.scene.createTextPanel({title,lines,position:[100,-10000,100],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT}).setEmission(.7);
    if(engine.xr.isPresenting)player.bindHead(benchmarkPanel,{anchor:'feet',offset:VR_DEBUG_PANEL_OFFSET});
}
function startBenchmark(){
    const original={ssr:enableSSR,glow:enableGlow};
    const ssrDisabled=ssrToggle instanceof HTMLInputElement&&ssrToggle.disabled;
    if(ssrToggle instanceof HTMLInputElement)ssrToggle.disabled=true;
    stopLaunches();
    for(const object of [...flying,...previewPieces])if(object.exists)object.remove();
    flying=[];previewPieces=[];engine.scene.clearCutFeedback();
    engine.set_simulation_paused(true);
    benchmarkObjects=world.fruits.slice(0,6).map((fruit,i)=>createFruitMesh(engine.scene,fruit)
        .setPosition([(i%3-1)*.34,.6+Math.floor(i/3)*.38,-1.7-Math.floor(i/3)*.3])
        .setScale([FRUIT_SCALE,FRUIT_SCALE,FRUIT_SCALE]).setEmission(2.4)
        .setColorCutout(solidFruit?0:.25).setOpacity(solidFruit?FRUIT_OPACITY:1,{opaqueAbove:.25,variation:solidFruit?FRUIT_OPACITY_VARIATION:0,coverage:fruit.coverage??.5})
        .setRenderOptions({castsShadow:false,receivesShadow:false}));
    let lastPhase=-1;
    performanceTest=new PerformanceTest({
        applyPhase:phase=>{
            enableSSR=phase.ssr;enableGlow=phase.glow;
            engine.setNeonGlow(enableGlow?NEON_GLOW:{strength:0});
            engine.postprocessing.configureSSR({resolutionScale:/** @type {.5|1} */(phase.resolutionScale),receiverBounds:FLOOR_SSR_BOUNDS});
            if(enableSSR)engine.postprocessing.enable('ssr',FLOOR_SSR);else engine.postprocessing.disable('ssr');
            if(ssrToggle instanceof HTMLInputElement)ssrToggle.checked=enableSSR;
            showBuild();
        },
        onUpdate:snapshot=>{
            if(snapshot.status==='running'){
                if(snapshot.phaseIndex!==lastPhase){
                    lastPhase=snapshot.phaseIndex;
                    benchmarkLabel(`TEST ${snapshot.phaseIndex+1}/${PERFORMANCE_PHASES.length}`,['DRZ POHLED NA ODRAZY PRED SEBOU']);
                }
                const progress=`Test ${snapshot.phaseIndex+1}/${PERFORMANCE_PHASES.length}: ${snapshot.phase?.label}. ${snapshot.sampleCount}/${snapshot.samplePackets} vzorků. Drž stejný pohled na odrazy; nehýbej ovladači.`;
                if(status)status.textContent=progress;
                if(benchmarkResults){benchmarkResults.hidden=false;benchmarkResults.textContent=progress;}
            }
        },
        onFinish:snapshot=>{
            window.clearTimeout(benchmarkTimeout);benchmarkTimeout=0;
            enableSSR=original.ssr;enableGlow=original.glow;
            engine.setNeonGlow(enableGlow?NEON_GLOW:{strength:0});
            engine.postprocessing.configureSSR({resolutionScale:.5,receiverBounds:FLOOR_SSR_BOUNDS});
            if(enableSSR)engine.postprocessing.enable('ssr',FLOOR_SSR);else engine.postprocessing.disable('ssr');
            if(ssrToggle instanceof HTMLInputElement){ssrToggle.checked=enableSSR;ssrToggle.disabled=ssrDisabled;}
            showBuild();engine.set_simulation_paused(false);
            for(const object of benchmarkObjects)if(object.exists)object.remove();
            benchmarkObjects=[];
            if(benchmarkPanel?.exists)benchmarkPanel.remove();benchmarkPanel=null;syncVrInfo();
            const report={...snapshot,revision,production:releaseBuild,renderer:engine.renderer_backend(),profiling:engine.frameProfile().enabled,created:new Date().toISOString(),
                note:'FPS a medián/minimum oken po 180 snímcích; nejde o 1% low ani GPU čas. Pohled a pohyb hlavy nejsou uzamčené.'};
            benchmarkReport=JSON.stringify(report,null,2);
            reportDiagnostic('xr-benchmark-result',undefined,report);
            if(benchmarkDownload instanceof HTMLButtonElement)benchmarkDownload.hidden=false;
            if(benchmarkResults){
                benchmarkResults.replaceChildren();
                const heading=document.createElement('p');heading.textContent=snapshot.status==='complete'?'Test hotov. Nastavení obnoveno.':'Test přerušen: '+snapshot.reason;
                benchmarkResults.append(heading);
                const table=document.createElement('table');
                for(const cells of [['Fáze','FPS','Medián oken','Nejhorší okno','CPU ms'],...snapshot.results.map(r=>[r.label,r.fps.toFixed(1),r.windowFps.median.toFixed(1),r.windowFps.min.toFixed(1),r.cpuSubmitMs.toFixed(2)])]){
                    const row=table.insertRow();for(const value of cells)row.insertCell().textContent=value;
                }
                benchmarkResults.append(table);
                if(snapshot.results.length===PERFORMANCE_PHASES.length){
                    const drift=document.createElement('p');
                    const first=snapshot.results[0].fps,last=snapshot.results.at(-1)?.fps??first;
                    drift.textContent=`Kontrolní základ na konci: ${(100*(last/first-1)).toFixed(1)} % oproti začátku. Rozdíl může znamenat změnu pohledu nebo zahřátí zařízení. Medián se počítá z oken, nikoli jednotlivých snímků.`;
                    benchmarkResults.append(drift);
                }
            }
            if(snapshot.status==='complete'&&engine.xr.isPresenting)benchmarkLabel('TEST HOTOV',['VYSLEDKY JSOU PO VYSTUPU Z VR']);
        }
    });
    performanceTest.start();
    benchmarkTimeout=window.setTimeout(()=>performanceTest?.cancel('Bez dostatečných vzorků do 3 minut'),180000);
}
if(benchmarkDownload instanceof HTMLButtonElement)benchmarkDownload.onclick=()=>{
    const url=URL.createObjectURL(new Blob([benchmarkReport],{type:'application/json'}));
    const link=document.createElement('a');link.href=url;link.download=`xr-test-${engine.renderer_backend()}.json`;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
};
const parkStartButton = () => {
    world.restartButton.setPosition(RESTART_POSITION).setRotation([0,-Math.atan2(RESTART_POSITION[0],-RESTART_POSITION[2]),0]);
};
const showStartButton = () => {
    const head=engine.xr.player.head;
    const position=head.position??[0,1.55,0],matrix=head.matrix;
    const fx=matrix?-matrix[8]:0,fz=matrix?-matrix[10]:-1;
    const length=Math.hypot(fx,fz);
    const x=length>.01?fx/length:0,z=length>.01?fz/length:-1;
    world.restartButton.setPosition([position[0]+x*.95,Math.max(.85,position[1]-.35),position[2]+z*.95])
        .setRotation([0,Math.atan2(-x,-z),0]);
    engine.xr.bindButton(world.restartButton);
};
const startRound = () => {
    if(performanceTest?.running)return;
    parkStartButton();
    stopLaunches();
    for(const piece of previewPieces)if(piece.exists)piece.remove();
    previewPieces=[];
    engine.scene.clearCutFeedback();feedback=null;
    for (const apple of flying) if (apple.exists) apple.remove();
    flying = []; shot = 0; cuts = 0; waveIndex=0;nextCueIndex=0;previewIndex=-1;nextPracticeAt=0;cueHistory=[];showPhase('');
    engine.xr.startRound(minutePlan.durationSeconds);
    const count = document.getElementById('cuts'); if (count) count.textContent = '0';
    status.textContent = 'Kolo 60 s. Každý řez +10; bonus série se připisuje hned. Trigger uvolní zásobník.';
    tickProgram();
    if(arenaTools.config.automatic)launchTimer=window.setInterval(tickProgram,100);
};
document.getElementById('shoot')?.addEventListener('click', shootApple);
engine.xr.addEventListener('select', event => {
    if(performanceTest?.running)return;
    const {handedness,target}=/** @type {CustomEvent<{handedness:string,target:import('../../js/engine.js').SceneObject|null}>} */(event).detail;
    if (target?.id===world.restartButton.id) startRound();
    else if (!engine.xr.roundState?.finished && engine.xr.roundState?.mode==='chains' && (handedness==='left'||handedness==='right')) {
        arenaTools.call('arena_bank',{hand:handedness});
    } else if (handedness==='left' && !engine.xr.roundState?.finished && arenaTools.config.program==='practice') spawnWave();
});
engine.xr.addEventListener('buttonpress', () => startRound());
engine.xr.addEventListener('cut', event => {
    const report = /** @type {CustomEvent<import('../../js/engine.d.ts').XRCutReport>} */(event).detail;
    for (const piece of report.pieces) piece.setEmission(1.1);
    feedback=engine.scene.playCutFeedback(report.pieces,{mirrorText:!engine.xr.isPresenting,size:CUT_EFFECT_SIZE,scoreLabel:engine.xr.roundState?.mode==='chains'?'stock-total':undefined});
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
            : 'Připraveno. Sekej katanami v obou rukou. Neonové ovoce vystřeluje samo.'+(runtimeInfo.visible?' Statistiky jsou uvnitř VR vpravo u nohou.':' Statistiky ve VR jsou vypnuté.');
    } catch (error) { status.textContent = String(error); }
    reportDiagnostic('xr-ready',undefined,{available:!enter.disabled,hint:status.textContent});
};
enter.onclick = () => {
    enter.disabled = true;
    reportDiagnostic('xr-enter-click');
    void engine.xr.enter().catch(async error => { status.textContent = String(error); reportDiagnostic('xr-enter',error); enter.disabled = !(await engine.xr.isSupported().catch(()=>false)); });
};
exit.onclick = () => { void engine.xr.exit().catch(error => { status.textContent = String(error); }); };
engine.xr.addEventListener('firstframe', event => reportDiagnostic('xr-first-frame',undefined,{.../** @type {CustomEvent} */(event).detail,postprocessing:engine.postprocessing.stats(),profile:engine.frameProfile(),graph:engine.postprocessing.graph()}));
engine.xr.addEventListener('start', () => { document.body.classList.add('xr');syncVrInfo();reportDiagnostic('xr-start'); if(fruitStyle instanceof HTMLInputElement)fruitStyle.disabled=true; if(benchmarkToggle instanceof HTMLInputElement)benchmarkToggle.disabled=true; setPanelFacing(true); exit.hidden = false; if(benchmarkToggle instanceof HTMLInputElement&&benchmarkToggle.checked)startBenchmark();else startRound(); });
engine.xr.addEventListener('roundend', () => {
    stopLaunches();showPhase('');showStartButton();
    const count=document.getElementById('cuts');if(count)count.textContent=String(engine.xr.roundState?.score??0);
    world.apples.forEach((fruit,i)=>fruit.setPosition(world.previewPositions[i]));
    status.textContent = `Konec kola — skóre ${engine.xr.roundState?.score ?? 0}, trojic ${engine.xr.roundState?.triples ?? 0}. Před tebou je START — klepni mečem nebo namiř a stiskni trigger.`;
});
engine.xr.addEventListener('end', () => { document.body.classList.remove('xr');syncVrInfo();performanceTest?.cancel('VR ukončeno');if(benchmarkPanel?.exists)benchmarkPanel.remove();benchmarkPanel=null;if(benchmarkToggle instanceof HTMLInputElement)benchmarkToggle.disabled=false;if(fruitStyle instanceof HTMLInputElement)fruitStyle.disabled=false; parkStartButton();setPanelFacing(false); stopLaunches();showPhase(''); engine.scene.clearCutFeedback(); world.pointers.forEach(p=>p.setPosition([100,100,100])); exit.hidden = true; void ready(); home(); fit(); status.textContent = 'VR ukončeno. Můžete vstoupit znovu.'; });
engine.xr.addEventListener('error', () => { performanceTest?.cancel('Chyba XR');stopLaunches();showPhase('');status.textContent = `VR se zastavilo: ${engine.xr.lastError}`; reportDiagnostic('xr-frame',engine.xr.lastError); });
window.addEventListener('pagehide', () => { performanceTest?.cancel('Stránka zavřena');disconnectArenaBridge();stopLaunches(); window.removeEventListener('resize', fit); engine.dispose(); }, { once: true });
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
