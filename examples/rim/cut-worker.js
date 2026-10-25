// Platform shim only. Geometry calculations run in this worker's Rust/WASM instance.
import init,{rim_split_geometry} from '../../pkg/webgpu_wasm.js';
// Match the main runtime: a normal page refresh must fetch current worker WASM.
const ready=init({module_or_path:new Request(new URL('../../pkg/webgpu_wasm_bg.wasm',import.meta.url),{cache:'no-store'})});
void ready.then(()=>self.postMessage({ready:true})).catch(error=>self.postMessage({startupError:String(error)}));
self.onmessage=async event=>{try{await ready;const result=rim_split_geometry(new Uint8Array(event.data));self.postMessage({result:result.buffer},{transfer:[result.buffer]});}catch(error){self.postMessage({error:String(error),token:new DataView(event.data).getUint32(0,true)});}};
