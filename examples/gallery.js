import { examples } from './scenes.js';
const element=id=>document.getElementById(id);
const select=element('example'),status=element('status');
const query=new URLSearchParams(location.search);
let engine, message='', busy=false;
for(const example of examples) select.add(new Option(example.title,example.id));
select.value=examples.some(example=>example.id===query.get('scene'))?query.get('scene'):'mirror';
const selected=()=>examples.find(example=>example.id===select.value);
const describe=()=>element('description').textContent=selected().description;
describe();
function apply() {
    engine.set_render_effects(element('shadows').checked,element('mirror').checked);
    engine.set_output_mode(element('texture').checked?'texture':'canvas');
    engine.set_light_helpers_visible(element('helpers').checked);
    engine.set_grid_visible(element('grid').checked);
    engine.set_simulation_paused(element('pause').checked);
    engine.set_frustum_culling(element('culling').checked);
}
function load(example) {
    example.build(engine.scene);
    engine.set_camera_mode(example.camera);
    for(const key of ['shadows','mirror','grid','helpers'])element(key).checked=example[key];
    element('pause').checked=false;
    apply();
    message='';
    describe();
}
const fit=()=>{const bounds=element('gpu-canvas').getBoundingClientRect();engine?.resize(Math.round(bounds.width),Math.round(bounds.height));};
window.addEventListener('resize',fit);
select.onchange=()=>{describe();if(engine&&!busy)load(selected());};
for(const key of ['shadows','mirror','texture','helpers','grid','pause','culling'])element(key).onchange=apply;
element('start').onclick=async()=>{
    if(busy)return;
    busy=true;element('start').disabled=true;
    try {
        if(!engine) {
            status.textContent='Načítám Rust/WASM…';
            const {initEngine}=await import(`../js/engine.js?v=${Date.now()}`);
            engine=await initEngine(element('gpu-canvas'),{backend:query.get('backend')||'auto'});
            for(const input of document.querySelectorAll('input'))input.disabled=false;
            element('stop').disabled=false;element('verify').disabled=false;
            element('idle').hidden=true;
        }
        fit();load(selected());element('start').textContent='Obnovit ukázku';
    }catch(error){message=`FAIL: ${error.message}`;status.textContent=message;console.error(error);}
    finally{busy=false;element('start').disabled=false;}
};
// Navigating unloads the WASM instance, frame loop and GPU resources, not just animation.
element('stop').onclick=()=>{
    const params=new URLSearchParams(location.search);params.set('scene',select.value);
    location.replace(`${location.pathname}?${params}`);
};
const frames=async(n=12)=>{for(let i=0;i<n;i++)await new Promise(requestAnimationFrame);};
element('verify').onclick=async()=>{
    if(busy)return;
    busy=true;select.disabled=true;element('start').disabled=true;element('verify').disabled=true;
    for(const input of document.querySelectorAll('input'))input.disabled=true;
    const original=select.value, originalTexture=element('texture').checked;
    const assert=(ok,text)=>{if(!ok)throw new Error(text);};
    try {
        const counts=[3,6,11,5,10000];
        for(const [i,example] of examples.entries()) {
            select.value=example.id;load(example);await frames();
            assert(engine.scene.objectCount===counts[i],`${example.id}: object count`);
            assert(engine.scene.lightCount===(example.id==='culling'?4:2),`${example.id}: lights`);
            const effects=engine.effects_stats();
            if(example.shadows)assert(effects.shadowLightCount===2,`${example.id}: shadow maps`);
            engine.set_output_mode('texture');await frames(3);engine.set_output_mode('canvas');
            element('pause').checked=true;apply();await frames();
            const before=engine.effects_stats();await frames();const after=engine.effects_stats();
            assert(before.staticUpdates===after.staticUpdates && before.dynamicUpdates===after.dynamicUpdates && before.reflectionUpdates===after.reflectionUpdates,`${example.id}: idle cache`);
        }
        select.value=original;element('texture').checked=originalTexture;load(selected());
        message='PASS: všech 5 JS scén, světla, stíny, cache a výstup do textury';
    }catch(error){message=`FAIL: ${error.message}`;console.error(error);}
    finally{busy=false;select.disabled=false;element('start').disabled=false;element('verify').disabled=false;for(const input of document.querySelectorAll('input'))input.disabled=false;}
};
setInterval(()=>{
    if(!engine)return;
    const r=engine.renderer_stats(),e=engine.effects_stats();
    status.textContent=`${message}\n${engine.renderer_backend()} · ${r.fps.toFixed(0)} FPS\nViditelné: ${r.visible}/${r.total} · vyřazené: ${r.culled}\nDávky: ${r.drawCalls} · upload: ${r.uploadBytes} B\nStínující světla: ${e.shadowLightCount}\nStatické / dynamické mapy: ${e.staticUpdates} / ${e.dynamicUpdates}\nOdraz: ${e.reflectionWidth}×${e.reflectionHeight}\nPrůchody: ${e.passes}`;
},250);
