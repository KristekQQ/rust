import {initEngine} from '../shared/runtime-info.js';
import {createScene} from './scene.js';

const canvas=document.getElementById('gpu-canvas');
if(!(canvas instanceof HTMLCanvasElement))throw new Error('Chybí plátno modelu.');
const parameters=new URLSearchParams(location.search);
export const engine=await initEngine(canvas,{backend:parameters.get('backend')==='webgl'?'webgl':'auto'});
engine.set_render_effects(false,false);
engine.set_grid_visible(false);
engine.set_light_helpers_visible(false);
engine.setBackgroundColor([.001,.0004,.002]);
engine.postprocessing.configureSSR({resolutionScale:.5,receiverBounds:{min:[-100,-.025,-100],max:[100,-.015,100]}});
engine.postprocessing.enable('ssr',{strength:.3});
engine.postprocessing.enable('bloom',{strength:.85});
export const front=()=>engine.setOrbitView([.1,1.36,6.6],[.25,1.76,0]);
export const rear=()=>engine.setOrbitView([-2.5,2.2,-6.5],[0,1.65,0]);
front();
const fit=()=>engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
fit();window.addEventListener('resize',fit);
window.addEventListener('pagehide',()=>{window.removeEventListener('resize',fit);engine.dispose();},{once:true});

/** @type {import('../../js/engine.js').ModelObject} */
export let robot;
let clip='Idle',paused=false,speed=1;
const status=document.getElementById('status');
try {
    ({robot}=await createScene(engine.scene));
    document.documentElement.dataset.rendererBackend=engine.renderer_backend();
    const play=()=>{robot.playAnimation(clip,{loop:true,speed}).pauseAnimation(paused);};
    document.querySelectorAll('button[data-clip]').forEach(button=>{
        if(!(button instanceof HTMLButtonElement))return;
        button.disabled=false;
        button.onclick=()=>{
            clip=button.dataset.clip??'Idle';paused=false;play();
            document.querySelectorAll('[data-clip]').forEach(item=>item.setAttribute('aria-pressed',String(item===button)));
            const pause=document.getElementById('pause');if(pause)pause.textContent='Pozastavit';
        };
    });
    const pause=document.getElementById('pause'),frontButton=document.getElementById('front'),backButton=document.getElementById('back');
    if(pause instanceof HTMLButtonElement){pause.disabled=false;pause.onclick=()=>{paused=!paused;robot.pauseAnimation(paused);pause.textContent=paused?'Pokračovat':'Pozastavit';};}
    if(frontButton instanceof HTMLButtonElement){frontButton.disabled=false;frontButton.onclick=front;}
    if(backButton instanceof HTMLButtonElement){backButton.disabled=false;backButton.onclick=rear;}
    const speedControl=document.getElementById('speed');
    if(speedControl instanceof HTMLInputElement){speedControl.disabled=false;speedControl.oninput=()=>{speed=Number(speedControl.value);play();const value=document.getElementById('speed-value');if(value)value.textContent=`${speed}×`;};}
    if(status)status.textContent=`${robot.info.animations.length} animace v Rustu · ${engine.renderer_backend()==='Gl'?'WebGL':'WebGPU'} · plný prostorový GLB`;
    const verify=document.getElementById('verify');
    if(verify instanceof HTMLButtonElement){verify.disabled=false;verify.onclick=async()=>{
        const result=document.getElementById('result');
        try{const {verifyRobot}=await import('./verify.js');await verifyRobot(engine,robot,canvas,front,rear);if(result)result.textContent='PASS: GLB, tři animace v Rustu, přední i zadní geometrie a neonové materiály.';}
        catch(error){if(result)result.textContent=`FAIL: ${error}`;console.error(error);}
    };}
    if(parameters.has('verify'))document.querySelector('details')?.setAttribute('open','');
}catch(error){if(status)status.textContent=`Načtení selhalo: ${error}`;engine.dispose();throw error;}
