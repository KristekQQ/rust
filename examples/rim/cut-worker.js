// Platform shim only. Geometry calculations run in this worker's Rust/WASM instance.
import init,{rim_split_geometry} from '../../pkg/webgpu_wasm.js';
const ready=init();
self.onmessage=async event=>{try{await ready;const result=rim_split_geometry(new Uint8Array(event.data));self.postMessage({result:result.buffer},{transfer:[result.buffer]});}catch(error){self.postMessage({error:String(error)});}};
