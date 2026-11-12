import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { WebSocket } from 'ws';
import { WireCache, encodePacket, decodePacket, encodeRecording, decodeRecording } from '../tools/scene-observer/protocol.mjs';
import { createObserverServer } from '../tools/scene-observer/server.mjs';
import { ScenePublisher } from '../tools/scene-observer/publisher.js';
class Writer {
  a = [];
  byte(v) { this.a.push(v); return this; }
  uint(v) { const b = Buffer.alloc(4); b.writeUInt32LE(v); this.a.push(...b); return this; }
  big(v) { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(v)); this.a.push(...b); return this; }
  float(v = 0) { const b = Buffer.alloc(4); b.writeFloatLE(v); this.a.push(...b); return this; }
  floats(n, v = 0) { for (let i=0;i<n;i++) this.float(v); return this; }
  raw(v) { this.a.push(...v); return this; }
  done() { return new Uint8Array(this.a); }
}
function mesh(w, shade = 1) {
  w.floats(3).floats(3, 1).uint(3);
  for (let i=0;i<3;i++) w.float(i === 1 ? 1:0).float(i === 2 ? 1:0).float(0).floats(3).floats(3,shade);
  w.uint(3).uint(0).uint(1).uint(2);
}
function meshData(w, shade) { w.floats(3).floats(3,1).uint(1).uint(3).big(0); mesh(w,shade); w.byte(0); }
function object(w, id, meshId = id) {
  w.uint(id).byte(3).uint(meshId).floats(6).floats(3,1).byte(0).byte(1).byte(1).float(0).byte(0).floats(8).byte(0).byte(0).byte(0).floats(50);
}
function frame(sequence, {changed = [], live = [], shade = 1, variant = false, variantChanged = variant, panel = false, instances = {}, tags = false, lights = false, version = 1, lineage = []} = {}) {
  const w = new Writer().raw(Buffer.from('NWR1')).uint(version).big(sequence).big(sequence).big(1).big(1).big(1).uint(0xffffffff).uint(100);
  if (version >= 2) w.uint(0xffffffff).uint(0xffffffff);
  w.uint(changed.length); for (const id of changed) { w.uint(id); meshData(w,shade); }
  w.uint(variantChanged ? 1:0);
  if (variantChanged) { w.uint(80); meshData(w,shade); mesh(w,shade); w.uint(0).uint(1).floats(4,1).uint(0).uint(1); }
  w.uint(panel ? 1:0);
  if (panel) { w.uint(90).byte(0).float(1).float(1).big(1).byte(0); mesh(w); w.byte(0); }
  w.uint(live.length + (panel ? 1:0)); for (const id of live) object(w,id,instances[id] ?? id); if (panel) object(w,90);
  w.uint(live.length); for (const id of live) { w.uint(id).byte(1).floats(3).byte(instances[id] === undefined ? 0:1); if (instances[id] !== undefined) w.uint(instances[id]).uint(0); }
  w.uint(variant ? 1:0); if (variant) w.uint(80).byte(1);
  w.uint(panel ? 1:0); if (panel) w.uint(90).big(1);
  w.uint(lights ? 1:0); if (lights) w.uint(70).floats(6,1);
  w.byte(tags ? 1:0); if (tags) for(let i=0;i<10;i++) w.uint(80);
  if (tags) w.uint(1).uint(live[0] ?? 90).uint(1).uint(80).uint(1).uint(live[1] ?? 90).uint(1).uint(90);
  else w.uint(0).uint(0).uint(0).uint(0);
  if (version >= 3) w.uint(0);
  if (version >= 4) { w.uint(lineage.length); for (const [child,parent,relative] of lineage) { w.uint(child).uint(parent); for (const v of relative ?? [1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]) w.float(v); } }
  return w.done();
}
const meta = (session='one', sampledAt=Date.now()) => ({session,sampledAt,state:{round:1},mode:'demo'});
const peer = () => ({sequence:0,known:[new Map(),new Map(),new Map()]});
test('v4 lineage survives coalescing, late peers, reconnect, recording and child projection', () => {
  const relative = [1,0,0,0,0,1,0,0,0,0,1,0,.125,.25,-.1,1];
  const lineage = [[1,0,relative],[2,0,relative]];
  const expectedLineage = entries => { const w=new Writer().uint(entries.length); for(const [child,parent,model] of entries) { w.uint(child).uint(parent); for(const v of model)w.float(v); } return w.done(); };
  const cache = new WireCache();
  cache.ingest(frame(1,{version:4,changed:[0],live:[0]}));
  const connected = peer(); cache.packetFor(connected);
  cache.ingest(frame(2,{version:4,changed:[1,2],live:[1,2],lineage}));
  cache.ingest(frame(3,{version:4,live:[1,2],lineage}));
  for (const recipient of [connected,peer(),peer()]) {
    const wire = cache.packetFor(recipient);
    assert.deepEqual(wire.slice(-148), expectedLineage(lineage));
    const projected = new WireCache({objectIds:[2]}); projected.ingest(wire);
    const selected = projected.packetFor(peer());
    assert.deepEqual(selected.slice(-76), expectedLineage([lineage[1]]));
    const packets = decodeRecording(encodeRecording([encodePacket(meta(), selected)]));
    assert.deepEqual(decodePacket(packets[0]).wire, selected);
  }
  const prior = cache.sequence;
  for (const lineage of [[[1,2]], [[1,0],[1,0]], [[0,1]], [[1,0xffffffff]], [[1,0,new Array(16).fill(0)]]]) {
    assert.throws(() => cache.ingest(frame(4,{version:4,live:[1,2],lineage})));
    assert.equal(cache.sequence, prior);
  }
  const legacy = new WireCache(); legacy.ingest(frame(1,{version:3,changed:[1],live:[1]}));
  assert.equal(new DataView(legacy.packetFor(peer()).buffer).getUint32(4,true),3);
});

test('live publisher reconnects after relay restart with a complete current sword snapshot', async () => {
  let relay = await createObserverServer({port:0}), publisher;
  const url = relay.url, port = Number(new URL(url).port);
  const until = async check => { const end = Date.now()+6000; while (!await check()) { if(Date.now()>end)throw Error('Reconnect timeout'); await new Promise(resolve=>setTimeout(resolve,20)); } };
  try {
    publisher = new ScenePublisher(url.replace('http:','ws:')+'/observer',{objectIds:[1]});
    publisher.publish(frame(1,{changed:[1,2],live:[1,2]}),{});
    await until(async()=>(await(await fetch(url+'/api/status')).json()).sequence==='1');
    const first = publisher.session;
    await relay.close();
    publisher.publish(frame(2,{live:[1,2]}),{});
    relay = await createObserverServer({port});
    await until(async()=>(await(await fetch(url+'/api/status')).json()).sequence==='1');
    assert.notEqual(publisher.session,first);
    const status=await(await fetch(url+'/api/status')).json();
    assert.equal(status.latest.state.swords,1);
    const packets=decodeRecording(new Uint8Array(await(await fetch(url+'/api/recording')).arrayBuffer()));
    const mirror=new WireCache();mirror.ingest(decodePacket(packets.at(-1)).wire);
    assert.deepEqual([...mirror.assets[0].keys()],[1]);
  } finally { publisher?.close();await relay.close(); }
});
test('latest cache preserves skipped creations, replaces updates, prunes removed IDs and initializes late joiners', () => {
  const cache = new WireCache(), viewer = peer(), mirror = new WireCache();
  cache.ingest(frame(1,{changed:[1],live:[1]})); mirror.ingest(cache.packetFor(viewer));
  cache.ingest(frame(2,{changed:[2],live:[1,2]}));
  cache.ingest(frame(3,{live:[2]}));
  const latest = cache.packetFor(viewer); mirror.ingest(latest);
  assert.equal(new DataView(latest.buffer).getBigUint64(8,true),2n);
  assert.deepEqual([...mirror.assets[0].keys()],[2]);
  assert.equal(viewer.known[0].has(1),false);
  const late = new WireCache(); late.ingest(cache.packetFor(peer())); assert.deepEqual([...late.assets[0].keys()],[2]);
  cache.ingest(frame(4,{changed:[2],live:[2],shade:.25})); mirror.ingest(cache.packetFor(viewer));
  assert.deepEqual(mirror.assets[0].get(2),cache.assets[0].get(2));
  cache.ingest(frame(5)); mirror.ingest(cache.packetFor(viewer)); assert.equal(mirror.assets[0].size,0);
});
test('all three asset tables are independently cached and emitted', () => {
  const cache = new WireCache(); cache.ingest(frame(1,{changed:[1],live:[1],variant:true,panel:true}));
  const mirror = new WireCache(); mirror.ingest(cache.packetFor(peer()));
  assert.deepEqual(mirror.assets.map(map=>[...map.keys()]),[[1],[80],[90]]);
});
test('malformed, missing assets, source order and cache limits fail transactionally', () => {
  const cache = new WireCache(); cache.ingest(frame(1));
  assert.throws(()=>cache.ingest(frame(1)),/Out-of-order/);
  assert.throws(()=>cache.ingest(frame(2,{live:[3]})),/Missing live asset/);
  assert.throws(()=>cache.ingest(frame(2).slice(0,-1)),/Truncated/);
  const trailing = new Uint8Array([...frame(2),0]); assert.throws(()=>cache.ingest(trailing),/Trailing/);
  assert.equal(cache.sequence,1n);
  assert.throws(()=>new WireCache({maxBytes:100}).ingest(frame(1,{changed:[1],live:[1]})),/budget/);
  const corrupt = frame(2,{changed:[1],live:[1]}); new DataView(corrupt.buffer).setUint32(60,0xffffffff,true);
  assert.throws(()=>cache.ingest(corrupt),/Invalid wire ID/);
});
test('packet and recording roundtrip exact envelopes and reject corruption', () => {
  const packets = [encodePacket(meta(),frame(1)),encodePacket(meta(),frame(2))];
  assert.deepEqual(decodeRecording(encodeRecording(packets)),packets);
  assert.deepEqual(decodePacket(packets[0]).wire,frame(1));
  assert.throws(()=>decodeRecording(encodeRecording(packets).slice(0,-1)),/Truncated/);
  assert.throws(()=>decodePacket(new Uint8Array([255,255,255,255])),/budget/);
  assert.throws(()=>encodePacket({...meta(),mode:'fake'},frame(1)),/metadata/);
});
async function connect(url) {
  const ws = new WebSocket(url), queue = [], waiting = [];
  ws.on('message',(data,binary)=> { const message = binary ? {binary:true,...decodePacket(new Uint8Array(data))} : JSON.parse(data.toString()); if (waiting.length) waiting.shift()(message); else queue.push(message); });
  ws.next = () => queue.length ? Promise.resolve(queue.shift()) : new Promise((resolve,reject)=> { const timer=setTimeout(()=>reject(new Error('WS message timeout')),2000); waiting.push(v=>{clearTimeout(timer);resolve(v);}); });
  ws.pending = () => queue.length;
  await once(ws,'open'); return ws;
}
test('WS readiness, bounded in-flight coalescing, reconnect, session reset, exact recording and origin checks', async t => {
  const observer = await createObserverServer({root:new URL('..',import.meta.url).pathname,port:0}); t.after(()=>observer.close());
  const url=observer.url.replace('http:','ws:')+'/observer?role=';
  const viewer=await connect(url+'viewer'), publisher=await connect(url+'publisher');
  publisher.send(JSON.stringify({type:'clock',sentAt:123})); const clock=await publisher.next(); assert.equal(clock.sentAt,123); assert.equal(clock.type,'clock'); assert.ok(clock.serverAt>0);
  const packets=[];
  async function publish(seq,options={},session='one') { const packet=encodePacket(meta(session,seq),frame(seq,options)); packets.push(packet); publisher.send(packet); assert.equal((await publisher.next()).type,'ack'); }
  await publish(1,{changed:[1],live:[1]}); assert.deepEqual(await viewer.next(),{type:'session',session:'one'}); assert.equal(viewer.pending(),0);
  viewer.send(JSON.stringify({type:'ready'})); const first=await viewer.next(); assert.equal(first.binary,true);
  await publish(2,{changed:[2],live:[1,2]}); await publish(3,{live:[2]});
  assert.equal(viewer.pending(),0); viewer.send(JSON.stringify({type:'ack'})); const latest=await viewer.next(); assert.equal(latest.meta.sampledAt,3);
  const mirror=new WireCache(); mirror.ingest(first.wire); mirror.ingest(latest.wire); assert.deepEqual([...mirror.assets[0].keys()],[2]);
  const reconnect=await connect(url+'viewer'); assert.equal((await reconnect.next()).type,'session'); reconnect.send(JSON.stringify({type:'ready'})); const late=await reconnect.next(); const lateMirror=new WireCache(); lateMirror.ingest(late.wire); assert.deepEqual([...lateMirror.assets[0].keys()],[2]);
  const tape=new Uint8Array(await (await fetch(observer.url+'/api/recording')).arrayBuffer()); assert.deepEqual(decodeRecording(tape),packets);
  const status=await (await fetch(observer.url+'/api/status')).json(); assert.equal(status.recording.packets,3); assert.equal(status.recording.full,false);
  assert.equal((await fetch(observer.url+'/api/status',{headers:{Origin:'https://evil.example'}})).status,403);
  await publish(1,{},'two'); assert.equal((await viewer.next()).session,'two'); assert.equal(viewer.pending(),0); viewer.send(JSON.stringify({type:'ready'})); assert.equal((await viewer.next()).meta.session,'two');
  const reset=await (await fetch(observer.url+'/api/status')).json(); assert.equal(reset.recording.packets,1);
  publisher.close(); await once(publisher,'close'); const newPublisher=await connect(url+'publisher'); newPublisher.send(encodePacket(meta('two'),frame(1))); assert.match((await newPublisher.next()).message,/Out-of-order/); await once(newPublisher,'close');
});
test('recording full is explicit and does not interrupt latest-state delivery', async t => {
  const first=encodePacket(meta('limited',1),frame(1));
  const observer=await createObserverServer({root:new URL('..',import.meta.url).pathname,port:0,recordingLimit:first.length+8}); t.after(()=>observer.close());
  const publisher=await connect(observer.url.replace('http:','ws:')+'/observer?role=publisher');
  publisher.send(first); assert.equal((await publisher.next()).recordingFull,false);
  publisher.send(encodePacket(meta('limited',2),frame(2))); assert.equal((await publisher.next()).recordingFull,true);
  const status=await (await fetch(observer.url+'/api/status')).json(); assert.equal(status.recording.full,true); assert.equal(status.recording.packets,1); assert.equal(status.sequence,'2');
  const response=await fetch(observer.url+'/api/recording'); assert.equal(response.headers.get('x-recording-full'),'true');
  assert.deepEqual(decodeRecording(new Uint8Array(await response.arrayBuffer())),[first]);
});

test('objectIds sends only selected authoritative objects/assets, preserves lights, strips UI and handles variant rebinding', () => {
  const cache = new WireCache({objectIds:[1,2]}), viewer = peer(), receiver = new WireCache();
  cache.ingest(frame(1,{changed:[1,3],live:[1,2,3],instances:{2:80},variant:true,panel:true,tags:true,lights:true}));
  const first = cache.packetFor(viewer);
  assert.deepEqual(first,frame(1,{changed:[1],live:[1,2],instances:{2:80},variant:true,lights:true}));
  receiver.ingest(first);
  assert.deepEqual(cache.assets.map(map=>[...map.keys()]),[[1],[80],[]]);
  assert.ok(cache.sourceAssets[0].has(3));
  cache.ingest(frame(2,{live:[3],variant:true,variantChanged:false,panel:true,tags:true,lights:true}));
  const removed = cache.packetFor(viewer);
  assert.deepEqual(removed,frame(2,{lights:true})); receiver.ingest(removed);
  assert.deepEqual(receiver.assets.map(map=>map.size),[0,0,0]);
  cache.ingest(frame(3,{changed:[1],live:[1,2,3],instances:{2:80},variant:true,variantChanged:false,panel:true,tags:true,lights:true}));
  const rebound = cache.packetFor(viewer);
  assert.deepEqual(rebound,frame(3,{changed:[1],live:[1,2],instances:{2:80},variant:true,lights:true}));
  receiver.ingest(rebound); assert.deepEqual([...receiver.assets[1].keys()],[80]);
  assert.throws(()=>new WireCache({objectIds:[-1]}),/objectIds/);
});
