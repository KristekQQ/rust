import { initEngine } from '../shared/runtime-info.js';
import { createScene } from './scene.js';
// DOM controls are optional application tools; the engine has no UI dependency.
import { createUI } from './ui.js';
const canvas = document.getElementById('gpu-canvas');
if (!(canvas instanceof HTMLCanvasElement)) throw new Error('Canvas nebyl nalezen');
export const engine = await initEngine(canvas, {backend:new URLSearchParams(location.search).get('backend') === 'webgl' ? 'webgl' : 'auto'});
engine.set_camera_mode('free');
engine.setCameraView([6,4,-8],[0,1.6,0]);
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
engine.set_render_effects(true,false);
const fit = () => engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
fit(); window.addEventListener('resize',fit);
// Expose authored loading readiness to lifecycle/acceptance tools.
export let sceneReady=/** @type {Promise<unknown>|undefined} */(undefined);
createUI(engine,kind => {const loading=createScene(engine.scene,kind);sceneReady=loading;return loading;});
window.addEventListener('pagehide',()=>window.removeEventListener('resize',fit),{once:true});
