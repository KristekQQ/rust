import {initEngine} from '../shared/runtime-info.js';
import {createScene} from './scene.js';
const canvas=document.querySelector('#gpu-canvas');
const status=document.querySelector('#status');
const result=document.querySelector('#result');
if(!(canvas instanceof HTMLCanvasElement)||!status||!result)throw new Error('Chybí UI náhledu.');
const controls=[...document.querySelectorAll('button,input,select')];
for(const control of controls)if(control instanceof HTMLButtonElement||control instanceof HTMLInputElement||control instanceof HTMLSelectElement)control.disabled=true;
const parameters=new URLSearchParams(location.search);
export const engine=await initEngine(canvas,{backend:parameters.get('backend')==='webgl'?'webgl':'auto'});
let disposed=false;
let cleanupScene=()=>{};
let cleanupResize=()=>{};
const dispose=()=>{if(disposed)return;disposed=true;cleanupResize();cleanupScene();engine.dispose();};
window.addEventListener('pagehide',dispose,{once:true});
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_render_effects(false,false);
engine.set_camera_mode('orbit');engine.setBackgroundColor([.001,.002,.006]);
const glow={strength:.4,radius:.045};engine.setNeonGlow(glow);
async function loadPreview(){
    try{
        return await createScene(engine.scene);
    }catch(error){
        if(status)status.textContent='Načtení fontu selhalo.';if(result)result.textContent=`FAIL: ${error}`;dispose();throw error;
    }
}
export const preview=await loadPreview();
cleanupScene=()=>preview.dispose();
let angled=false;
const view=()=>{
    const width=Math.max(...preview.objects.map(text=>text.info.width));
    const aspect=canvas.clientWidth/Math.max(1,canvas.clientHeight);
    const distance=Math.max(5.6,(width+1)/(2*Math.tan(Math.PI/8)*aspect));
    engine.setOrbitView(angled?[distance*.44,1.8,distance*.9]:[0,1.5,distance],[0,1.5,0]);
};
const front=()=>{angled=false;view();};
const fit=()=>{engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));view();};
fit();window.addEventListener('resize',fit);
cleanupResize=()=>window.removeEventListener('resize',fit);
document.querySelector('#front')?.addEventListener('click',front);
document.querySelector('#angle')?.addEventListener('click',()=>{angled=true;view();});
const text=document.querySelector('#text');
if(text instanceof HTMLInputElement)text.addEventListener('input',()=>{
    try{preview.setText(text.value);view();text.setCustomValidity('');result.textContent='';}
    catch(error){text.setCustomValidity(String(error));result.textContent=String(error);}
});
const color=document.querySelector('#color');
if(color instanceof HTMLSelectElement)color.addEventListener('change',()=>preview.setColor(color.value==='magenta'?'magenta':'cyan'));
for(const id of ['glow','emission']){
    const input=document.querySelector(`#${id}`),output=document.querySelector(`#${id}-value`);
    if(input instanceof HTMLInputElement)input.addEventListener('input',()=>{
        const value=Number(input.value);if(output)output.textContent=value.toFixed(2);
        if(id==='glow'){glow.strength=value;engine.setNeonGlow(glow);}else preview.setEmission(value);
    });
}
const updateStatus=()=>{const info=preview.font.info;status.textContent=`${engine.renderer_backend()==='BrowserWebGpu'?'WebGPU':'WebGL'} · ${info.name} · ${info.characters.length} znaků · Rust layout a GPU instance`;};
const fontSelect=document.querySelector('#font');
if(fontSelect instanceof HTMLSelectElement)fontSelect.addEventListener('change',()=>{
    try{
        const emission=document.querySelector('#emission');
        preview.setFont(fontSelect.value==='pixel'?'pixel':'cynatar',emission instanceof HTMLInputElement?Number(emission.value):4);
        view();updateStatus();result.textContent='';
    }catch(error){fontSelect.value=preview.selected;result.textContent=String(error);}
});
updateStatus();
const verify=document.querySelector('#verify');
if(verify instanceof HTMLButtonElement)verify.onclick=async()=>{
    verify.disabled=true;result.textContent='Ověřování…';
    try{await (await import('./verify.js')).verifyFont(engine,canvas,preview,glow);result.textContent='PASS: Rust fonty · 32 glyphů / 1 draw · sdílené GPU meshe · stabilní cache · 3D hloubka, otvor O, boční emise a glow';}
    catch(error){result.textContent=`FAIL: ${error}`;}
    finally{verify.disabled=false;}
};

for(const control of controls)if(control instanceof HTMLButtonElement||control instanceof HTMLInputElement||control instanceof HTMLSelectElement)control.disabled=false;
