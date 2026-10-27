// Renderer-independent optional example diagnostics; no SDK import.
/** Adapter snapshot: timings in ms, resolution in actual target pixels. CPU keys
 * name measured scopes; GPU values come only from timestamp queries. Never infer
 * submitted triangles from an arbitrary index count. Omit unavailable counters.
 * @typedef {Object} RuntimeSnapshot
 * @property {string} backend
 * @property {string} [renderer]
 * @property {string} [mode]
 * @property {{width:number,height:number,dpr?:number,scope?:string}} [resolution]
 * @property {number} [fps]
 * @property {number} [frameMs]
 * @property {number} [frameP95Ms]
 * @property {Record<string,{meanMs:number,p95Ms:number}>} [cpu]
 * @property {{supported:boolean,available:boolean,passesMs?:Record<string,number>|null,ageFrames?:number|null,error?:string|null}} [gpu]
 * @property {number} [drawCalls]
 * @property {number} [triangles]
 * @property {number} [visible]
 * @property {number} [culled]
 * @property {number} [bvhNodes]
 * @property {number} [objectTests]
 * @property {number} [uploadBytes]
 * @property {number} [writeCalls]
 * @property {Record<string,number>} [physics]
 * @property {Record<string,number>} [cuts]
 */
const number=(/** @type {number|undefined|null} */ v,digits=2)=>typeof v==='number'&&Number.isFinite(v)?v.toFixed(digits):'nedostupné';
/** @param {RuntimeSnapshot} s @returns {Array<[string,string]>} */
export function formatSnapshot(s){
    /** @type {Array<[string,string]>} */
    const rows=[['Renderer',`${s.renderer??'Rust/WASM'} · ${s.backend} · ${s.mode??'desktop'}`]];
    if(s.resolution){const r=s.resolution;rows.push(['Rozlišení',`${r.width} × ${r.height}${r.scope?' '+r.scope:''} · DPR ${number(r.dpr)}`]);}
    rows.push(['Aplikace',`${number(s.fps,1)} FPS · ${number(s.frameMs)} ms · p95 ${number(s.frameP95Ms)} ms`]);
    if(!Object.keys(s.cpu??{}).length)rows.push(['CPU','profilování nedostupné']);
    for(const [scope,t] of Object.entries(s.cpu??{}).slice(0,16))rows.push([`CPU ${scope}`,`${number(t.meanMs)} / p95 ${number(t.p95Ms)} ms`]);
    if(s.gpu?.available&&s.gpu.passesMs){
        rows.push(['GPU vzorek',`stáří ${number(s.gpu.ageFrames,0)} snímků`]);
        for(const [pass,ms] of Object.entries(s.gpu.passesMs).slice(0,24))rows.push([`GPU ${pass}`,`${number(ms)} ms`]);
    }else rows.push(['GPU',s.gpu?.error??(s.gpu?.supported?'čeká na timestamp vzorek':'timestamp měření nedostupné')]);
    rows.push(['Draw calls / trojúhelníky',`${number(s.drawCalls,0)} / ${number(s.triangles,0)}`]);
    if(s.visible!==undefined||s.culled!==undefined)rows.push(['Viditelné / vyřazené',`${number(s.visible,0)} / ${number(s.culled,0)}`]);
    if(s.bvhNodes!==undefined||s.objectTests!==undefined)rows.push(['BVH uzly / testy',`${number(s.bvhNodes,0)} / ${number(s.objectTests,0)}`]);
    rows.push(['Upload bajty / zápisy',`${number(s.uploadBytes,0)} / ${number(s.writeCalls,0)}`]);
    for(const [kind,values] of [['Fyzika',s.physics],['Řezání',s.cuts]])if(values&&typeof values==='object')for(const [key,v] of Object.entries(values).slice(0,12))if(typeof v==='number')rows.push([`${kind} ${key}`,number(v)]);
    return rows;
}
let serial=0;
/** Shared DOM adapter; polls at most 4 Hz, with no RAF or keyboard handler.
 * @param {{readSnapshot:()=>RuntimeSnapshot,isPresenting?:()=>boolean,onVisibilityChange?:(visible:boolean)=>void,title?:string}} options */
export function mountRuntimeInfo({readSnapshot,isPresenting=()=>false,onVisibilityChange=()=>{},title='Informace o běhu'}){
    let visible=new URLSearchParams(location.search).get('info')!=='0',disposed=false;
    const root=document.createElement('aside'),button=document.createElement('button'),output=document.createElement('pre');
    root.dataset.runtimeInfo='';root.setAttribute('aria-label',title);
    root.style.cssText='position:fixed;right:8px;bottom:8px;z-index:10000;max-width:min(560px,calc(100vw - 16px));font:11px/1.4 monospace;color:#e0f7ff;background:#07131eee;border:1px solid #4d849b;border-radius:6px;padding:6px;pointer-events:none';
    button.type='button';button.style.pointerEvents='auto';button.style.cssText='color:inherit;background:#173347;border:1px solid #4d849b;border-radius:3px;cursor:pointer;padding:4px 8px;pointer-events:auto';
    output.style.cssText='margin:6px 0 0;max-height:45vh;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere';
    output.tabIndex=0;output.setAttribute('aria-label','Podrobné statistiky; posun šipkami');
    output.id=`runtime-info-${++serial}`;button.setAttribute('aria-controls',output.id);
    root.append(button,output);document.body.append(root);
    function refresh(){
        if(disposed)return;
        root.hidden=isPresenting();output.hidden=!visible;
        button.textContent=visible?'Skrýt informace':'Zobrazit informace';button.setAttribute('aria-expanded',String(visible));
        if(!visible||document.hidden)return;
        try{output.textContent=formatSnapshot(readSnapshot()).map(([label,value])=>`${label}: ${value}`).join('\n');}
        catch{output.textContent='Diagnostika není dostupná.';}
    }
    function setVisible(/** @type {boolean} */ value){visible=Boolean(value);onVisibilityChange(visible);refresh();}
    const click=()=>setVisible(!visible);button.addEventListener('click',click);
    const timer=setInterval(refresh,250);
    function dispose(){if(disposed)return;disposed=true;clearInterval(timer);button.removeEventListener('click',click);window.removeEventListener('pagehide',dispose);root.remove();}
    window.addEventListener('pagehide',dispose,{once:true});refresh();
    return{dispose,refresh,setVisible,get visible(){return visible;}};
}
