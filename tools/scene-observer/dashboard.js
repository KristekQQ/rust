const $=selector=>document.querySelector(selector);
const frame=$('#observer'),params=new URLSearchParams(location.search);
if(params.get('backend')==='webgl')frame.src+='&backend=webgl';
if(params.get('source')==='native'){
    frame.src+='&source=native';$('#mode').textContent='BRÝLE · ČEKÁ NA TRACKING';
    $('#notice').textContent='Skutečný stav scény z brýlí: ovoce, řezy, objekty a jejich pohyb. Čekám na nativní přenos.';
}
let worker=null,metrics={},lastMode='';
const wsUrl=new URL('/observer',location.href);wsUrl.protocol=location.protocol==='https:'?'wss:':'ws:';
$('#demo').onclick=()=>{
    if(params.get('source')==='native'){location.href='./?demo=1';return;}
    worker?.terminate();worker=new Worker(new URL('./demo-worker.js',import.meta.url),{type:'module',name:'observer-demo-source'});
    worker.onmessage=({data})=>{if(data.error)$('#error').textContent=data.error;};
    worker.postMessage({type:'start',url:wsUrl.href});$('#demo').textContent='Spustit znovu';$('#pause').disabled=false;$('#error').textContent='';
    $('#pause').textContent='Pozastavit zdroj';$('#pause').dataset.paused='0';
};
$('#pause').onclick=()=>{const value=$('#pause').dataset.paused!=='1';$('#pause').dataset.paused=value?'1':'0';worker?.postMessage({type:'pause',value});$('#pause').textContent=value?'Pokračovat':'Pozastavit zdroj';};
document.querySelectorAll('[data-camera]').forEach(button=>button.onclick=()=>{
    document.querySelectorAll('[data-camera]').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));
    frame.contentWindow.postMessage({type:'camera',camera:button.dataset.camera},location.origin);
});
$('#compare').onclick=()=>{const view=new URL('./view.html?camera=player',location.href);if(params.get('backend'))view.searchParams.set('backend',params.get('backend'));if(params.get('source')==='native')view.searchParams.set('source','native');window.open(view,'apollo-observer-second','width=960,height=640');};
window.addEventListener('message',event=>{
    if(event.origin!==location.origin||event.source!==frame.contentWindow)return;
    if(event.data.type==='error'){$('#error').textContent=event.data.error;return;}
    if(event.data.type!=='observer-metrics')return;
    metrics=event.data;
    const age=Number.isFinite(metrics.ageMs)?Math.max(0,Math.round(metrics.ageMs)):null;
    $('#age').innerHTML=(age===null?'—':age)+' <small>ms</small>';
    $('#latency-note').textContent=metrics.presentation==='paused'?'Pozastaveno · zobrazen poslední přijatý stav':metrics.presentation==='delayed'?'Přenos se zpozdil · obraz zůstává viditelný':metrics.replay?'Přehrávání uložené scény':`Stáří přijatého stavu · nejistota ±${Math.ceil(metrics.uncertaintyMs??0)} ms`;
    $('#connection').textContent=metrics.presentation==='paused'?'Pozastaveno · poslední stav':metrics.presentation==='delayed'?'Přenos má zpoždění':metrics.replay?'Záznam · vlastní kamera':'Scéna připojena';
    $('#rate').innerHTML=(metrics.kibPerSec??0).toFixed(0)+' <small>KiB/s</small>';
    if($('#fruit'))$('#fruit').textContent=metrics.state?.flying?.length??0; if($('#pieces'))$('#pieces').textContent=metrics.state?.fragments??0;
    $('#swords').textContent=metrics.state?.swords??0;$('#head').textContent=metrics.state?.head?1:0;
    const mode=metrics.presentation==='paused'?'POZASTAVENO':metrics.replay?'ZÁZNAM':metrics.mode==='demo'?'DEMO · RUST HRA':'ŽIVÁ SCÉNA';
    if(mode!==lastMode){$('#mode').textContent=mode;lastMode=mode;}
});
window.addEventListener('pagehide',()=>{worker?.postMessage({type:'stop'});worker?.terminate();},{once:true});
globalThis.observerDemo={get metrics(){return metrics;},get source(){return worker;},start:()=>$('#demo').click(),pause:()=>$('#pause').click()};
if(params.has('demo')&&params.get('source')!=='native')$('#demo').click();
