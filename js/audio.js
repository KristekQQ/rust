/** Audio output adapter. Rust gameplay owns cue selection, timing and variants. */
const finite3=v=>Array.isArray(v)&&v.length===3&&v.every(Number.isFinite);
const clamp=(v,min,max)=>Math.min(max,Math.max(min,v));
export class AudioClip {
    constructor(id,buffer,{gain=1,spatial=true}={}) {this.id=id;this.buffer=buffer;this.gain=gain;this.spatial=spatial;}
}
export class AudioListener {
    constructor(context){this.context=context;}
    setPose(position,forward=[0,0,-1],up=[0,1,0]){
        if(!finite3(position)||!finite3(forward)||!finite3(up))return;
        const listener=this.context.listener,now=this.context.currentTime;
        if(listener.positionX){
            for(const [prefix,values]of [['position',position],['forward',forward],['up',up]])
                for(const [i,axis]of ['X','Y','Z'].entries())listener[prefix+axis].setValueAtTime(values[i],now);
        }else {listener.setPosition(...position);listener.setOrientation(...forward,...up);}
    }
    setMatrix(matrix){if(matrix?.length===16&&Array.from(matrix).every(Number.isFinite))this.setPose([matrix[12],matrix[13],matrix[14]],[-matrix[8],-matrix[9],-matrix[10]],[matrix[4],matrix[5],matrix[6]]);}
}
export class AudioSource {
    constructor(mixer,clip,{position=null,gain=1,pitch=1,priority=1,delay=0,loop=false,bus='effects'}={}){
        this.mixer=mixer;this.clip=clip;this.priority=priority;this.stopped=false;
        const context=mixer.context;this.node=context.createBufferSource();this.node.buffer=clip.buffer;
        this.node.playbackRate.value=clamp(pitch,.5,2);this.node.loop=loop;
        this.volume=context.createGain();this.volume.gain.value=clamp(gain,0,1)*clip.gain;
        this.node.connect(this.volume);
        if(position&&clip.spatial){
            this.panner=context.createPanner();this.panner.panningModel='equalpower';this.panner.distanceModel='inverse';
            this.panner.refDistance=1;this.panner.maxDistance=20;this.panner.rolloffFactor=1;
            this.setPosition(position);this.volume.connect(this.panner);this.panner.connect(mixer.buses[bus]??mixer.buses.effects);
        }else this.volume.connect(mixer.buses[bus]??mixer.buses.effects);
        this.node.onended=()=>this.dispose();this.node.start(context.currentTime+clamp(delay,0,.25));
    }
    setPosition(position){if(!this.panner||!finite3(position))return;for(const [i,axis]of ['X','Y','Z'].entries())this.panner['position'+axis].value=position[i];}
    stop(){if(this.stopped)return;this.stopped=true;try{this.node.stop();}catch{}this.dispose();}
    dispose(){this.node.disconnect();this.volume.disconnect();this.panner?.disconnect();this.mixer.voices.delete(this);}
}
export class AudioMixer {
    constructor({context=null,maxVoices=24,master=.6}={}){
        this.context=context??new AudioContext({latencyHint:'interactive'});this.maxVoices=clamp(maxVoices,1,64);
        this.groups=[];this.outputEnabled=true;this.clips=new Map();this.voices=new Set();this.listener=new AudioListener(this.context);
        this.policyMaster=master;this.userGain=1;this.master=this.context.createGain();this.master.gain.value=master;
        this.limiter=this.context.createDynamicsCompressor();this.limiter.threshold.value=-6;this.limiter.knee.value=0;
        this.limiter.ratio.value=20;this.limiter.attack.value=.003;this.limiter.release.value=.1;
        this.master.connect(this.limiter);this.limiter.connect(this.context.destination);
        this.buses={};for(const name of ['effects','ui','music']){const bus=this.context.createGain();bus.connect(this.master);this.buses[name]=bus;}
        this.epoch=null;this.latest=0;this.disposed=false;this.stats={played:0,expired:0,limited:0,gaps:0};
    }
    async load(manifestUrl){
        const response=await fetch(manifestUrl);if(!response.ok)throw Error(`Audio manifest: ${response.status}`);
        const manifest=await response.json();if(!Array.isArray(manifest.sounds)||manifest.sounds.length>128)throw Error('Invalid sound manifest');
        await Promise.all(manifest.sounds.map(async item=>{
            if(typeof item.id!=='string'||!/^[-\w]+\.wav$/.test(item.file))throw Error('Invalid sound asset');
            const reply=await fetch(new URL(item.file,manifestUrl));if(!reply.ok)throw Error(`Audio asset: ${reply.status}`);
            const buffer=await this.context.decodeAudioData(await reply.arrayBuffer());
            if(!this.disposed)this.clips.set(item.id,new AudioClip(item.id,buffer,{spatial:item.spatial,gain:1}));
        }));
    }
    resume(){return this.context.resume();}
    setVolume(value,bus=null){if(!Number.isFinite(value))return;const node=bus?this.buses[bus]:this.master;if(!bus)this.userGain=clamp(value,0,1);if(node)node.gain.setTargetAtTime(clamp(value,0,1)*(bus?1:this.policyMaster),this.context.currentTime,.015);}
    play(id,options={}){
        if(this.disposed||!this.outputEnabled||(this.context.state!=='running'&&typeof this.context.startRendering!=='function'))return null;
        const clip=this.clips.get(id);if(!clip)return null;
        const {gain=1,pitch=1,position=null,priority=1}=options;
        if(!Number.isFinite(gain)||gain<=0||!Number.isFinite(pitch)||!Number.isFinite(priority)||(position!==null&&!finite3(position)))return null;
        const rule=this.groups.find(rule=>id.startsWith(rule.prefix)),group=rule?.prefix??id;
        const same=[...this.voices].filter(v=>(this.groups.find(rule=>v.clip.id.startsWith(rule.prefix))?.prefix??v.clip.id)===group);
        const cap=rule?.limit??4;
        if(same.length>=cap){const victim=same.find(v=>v.priority<=priority);if(!victim){this.stats.limited++;return null;}victim.stop();}
        if(this.voices.size>=this.maxVoices){const victim=[...this.voices].find(v=>v.priority<priority);if(!victim){this.stats.limited++;return null;}victim.stop();}
        const source=new AudioSource(this,clip,options);this.voices.add(source);this.stats.played++;return source;
    }
    /** A retained journal survives frame coalescing. Cold observer joins suppress historical hits. */
    ingest(snapshot,{suppressInitial=false,delay=0,maxAge=.25,transportAge=0}={}){
        if(!snapshot||!Number.isSafeInteger(snapshot.epoch)||!Number.isSafeInteger(snapshot.latest)||snapshot.latest<0||!Number.isFinite(snapshot.clock)||!Array.isArray(snapshot.events)||snapshot.events.length>64)return;
        const policy=snapshot.policy;
        if(policy&&Number.isInteger(policy.maxVoices)&&policy.maxVoices>=1&&policy.maxVoices<=24&&Array.isArray(policy.groups)&&policy.groups.length<=16
            &&Number.isFinite(policy.master)&&policy.master>=0&&policy.master<=1
            &&policy.groups.every(g=>typeof g.prefix==='string'&&g.prefix.length<=64&&Number.isInteger(g.limit)&&g.limit>=1&&g.limit<=24)){
            this.maxVoices=policy.maxVoices;this.groups=policy.groups;this.policyMaster=policy.master;this.master.gain.setTargetAtTime(this.policyMaster*this.userGain,this.context.currentTime,.015);
        }
        if(this.epoch!==snapshot.epoch){const initial=this.epoch===null;this.stopAll();this.epoch=snapshot.epoch;this.latest=0;if(initial&&suppressInitial){this.latest=snapshot.latest;return;}}
        if(snapshot.latest<this.latest)return;
        for(const event of snapshot.events){
            if(!Number.isSafeInteger(event.id)||event.id<=this.latest||event.id>snapshot.latest||!Number.isFinite(event.at))continue;
            if(event.id>this.latest+1)this.stats.gaps+=event.id-this.latest-1;
            this.latest=event.id;
            const age=snapshot.clock-event.at+Math.max(0,Number.isFinite(transportAge)?transportAge:Infinity);
            if(age<-.001||age>maxAge){this.stats.expired++;continue;}
            this.play(event.clip,{...event,delay:Math.max(0,delay-age),bus:event.position?'effects':'ui'});
        }
        this.latest=snapshot.latest;
    }
    setOutputEnabled(enabled){this.outputEnabled=!!enabled;if(!enabled)this.stopAll();}
    reset(){this.stopAll();this.epoch=null;this.latest=0;}
    stopAll(){for(const voice of [...this.voices])voice.stop();}
    async suspend(){this.stopAll();await this.context.suspend();}
    async dispose(){if(this.disposed)return;this.disposed=true;this.stopAll();this.clips.clear();await this.context.close?.();}
}
