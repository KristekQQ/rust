import {WireCache} from './protocol.mjs';
import {SimulatedDelivery} from './network-demo-transport.mjs';
const $=s=>document.querySelector(s),params=new URLSearchParams(location.search);
const frames=[$('#raw'),$('#smooth-view')],ready=new Set(),receipts=[];
const cache=new WireCache(),peer={sequence:0,known:[new Map(),new Map(),new Map()]};
let manifest,recording,network=new SimulatedDelivery(),start=0,nextSample=0,ingested=-1,lastReceived=0,packets=0,sent=0,coalesced=0,packetBytes=0,started=false;
let hiddenAt=null,hiddenMs=0,clock=0;
const config=()=>({hz:Number($('#hz').value),delay:Number($('#delay').value),jitter:Number($('#jitter').value),enabled:$('#smooth').checked});
function configure(){
 const c=config();network.delay=c.delay;network.jitter=c.jitter;network.queue=[];network.lastDue=clock;nextSample=clock;
 $('#delay-value').textContent=c.delay+' ms';$('#jitter-value').textContent='±'+c.jitter+' ms';
 $('h1').textContent=c.hz===4?'Jen čtyři aktualizace za sekundu.':c.hz+' aktualizací za sekundu.';
 for(const f of frames)if(ready.has(f))f.contentWindow.postMessage({type:'demo-config',enabled:c.enabled,duration:1000/c.hz},location.origin);
}
for(const id of ['#hz','#delay','#jitter','#smooth'])$(id).addEventListener('input',configure);
$('#outage').onclick=()=>network.stall(clock,2000);
window.addEventListener('message',event=>{
 if(event.origin!==location.origin)return;
 const f=frames.find(f=>event.source===f.contentWindow);if(!f)return;
 if(event.data.type==='demo-error')fail(event.data.error);
 if(event.data.type==='demo-ready'){
  ready.add(f);document.documentElement.dataset.rendererBackend=event.data.backend;configure();
  if(ready.size===2&&!started){started=true;start=performance.now();requestAnimationFrame(tick);if(params.has('verify'))void verify();}
 }
});
document.addEventListener('visibilitychange',()=>{if(document.hidden)hiddenAt=performance.now();else if(hiddenAt!==null){hiddenMs+=performance.now()-hiddenAt;hiddenAt=null;}});
function sourceThrough(target){
 for(let i=ingested+1;i<=target;i++){
  const entry=manifest.frames[i%manifest.frames.length],wire=new Uint8Array(recording,entry.offset,entry.length).slice();
  new DataView(wire.buffer).setBigUint64(8,BigInt(i+1),true);cache.ingest(wire);
 }
 ingested=target;return cache.packetFor(peer);
}
function tick(now){
 try{
  if(!document.hidden){
   clock=now-start-hiddenMs;const c=config(),period=1000/c.hz;
   if(clock>=nextSample){network.enqueue(Math.floor(clock*manifest.sourceHz/1000),clock,Date.now());sent++;nextSample=Math.max(nextSample+period,clock+period*.1);}
   const delivery=network.receive(clock);
   if(delivery){
    const wire=sourceThrough(delivery.frame);packetBytes=wire.length;packets++;coalesced+=delivery.coalesced;lastReceived=clock;
    receipts.push({at:clock,latency:Date.now()-delivery.sampledAt});if(receipts.length>24)receipts.shift();
    const sourceFrame=delivery.frame%manifest.frames.length;
    const liveCuts=manifest.cuts.filter(c=>c.frame<=sourceFrame&&c.pieces.every(id=>cache.assets[0].has(id)));
    const cut=liveCuts.at(-1);
    $('#cuts').textContent='Řezy ve scéně: '+manifest.cuts.filter(c=>c.frame<=sourceFrame).length+' · skutečná geometrie půlek';
    for(const f of frames)f.contentWindow.postMessage({type:'demo-frame',wire,cut,sampledAt:delivery.sampledAt},location.origin);
   }
   const secondsLeft=Math.max(0,(network.stallUntil-clock)/1000);
   $('#connection').textContent=secondsLeft>0?'Simulovaný výpadek · '+secondsLeft.toFixed(1)+' s':packets?'Simulace běží · stejné pakety pro oba pohledy':'Čekám na první zpožděný paket…';
   const observed=receipts.length>1?(receipts.length-1)*1000/(receipts.at(-1).at-receipts[0].at):0;
   const latency=receipts.length?receipts.reduce((n,v)=>n+v.latency,0)/receipts.length:0;
   $('#stats').textContent=c.hz+' pakety/s nastaveno · '+observed.toFixed(1)+'/s přijato · zpoždění '+Math.round(latency)+' ms';
   document.documentElement.dataset.networkDemo=JSON.stringify({sent,packets,observedHz:observed,latencyMs:latency,pending:network.queue.length,coalesced,packetBytes,sourceHz:manifest.sourceHz});
  }
  requestAnimationFrame(tick);
 }catch(e){fail(e.message);}
}
function fail(message){$('#connection').textContent='Chyba: '+message;$('#result').hidden=false;$('#result').textContent='FAIL: '+message;console.error(message);}
const until=async fn=>{for(let i=0;i<800;i++){if(fn())return;await new Promise(r=>setTimeout(r,20));}throw Error('Test timeout');};
async function verify(){
 try{
  await until(()=>packets>=7);const next=packets+1;await until(()=>packets>=next);await new Promise(r=>setTimeout(r,25));
  const left=frames[0].contentWindow.networkDemoFrame,right=frames[1].contentWindow.networkDemoFrame;
  await until(()=>left.received>=packets&&right.received>=packets);
  const [a,b]=await Promise.all([left.capture(),right.capture()]);
  const {assertForeground,assertChanged}=await import('../../tests/pixels.js');
  if(left.cutChecks<2||right.cutChecks<2)throw Error('Actual cut halves and source removal were not verified in both renderers');
  assertForeground(a,'raw4Hz scene');assertForeground(b,'interpolated4Hz scene');assertChanged(a,b,'visible interpolation between identical packets');
  if(frames[0].contentDocument.documentElement.dataset.interpolationMs!=='0'||frames[1].contentDocument.documentElement.dataset.interpolationMs!=='250')throw Error('Wrong comparison configuration');
  if(receipts.slice(-6).some(r=>r.latency<140))throw Error('Configured150ms delay was not applied');
  const hz=(receipts.length-1)*1000/(receipts.at(-1).at-receipts[0].at);if(hz<3.2||hz>4.8)throw Error('Delivery is not4Hz: '+hz);
  network.stall(clock,2000);const before=packets;await new Promise(r=>setTimeout(r,700));if(packets!==before)throw Error('Simulated outage still delivers frames');
  await until(()=>packets>before);if(!coalesced)throw Error('Stall did not coalesce old updates');
  $('#result').textContent='PASS: same Rust scene at4Hz+150ms; real cut halves and parent removal; visible interpolation difference;2s outage and coherent recovery';
 }catch(e){fail(e.message);}
}
try{
 const [m,b]=await Promise.all([fetch('./fixtures/network-demo/manifest.json'),fetch('./fixtures/network-demo/frames.bin')]);
 if(!m.ok||!b.ok)throw Error('Testovací záznam není dostupný');manifest=await m.json();recording=await b.arrayBuffer();
 if(manifest.sourceHz!==60||manifest.frames.length!==manifest.sourceHz*manifest.durationSeconds)throw Error('Unexpected test recording');
 const backend=params.get('backend')==='webgl'?'webgl':'auto';
 frames[0].src='./network-demo-view.html?side=raw&backend='+backend;
 frames[1].src='./network-demo-view.html?side=smooth&backend='+backend;
}catch(e){fail(e.message);}
globalThis.networkDemo={get stats(){return {packets,sent,coalesced,pending:network.queue.length};},outage:()=>network.stall(clock,2000)};
