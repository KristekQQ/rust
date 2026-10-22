import {initEngine} from '../../js/engine.js';
import {createScene} from './scene.js';

const canvas = document.getElementById('gpu-canvas');
if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Canvas nebyl nalezen.');
const backend = new URLSearchParams(location.search).get('backend') === 'webgl' ? 'webgl' : 'auto';
export const engine = await initEngine(canvas, {backend});
engine.set_render_effects(false, false);
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
engine.setBackgroundColor([0.055, 0.085, 0.080]);
const front = () => engine.setOrbitView([3.1, 2.5, 6.5], [0.1, 1.35, 0]);
const back = () => engine.setOrbitView([-3.2, 2.4, -6], [0.1, 1.35, 0]);
front();
const fit = () => engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight));
fit();
window.addEventListener('resize', fit);
window.addEventListener('pagehide', () => {
    window.removeEventListener('resize', fit);
    engine.dispose();
}, {once: true});

/** @type {import('../../js/engine.js').ModelObject} */
export let monkey;
const status = document.getElementById('status');
try {
    ({monkey} = await createScene(engine.scene));
    document.documentElement.dataset.rendererBackend = engine.renderer_backend();
    let paused = false;
    for (const id of ['animation', 'front', 'back']) {
        const control = document.getElementById(id);
        if (control instanceof HTMLButtonElement) control.disabled = false;
    }
    const animation = document.getElementById('animation');
    if (animation) animation.onclick = () => {
        paused = !paused;
        monkey.pauseAnimation(paused);
        animation.textContent = paused ? 'Spustit mávání' : 'Pozastavit mávání';
    };
    const frontButton = document.getElementById('front');
    const backButton = document.getElementById('back');
    if (frontButton) frontButton.onclick = front;
    if (backButton) backButton.onclick = back;
    if (status) status.textContent = `GLB načteno · ${engine.renderer_backend()} · animace v Rustu`;
    // Verification is an optional DOM tool, outside the SDK and normal startup.
    const button = document.getElementById('verify');
    if (button) button.onclick = async () => {
        try {
            const {verify} = await import('./verify.js');
            await verify(engine, monkey, canvas, front);
        } catch (error) {
            const result = document.getElementById('result');
            if (result) result.textContent = `FAIL: ${error}`;
            console.error(error);
        }
    };
    if (new URLSearchParams(location.search).has('verify')) {
        document.querySelector('details')?.setAttribute('open', '');
    }
} catch (error) {
    if (status) status.textContent = `Načtení selhalo: ${error}`;
    const result = document.getElementById('result');
    if (result) result.textContent = `FAIL: ${error}`;
    engine.dispose();
    throw error;
}
