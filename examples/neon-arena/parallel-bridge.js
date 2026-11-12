// Ordered binary transport. No physics, scene calculation or per-frame RAF here.
import {createSimulation} from './parallel-runtime.js';
const STRIDE=88,CAPACITY=128;
export class ParallelBridge extends EventTarget {
    #engine;#locals;#worker;#inline;#queue=new Float32Array(CAPACITY*STRIDE);#spare=new Float32Array(CAPACITY*STRIDE);
    #count=0;#busy=false;#reply=null;#error=null;#flags=0;#disposed=false;#waiters=[];#sampledAt=0;
    state=null;stats={workerMs:0,applyMs:0,ageMs:0,bytes:0,dropped:0,batches:0,frames:0};
    static async create(engine,world,{mode='worker',scores=[],observerUrl=null}={}){
        const bridge=new ParallelBridge(engine,[world.infoPanel,world.leftSword,world.sword]);
        const assets=new URL('./parallel-assets/',import.meta.url).href;
        let initial;
        if(mode==='inline'){
            bridge.#inline=await createSimulation(assets,scores);
            initial={type:'ready',bytes:bridge.#inline.initial().buffer,refs:JSON.parse(bridge.#inline.refs()),state:JSON.parse(bridge.#inline.state()),workerMs:0};
        }else{
            bridge.#worker=new Worker(new URL('./parallel-worker.js',import.meta.url),{type:'module',name:'neon-simulation'});
            initial=await new Promise((resolve,reject)=>{
                const timeout=setTimeout(()=>reject(new Error('Simulation startup timeout')),30000);
                bridge.#worker.onerror=event=>{clearTimeout(timeout);reject(new Error(event.message));};
                bridge.#worker.onmessage=event=>{clearTimeout(timeout);event.data.type==='error'?reject(new Error(event.data.error)):resolve(event.data);};
                bridge.#worker.postMessage({type:'init',assets,scores,observerUrl});
            });
            bridge.#worker.onmessage=event=>{
                if(event.data.type==='error'){bridge.#error=new Error(event.data.error);bridge.#rejectWaiters();return;}
                if(bridge.#reply){bridge.#error=new Error('Unapplied simulation response');bridge.#rejectWaiters();return;}
                bridge.#reply=event.data;bridge.#spare=new Float32Array(event.data.frames);
                bridge.#resolveWaiters();
            };
            bridge.#worker.onerror=event=>{bridge.#error=new Error(event.message);bridge.#rejectWaiters();};
        }
        for(const name of ['infoPanel','leftSword','sword','timerPanel','scorePanel','restartButton','scoreDisplay']){
            if(initial.refs[name]!==world[name]?.id)throw new Error(`Native/web scene identity differs: ${name}: ${initial.refs[name]} / ${world[name]?.id}`);
        }
        bridge.#apply(initial);return bridge;
    }
    constructor(engine,locals){super();this.#engine=engine;this.#locals=locals;}
    #resolveWaiters(){for(const {resolve}of this.#waiters.splice(0))resolve();}
    #rejectWaiters(){for(const {reject}of this.#waiters.splice(0))reject(this.#error);}
    #apply(reply){
        const start=performance.now();
        this.#engine.applySimulationPublication(new Uint8Array(reply.bytes),this.#locals);
        this.stats.applyMs=performance.now()-start;this.stats.workerMs=reply.workerMs;
        this.stats.bytes=reply.bytes.byteLength;this.stats.ageMs=reply.sent===undefined?0:performance.now()-reply.sent;
        const previous=this.state;
        this.state=reply.state;if(reply.haptics)this.#engine.xr.pulseHaptics(reply.haptics);
        if(!previous?.round?.finished&&this.state?.round?.finished)this.dispatchEvent(new Event('roundend'));
        this.dispatchEvent(new Event('state'));

        this.#busy=false;
    }
    applyPending(){if(this.#error)throw this.#error;if(this.#reply){const reply=this.#reply;this.#reply=null;this.#apply(reply);}}
    get roundState(){return this.state?.round?{...this.state.round,score:this.state.score}:null;}
    restoreScores(scores){if(this.#inline)this.#inline.restore_scores(JSON.stringify(scores));else this.#worker.postMessage({type:'scores',scores});}
    setImmersive(value){if(this.#inline)this.#inline.set_immersive(value);else this.#worker.postMessage({type:'immersive',value});}
    newSession(){this.#flags|=2;}
    resetTracking(){this.#flags|=4;}
    step(sample){
        if(this.#disposed)return;
        this.applyPending();
        if(this.#count===CAPACITY){this.stats.dropped++;this.resetTracking();return;}
        this.#queue.set(sample,this.#count*STRIDE);this.#queue[this.#count*STRIDE+3]|=this.#flags;this.#flags=0;this.#count++;this.stats.frames++;this.#sampledAt=performance.timeOrigin+performance.now();
        this.#flush();
    }
    #flush(){
        if(this.#busy||!this.#count)return;
        const count=this.#count,frames=this.#queue.buffer,sent=performance.now();this.stats.batches++;
        this.#busy=true;this.#count=0;
        if(this.#inline){
            const start=performance.now(),bytes=this.#inline.tick(new Float32Array(frames,0,count*STRIDE));
            this.#apply({bytes:bytes.buffer,state:JSON.parse(this.#inline.state()),haptics:this.#inline.take_haptics(),workerMs:performance.now()-start,sent});
        }else{
            this.#queue=this.#spare;this.#spare=null;
            this.#worker.postMessage({type:'tick',frames,count,sent,sampledAt:this.#sampledAt},[frames]);
        }
    }
    /** Deterministic test/benchmark drain only. XR uses step()/applyPending(), never awaits. */
    async settle(){
        while(this.#busy||this.#count){
            if(this.#error)throw this.#error;
            if(this.#busy&&!this.#reply)await new Promise((resolve,reject)=>this.#waiters.push({resolve,reject}));
            this.applyPending();this.#flush();
        }
        return this.state;
    }
    dispose(){if(this.#disposed)return;this.#disposed=true;this.#engine.xr.setSimulationFrameHandler(null);this.#worker?.terminate();this.#inline?.free();this.#error=new Error('Simulation disposed');this.#rejectWaiters();}
}
