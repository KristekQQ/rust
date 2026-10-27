// Standalone browser injection. Queries cover GPU commands between the first
// application RAF callback and the measurement sampler, never compositor time.
export function installGlRafTimer(enabled){
 const scope='application RAF GPU commands: first callback -> measurement sampler; excludes compositor/worker/between-RAF work';
 const state={enabled:!!enabled,supported:null,scope,units:'milliseconds (TIME_ELAPSED_EXT nanoseconds / 1e6)',sampleEveryFrames:30,maxInflight:3,maxAgeFrames:240,frames:0,queriesCreated:0,queriesBegun:0,queriesEnded:0,queriesDeleted:0,skippedBusy:0,skippedInflight:0,skippedDisjoint:0,disjointDiscarded:0,samples:[],invalid:[],incomplete:[]};
 let gl=null,ext=null,stamp=null,active=null,pending=[],armed=false;
 const invalid=(reason,entry=null)=>{if(state.invalid.length<100)state.invalid.push({reason,sampleFrame:entry?.frame??null,atFrame:state.frames});};
 const remove=entry=>{try{gl.deleteQuery(entry.query);state.queriesDeleted++;}catch(error){invalid('deleteQuery: '+String(error),entry);}};
 function endFrame(accepted=true){
 if(!active)return;const entry=active;active=null;
 try{if(gl.getQuery(ext.TIME_ELAPSED_EXT,gl.CURRENT_QUERY)!==entry.query){invalid('active query ownership changed',entry);remove(entry);return;}gl.endQuery(ext.TIME_ELAPSED_EXT);state.queriesEnded++;if(!accepted){remove(entry);return;}if(gl.getParameter(ext.GPU_DISJOINT_EXT)){state.disjointDiscarded++;invalid('GPU disjoint at query end',entry);remove(entry);}else pending.push(entry);}catch(error){invalid('endQuery: '+String(error),entry);remove(entry);}
 }
 function poll(){
 if(!gl||!ext||!pending.length)return;
 try{
 if(gl.isContextLost()){for(const entry of pending){invalid('context lost',entry);remove(entry);}pending=[];return;}
 if(gl.getParameter(ext.GPU_DISJOINT_EXT)){state.disjointDiscarded+=pending.length;for(const entry of pending){invalid('GPU disjoint',entry);remove(entry);}pending=[];return;}
 const keep=[];
 for(const entry of pending){const age=state.frames-entry.frame;if(age<=0){keep.push(entry);continue;}if(age>state.maxAgeFrames){invalid('query stale',entry);remove(entry);continue;}
 if(!gl.getQueryParameter(entry.query,gl.QUERY_RESULT_AVAILABLE)){keep.push(entry);continue;}
 const ns=gl.getQueryParameter(entry.query,gl.QUERY_RESULT);if(!Number.isFinite(ns)||ns<0){invalid('nonfinite or negative elapsed nanoseconds',entry);remove(entry);continue;}
 // Recheck disjoint after retrieving the available result. Never publish invalid time.
 if(gl.getParameter(ext.GPU_DISJOINT_EXT)){state.disjointDiscarded++;invalid('GPU disjoint after available result',entry);remove(entry);continue;}
 state.samples.push({sampleFrame:entry.frame,stamp:entry.stamp,availableAtFrame:state.frames,ageFrames:age,elapsedMs:ns/1e6});remove(entry);
 }
 pending=keep;
 }catch(error){invalid('query polling: '+String(error));}
 }
 function beginFrame(t){
 if(!state.enabled||!armed||stamp===t)return;stamp=t;state.frames++;poll();
 // A sampler must have ended our previous query. Never end another owner's query.
 if(active){invalid('previous frame missing sampler',active);endFrame(false);}
 if(!gl||!ext||state.frames%state.sampleEveryFrames!==1)return;
 if(pending.length>=state.maxInflight){state.skippedInflight++;return;}
 let entry=null;try{if(gl.getParameter(ext.GPU_DISJOINT_EXT)){state.skippedDisjoint++;return;}if(gl.getQuery(ext.TIME_ELAPSED_EXT,gl.CURRENT_QUERY)!==null){state.skippedBusy++;return;}
 const query=gl.createQuery();if(!query){invalid('createQuery returned null');return;}state.queriesCreated++;entry={query,frame:state.frames,stamp:t};gl.beginQuery(ext.TIME_ELAPSED_EXT,query);
 if(gl.getQuery(ext.TIME_ELAPSED_EXT,gl.CURRENT_QUERY)!==query){invalid('beginQuery did not acquire TIME_ELAPSED target',entry);remove(entry);return;}state.queriesBegun++;active=entry;
 }catch(error){invalid('query begin: '+String(error),entry);if(entry)remove(entry);}
 }
 function stop(){
 armed=false;endFrame();poll();for(const entry of pending){state.incomplete.push({sampleFrame:entry.frame,stamp:entry.stamp,ageFrames:state.frames-entry.frame,reason:'measurement ended before asynchronous result available'});remove(entry);}pending=[];
 }
 window.renderGlRafTimer={start(){armed=state.enabled;},beginFrame,endFrame,poll,stop,snapshot(){return {...state,samples:[...state.samples],invalid:[...state.invalid],incomplete:[...state.incomplete],pendingQueries:pending.length,activeQuery:active!==null};}};
 if(!state.enabled)return;
 if(typeof HTMLCanvasElement==='undefined'){state.supported=false;return;}
 const original=HTMLCanvasElement.prototype.getContext;
 HTMLCanvasElement.prototype.getContext=function(kind,...args){const context=original.call(this,kind,...args);if(kind==='webgl2'&&context&&this.id==='gpu-canvas'){
 if(gl&&gl!==context){invalid('multiple engine WebGL contexts');return context;}gl=context;
 try{ext=gl.getExtension('EXT_disjoint_timer_query_webgl2');state.supported=ext!==null;}catch(error){state.supported=false;invalid('extension detection: '+String(error));}
 }return context;};
}
