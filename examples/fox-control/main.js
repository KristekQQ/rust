import { initEngine } from '../../js/engine.js';
import { createScene } from './scene.js';
import { createGameInput } from './input.js';

const canvas = document.getElementById('gpu-canvas');
if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Canvas nebyl nalezen.');

const backend = new URLSearchParams(location.search).get('backend') === 'webgl' ? 'webgl' : 'auto';
export const engine = await initEngine(canvas, { backend });
/** @type {import('../../js/engine.js').ModelObject|null} */
export let fox = null;

/** @type {import('../../js/engine.js').CharacterOptions} */
const characterOptions = {
    speed: 2.5,
    runSpeed: 5,
    animations: { idle: 'Survey', walk: 'Walk', run: 'Run' },
    physics: { halfExtents: [0.4, 0.8, 0.8], gravity: 18, jumpSpeed: 7 },
};

/** @param {KeyboardEvent} event */
const reset = event => {
    if (event.code === 'F3' && !event.repeat) {
        event.preventDefault();
        debugOverlay = !debugOverlay;
        engine.setDebugOverlay(debugOverlay);
    }
    if (event.code === 'KeyR' && fox && document.pointerLockElement === canvas) {
        fox.setPosition([0, 0, 0]);
        engine.controlCharacter(fox, characterOptions);
    }
};
let debugOverlay = false;
document.addEventListener('keydown', reset);

// Konfigurace aplikace: žádná scéna se nevytváří uvnitř enginu nebo UI.
engine.set_render_effects(true, false);
engine.set_grid_visible(true);
engine.set_light_helpers_visible(false);
const gameInput = createGameInput(canvas, engine, document.getElementById('status'));

const fit = () => engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight));
fit();
window.addEventListener('resize', fit);
document.addEventListener('fullscreenchange', fit);
window.addEventListener('pagehide', () => {
    window.removeEventListener('resize', fit);
    document.removeEventListener('fullscreenchange', fit);
    document.removeEventListener('keydown', reset);
    gameInput.dispose();
    engine.dispose();
}, { once: true });

try {
    const world = await createScene(engine.scene);
    fox = world.fox;

    // Jednorázový příkaz. Pohyb, kamera a přepínání klipů se počítají v Rustu.
    engine.controlCharacter(fox, characterOptions);

    document.documentElement.dataset.rendererBackend = engine.renderer_backend();
    gameInput.enable();
} catch (error) {
    const status = document.getElementById('status');
    if (status) status.textContent = `Načtení selhalo: ${error}`;
    gameInput.dispose();
    engine.dispose();
    throw error;
}
