import net from 'node:net';
import {timingSafeEqual} from 'node:crypto';
import {WireCache,CACHE_LIMIT} from './protocol.mjs';
import {decodeByteDelta,isByteDelta} from './byte-delta.mjs';

/** Authenticated, ordered generic scene ingress. One publisher, one ACK in flight. */
export async function createNativeSceneBridge({port=8793,host='0.0.0.0',token,initialSnapshot=null,onFrame=()=>{}}={}){
  const key=Buffer.from(token,'hex');if(key.length!==16)throw Error('Invalid scene token');
  let publisher=null,cache=new WireCache(),latest=null,closed=false;
  const counters={accepted:0,rejected:0,connections:0,disconnects:0,receivedBytes:0,rawBytes:0,fullFrames:0,deltaFrames:0};
  let lastError=null,normalizedSnapshot=!!initialSnapshot,everSnapshot=!!initialSnapshot;
  if(initialSnapshot){cache.ingest(initialSnapshot.wire);latest={...initialSnapshot.meta,mode:'live'};}
  const peers=new Set();
  const server=net.createServer(socket=>{
    if(publisher||peers.size>=4){socket.destroy();return;}
    peers.add(socket);
    const authDeadline=setTimeout(()=>socket.destroy(),3000);authDeadline.unref();socket.setNoDelay(true);socket.setTimeout(5000,()=>{lastError='Source idle for 5 seconds';socket.destroy();});
    let pending=Buffer.alloc(0),authenticated=false,session=null,deltaBase=null;
    socket.on('error',()=>{});
    socket.on('close',()=>{clearTimeout(authDeadline);peers.delete(socket);if(publisher===socket){publisher=null;counters.disconnects++;}});
    socket.on('data',chunk=>{
      try{
        if(pending.length+chunk.length>CACHE_LIMIT+65588)throw Error('Scene receive budget');
        pending=Buffer.concat([pending,chunk]);
        if(!authenticated){
          if(pending.length<20)return;
          if(pending.toString('ascii',0,4)!=='NSC1'||!timingSafeEqual(pending.subarray(4,20),key))throw Error('Scene authentication');
          if(publisher&&publisher!==socket)throw Error('Scene publisher already authenticated');
          publisher=socket;clearTimeout(authDeadline);socket.setTimeout(0);socket.setKeepAlive(true,1000);authenticated=true;pending=pending.subarray(20);counters.connections++;cache=new WireCache();latest=null;normalizedSnapshot=false;
        }
        while(pending.length>=8){
          const jsonLength=pending.readUInt32LE(0),wireLength=pending.readUInt32LE(4);
          if(!jsonLength||jsonLength>65536||wireLength<24||wireLength>CACHE_LIMIT+24)throw Error('Scene frame budget');
          const size=8+jsonLength+wireLength;if(pending.length<size)return;
          const meta=JSON.parse(pending.toString('utf8',8,8+jsonLength));
          if(typeof meta.session!=='string'||!meta.session.length||meta.session.length>256||!Number.isFinite(meta.sampledAt)||meta.sampledAt<=0||!meta.state||typeof meta.state!=='object')throw Error('Scene metadata');
          if(session!==null&&session!==meta.session)throw Error('Session changed within scene stream');
          session=meta.session;
          // All native deltas are ingested in order; per-browser snapshots can coalesce.
          const payload=new Uint8Array(pending.subarray(8+jsonLength,size));
          let raw=payload;
          if(isByteDelta(payload)){deltaBase=decodeByteDelta(deltaBase,payload,{maxBytes:CACHE_LIMIT});raw=deltaBase.bytes;counters[payload[4]?'deltaFrames':'fullFrames']++;}
          else{if(payload.length<68)throw Error('Truncated native wire');if(deltaBase)throw Error('Native transport encoding changed');counters.fullFrames++;}
          cache.ingest(raw);counters.receivedBytes+=payload.length;counters.rawBytes+=raw.length;
          pending=pending.subarray(size);latest={...meta,mode:'live'};everSnapshot=true;counters.accepted++;
          onFrame(latest,cache);socket.write(Buffer.from([1]));
        }
      }catch(error){lastError=String(error.message).slice(0,160);counters.rejected++;socket.destroy();}
    });
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,host,resolve);});
  return {get normalizedSnapshot(){return normalizedSnapshot;},get hasSnapshot(){return everSnapshot;},get latest(){return latest;},get cache(){return cache;},get port(){return server.address().port;},
    status:()=>({port:server.address().port,publisher:!!publisher,connected:!!latest&&!!publisher,sequence:cache.sequence.toString(),cacheBytes:cache.byteLength,lastError,...counters}),
    close:async()=>{closed=true;for(const p of peers)p.destroy();await new Promise(resolve=>server.close(resolve));}};
}
