// Decision 2026-10-16: local tests found CPU cutting faster than hybrid/resident
// WebGPU compute for this workload. Keep auto=CPU; GPU modes are experiments.
// Future instances: read ./README.md before revisiting this; Quest 3 is unmeasured.
import {initEngine} from '../../js/engine.js';
import {createScene,createSword,BOARD_Y} from './scene.js';

const canvas=document.querySelector('#gpu-canvas');
const status=document.querySelector('#status');
if(!(canvas instanceof HTMLCanvasElement)||!(status instanceof HTMLElement))throw new Error('Chybí canvas nebo stav');
/** @param {string} id @returns {HTMLInputElement} */
function input(id){const element=document.getElementById(id);if(!(element instanceof HTMLInputElement))throw new Error(`Chybí ${id}`);return element;}
const angle=input('angle'),offset=input('offset'),depth=input('depth'),tilt=input('tilt'),guideToggle=input('guide');
const backend=new URLSearchParams(location.search).get('backend')==='webgl'?'webgl':'auto';
export const engine=await initEngine(canvas,{backend});
document.documentElement.dataset.rendererBackend=engine.renderer_backend();
for(const gpuChoice of document.querySelectorAll('#compute option[value^=gpu]'))if(gpuChoice instanceof HTMLOptionElement)gpuChoice.disabled=engine.renderer_backend()!=='BrowserWebGpu';
export const homeCamera=()=>engine.setOrbitView([7.5,8.5,12],[-.8,.65,-.6]);
homeCamera();engine.set_render_effects(true,false);engine.set_grid_visible(false);engine.set_light_helpers_visible(false);
/** @type {'held'|'plan'|'camera'} */
let mode='held';
export let objects=createScene(engine.scene,true);
/** @type {import('../../js/engine.d.ts').AsyncCutResult|null} */
export let lastCut=null;
let sceneEpoch=0;
let fruit=objects.apples.map(p=>p.apple);
let cuts=0,busyUntil=0,debugOverlay=false;
let previewTimer=0;
const settings={angle:0,x:0,z:0,tilt:0};
/** @type {null|{kind:'plan'|'held'|'camera',id:number,start:[number,number],last:[number,number]}} */
let gesture=null;
let validStroke=false;

function updateLabels(){
    for(const [id,value] of [['angle-value',`${Math.round(settings.angle)}°`],['tilt-value',`${Math.round(settings.tilt)}°`],['offset-value',settings.x.toFixed(1)],['depth-value',settings.z.toFixed(1)]]){const element=document.getElementById(id);if(element)element.textContent=value;}
}
function preview(){
    updateLabels();
    if(performance.now()<busyUntil)return null;
    return engine.scene.aimCutter(objects.sword,{center:[settings.x,1.15,settings.z],yaw:settings.angle*Math.PI/180+(mode==='held'?Math.PI:0),tilt:settings.tilt*Math.PI/180,height:3.6,floorY:BOARD_Y,guides:guideToggle.checked?objects.guides:[]});
}
function readControls(){settings.angle=Number(angle.value);settings.x=Number(offset.value);settings.z=Number(depth.value);settings.tilt=Number(tilt.value);preview();}
function syncControls(){angle.value=String(settings.angle);offset.value=String(settings.x);depth.value=String(settings.z);tilt.value=String(settings.tilt);}
function updateHud(/** @type {string} */ message){
    if(status)status.textContent=message;
    const count=document.querySelector('#pieces');if(count)count.textContent=String(fruit.length);
    const hits=document.querySelector('#cuts');if(hits)hits.textContent=String(cuts);
}
export function reset(){
    sceneEpoch++;
    cancelGesture();clearTimeout(previewTimer);objects=createScene(engine.scene,mode==='held');fruit=objects.apples.map(p=>p.apple);cuts=0;lastCut=null;busyUntil=0;
    settings.angle=0;settings.x=0;settings.z=0;settings.tilt=0;syncControls();preview();updateHud('Pět jablek připraveno. Vyber si, které rozřízneš.');
    if(!guideToggle.checked)for(const guide of objects.guides)guide.setScale([.00001,.00001,.00001]);
}
export async function slash(){
    const motion=preview();if(!motion)return null;
    const epoch=sceneEpoch;
    const choice=document.querySelector('#compute');
    const execution=choice instanceof HTMLSelectElement?/** @type {import('../../js/engine.d.ts').AsyncCutOptions['execution']} */(choice.value):'auto';
    busyUntil=Infinity;
    let report;
    try {report=await engine.scene.cutWithAsync(objects.sword,{motion,separation:.04,scatter:{floorY:BOARD_Y+.005,distance:.62,duration:.5},execution});}
    catch(error){if(epoch!==sceneEpoch)return null;busyUntil=0;throw error;}
    if(epoch!==sceneEpoch)return null;
    lastCut=report;busyUntil=performance.now()+620;
    clearTimeout(previewTimer);previewTimer=window.setTimeout(preview,630);
    if(report.removed.length){
        for(const apple of objects.apples)if(report.removed.includes(apple.apple.id)){if(apple.stem.exists)apple.stem.remove();if(apple.leaf.exists)apple.leaf.remove();}
        fruit=[...fruit.filter(p=>!report.removed.includes(p.id)),...report.pieces];cuts++;
        updateHud(`${fruit.length} kusů · ${report.execution==='webgpu-resident'?'WebGPU resident':report.execution==='webgpu-compute'?'WebGPU + CPU':'CPU / WASM'} · ${report.totalMs.toFixed(1)} ms${report.fallbackReason?` · fallback: ${report.fallbackReason}`:''}. Můžeš řezat dál.`);
    }else updateHud('Vedle nebo příliš krátká čepel. Zamiř zelenou čárou přes jablko.');
    return report;
}
async function attemptSlash(){try{await slash();}catch(error){updateHud(`Řez neprošel: ${error instanceof Error?error.message:String(error)}`);}}
/** @param {'held'|'plan'|'camera'} next */
function setMode(next){
    if(busyUntil===Infinity)return;
    cancelGesture();
    if((mode==='held')!==(next==='held')){objects.sword.remove();objects.sword=createSword(engine.scene,next==='held');busyUntil=0;}
    mode=next;
    for(const name of ['held','plan','camera'])document.querySelector(`#mode-${name}`)?.setAttribute('aria-pressed',String(name===mode));
    canvas?.setAttribute('data-mode',mode);preview();
    updateHud(mode==='held'?'Meč v ruce: pohybem myši zaměř, levým klikem sekni.':mode==='camera'?'Kamera: táhni pro otočení, kolečko přibližuje.':'Plánování: táhni přes jablko, puštěním proveď sek.');
}
for(const name of /** @type {const} */(['held','plan','camera']))document.querySelector(`#mode-${name}`)?.addEventListener('click',()=>setMode(name));
document.querySelector('#cut')?.addEventListener('click',attemptSlash);
document.querySelector('#reset')?.addEventListener('click',reset);
document.querySelector('#view-reset')?.addEventListener('click',homeCamera);
document.querySelector('#zoom-in')?.addEventListener('click',()=>engine.adjustOrbit(0,0,-160));
document.querySelector('#zoom-out')?.addEventListener('click',()=>engine.adjustOrbit(0,0,160));
for(const control of [angle,offset,depth,tilt])control.addEventListener('input',readControls);
guideToggle.addEventListener('change',()=>{if(!guideToggle.checked)for(const guide of objects.guides)guide.setScale([.00001,.00001,.00001]);preview();});

/** @param {PointerEvent} event @returns {[number,number]} */
function pointer(event){if(!canvas)throw new Error('Chybí canvas');const r=canvas.getBoundingClientRect();return [Math.min(1,Math.max(0,(event.clientX-r.left)/r.width)),Math.min(1,Math.max(0,(event.clientY-r.top)/r.height))];}
/** @param {[number,number]} screen */
function aimAt(screen){
    try{const p=engine.screenToPlane(screen,1.15);if(Math.abs(p[0])>5.1||p[2]<-4.3||p[2]>3.1)return false;settings.x=Math.max(-4.3,Math.min(4.3,p[0]));settings.z=Math.max(-3.5,Math.min(2.8,p[2]));syncControls();preview();return true;}catch{return false;}
}
// Capture all pointer downs before the engine's default LMB camera listener.
canvas.addEventListener('pointerdown',event=>{
    event.stopImmediatePropagation();
    if(!event.isPrimary||gesture||![0,2].includes(event.button))return;
    event.preventDefault();canvas.focus({preventScroll:true});
    const kind=event.button===2||event.shiftKey||mode==='camera'?'camera':mode;
    if(kind!=='camera'&&performance.now()<busyUntil)return;
    canvas.setPointerCapture(event.pointerId);gesture={kind,id:event.pointerId,start:pointer(event),last:[event.clientX,event.clientY]};validStroke=false;
    if(kind==='held'){if(aimAt(pointer(event)))attemptSlash();else updateHud('Zamiř na stůl; tady není platný řez.');}
},{capture:true});
canvas.addEventListener('pointermove',event=>{
    if(gesture){
        if(event.pointerId!==gesture.id)return;
        if(gesture.kind==='camera'){
            engine.adjustOrbit(event.clientX-gesture.last[0],event.clientY-gesture.last[1]);gesture.last=[event.clientX,event.clientY];return;
        }
        if(gesture.kind==='held')return;
        const end=pointer(event);if(Math.hypot(end[0]-gesture.start[0],end[1]-gesture.start[1])<.015)return;
        validStroke=false;
        try{
            const a=engine.screenToPlane(gesture.start,1.15),b=engine.screenToPlane(end,1.15);
            if([a,b].some(p=>Math.abs(p[0])>5.1||p[2]<-4.3||p[2]>3.1))return;
            settings.angle=Math.atan2(b[0]-a[0],b[2]-a[2])*180/Math.PI;
            settings.x=Math.max(-4.3,Math.min(4.3,(a[0]+b[0])/2));settings.z=Math.max(-3.5,Math.min(2.8,(a[2]+b[2])/2));syncControls();preview();validStroke=true;
        }catch{/* Camera is above the horizon. Keep last valid line. */}
    }else if(mode==='held'&&event.isPrimary&&performance.now()>=busyUntil)aimAt(pointer(event));
});
canvas.addEventListener('pointerup',event=>{
    if(!gesture||event.pointerId!==gesture.id)return;
    const old=gesture,end=pointer(event);gesture=null;
    if(canvas.hasPointerCapture(event.pointerId))canvas.releasePointerCapture(event.pointerId);
    if(old.kind==='plan'&&validStroke&&Math.hypot(end[0]-old.start[0],end[1]-old.start[1])>.04)attemptSlash();validStroke=false;
});
/** @param {Event} [event] */
function cancelGesture(event){
    if(event instanceof PointerEvent&&event.pointerId!==gesture?.id)return;
    const id=gesture?.id;gesture=null;validStroke=false;
    if(id!==undefined&&canvas instanceof HTMLCanvasElement&&canvas.hasPointerCapture(id))canvas.releasePointerCapture(id);
}
canvas.addEventListener('pointercancel',cancelGesture);canvas.addEventListener('lostpointercapture',cancelGesture);
const context=(/** @type {MouseEvent} */ event)=>event.preventDefault();canvas.addEventListener('contextmenu',context);
canvas.addEventListener('wheel',event=>{event.preventDefault();engine.adjustOrbit(0,0,Math.max(-500,Math.min(500,event.deltaY)));},{passive:false});
window.addEventListener('blur',cancelGesture);
const keydown=(/** @type {KeyboardEvent} */ event)=>{
    if(event.code==='F3'){event.preventDefault();if(!event.repeat){debugOverlay=!debugOverlay;engine.setDebugOverlay(debugOverlay);document.documentElement.dataset.debugOverlay=String(debugOverlay);}return;}
    if(event.target instanceof HTMLInputElement&&event.target.type!=='range'&&event.target.type!=='checkbox')return;
    if(event.code==='Space'){event.preventDefault();if(!event.repeat)attemptSlash();}
    if(event.code==='KeyR'&&!event.repeat)reset();
    if(event.code==='KeyV')homeCamera();
    if(event.code==='KeyH')setMode(mode==='held'?'plan':'held');
    if(event.code==='KeyC')setMode(mode==='camera'?'held':'camera');
    if(event.code==='KeyF'){if(document.fullscreenElement)void document.exitFullscreen();else void document.documentElement.requestFullscreen();}
};
window.addEventListener('keydown',keydown);
const hooks=/** @type {Window & {render_game_to_text?:()=>string}} */(window);
hooks.render_game_to_text=()=>JSON.stringify({mode,handVisible:mode==='held',coordinates:'world Y up; normalized mouse top-left; blade descends',pieces:fruit.filter(p=>p.exists).map(p=>({id:p.id,...p.meshInfo})),apples:objects.apples.map(p=>({id:p.apple.id,center:p.center,intact:p.apple.exists})),cuts,settings,camera:engine.cameraViewState(),dragging:gesture?.kind??null,busy:performance.now()<busyUntil,board:objects.board.exists,guideVisible:guideToggle.checked,guides:objects.guides.map(g=>g.meshInfo.position),lastCut:lastCut?{removed:lastCut.removed,created:lastCut.pieces.length,triangleTests:lastCut.triangleTests,execution:lastCut.execution,gpuMeshes:lastCut.gpuMeshes,totalMs:lastCut.totalMs,computeMs:lastCut.computeMs}:null});
const fit=()=>engine.resize(Math.max(1,canvas.clientWidth),Math.max(1,canvas.clientHeight));
window.addEventListener('resize',fit);
window.addEventListener('pagehide',()=>{cancelGesture();clearTimeout(previewTimer);window.removeEventListener('resize',fit);window.removeEventListener('keydown',keydown);window.removeEventListener('blur',cancelGesture);delete hooks.render_game_to_text;engine.dispose();},{once:true});
const controls=document.querySelector('#blade-controls');if(controls instanceof HTMLDetailsElement)controls.open=window.innerWidth>650;
preview();fit();updateHud('Pět jablek. Zamiř myší a levým klikem sekni; pravé tažení otáčí kamerou.');
