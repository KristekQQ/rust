// Optional transport adapter, run on the existing simulation worker only.
// No gameplay, geometry generation, renderer calls or waiting on the XR thread.
import {WireCache,encodePacket} from './protocol.mjs';

export class ScenePublisher {
    constructor(url,{mode='live',objectIds=null,onStatus=()=>{}}={}){
        const endpoint=new URL(url);
        endpoint.searchParams.set('role','publisher');
        this.session=crypto.randomUUID();this.mode=mode;this.onStatus=onStatus;
        this.objectIds=objectIds;this.cache=new WireCache(objectIds?{objectIds}:{});this.peer={sequence:0,known:[new Map(),new Map(),new Map()]};
        this.pending=false;this.inFlight=false;this.closed=false;this.clockReady=false;
        this.offset=0;this.uncertainty=Infinity;this.dropped=0;
        this.endpoint=endpoint;this.connect();
    }
    connect(){
        if(this.closed)return;
        this.clockReady=false;this.inFlight=false;
        this.session=crypto.randomUUID();this.peer={sequence:0,known:[new Map(),new Map(),new Map()]};
        if(this.latest){this.latest.session=this.session;this.pending=true;}
        this.socket=new WebSocket(this.endpoint);this.socket.binaryType='arraybuffer';
        this.socket.onopen=()=>{this.socket.send(JSON.stringify({type:'clock',sentAt:Date.now()}));};
        this.socket.onmessage=event=>{
            if(typeof event.data!=='string')return;
            const message=JSON.parse(event.data);
            if(message.type==='clock'){
                const rtt=Date.now()-message.sentAt;
                this.offset=message.serverAt-(message.sentAt+Date.now())/2;
                this.uncertainty=rtt/2;this.clockReady=true;this.flush();
            }else if(message.type==='ack'){this.inFlight=false;this.flush();}
            else if(message.type==='error'){this.onStatus({error:message.error});this.close();}
        };
        this.socket.onerror=()=>this.onStatus({error:'Spojení observeru selhalo.'});
        this.socket.onclose=()=>{
            if(!this.closed){this.onStatus({disconnected:true});this.reconnectTimer=setTimeout(()=>this.connect(),1000);}
        };
    }
    publish(bytes,state,sampledAt=Date.now()){
        if(this.closed)return;
        try{
            this.cache.ingest(bytes);
            if(this.pending)this.dropped++;
            this.latest={session:this.session,mode:this.mode,state:this.objectIds?{swords:this.objectIds.length,head:state?.head??null}:state,sampledAt};this.pending=true;this.flush();
        }catch(error){this.onStatus({error:String(error)});this.close();}
    }
    flush(){
        if(!this.pending||this.inFlight||!this.clockReady||this.socket.readyState!==WebSocket.OPEN)return;
        const meta={...this.latest,sampledAt:this.latest.sampledAt+this.offset,
            clockUncertaintyMs:this.uncertainty,coalesced:this.dropped};
        const wire=this.cache.packetFor(this.peer);
        this.socket.send(encodePacket(meta,wire));this.pending=false;this.inFlight=true;
        this.onStatus({connected:true,bytes:wire.byteLength,coalesced:this.dropped,state:meta.state});
    }
    close(){this.closed=true;clearTimeout(this.reconnectTimer);this.socket.close();}
}
