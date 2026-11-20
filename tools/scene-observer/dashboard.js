const $=selector=>document.querySelector(selector);
const frame=$('#observer'),params=new URLSearchParams(location.search);
const viewerOnly=params.get('viewer')==='1';
if(viewerOnly)document.querySelector('.actions').style.display='none';
if(params.get('backend')==='webgl')frame.src+='&backend=webgl';
if(params.get('source')==='native'){
    frame.src+='&source=native';$('#mode').textContent='BRÝLE';
    $('#notice').textContent='Skutečný stav scény z brýlí: ovoce, řezy, objekty a jejich pohyb. Čekám na nativní přenos.';
}
let worker=null,metrics={},lastMode='';
const stage=document.querySelector('.stage'),fullscreenButton=$('#fullscreen');
function fullscreenState(){
    const active=document.fullscreenElement===stage||stage.classList.contains('stage-expanded');
    fullscreenButton.setAttribute('aria-pressed',String(active));
    const label=active?'Ukončit celou obrazovku':'Celá obrazovka';
    fullscreenButton.title=label;fullscreenButton.setAttribute('aria-label',label);
    document.body.classList.toggle('observer-expanded',active);
}
function closeExpanded(){stage.classList.remove('stage-expanded');fullscreenState();}
fullscreenButton.onclick=async()=>{
    if(document.fullscreenElement===stage){await document.exitFullscreen();return;}
    if(stage.classList.contains('stage-expanded')){closeExpanded();return;}
    try{if(!stage.requestFullscreen)throw Error('Fullscreen unavailable');await stage.requestFullscreen();}
    catch{stage.classList.add('stage-expanded');}
    fullscreenState();
};
document.addEventListener('fullscreenchange',fullscreenState);
document.addEventListener('keydown',event=>{if(event.key==='Escape')closeExpanded();});
const wsUrl=new URL('/observer',location.href);wsUrl.protocol=location.protocol==='https:'?'wss:':'ws:';
$('#demo').onclick=()=>{
    if(viewerOnly)return;
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
const glowButton=$('#glow');
glowButton.onclick=()=>{
    const enabled=glowButton.getAttribute('aria-pressed')!=='true';
    glowButton.setAttribute('aria-pressed',String(enabled));
    frame.contentWindow.postMessage({type:'glow',enabled},location.origin);
};
$('#compare').onclick=()=>{const view=new URL('./view.html?camera=player',location.href);if(params.get('backend'))view.searchParams.set('backend',params.get('backend'));if(params.get('source')==='native')view.searchParams.set('source','native');window.open(view,'apollo-observer-second','width=960,height=640');};
window.addEventListener('message',event=>{
    if(event.origin!==location.origin||event.source!==frame.contentWindow)return;
    if(event.data.type==='observer-debug-capabilities'){
        let button=$('#debug-bounds-toggle');
        if(!event.data.bounds){button?.remove();return;}
        if(!button){button=document.createElement('button');button.id='debug-bounds-toggle';button.textContent='Debug AABB';button.setAttribute('aria-pressed','false');button.title='Hranice objektů, částí, glow a oblasti SSR. Jen debug sestavení.';
            button.onclick=()=>{const enabled=button.getAttribute('aria-pressed')!=='true';button.setAttribute('aria-pressed',String(enabled));frame.contentWindow.postMessage({type:'debug-bounds',enabled},location.origin);};
            $('#glow').before(button);
        }return;
    }
    if(event.data.type==='error'){$('#error').textContent=event.data.error;return;}
    if(event.data.type!=='observer-metrics')return;
    metrics=event.data;
    const age=Number.isFinite(metrics.ageMs)?Math.max(0,Math.round(metrics.ageMs)):null;
    $('#age').innerHTML=(age===null?'—':age)+' <small>ms</small>';
    const state=metrics.presentation??'waiting';
    const label=state==='paused'?'Přenos pozastaven · poslední zobrazený stav':state==='delayed'?'Zpoždění přenosu':metrics.replay?'Záznam':'Přenos v pořádku';
    const indicator=$('#connection');indicator.dataset.state=state;
    indicator.textContent=state==='paused'?'Ⅱ':state==='waiting'?'○':'●';
    indicator.title=label;indicator.setAttribute('aria-label',label);
    $('#age').title=`Nejistota ±${Math.ceil(metrics.uncertaintyMs??0)} ms`;
    $('#rate').innerHTML=(metrics.kibPerSec??0).toFixed(0)+' <small>KiB/s</small>';
    if($('#fruit'))$('#fruit').textContent=metrics.state?.flying?.length??0; if($('#pieces'))$('#pieces').textContent=metrics.state?.fragments??0;
    $('#swords').textContent=metrics.state?.swords??0;$('#head').textContent=metrics.state?.head?1:0;
    const mode=metrics.replay?'ZÁZNAM':metrics.mode==='demo'?'UKÁZKA':'BRÝLE';
    if(mode!==lastMode){$('#mode').textContent=mode;lastMode=mode;}
});
window.addEventListener('pagehide',()=>{worker?.postMessage({type:'stop'});worker?.terminate();},{once:true});
globalThis.observerDemo={get metrics(){return metrics;},get source(){return worker;},start:()=>$('#demo').click(),pause:()=>$('#pause').click()};
if(!viewerOnly&&params.has('demo')&&params.get('source')!=='native')$('#demo').click();
