// Optional development-page diagnostics. No engine state or simulation lives here.
let installed=false;
const sent=new Set();
/** @param {string} stage @param {unknown} [error] @param {Record<string,unknown>} [details] */
export function reportDiagnostic(stage,error,details={}) {
    const detail=error instanceof Error ? {name:error.name,message:error.message,stack:error.stack} : {message:error===undefined?'':String(error)};
    const record={stage,...detail,details,page:location.pathname,backend:new URLSearchParams(location.search).get('backend')??'auto',
        renderer:document.documentElement.dataset.rendererBackend??null,secure:isSecureContext,
        webgpu:!!navigator.gpu,webxr:!!Reflect.get(navigator,'xr'),gpuBinding:typeof Reflect.get(globalThis,'XRGPUBinding'),userAgent:navigator.userAgent};
    const key=JSON.stringify([stage,detail.message]);
    if(sent.has(key)||sent.size>=20)return;
    sent.add(key);
    if(error!==undefined) {
        const status=document.getElementById('status');
        if(status)status.textContent=`Chyba (${stage}): ${detail.message}`;
    }
    // Same-origin local development server; hosted static pages may not offer this endpoint.
    void fetch('/__diagnostics',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(record),keepalive:true}).catch(()=>{});
}
export function installDiagnostics() {
    if(installed)return;
    installed=true;
    window.addEventListener('error',event=>reportDiagnostic('uncaught',event.error??event.message));
    window.addEventListener('unhandledrejection',event=>reportDiagnostic('unhandledrejection',event.reason));
    reportDiagnostic('page-load');
}

/** Native session probe, independent of engine/renderer capability checks. */
export async function probeWebGpuXR() {
    const xr=Reflect.get(navigator,'xr');
    reportDiagnostic('xr-probe-before',undefined,{interfaces:Object.getOwnPropertyNames(globalThis).filter(name=>/^XR.*(Binding|Layer)$/.test(name))});
    if(!xr)throw new Error('Prohlížeč neposkytuje WebXR.');
    // This is intentionally the first awaited operation in the click gesture.
    const session=await xr.requestSession('immersive-vr',{requiredFeatures:['webgpu'],optionalFeatures:['local-floor','layers']});
    try {
        reportDiagnostic('xr-probe-session',undefined,{enabledFeatures:Array.from(session.enabledFeatures??[]),gpuBinding:typeof Reflect.get(globalThis,'XRGPUBinding')});
        if(typeof Reflect.get(globalThis,'XRGPUBinding')!=='function')throw new Error('WebGPU VR relace vznikla, ale XRGPUBinding není dostupný ani uvnitř relace.');
        const adapter=await navigator.gpu.requestAdapter(/** @type {GPURequestAdapterOptions} */ ({xrCompatible:true}));
        if(!adapter)throw new Error('Prohlížeč neposkytl XR kompatibilní GPU adaptér.');
        const device=await adapter.requestDevice();
        let layer;
        try {
            const binding=new (Reflect.get(globalThis,'XRGPUBinding'))(session,device);
            layer=binding.createProjectionLayer({colorFormat:binding.getPreferredColorFormat(),depthStencilFormat:'depth32float',scaleFactor:.75});
            reportDiagnostic('xr-probe-layer',undefined,{colorFormat:binding.getPreferredColorFormat()});
        } finally {layer?.destroy?.();device.destroy();}
        return 'Nativní WebGPU VR relace i projekční vrstva fungují. Výsledek je uložen na Macu.';
    } catch(error) {
        reportDiagnostic('xr-probe-binding',error);
        throw error;
    } finally {await session.end();}
}
