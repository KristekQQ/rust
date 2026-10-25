// Optional example diagnostics: no rendering or simulation loop.
import {initEngine as createEngine} from '../../js/engine.js';
import {VR_DEBUG_PANEL_OFFSET,VR_DEBUG_PANEL_WIDTH,VR_DEBUG_PANEL_HEIGHT} from './vr-debug-panel.js';

import {mountRuntimeInfo} from './runtime-info-ui.js';
export {mountRuntimeInfo,formatSnapshot} from './runtime-info-ui.js';
/** @typedef {import('./runtime-info-ui.js').RuntimeSnapshot} RuntimeSnapshot */

/** @type {WeakMap<object,ReturnType<typeof mountRuntimeInfo>>} */
const installed=new WeakMap();
/** Idempotent explicit example installation, outside the core SDK.
 * Existing scene panels can be adopted without allocating a second native node.
 * @param {import('../../js/engine.d.ts').Engine} engine
 * @param {{panel?:import('../../js/engine.d.ts').SceneObject,toggle?:HTMLInputElement|null}} [options] */
export function installRuntimeInfo(engine,{panel:scenePanel,toggle=null}={}){
    const existing=installed.get(engine);if(existing)return existing;
    let panel=scenePanel??null;
    const xr=engine.xr;
    engine.setDebugOverlay(false);
    function hidePanel(){
        if(!panel)return;
        try{if(xr.player.head.object?.id===panel.id)xr.player.bindHead(null);}catch{/* Already disposed. */}
        try{panel.setPosition([100,-10000,100]);}catch{/* Scene reset. */}
    }
    function syncPanel(visible=true){
        if(!xr.isPresenting||!visible){hidePanel();return;}
        if(panel&&!panel.exists)panel=null;
        // Respect equipment or a temporary benchmark occupying the native slot.
        const attached=xr.player.head.object;
        if(attached&&attached.id!==panel?.id)return;
        panel??=engine.scene.createInfoPanel('diagnostics',{position:[100,-10000,100],width:VR_DEBUG_PANEL_WIDTH,height:VR_DEBUG_PANEL_HEIGHT});
        panel.setScale([1,1,1]).setRenderOptions({castsShadow:false,receivesShadow:false});
        xr.player.bindHead(panel,{anchor:'feet',offset:VR_DEBUG_PANEL_OFFSET});
    }
    /** @returns {RuntimeSnapshot} */
    function readSnapshot(){
        const debug=engine.debugStats(),profile=engine.frameProfile(),stats=engine.renderer_stats();
        const canvas=document.getElementById('gpu-canvas');
        return{backend:engine.renderer_backend(),renderer:'Rust/WASM',mode:xr.isPresenting?'XR aplikace':'desktop',
            resolution:canvas instanceof HTMLCanvasElement?{width:canvas.width,height:canvas.height,dpr:devicePixelRatio,scope:'canvas'}:undefined,
            fps:debug.fps,frameMs:debug.frameMs,frameP95Ms:debug.frameP95Ms,cpu:profile.cpu,gpu:profile.gpu,drawCalls:debug.drawCalls,
            visible:debug.visible,culled:debug.culled,bvhNodes:debug.bvhNodes,objectTests:debug.objectTests,uploadBytes:stats.uploadBytes,cuts:{celkem:debug.xrCuts}};
    }
    const controller=mountRuntimeInfo({readSnapshot,isPresenting:()=>xr.isPresenting,onVisibilityChange:syncPanel});
    if(toggle){toggle.checked=controller.visible;toggle.addEventListener('change',change);}
    function change(){if(toggle)controller.setVisible(toggle.checked);}
    const start=()=>{syncPanel(controller.visible);controller.refresh();},end=()=>{hidePanel();controller.refresh();};
    xr.addEventListener('start',start);xr.addEventListener('end',end);hidePanel();
    const disposeUI=controller.dispose;
    controller.dispose=()=>{hidePanel();try{panel?.remove();}catch{/* Already removed. */}panel=null;toggle?.removeEventListener('change',change);xr.removeEventListener('start',start);xr.removeEventListener('end',end);window.removeEventListener('pagehide',controller.dispose);disposeUI();installed.delete(engine);};
    window.addEventListener('pagehide',controller.dispose,{once:true});installed.set(engine,controller);return controller;
}
/** Same API with diagnostics cleanup; detailed profiling is opt-in; native FPS/counters stay available.
 * @param {HTMLCanvasElement} canvas @param {{backend?:'auto'|'webgl',profiling?:boolean}} [options] */
export async function initEngine(canvas,options={}){
    const query=new URLSearchParams(location.search);
    const profiling=query.get('info')!=='0'&&(options.profiling??query.get('profile')==='1');
    const engine=await createEngine(canvas,{...options,profiling});
    const wrapped=Object.freeze({...engine,dispose(){controller.dispose();engine.dispose();}});
    const controller=installRuntimeInfo(wrapped);
    return wrapped;
}
