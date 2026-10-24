/** Thin transferable-buffer bridge; native frame loop owns every simulation step.
 * @param {import('../../js/engine.js').Scene} scene */
export function installCutTransport(scene){
 const worker=new Worker(new URL('./cut-worker.js',import.meta.url),{type:'module'});
 let stopped=false,frame=0,inflight=0;
 worker.onmessage=event=>{inflight=Math.max(0,inflight-1);if(stopped)return;if(event.data.error){scene.rimCutFailed();console.error(event.data.error);}else scene.rimAcceptCut(new Uint8Array(event.data.result));};
 worker.onerror=()=>{if(!stopped)scene.rimCutFailed();inflight=0;};
 const pump=()=>{if(stopped)return;if(inflight<2){const bytes=scene.rimTakeCutJob();if(bytes.length){inflight++;worker.postMessage(bytes.buffer,[bytes.buffer]);}}frame=requestAnimationFrame(pump);};
 frame=requestAnimationFrame(pump);
 return ()=>{stopped=true;cancelAnimationFrame(frame);worker.terminate();};
}
