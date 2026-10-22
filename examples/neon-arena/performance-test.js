// Window statistics describe XR telemetry packets, never per-frame percentiles.
/** @typedef {{id:string,label:string,ssr:boolean,glow:boolean,resolutionScale:.5|1}} Phase */
/** @typedef {{eye?:string,width:number,height:number}} EyeViewport */
/** @typedef {Record<string, any> & {frames:number,frameMs:number,cpuSubmitMs:number,sessionId?:string|number,eyeViewports?:EyeViewport[]}} Packet */
/** @typedef {Phase & {frames:number,fps:number,frameMs:number,cpuSubmitMs:number,windowFps:{median:number,min:number},metadata:Record<string,any>,windows:Packet[]}} Result */
/** @typedef {{status:string,reason:string|null,phase:Readonly<Phase>|null,phaseIndex:number,settleCount:number,sampleCount:number,settlePackets:number,samplePackets:number,results:Result[]}} Snapshot */
export const PERFORMANCE_PHASES = Object.freeze(/** @type {Phase[]} */ ([
    {id:'base-start',label:'Bez efektů',ssr:false,glow:false,resolutionScale:.5},
    {id:'glow-only',label:'Pouze glow',ssr:false,glow:true,resolutionScale:.5},
    {id:'ssr-half',label:'SSR 50 %',ssr:true,glow:false,resolutionScale:.5},
    {id:'ssr-full',label:'SSR 100 %',ssr:true,glow:false,resolutionScale:1},
    {id:'both-half',label:'SSR 50 % + glow',ssr:true,glow:true,resolutionScale:.5},
    {id:'base-end',label:'Bez efektů – kontrola',ssr:false,glow:false,resolutionScale:.5},
]).map(phase=>Object.freeze(phase)));
const SETTLE_PACKETS=2, SAMPLE_PACKETS=3;
/** @param {number} value */
const positive=value=>Number.isFinite(value)&&value>0;
/** @template T @param {T} value @returns {T} */
const copy=value=>structuredClone(value);

/** @param {Packet} detail */
function validPacket(detail){
    return detail&&Number.isInteger(detail.frames)&&detail.frames>0&&positive(detail.frameMs)
        &&Number.isFinite(detail.cpuSubmitMs)&&detail.cpuSubmitMs>=0
        &&(detail.eyeViewports===undefined||(Array.isArray(detail.eyeViewports)&&detail.eyeViewports.length>0
            &&detail.eyeViewports.every(view=>view&&positive(view.width)&&positive(view.height))));
}
/** @param {EyeViewport[]|undefined} views */
function viewportKey(views){
    return views?.map(view=>`${view.eye??''}:${view.width}x${view.height}`).sort().join('|');
}
/** @param {Readonly<Phase>} phase @param {Packet[]} windows @returns {Result} */
function summarize(phase,windows){
    const frames=windows.reduce((total,window)=>total+window.frames,0);
    const elapsed=windows.reduce((total,window)=>total+window.frames*window.frameMs,0);
    const cpu=windows.reduce((total,window)=>total+window.frames*window.cpuSubmitMs,0);
    const fps=windows.map(window=>1000/window.frameMs).sort((a,b)=>a-b);
    // Runtime metadata is separate from statistics belonging to individual windows.
    /** @type {Record<string,any>} */
    const metadata={...windows.at(-1)};
    for(const key of Object.keys(metadata)){
        if(['frames','fps','frameMs','cpuSubmitMs','sample'].includes(key)||/median|percentile|p\d/i.test(key))delete metadata[key];
    }
    return {...phase,frames,fps:1000*frames/elapsed,frameMs:elapsed/frames,cpuSubmitMs:cpu/frames,
        windowFps:{median:fps[1],min:fps[0]},metadata,windows};
}

/** Pure controller; the caller applies effects and restores settings in onFinish. */
export class PerformanceTest {
    /** @param {{applyPhase:(phase:Readonly<Phase>)=>void,onUpdate?:(state:Snapshot)=>void,onFinish?:(state:Snapshot)=>void}} callbacks */
    constructor({applyPhase,onUpdate=()=>{},onFinish=()=>{}}){
        this.applyPhase=applyPhase;this.onUpdate=onUpdate;this.onFinish=onFinish;
        this.status='idle';this.reason=null;this.phaseIndex=-1;this.settleCount=0;
        /** @type {Packet[]} */ this.windows=[];
        /** @type {Result[]} */ this.results=[];
        /** @type {string|number|undefined} */ this.sessionId=undefined;
        /** @type {string|undefined} */ this.viewports=undefined;
    }
    get running(){return this.status==='running';}
    /** @returns {Snapshot} */
    snapshot(){
        return copy({status:this.status,reason:this.reason,phase:PERFORMANCE_PHASES[this.phaseIndex]??null,
            phaseIndex:this.phaseIndex,settleCount:this.settleCount,sampleCount:this.windows.length,
            settlePackets:SETTLE_PACKETS,samplePackets:SAMPLE_PACKETS,results:this.results});
    }
    start(){
        if(this.running)return false;
        this.status='running';this.reason=null;this.phaseIndex=0;this.settleCount=0;
        this.windows=[];this.results=[];this.sessionId=undefined;this.viewports=undefined;
        this.applyPhase(PERFORMANCE_PHASES[0]);this.onUpdate(this.snapshot());
        return true;
    }
    /** @param {Packet} detail */
    addSample(detail){
        if(!this.running||!validPacket(detail))return false;
        const dimensions=viewportKey(detail.eyeViewports);
        if(this.sessionId!==undefined&&detail.sessionId!==undefined&&detail.sessionId!==this.sessionId){
            this.cancel('session-changed');return false;
        }
        if(this.viewports!==undefined&&dimensions!==undefined&&dimensions!==this.viewports){
            this.cancel('viewport-changed');return false;
        }
        if(detail.sessionId!==undefined)this.sessionId=detail.sessionId;
        if(dimensions!==undefined)this.viewports=dimensions;
        if(this.settleCount<SETTLE_PACKETS)this.settleCount++;
        else this.windows.push(copy(detail));
        if(this.windows.length===SAMPLE_PACKETS){
            this.results.push(summarize(PERFORMANCE_PHASES[this.phaseIndex],this.windows));
            if(this.phaseIndex===PERFORMANCE_PHASES.length-1){
                this.status='complete';this.onUpdate(this.snapshot());this.onFinish(this.snapshot());return true;
            }
            this.phaseIndex++;this.settleCount=0;this.windows=[];
            this.applyPhase(PERFORMANCE_PHASES[this.phaseIndex]);
        }
        this.onUpdate(this.snapshot());return true;
    }
    cancel(reason='cancelled'){
        if(!this.running)return false;
        this.status='cancelled';this.reason=reason;
        this.onUpdate(this.snapshot());this.onFinish(this.snapshot());return true;
    }
}
