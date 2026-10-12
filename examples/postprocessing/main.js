import { initEngine } from '../../js/engine.js';
import { createScene } from './scene.js';
import { createUI } from './ui.js';
import { loadPuddleMask } from './texture.js';

const canvas = document.getElementById('gpu-canvas');
if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Chybí canvas');
const backend = new URLSearchParams(location.search).get('backend') === 'webgl' ? 'webgl' : 'auto';
const engine = await initEngine(canvas, { backend });
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
engine.set_render_effects(true, false);
await createScene(engine.scene);
await loadPuddleMask(engine);

// Add effects through the same SDK. Rust runs the ordered chain once per frame.
engine.postprocessing.enable('ssr', { strength: 0.85 });
engine.postprocessing.enable('godRays', { strength: 0.65 });
const ui = createUI(document.getElementById('ui'), engine);
const fit = () => engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight));
fit();
window.addEventListener('resize', fit);
window.addEventListener('pagehide', () => {
    ui.dispose();
    window.removeEventListener('resize', fit);
}, { once: true });
