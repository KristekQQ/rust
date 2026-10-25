// Shared diagnostics use one canonical module identity across examples and tools.
import {initEngine} from '../shared/runtime-info.js';
import { createScene } from './scene.js';
/** @type {typeof import('./ui.js')} */
const { createUI } = await import(new URL('./ui.js?v=window-rays', import.meta.url).href);
import { loadPuddleMask } from './texture.js';

const canvas = document.getElementById('gpu-canvas');
if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Chybí canvas');
const backend = new URLSearchParams(location.search).get('backend') === 'webgl' ? 'webgl' : 'auto';
export const engine = await initEngine(canvas, { backend });
const showRays = () => engine.setCameraView([6, 2.5, 2], [-12, 8, 0]);
showRays();
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
engine.set_render_effects(true, false);
await createScene(engine.scene);
await loadPuddleMask(engine);

// Add effects through the same SDK. Rust runs the ordered chain once per frame.
engine.postprocessing.enable('ssr', { strength: 0.85 });
engine.postprocessing.configureGodRays({ density: 0.055, exposure: 40 });
engine.postprocessing.enable('godRays', { strength: 0.8 });
const ui = createUI(document.getElementById('ui'), engine, { resetView: showRays });
const fit = () => engine.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight));
fit();
window.addEventListener('resize', fit);
window.addEventListener('pagehide', () => {
    ui.dispose();
    window.removeEventListener('resize', fit);
}, { once: true });
