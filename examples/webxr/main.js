import { initEngine } from '../../js/engine.js';
import { createScene, launchApple } from './scene.js';

const canvas = document.getElementById('gpu-canvas');
const status = document.getElementById('status');
const enter = document.getElementById('enter');
const exit = document.getElementById('exit');
if (!(canvas instanceof HTMLCanvasElement) || !status || !(enter instanceof HTMLButtonElement) || !(exit instanceof HTMLButtonElement)) throw new Error('Chybí UI.');

// WebXR compositor uses WebGL; no desktop/WebGPU fallback is counted as VR.
export const engine = await initEngine(canvas, { backend: 'webgl' });
engine.set_render_effects(false, false);
engine.set_grid_visible(true);
engine.set_light_helpers_visible(false);
engine.setDebugOverlay(false);
engine.setCameraView([0, 1.8, 1.5], [0, 0.8, -1]);
export const world = createScene(engine.scene);
engine.xr.bindController('right', world.sword, {cutting: true, requireTrigger: false});
engine.xr.bindController('left', world.leftSword, {cutting: true, requireTrigger: false});
engine.xr.bindButton(world.restartButton);
engine.xr.addEventListener('buttonhover', event => {
    const {handedness,point}=/** @type {CustomEvent<{handedness:'left'|'right',point:import('../../js/engine.d.ts').Vector3|null}>} */(event).detail;
    world.pointers[handedness==='left'?0:1].setPosition(point ?? [100,100,100]);
});
let shot = 0, cuts = 0;
/** @type {import('../../js/engine.js').SceneObject[]} */
let flying = [];
let launchTimer = 0;
export function shootApple() {
    flying = flying.filter(apple => apple.exists);
    if (flying.length >= 6) return null;
    const apple = launchApple(engine.scene, world.geometry, shot++ % 2 === 0 ? -1 : 1);
    flying.push(apple);
    return apple;
}
export const gameState = () => ({shots: shot, cuts, round: engine.xr.roundState, liveApples: flying.filter(apple => apple.exists), launching: launchTimer !== 0});
const stopLaunches = () => { window.clearInterval(launchTimer); launchTimer = 0; };
const startRound = () => {
    stopLaunches();
    for (const apple of flying) if (apple.exists) apple.remove();
    flying = []; shot = 0; cuts = 0;
    engine.xr.startRound(30);
    const count = document.getElementById('cuts'); if (count) count.textContent = '0';
    status.textContent = 'Kolo 30 s. Sekej oběma katanami. Čas a skóre vidíš ve VR.';
    shootApple(); launchTimer = window.setInterval(shootApple, 2000);
};
document.getElementById('shoot')?.addEventListener('click', shootApple);
engine.xr.addEventListener('select', event => {
    const {handedness,target}=/** @type {CustomEvent<{handedness:string,target:import('../../js/engine.js').SceneObject|null}>} */(event).detail;
    if (target?.id===world.restartButton.id) startRound();
    else if (handedness==='left' && !engine.xr.roundState?.finished) shootApple();
});
engine.xr.addEventListener('buttonpress', () => startRound());
engine.xr.addEventListener('cut', event => {
    cuts += /** @type {CustomEvent<import('../../js/engine.d.ts').XRCutReport>} */(event).detail.removed.length;
    const count = document.getElementById('cuts'); if (count) count.textContent = String(cuts);
});
const fit = () => { if (!engine.xr.isPresenting) engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight)); };
fit();
window.addEventListener('resize', fit);
const ready = async () => {
    try {
        enter.disabled = !(await engine.xr.isSupported());
        status.textContent = enter.disabled
            ? (isSecureContext ? 'Náhled scény. Pro VR otevřete v Meta Quest Browser.' : 'Pro VR je potřeba důvěryhodné HTTPS nebo localhost přes USB/ADB.')
            : 'Připraveno. Sekej katanami v obou rukou. Jablka vystřelují sama, informace o scéně vidíš i ve VR.';
    } catch (error) { status.textContent = String(error); }
};
enter.onclick = () => {
    enter.disabled = true;
    void engine.xr.enter().catch(error => { status.textContent = String(error); enter.disabled = false; });
};
exit.onclick = () => { void engine.xr.exit().catch(error => { status.textContent = String(error); }); };
engine.xr.addEventListener('start', () => { exit.hidden = false; startRound(); });
engine.xr.addEventListener('roundend', () => {
    stopLaunches();
    status.textContent = `Konec kola — rozkrojeno ${cuts} plodů. Namiř na DALSI KOLO a stiskni trigger.`;
});
engine.xr.addEventListener('end', () => { stopLaunches(); world.pointers.forEach(p=>p.setPosition([100,100,100])); exit.hidden = true; enter.disabled = false; fit(); status.textContent = 'VR ukončeno. Můžete vstoupit znovu.'; });
engine.xr.addEventListener('error', () => { status.textContent = `VR se zastavilo: ${engine.xr.lastError}`; });
window.addEventListener('pagehide', () => { stopLaunches(); window.removeEventListener('resize', fit); engine.dispose(); }, { once: true });
await ready();
