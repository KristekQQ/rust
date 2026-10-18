import { initEngine } from '../../js/engine.js';
import { createScene, launchApple, previewSlice, CUT_EFFECT_SIZE } from './scene.js';

const canvas = document.getElementById('gpu-canvas');
const status = document.getElementById('status');
const enter = document.getElementById('enter');
const exit = document.getElementById('exit');
if (!(canvas instanceof HTMLCanvasElement) || !status || !(enter instanceof HTMLButtonElement) || !(exit instanceof HTMLButtonElement)) throw new Error('Chybí UI.');

const backend = new URLSearchParams(location.search).get('backend') === 'webgl' ? 'webgl' : 'auto';
export const engine = await initEngine(canvas, {backend});
document.documentElement.dataset.rendererBackend = engine.renderer_backend();
engine.set_render_effects(false, false);
engine.set_grid_visible(false);
engine.setBackgroundColor([0,0,0]);
engine.set_light_helpers_visible(false);
engine.setDebugOverlay(false);
const home = () => engine.setOrbitView([0,1.65,1.6],[-.25,1.35,-2]);
home();
export const world = await createScene(engine.scene);
// Desktop orbit and immersive eye views use opposite horizontal handedness.
// Keep this preview correction local to the example's world-space text.
const setPanelFacing = (/** @type {boolean} */ immersive) => {
    for (const panel of [world.infoPanel,world.scorePanel,world.restartButton,world.credits,world.logo])
        panel.setScale([immersive ? 1 : -1,1,1]);
    world.apples.forEach((apple,index)=>apple.setPosition(immersive?[100,100,100]:world.previewPositions[index]));
};
setPanelFacing(false);
document.querySelector('#reset')?.addEventListener('click',home);
const verify = document.querySelector('#verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    const result=document.querySelector('#result');
    try{
        if(result)result.textContent='RUNNING: pixelové kontroly';
        const {verifyArena}=await import('./verify.js');
        await verifyArena(engine,world,canvas);
        if(result)result.textContent='PASS: černé pozadí, ovoce, bitmapové panely a žádný postprocessing';
    }catch(error){if(result)result.textContent=`FAIL: ${error}`;}
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
/** @param {number} [size] */
export function previewCut(size=CUT_EFFECT_SIZE) {
    const result=previewSlice(engine.scene,world.fruits.find(f=>f.name==='watermelon')??world.fruits[0],size);
    feedback=result.feedback;return result;
}
document.getElementById('slice-preview')?.addEventListener('click',()=>{if(!engine.xr.isPresenting)previewCut();});
/** @type {import('../../js/engine.js').SceneObject[]} */
let flying = [];
let launchTimer = 0;
export function shootApple() {
    flying = flying.filter(apple => apple.exists);
    if (flying.length >= 6) return null;
    const index = shot++;
    const apple = launchApple(engine.scene, world.playFruits[index % world.playFruits.length], index % 2 === 0 ? -1 : 1);
    flying.push(apple);
    return apple;
}
export const gameState = () => ({shots: shot, cuts, round: engine.xr.roundState, liveApples: flying.filter(apple => apple.exists), launching: launchTimer !== 0, feedback});
const stopLaunches = () => { window.clearInterval(launchTimer); launchTimer = 0; };
const startRound = () => {
    stopLaunches();
    engine.scene.clearCutFeedback();feedback=null;
    for (const apple of flying) if (apple.exists) apple.remove();
    flying = []; shot = 0; cuts = 0;
    if (engine.xr.isPresenting) world.apples.forEach(fruit=>fruit.setPosition([100,100,100]));
    engine.xr.startRound(30);
    const count = document.getElementById('cuts'); if (count) count.textContent = '0';
    status.textContent = 'Kolo 30 s. Sekej oběma katanami. Čas a skóre vidíš ve VR.';
    shootApple(); launchTimer = window.setInterval(shootApple, 850);
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
    const count = document.getElementById('cuts'); if (count) count.textContent = String(cuts);
});
const fit = () => { if (!engine.xr.isPresenting) engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight)); };
fit();
window.addEventListener('resize', fit);
const ready = async () => {
    try {
        enter.disabled = backend !== 'webgl' || !(await engine.xr.isSupported());
        status.textContent = enter.disabled
            ? (isSecureContext ? 'Náhled arény. Pro VR použij WebGL v Meta Quest Browser.' : 'Pro VR je potřeba důvěryhodné HTTPS nebo localhost přes USB/ADB.')
            : 'Připraveno. Sekej katanami v obou rukou. Neonové ovoce vystřeluje samo, informace o scéně vidíš i ve VR.';
    } catch (error) { status.textContent = String(error); }
};
enter.onclick = () => {
    enter.disabled = true;
    void engine.xr.enter().catch(error => { status.textContent = String(error); enter.disabled = false; });
};
exit.onclick = () => { void engine.xr.exit().catch(error => { status.textContent = String(error); }); };
engine.xr.addEventListener('start', () => { setPanelFacing(true); exit.hidden = false; startRound(); });
engine.xr.addEventListener('roundend', () => {
    stopLaunches();
    world.apples.forEach((fruit,i)=>fruit.setPosition(world.previewPositions[i]));
    status.textContent = `Konec kola — rozkrojeno ${cuts} plodů. Namiř na DALSI KOLO a stiskni trigger.`;
});
engine.xr.addEventListener('end', () => { setPanelFacing(false); stopLaunches(); engine.scene.clearCutFeedback(); world.pointers.forEach(p=>p.setPosition([100,100,100])); exit.hidden = true; enter.disabled = backend !== 'webgl'; home(); fit(); status.textContent = 'VR ukončeno. Můžete vstoupit znovu.'; });
engine.xr.addEventListener('error', () => { status.textContent = `VR se zastavilo: ${engine.xr.lastError}`; });
window.addEventListener('pagehide', () => { stopLaunches(); window.removeEventListener('resize', fit); engine.dispose(); }, { once: true });
await ready();
