import test from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import {once} from 'node:events';
import {readFile} from 'node:fs/promises';
import WebSocket from 'ws';
import {createObserverServer} from '../tools/scene-observer/server.mjs';
import {WireCache,decodePacket} from '../tools/scene-observer/protocol.mjs';
import {encodeByteDelta,decodeByteDelta,isByteDelta} from '../tools/scene-observer/byte-delta.mjs';
import {startNativePoseFixture} from './scene-observer-native-fixture.mjs';
const root=new URL('./fixtures/native-scene/',import.meta.url);
const token='05'.repeat(16);
const wait=async predicate=>{for(let i=0;i<200;i++){if(await predicate())return;await new Promise(r=>setTimeout(r,10));}throw Error('Condition timeout');};
const sourceBases=new WeakMap();
async function frame(socket,wire,state={}){
 const encoded=encodeByteDelta(sourceBases.get(socket),new Uint8Array(wire));sourceBases.set(socket,{sequence:encoded.sequence,bytes:new Uint8Array(wire)});wire=encoded.payload;
 const meta=Buffer.from(JSON.stringify({session:'rust-cut',sampledAt:Date.now(),state}));
 const lengths=Buffer.alloc(8);lengths.writeUInt32LE(meta.length);lengths.writeUInt32LE(wire.length,4);
 const ack=once(socket,'data');socket.write(Buffer.concat([lengths,meta,wire]));assert.deepEqual((await ack)[0],Buffer.from([1]));
}
async function viewer(url){
 const ws=new WebSocket(url.replace('http:','ws:')+'/observer?role=viewer&source=native&encoding=ndp1');
 const packets=[],sessions=[];let transport=null;
 ws.on('message',(data,binary)=>{if(binary){const p=new Uint8Array(data);if(isByteDelta(p)){transport=decodeByteDelta(transport,p);packets.push(decodePacket(transport.bytes));}else packets.push(decodePacket(p));}else{const m=JSON.parse(data);if(m.type==='session'){sessions.push(m);ws.send(JSON.stringify({type:'ready'}));}}});
 await once(ws,'open');return {ws,packets,sessions};
}
test('real Rust variant cut, coalescing, removal and late subscriber retain coherent generic scene',async()=>{
 const initial=await readFile(new URL('initial.nwr',root)),cut=await readFile(new URL('cut.nwr',root)),removed=await readFile(new URL('removed.nwr',root));
 const ids=JSON.parse(await readFile(new URL('ids.json',root),'utf8'));
 const relay=await createObserverServer({port:0,nativePoses:{port:0,scenePort:0,host:'127.0.0.1',token}});
 let pose,source,first,late;
 try{
  pose=await startNativePoseFixture({port:relay.nativeBridge.port,token});await wait(()=>relay.nativeBridge.latest);
  const bridge=await(await fetch(relay.url+'/api/scene')).json();
  source=net.connect(bridge.port,'127.0.0.1');await once(source,'connect');source.write(Buffer.concat([Buffer.from('NSC1'),Buffer.from(token,'hex')]));
  await frame(source,initial,{flying:[ids.source],fragments:0});
  first=await viewer(relay.url);await wait(()=>first.packets.length===1);
  assert.equal(first.sessions[0].scene,true);
  const mirror=new WireCache();mirror.ingest(first.packets[0].wire);
  assert.deepEqual([...mirror.assets[1].keys()],[ids.model]);
  // Native ingest advances while this browser deliberately withholds ACK.
  await frame(source,cut,{flying:[],fragments:2});
  late=await viewer(relay.url);await wait(()=>late.packets.length===1);
  const lateMirror=new WireCache();lateMirror.ingest(late.packets[0].wire);
  assert.deepEqual([...lateMirror.assets[0].keys()],ids.halves);
  first.ws.send(JSON.stringify({type:'ack'}));await wait(()=>first.packets.length===2);
  mirror.ingest(first.packets[1].wire);assert.deepEqual([...mirror.assets[0].keys()],ids.halves);
  await frame(source,removed,{fragments:0});first.ws.send(JSON.stringify({type:'ack'}));await wait(()=>first.packets.length===3);
  mirror.ingest(first.packets[2].wire);assert.equal(mirror.assets[0].size,0);
 }finally{first?.ws.terminate();late?.ws.terminate();source?.destroy();await pose?.close();await relay.close();}
});
test('an unauthenticated idle socket does not occupy the authenticated publisher slot',async()=>{
 const relay=await createObserverServer({port:0,nativePoses:{port:0,scenePort:0,host:'127.0.0.1',token}});let idle,source;
 try{
  const {port}=await(await fetch(relay.url+'/api/scene')).json();
  idle=net.connect(port,'127.0.0.1');await once(idle,'connect');idle.write('N');
  source=net.connect(port,'127.0.0.1');await once(source,'connect');source.write(Buffer.concat([Buffer.from('NSC1'),Buffer.from(token,'hex')]));
  await frame(source,await readFile(new URL('initial.nwr',root)));assert.equal((await(await fetch(relay.url+'/api/scene')).json()).accepted,1);
 }finally{idle?.destroy();source?.destroy();await relay.close();}
});

test('a restored real snapshot can be viewed while native tracking is offline',async()=>{
 const nativeSnapshot={meta:{session:'restored-session',mode:'live',sampledAt:Date.now()-6000,clockUncertaintyMs:5,state:{flying:[],fragments:0}},wire:new Uint8Array(await readFile(new URL('initial.nwr',root)))};
 const relay=await createObserverServer({port:0,nativeSnapshot,nativePoses:{port:0,scenePort:0,host:'127.0.0.1',token}});let client;
 try{client=await viewer(relay.url);await wait(()=>client.packets.length===1);assert.equal(client.packets[0].meta.session,'restored-session');assert.ok(Date.now()-client.packets[0].meta.sampledAt>5000);assert.equal((await(await fetch(relay.url+'/api/scene')).json()).connected,false);}
 finally{client?.ws.terminate();await relay.close();}
});
