import {initEngine} from '../shared/runtime-info.js';
import {createScene} from './scene.js';
const canvas=document.querySelector('#gpu-canvas');
const status=document.querySelector('#status');
if(!(canvas instanceof HTMLCanvasElement)||!status)throw new Error('Preview UI unavailable');
export const engine=await initEngine(canvas,{backend:new URLSearchParams(location.search).get('backend')==='webgl'?'webgl':'auto'});
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_render_effects(false,false);
engine.set_camera_mode('orbit');
engine.setBackgroundColor([.001,.002,.006]);
const glow={strength:.4,radius:.045};engine.setNeonGlow(glow);
export const panels=createScene(engine.scene);
const front=()=>engine.setOrbitView([0,1.5,5.1],[0,1.5,0]);front();
const fit=()=>engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
fit();window.addEventListener('resize',fit);
window.addEventListener('pagehide',()=>{window.removeEventListener('resize',fit);engine.dispose();},{once:true});
document.querySelector('#front')?.addEventListener('click',front);
document.querySelector('#angle')?.addEventListener('click',()=>engine.setOrbitView([2.1,1.7,4.7],[0,1.5,0]));
const slider=document.querySelector('#glow');
if(slider instanceof HTMLInputElement)slider.addEventListener('input',()=>{
    glow.strength=Number(slider.value);engine.setNeonGlow(glow);
    const value=document.querySelector('#glow-value');if(value)value.textContent=glow.strength.toFixed(2);
});
status.textContent=`${engine.renderer_backend()==='BrowserWebGpu'?'WebGPU':'WebGL'} · 10 číslic · sdílené 3D modely`;
const verify=document.querySelector('#verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    const result=document.querySelector('#result');
    try{
        await (await import('./verify.js')).verifyFont(engine,canvas,glow);
        if(result)result.textContent='PASS: all ten instanced numeric glyphs, glow and continuous oblique core';
    }catch(error){if(result)result.textContent=`FAIL: ${error}`;}
};
