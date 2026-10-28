// Inject in a separate diagnostic page only. Wrappers count calls that reach the
// actual browser API, never inferred Rust capacities. Unsupported APIs stay null.
export function installApiCounters(){
 let allocationLabelCount=0;
 const output={allocationEventsEnabled:false,allocationTotals:{},allocationLabels:{},allocationEvents:[],allocationEventLimit:50000,allocationEventOverflow:0,webgpu:null,webgl:null,webglContexts:[],errors:[]};window.renderApiCounters=output;
 const metrics=()=>({drawCalls:0,instances:0,primitives:0,triangles:0,lines:0,points:0,complete:true,incompleteReasons:[],topologies:{}});
 function incomplete(target,reason){target.complete=false;if(!target.incompleteReasons.includes(reason))target.incompleteReasons.push(reason);}
 function countPrimitives(target,topology,count,instances=1){target.drawCalls++;target.instances+=instances;const n=Math.max(0,Math.floor(count));let primitiveCount,kind;if(topology==='triangle-list'){primitiveCount=Math.floor(n/3);kind='triangles';}else if(topology==='triangle-strip'||topology==='triangle-fan'){primitiveCount=Math.max(0,n-2);kind='triangles';}else if(topology==='line-list'){primitiveCount=Math.floor(n/2);kind='lines';}else if(topology==='line-strip'){primitiveCount=Math.max(0,n-1);kind='lines';}else if(topology==='line-loop'){primitiveCount=n>=2?n:0;kind='lines';}else if(topology==='point-list'){primitiveCount=n;kind='points';}else{incomplete(target,'unknown topology: '+String(topology));return;}
  const submitted=primitiveCount*instances;target.primitives+=submitted;target[kind]+=submitted;target.topologies[topology]=(target.topologies[topology]||0)+submitted;
 }
 function countUnproven(target,instances,reason){target.drawCalls++;target.instances+=instances;incomplete(target,reason);}
 function wrap(prototype,name,counters,onSuccess,onBefore){if(!prototype||typeof prototype[name]!=='function'){counters[name]=null;return;}const original=prototype[name];counters[name]=0;try{prototype[name]=function(...args){let captured;try{captured=onBefore?.(args);}catch(e){output.errors.push(name+' instrumentation: '+e.message);}const result=original.apply(this,args);const succeeded=value=>{counters[name]++;
 if(['createBuffer','createBindGroup','bufferData'].includes(name)) {
  const bytes=name==='createBindGroup'?null:name==='createBuffer'?(args[0]?.size??null):captured?.allocationBytes??null;
  const label=typeof args[0]?.label==='string'?args[0].label.slice(0,160):null;
  const add=target=>{target.calls++;if(Number.isFinite(bytes))target.knownBytes+=bytes;else target.unknownSizeCalls++;};
  add(output.allocationTotals[name]??=( {calls:0,knownBytes:0,unknownSizeCalls:0} ));
  let key=name+':'+(label??'unlabeled');if(!output.allocationLabels[key]&&allocationLabelCount>=128)key='other';
  if(!output.allocationLabels[key]){output.allocationLabels[key]={calls:0,knownBytes:0,unknownSizeCalls:0};allocationLabelCount++;}add(output.allocationLabels[key]);
  if(output.allocationEventsEnabled){if(output.allocationEvents.length<output.allocationEventLimit)output.allocationEvents.push({atMs:performance.now(),api:name,bytes,label,scope:'actual browser API allocation call; GL createBuffer size unknown until bufferData'});else output.allocationEventOverflow++;}
 }
try{onSuccess?.call(this,args,value,captured);}catch(e){output.errors.push(name+' instrumentation: '+e.message);if(counters.submitted)incomplete(counters.submitted,name+' instrumentation failed');}return value;};if(result&&typeof result.then==='function')return result.then(succeeded);return succeeded(result);};}catch(e){counters[name]=null;output.errors.push(name+': '+e.message);}}
 if(typeof GPUDevice!=='undefined'){
 const c=output.webgpu={submitted:metrics()};const pipelineTopology=new WeakMap(),passTopology=new WeakMap();
 for(const name of ['createBuffer','createTexture','createBindGroup','createComputePipeline','createComputePipelineAsync','createCommandEncoder'])wrap(GPUDevice.prototype,name,c);
 for(const name of ['createRenderPipeline','createRenderPipelineAsync'])wrap(GPUDevice.prototype,name,c,function(args,pipeline,topology){if(pipeline&&typeof pipeline==='object')pipelineTopology.set(pipeline,topology);},args=>{const topology=args[0]?.primitive?.topology??'triangle-list';return typeof topology==='string'?topology:null;});
 for(const name of ['writeBuffer','writeTexture','submit'])wrap(globalThis.GPUQueue?.prototype,name,c);
 for(const name of ['beginRenderPass','beginComputePass','finish','copyBufferToBuffer','copyTextureToTexture','copyTextureToBuffer','resolveQuerySet'])wrap(globalThis.GPUCommandEncoder?.prototype,name,c);
 for(const name of ['dispatchWorkgroups','dispatchWorkgroupsIndirect'])wrap(globalThis.GPUComputePassEncoder?.prototype,name,c);
 const pass=globalThis.GPURenderPassEncoder?.prototype;
 wrap(pass,'setPipeline',c,function(args){if(args[0]&&pipelineTopology.has(args[0]))passTopology.set(this,pipelineTopology.get(args[0]));else passTopology.set(this,null);});
 for(const name of ['setBindGroup'])wrap(pass,name,c);
 wrap(pass,'draw',c,function(args){const count=Number(args[0]),instances=Number(args[1]??1),topology=passTopology.get(this);countPrimitives(c.submitted,topology,count,instances);if(topology===null||topology===undefined)incomplete(c.submitted,'draw without a known render pipeline topology');});
 wrap(pass,'drawIndexed',c,function(args){const count=Number(args[0]),instances=Number(args[1]??1),topology=passTopology.get(this);if(topology==='triangle-strip'||topology==='line-strip'){countUnproven(c.submitted,instances,'indexed strip may contain primitive-restart indices');return;}countPrimitives(c.submitted,topology,count,instances);if(topology===null||topology===undefined)incomplete(c.submitted,'drawIndexed without a known render pipeline topology');});
 for(const name of ['drawIndirect','drawIndexedIndirect'])wrap(pass,name,c,function(){incomplete(c.submitted,'indirect WebGPU draw');});
 wrap(pass,'executeBundles',c,function(){incomplete(c.submitted,'WebGPU render bundle execution');});
 }
 if(typeof HTMLCanvasElement!=='undefined'){
 const original=HTMLCanvasElement.prototype.getContext,seen=new WeakSet();try{HTMLCanvasElement.prototype.getContext=function(kind,...args){const context=original.call(this,kind,...args);if(context&&kind==='webgl2'&&!seen.has(context)){seen.add(context);try{const extension=context.getExtension('EXT_disjoint_timer_query_webgl2');output.webglContexts.push({kind,canvas:{width:this.width,height:this.height},attributes:context.getContextAttributes(),disjointTimerQueryWebgl2Supported:extension!==null,contextLost:context.isContextLost(),probeScope:'extension detection only; no queries issued and no GPU duration measured'});}catch(e){output.errors.push('WebGL context metadata: '+e.message);}}return context;};}catch(e){output.errors.push('getContext metadata wrapper: '+e.message);}
 }
 if(typeof WebGL2RenderingContext!=='undefined'){
 const c=output.webgl={submitted:metrics(),indexShadow:{retainedBytes:0,byteLimit:64*1024*1024,trackedBuffers:0,trackedVaos:0,overflowEvents:0}};const gl=WebGL2RenderingContext.prototype,modeNames=new Map();for(const name of ['POINTS','LINES','LINE_LOOP','LINE_STRIP','TRIANGLES','TRIANGLE_STRIP','TRIANGLE_FAN'])if(Number.isFinite(gl[name]))modeNames.set(gl[name],name.toLowerCase().replaceAll('_','-').replace(/^triangles$/,'triangle-list').replace(/^lines$/,'line-list').replace(/^points$/,'point-list'));
 const draw=(context,mode,count,instances)=>countPrimitives(c.submitted,modeNames.get(mode)||'unknown GLenum '+mode,count,instances);
 const maxShadowBytes=c.indexShadow.byteLimit,maxShadowBuffers=8192,defaultVao={};const states=new WeakMap();
 function stateFor(context){let state=states.get(context);if(!state){state={vao:defaultVao,vaoElements:new Map(),bindings:new Map(),buffers:new Map(),shadowBytes:0,overflowedBuffers:new WeakSet()};states.set(context,state);}return state;}
 function boundBuffer(context,state,target){return target===context.ELEMENT_ARRAY_BUFFER?state.vaoElements.get(state.vao)||null:state.bindings.get(target)||null;}
 function metadata(state,buffer){if(!buffer)return null;let meta=state.buffers.get(buffer);if(meta)return meta;if(state.buffers.size>=maxShadowBuffers){if(!state.overflowedBuffers.has(buffer)){state.overflowedBuffers.add(buffer);c.indexShadow.overflowEvents++;}return null;}meta={shadow:null,known:null,revision:0,proofCache:new Map(),overflowed:false};state.buffers.set(buffer,meta);c.indexShadow.trackedBuffers++;return meta;}
 function resetShadow(state,meta,payload,length,allKnown){const oldBytes=meta.shadow?meta.shadow.byteLength*2:0,needed=length*2;state.shadowBytes-=oldBytes;c.indexShadow.retainedBytes-=oldBytes;meta.shadowBytes=0;meta.revision++;meta.proofCache.clear();meta.overflowed=needed>maxShadowBytes||c.indexShadow.retainedBytes+needed>maxShadowBytes;
  if(meta.overflowed){meta.shadow=null;meta.known=null;c.indexShadow.overflowEvents++;return;}
  try{meta.shadow=new Uint8Array(length);meta.known=new Uint8Array(length);if(payload)meta.shadow.set(payload);if(allKnown)meta.known.fill(1);meta.shadowBytes=needed;state.shadowBytes+=needed;c.indexShadow.retainedBytes+=needed;}
  catch{meta.shadow=null;meta.known=null;meta.overflowed=true;c.indexShadow.overflowEvents++;}
 }
 function uploadView(source,offsetElements=0,lengthElements){if(ArrayBuffer.isView(source)){const bytesPerElement=source.BYTES_PER_ELEMENT||1;if((offsetElements||lengthElements!==undefined)&&!source.BYTES_PER_ELEMENT)return null;const start=offsetElements*bytesPerElement,totalElements=source.byteLength/bytesPerElement,elements=lengthElements===undefined||lengthElements===0?totalElements-offsetElements:lengthElements,byteLength=elements*bytesPerElement;if(!Number.isInteger(start)||!Number.isInteger(byteLength)||start<0||byteLength<0||start+byteLength>source.byteLength)return null;return new Uint8Array(source.buffer,source.byteOffset+start,byteLength);}
  if(source instanceof ArrayBuffer&&offsetElements===0&&lengthElements===undefined)return new Uint8Array(source);
  return null;
 }
 function trackBufferData(context,args){const state=stateFor(context),buffer=boundBuffer(context,state,args[0]);if(!buffer)return;const meta=metadata(state,buffer);if(!meta){incomplete(c.submitted,'WebGL index buffer metadata table overflow');return;}if(typeof args[1]==='number'){const size=args[1];if(!Number.isInteger(size)||size<0)return;resetShadow(state,meta,null,size,false);}
  else{const payload=uploadView(args[1],args[3]??0,args[4]);if(!payload){resetShadow(state,meta,null,0,false);return;}resetShadow(state,meta,payload,payload.byteLength,true);}
 }
 function trackBufferSubData(context,args){const state=stateFor(context),buffer=boundBuffer(context,state,args[0]);if(!buffer)return;const meta=metadata(state,buffer);if(!meta){incomplete(c.submitted,'WebGL index buffer metadata table overflow');return;}meta.revision++;meta.proofCache.clear();const payload=uploadView(args[2],args[3]??0,args[4]);if(meta.shadow&&payload){const start=args[1],end=start+payload.byteLength;if(Number.isInteger(start)&&start>=0&&end<=meta.shadow.byteLength){meta.shadow.set(payload,start);meta.known.fill(1,start,end);return;}}
  if(meta.known)meta.known.fill(0);
 }
 function invalidateCopiedRange(context,args){const state=stateFor(context),destination=boundBuffer(context,state,args[1]);if(!destination)return;const meta=metadata(state,destination);if(!meta){incomplete(c.submitted,'WebGL copy destination metadata table overflow');return;}meta.revision++;meta.proofCache.clear();const start=args[3],end=start+args[4];if(meta.known&&Number.isInteger(start)&&start>=0&&Number.isInteger(end)&&end<=meta.known.byteLength)meta.known.fill(0,start,end);else if(meta.known)meta.known.fill(0);}
 function proveNoRestart(context,state,offset,count,type){const buffer=state.vaoElements.get(state.vao)||null;if(!buffer)return {ok:false,reason:'WebGL indexed draw has no tracked element buffer'};const meta=state.buffers.get(buffer);if(!meta)return {ok:false,reason:'WebGL element buffer bytes were never recorded'};if(meta.overflowed)return {ok:false,reason:'WebGL element buffer shadow exceeded bounded memory'};const types=new Map([[context.UNSIGNED_BYTE,[1,0xff]],[context.UNSIGNED_SHORT,[2,0xffff]],[context.UNSIGNED_INT,[4,0xffffffff]]]),format=types.get(type);if(!format)return {ok:false,reason:'unsupported WebGL index type'};const [size,restart]=format,byteLength=count*size;if(!Number.isInteger(offset)||offset<0||!Number.isInteger(byteLength)||offset+byteLength>(meta.shadow?.byteLength??-1))return {ok:false,reason:'WebGL indexed byte range is unavailable'};const key=offset+':'+byteLength+':'+type,cached=meta.proofCache.get(key);if(cached&&cached.revision===meta.revision)return cached.result;let result;
  if(!meta.known||meta.known.subarray(offset,offset+byteLength).some(value=>value!==1))result={ok:false,reason:'WebGL indexed byte range contains unknown uploaded bytes'};
  else{let foundRestart=false;for(let i=offset;i<offset+byteLength;i+=size){let index;if(size===1)index=meta.shadow[i];else if(size===2)index=new Uint16Array(meta.shadow.buffer,i,1)[0];else index=new Uint32Array(meta.shadow.buffer,i,1)[0];if(index===restart){foundRestart=true;break;}}result=foundRestart?{ok:false,reason:'WebGL indexed byte range contains a fixed primitive-restart index'}:{ok:true};}
  if(meta.proofCache.size>=4096)meta.proofCache.clear();meta.proofCache.set(key,{revision:meta.revision,result});return result;
 }
 function indexedDraw(context,args,instances){const mode=args[0],count=args[1],type=args[2],offset=args[3],topology=modeNames.get(mode)||'unknown GLenum '+mode,proof=proveNoRestart(context,stateFor(context),offset,count,type);if(!proof.ok){countUnproven(c.submitted,instances,proof.reason);return;}countPrimitives(c.submitted,topology,count,instances);}
 wrap(gl,'bindBuffer',c,function(args){const state=stateFor(this),target=args[0],buffer=args[1]||null;if(target===this.ELEMENT_ARRAY_BUFFER){if(buffer){if(!state.vaoElements.has(state.vao))c.indexShadow.trackedVaos++;state.vaoElements.set(state.vao,buffer);}else if(state.vaoElements.delete(state.vao))c.indexShadow.trackedVaos--;}else state.bindings.set(target,buffer);});
 wrap(gl,'bindVertexArray',c,function(args){stateFor(this).vao=args[0]||defaultVao;});
 wrap(gl,'bufferData',c,function(args){trackBufferData(this,args);},args=>({allocationBytes:typeof args[1]==='number'?args[1]:uploadView(args[1],args[3]??0,args[4])?.byteLength??null}));wrap(gl,'bufferSubData',c,function(args){trackBufferSubData(this,args);});
 wrap(gl,'copyBufferSubData',c,function(args){invalidateCopiedRange(this,args);});
 wrap(gl,'deleteBuffer',c,function(args){const state=states.get(this),buffer=args[0];if(!state||!buffer)return;const meta=state.buffers.get(buffer);if(meta){const bytes=meta.shadow?meta.shadow.byteLength*2:0;state.shadowBytes-=bytes;c.indexShadow.retainedBytes-=bytes;state.buffers.delete(buffer);c.indexShadow.trackedBuffers--;}state.overflowedBuffers.delete(buffer);for(const [target,bound]of state.bindings)if(bound===buffer)state.bindings.delete(target);for(const [vao,bound]of state.vaoElements)if(bound===buffer){state.vaoElements.delete(vao);c.indexShadow.trackedVaos--;}});
 wrap(gl,'deleteVertexArray',c,function(args){const state=states.get(this),vao=args[0];if(!state||!vao)return;if(state.vaoElements.delete(vao))c.indexShadow.trackedVaos--;if(state.vao===vao)state.vao=defaultVao;});
 wrap(gl,'drawArrays',c,function(a){draw(this,a[0],a[2],1);});wrap(gl,'drawArraysInstanced',c,function(a){draw(this,a[0],a[2],Number(a[3]));});
 wrap(gl,'drawElements',c,function(a){indexedDraw(this,a,1);});wrap(gl,'drawElementsInstanced',c,function(a){indexedDraw(this,a,Number(a[4]));});
 for(const name of ['createBuffer','createTexture','createProgram','createShader','createVertexArray','texImage2D','texSubImage2D','createFramebuffer','bindFramebuffer','framebufferTexture2D','useProgram'])wrap(gl,name,c);
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
