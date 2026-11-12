import test from 'node:test';import assert from 'node:assert/strict';
import {SimulatedDelivery} from '../tools/scene-observer/network-demo-transport.mjs';
test('4Hz sampling is delivered exactly150ms later without inventing frames',()=>{
 const q=new SimulatedDelivery({delay:150});
 for(let i=0;i<8;i++){const t=i*250;q.enqueue(i,t,1000+t);assert.equal(q.receive(t+149),null);const packet=q.receive(t+150);assert.equal(packet.frame,i);assert.equal(packet.sampledAt,1000+t);assert.equal(packet.coalesced,0);}
 assert.equal(q.queue.length,0);
});
test('jitter preserves transport order and outage coalesces pending source states',()=>{
 const values=[1,0,1,0],q=new SimulatedDelivery({delay:100,jitter:150,random:()=>values.shift()??0});
 q.enqueue(0,0,1000);q.enqueue(1,50,1050);assert.equal(q.receive(249),null);assert.equal(q.receive(250).frame,0);assert.equal(q.receive(251).frame,1);
 q.stall(300,2000);for(let i=2;i<10;i++)q.enqueue(i,300+i*100,1000+i*100);
 assert.equal(q.receive(2200),null);const latest=q.receive(2300);assert.equal(latest.frame,9);assert.equal(latest.coalesced,7);assert.equal(q.queue.length,0);
});
test('timing queue is bounded rather than accumulating unlimited stalled data',()=>{
 const q=new SimulatedDelivery();for(let i=0;i<128;i++)q.enqueue(i,i,1000+i);assert.throws(()=>q.enqueue(128,128,1128),/queue/);
});

test('real cut recording retains both capped halves and releases old geometry across replay loops',async()=>{
 const {readFile}=await import('node:fs/promises');const {WireCache}=await import('../tools/scene-observer/protocol.mjs');
 const root=new URL('../tools/scene-observer/fixtures/network-demo/',import.meta.url);
 const manifest=JSON.parse(await readFile(new URL('manifest.json',root),'utf8')),data=await readFile(new URL('frames.bin',root));
 assert.ok(manifest.cuts.length>=40,'fixture must exercise repeated actual cuts');
 const events=new Map(manifest.cuts.map(c=>[c.frame,c])),cache=new WireCache();let checked=0;
 for(let loop=0;loop<2;loop++)for(let i=0;i<manifest.frames.length;i++){
  const {offset,length}=manifest.frames[i],wire=new Uint8Array(data.subarray(offset,offset+length));
  new DataView(wire.buffer).setBigUint64(8,BigInt(loop*manifest.frames.length+i+1),true);cache.ingest(wire);
  const cut=events.get(i);if(cut){assert.equal(cut.pieces.length,2);for(const id of cut.pieces)assert.ok(cache.assets[0].has(id),'missing actual cut mesh '+id);checked++;}
  assert.ok(cache.assets[0].size<=8,'expired cut geometry leaked');
 }
 assert.equal(checked,manifest.cuts.length*2);
});
