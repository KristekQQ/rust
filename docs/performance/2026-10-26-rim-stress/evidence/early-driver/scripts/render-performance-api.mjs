// Inject in a separate diagnostic page only. Wrappers count calls that reach the
// actual browser API, never inferred Rust capacities. Unsupported APIs stay null.
export function installApiCounters(){
 const output={webgpu:null,webgl:null,webglContexts:[],errors:[]};window.renderApiCounters=output;
 function wrap(prototype,name,counters){if(!prototype||typeof prototype[name]!=='function'){counters[name]=null;return;}const original=prototype[name];counters[name]=0;try{prototype[name]=function(...args){const result=original.apply(this,args);if(result&&typeof result.then==='function')return result.then(value=>{counters[name]++;return value;});counters[name]++;return result;};}catch(e){counters[name]=null;output.errors.push(name+': '+e.message);}}
 if(typeof GPUDevice!=='undefined'){
 const c=output.webgpu={};for(const name of ['createBuffer','createTexture','createBindGroup','createRenderPipeline','createRenderPipelineAsync','createComputePipeline','createComputePipelineAsync','createCommandEncoder'])wrap(GPUDevice.prototype,name,c);
 for(const name of ['writeBuffer','writeTexture','submit'])wrap(globalThis.GPUQueue?.prototype,name,c);
 for(const name of ['beginRenderPass','beginComputePass','finish','copyBufferToBuffer','copyTextureToTexture','copyTextureToBuffer','resolveQuerySet'])wrap(globalThis.GPUCommandEncoder?.prototype,name,c);
 for(const name of ['dispatchWorkgroups','dispatchWorkgroupsIndirect'])wrap(globalThis.GPUComputePassEncoder?.prototype,name,c);
 for(const name of ['draw','drawIndexed','drawIndirect','drawIndexedIndirect','setPipeline','setBindGroup'])wrap(globalThis.GPURenderPassEncoder?.prototype,name,c);
 }
 if(typeof HTMLCanvasElement!=='undefined'){
 const original=HTMLCanvasElement.prototype.getContext,seen=new WeakSet();try{HTMLCanvasElement.prototype.getContext=function(kind,...args){const context=original.call(this,kind,...args);if(context&&kind==='webgl2'&&!seen.has(context)){seen.add(context);try{const extension=context.getExtension('EXT_disjoint_timer_query_webgl2');output.webglContexts.push({kind,canvas:{width:this.width,height:this.height},attributes:context.getContextAttributes(),disjointTimerQueryWebgl2Supported:extension!==null,contextLost:context.isContextLost(),probeScope:'extension detection only; no queries issued and no GPU duration measured'});}catch(e){output.errors.push('WebGL context metadata: '+e.message);}}return context;};}catch(e){output.errors.push('getContext metadata wrapper: '+e.message);}
 }
 if(typeof WebGL2RenderingContext!=='undefined'){
 const c=output.webgl={};for(const name of ['drawArrays','drawElements','drawArraysInstanced','drawElementsInstanced','createBuffer','createTexture','createProgram','createShader','createVertexArray','bufferData','bufferSubData','texImage2D','texSubImage2D','createFramebuffer','bindFramebuffer','framebufferTexture2D','useProgram'])wrap(WebGL2RenderingContext.prototype,name,c);
 }
}

// Observers only: no GPU waits, query polling or extra rendering work. Installed
// before engine startup in both measured and separate diagnostic pages.
export function installRuntimeLossMonitor(){
 const losses=[];window.renderRuntimeLosses=losses;
 window.addEventListener('webglcontextlost',event=>losses.push({kind:'webgl-context-lost',atMs:performance.now(),message:event.statusMessage||null}),true);
 if(typeof GPUAdapter!=='undefined'&&typeof GPUAdapter.prototype.requestDevice==='function'){
 const original=GPUAdapter.prototype.requestDevice;
 GPUAdapter.prototype.requestDevice=function(...args){return original.apply(this,args).then(device=>{
 device.lost.then(info=>losses.push({kind:'webgpu-device-lost',atMs:performance.now(),reason:info.reason,message:info.message}));
 device.addEventListener('uncapturederror',event=>losses.push({kind:'webgpu-uncaptured-error',atMs:performance.now(),message:event.error?.message||String(event.error)}));return device;
 });};
 }
}
