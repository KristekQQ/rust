/** Transfer-only scheduling bridge; Rust owns geometry and simulation.
 * @param {import('../../js/engine.js').Scene} scene
 * @param {import('../../js/engine.d.ts').WebXRHost} xr */
export function installCutTransport(scene,xr){
 const worker=new Worker(new URL('./cut-worker.js',import.meta.url),{type:'module'});
 let stopped=false,frame=0;
 const inflight=new Map();
 const diagnostics={status:'starting',errors:0,lastError:'',messages:0,jobs:0,expired:0,late:0,onmessageMs:0,onmessageMaxMs:0,prepareMs:0,prepareMaxMs:0};
 Object.defineProperty(diagnostics,'inflight',{enumerable:true,get:()=>inflight.size});
 Object.assign(window,{rimCutTransportDiagnostics:diagnostics});
 /** @param {unknown} error */
 const fail=error=>{diagnostics.status='error';diagnostics.errors++;diagnostics.lastError=String(error).slice(0,2000);scene.rimCutFailed();inflight.clear();console.error(error);};
 worker.onmessage=event=>{
  const start=performance.now();
  try{
   if(stopped)return;
   if(event.data.ready){diagnostics.status='ready';return;}
   if(event.data.startupError){fail(event.data.startupError);return;}
   const result=event.data.result?new Uint8Array(event.data.result):null;
   const token=result&&result.byteLength>=4?new DataView(result.buffer,result.byteOffset,4).getUint32(0,true):event.data.token;
   // A late response cannot release a newer job's slot or revive an expired cut.
   if(!inflight.delete(token)){diagnostics.late++;return;}
   diagnostics.messages++;
   if(event.data.error){fail(event.data.error);}
   else if(result){scene.rimAcceptCut(result);diagnostics.status='ready';}
   else throw new Error('Missing cut worker result');
  }catch(error){fail(error);}
  finally{diagnostics.onmessageMs=performance.now()-start;diagnostics.onmessageMaxMs=Math.max(diagnostics.onmessageMaxMs,diagnostics.onmessageMs);}
 };
 worker.onerror=event=>{if(!stopped)fail(event.message||'Cut worker failed');};
 worker.onmessageerror=()=>{if(!stopped)fail('Cut worker message could not be decoded');};
 const pumpOnce=()=>{
  if(stopped)return;
  const now=performance.now();
  // Native per-enemy inactivity expiry also releases abandoned geometry. This
  // transport expiry prevents two lost worker replies from blocking future jobs.
  for(const [token,sent] of inflight)if(now-sent>10000){inflight.delete(token);diagnostics.expired++;}
  if(inflight.size>=2)return;
  const start=performance.now();
  try{
   const bytes=scene.rimTakeCutJob();
   if(bytes.length){const token=new DataView(bytes.buffer,bytes.byteOffset,4).getUint32(0,true);inflight.set(token,now);diagnostics.jobs++;worker.postMessage(bytes.buffer,[bytes.buffer]);}
  }catch(error){fail(error);}
  finally{diagnostics.prepareMs=performance.now()-start;diagnostics.prepareMaxMs=Math.max(diagnostics.prepareMaxMs,diagnostics.prepareMs);}
 };
 const desktop=()=>{frame=0;if(stopped||xr.isPresenting)return;pumpOnce();frame=requestAnimationFrame(desktop);};
 const start=()=>{cancelAnimationFrame(frame);frame=0;};
 const end=()=>{if(!stopped&&!frame)frame=requestAnimationFrame(desktop);};
 // XRSession frames keep running when document RAF is suspended in the headset.
 xr.addEventListener('frame',pumpOnce);xr.addEventListener('start',start);xr.addEventListener('end',end);
 end();
 return ()=>{stopped=true;cancelAnimationFrame(frame);xr.removeEventListener('frame',pumpOnce);xr.removeEventListener('start',start);xr.removeEventListener('end',end);worker.terminate();inflight.clear();scene.rimCutFailed();};
}
