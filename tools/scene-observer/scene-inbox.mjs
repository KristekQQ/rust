import {WireCache} from './protocol.mjs';

const u64=value=>BigInt.asUintN(64,value);
// Ingest every ordered transport delta before ACK. Rendering may skip frames,
// but must still receive assets introduced by any skipped publication.
export class SceneInbox {
    constructor({sequence=0n,revisions=null,buffered=false,maxFrames=128,maxBytes=32*1024*1024}={}) {
        this.cache=new WireCache();
        this.peer={sequence:0,known:[new Map(),new Map(),new Map()]};
        this.base=sequence;this.revisions=revisions;this.offsets=null;this.meta=null;
        this.buffered=buffered;this.maxFrames=maxFrames;this.maxBytes=maxBytes;this.queue=[];this.queuedBytes=0;
    }
    receive(frame) {
        this.cache.ingest(frame.wire);
        if(!this.buffered){this.meta=frame.meta;return;}
        const packet=this.packet(frame.meta);
        // Never drop an intermediate delta: a later frame may depend on its assets.
        // Caller reconnects for a fresh full snapshot if a suspended viewer overflows.
        if(this.queue.length>=this.maxFrames||this.queuedBytes+packet.wire.byteLength>this.maxBytes)throw Error('Observer snapshot backlog exceeded');
        this.queue.push(packet);this.queuedBytes+=packet.wire.byteLength;
    }
    take() {
        if(this.buffered){const frame=this.queue.shift();if(!frame)return null;this.queuedBytes-=frame.wire.byteLength;return frame;}
        if(!this.meta)return null;
        const meta=this.meta;this.meta=null;return this.packet(meta);
    }
    packet(meta) {
        const wire=this.cache.packetFor(this.peer),header=new DataView(wire.buffer,wire.byteOffset,wire.byteLength);
        header.setBigUint64(8,u64(this.base+BigInt(this.peer.sequence)),true);
        // A reconnect/new source can reset its counters while the renderer stays
        // alive. Force one invalidation, then preserve subsequent source deltas.
        if(this.revisions){
            this.offsets??=this.revisions.map((v,i)=>u64(v+1n-header.getBigUint64(16+i*8,true)));
            this.offsets.forEach((v,i)=>header.setBigUint64(16+i*8,u64(header.getBigUint64(16+i*8,true)+v),true));
        }
        return {meta,wire};
    }
}
