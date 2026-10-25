/** @typedef {Record<string, unknown>} Data */
/** @typedef {{xr: EventTarget & {isPresenting?: boolean,lastError?: unknown},scene:{readonly rimState: unknown},renderer_backend:()=>string}} DiagnosticEngine */
/** @typedef {{build:string,wasmBuild:string,getTransport?:()=>unknown,send?:(record:Data)=>unknown,now?:()=>number,eventTarget?:EventTarget,maxSamples?:number,userAgent?:string}} DiagnosticOptions */

/** Only scalar diagnostic fields are copied: never poses, geometry or projectile arrays.
 * @param {unknown} value @param {string[]} keys @returns {Data} */
function pick(value,keys){
 const source=value&&typeof value==='object'?/** @type {Data} */(value):{};
 return Object.fromEntries(keys.filter(key=>['string','number','boolean'].includes(typeof source[key])||source[key]===null).map(key=>[key,source[key]]));
}
/** @param {unknown} value @returns {Data} */
function errorInfo(value){
 if(value instanceof Error)return {name:value.name,message:value.message.slice(0,2000)};
 return {message:String(value??'Unknown XR error').slice(0,2000)};
}
/** Observational, bounded same-origin logs. No RAF/timers or simulation decisions.
 * Inject send/now/eventTarget for offline tests; sender failures are deliberately isolated.
 * @param {DiagnosticEngine} engine @param {DiagnosticOptions} options @returns {()=>void} */
export function installRimDiagnostics(engine,options){
 const now=options.now??(()=>performance.now());
 const target=options.eventTarget??globalThis.window;
 const sender=options.send??(record=>fetch('/__diagnostics',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(record),keepalive:true}));
 const maxSamples=Math.min(120,Math.max(0,Math.floor(options.maxSamples??120)));
 const userAgent=options.userAgent??globalThis.navigator?.userAgent??'unknown';
 /** @type {Array<()=>void>} */const cleanup=[];
 const runId=globalThis.crypto?.randomUUID?.()??`${Date.now()}-${Math.random().toString(36).slice(2)}`;
 let disposed=false,session=0,samples=0,lastSample=-Infinity,lastFrame=-1,frames=0,gapTotal=0,gapMax=0;
 /** @type {Set<string>} */const errors=new Set();
 /** @param {string} stage @param {Data} [details] */
 const emit=(stage,details={})=>{
  if(disposed)return;
  try{
   const record={stage,build:options.build,wasmBuild:options.wasmBuild,backend:engine.renderer_backend(),userAgent,page:'/examples/rim/',runId,session,timeMs:now(),details};
   void Promise.resolve(sender(record)).catch(()=>{});
  }catch{/* Diagnostics must never interrupt rendering, even when fetch throws. */}
 };
 /** @param {string} stage @param {unknown} error */
 const reportError=(stage,error)=>{
  const info=errorInfo(error),key=stage+':'+info.message;
  if(errors.has(key)||errors.size>=20)return;
  errors.add(key);emit(stage,info);
 };
 /** @param {EventTarget|undefined} source @param {string} name @param {(event:Event)=>void} listener */
 const listen=(source,name,listener)=>{
  if(!source)return;
  source.addEventListener(name,listener);cleanup.push(()=>source.removeEventListener(name,listener));
 };
 listen(engine.xr,'start',()=>{
  session++;samples=0;lastSample=-Infinity;lastFrame=-1;frames=0;gapTotal=0;gapMax=0;errors.clear();emit('rim-xr-start');
 });
 listen(engine.xr,'frame',()=>{
  if(!engine.xr.isPresenting||disposed)return;
  const time=now();
  if(lastFrame>=0){const gap=Math.max(0,time-lastFrame);gapTotal+=gap;gapMax=Math.max(gapMax,gap);frames++;}
  lastFrame=time;
  if(samples>=maxSamples||time-lastSample<1000)return;
  lastSample=time;samples++;
  try{
   const state=/** @type {Data|null} */(engine.scene.rimState);
   const enemies=Array.isArray(state?.enemies)?state.enemies:[];
   emit('rim-xr-sample',{
    sample:samples,game:pick(state,['active','paused','health','dead','wave','kills','deaths','hitsTaken','deathMode','simDt']),
    frame:pick(state?.frame,['simulation','visual']),
    blade:pick(state?.blade,['contactSamples','hits','slowSamples','discontinuities','maxSpeedForFrame']),
    enemies:enemies.slice(0,64).map(enemy=>pick(enemy,['id','slug','hp','maxHp','phase'])),
    cuts:pick(state?.cuts,['queued','pending','staging','stagedMeshes','pieces','errors','preparedTriangles','stagedVertices','commits','stageMs','stageMaxMs']),
    transport:pick(options.getTransport?.(),['status','messages','jobs','expired','late','errors','inflight','pending','lastError','onmessageMs','onmessageMaxMs','prepareMs','prepareMaxMs']),
    cadence:{frames,frameMs:frames?gapTotal/frames:null,maxGapMs:gapMax},
   });
  }catch(error){reportError('rim-snapshot-error',error);}
  frames=0;gapTotal=0;gapMax=0;
 });
 for(const name of ['firstframe','performance'])listen(engine.xr,name,event=>{
  if(name==='performance'&&samples>=maxSamples)return;
  if(name==='performance')samples++;
  const detail=/** @type {CustomEvent<unknown>} */(event).detail;
  const source=detail&&typeof detail==='object'?/** @type {Data} */(detail):{};
  const summary=pick(detail,['sessionId','sample','frames','fps','frameMs','cpuSubmitMs','resolutionScale','frameRate','framebufferScaleFactor','fixedFoveation','antialias','layerKind','presentation','usesDepthValues','ignoreDepthValues']);
  if(Array.isArray(source.supportedFrameRates))summary.supportedFrameRates=source.supportedFrameRates.filter(rate=>typeof rate==='number'&&Number.isFinite(rate)&&rate>0).slice(0,8);
  // First WebGPU frames expose viewport under eyes; performance has flat eyeViewports.
  const eyes=Array.isArray(source.eyeViewports)?source.eyeViewports:Array.isArray(source.eyes)?source.eyes:null;
  if(eyes)summary.eyeViewports=eyes.flatMap(eye=>{
   if(!eye||typeof eye!=='object'||!['left','right'].includes(eye.eye))return [];
   const viewport=eye.viewport&&typeof eye.viewport==='object'?eye.viewport:eye;
   const {width,height}=viewport;
   if(typeof width!=='number'||typeof height!=='number'||!Number.isFinite(width)||!Number.isFinite(height)||width<=0||height<=0)return [];
   return [{eye:eye.eye,width,height}];
  }).slice(0,2);
  emit('rim-xr-'+name,summary);
 });
 listen(engine.xr,'end',()=>emit('rim-xr-end',{samples}));
 for(const name of ['error','gpuerror','gpulost'])listen(engine.xr,name,event=>{
  const detail=/** @type {CustomEvent<unknown>} */(event).detail;
  reportError('rim-xr-'+name,(detail&&typeof detail==='object'?Reflect.get(detail,'message'):detail)??engine.xr.lastError);
 });
 listen(target,'error',event=>reportError('rim-uncaught',/** @type {ErrorEvent} */(event).error??/** @type {ErrorEvent} */(event).message));
 listen(target,'unhandledrejection',event=>reportError('rim-unhandledrejection',/** @type {PromiseRejectionEvent} */(event).reason));
 emit('rim-ready');
 return ()=>{if(disposed)return;disposed=true;for(const remove of cleanup)remove();cleanup.length=0;};
}
