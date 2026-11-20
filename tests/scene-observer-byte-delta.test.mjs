import test from 'node:test';import assert from 'node:assert/strict';
import {encodeByteDelta,decodeByteDelta,BYTE_LIMIT} from '../tools/scene-observer/byte-delta.mjs';
test('binary patches preserve exact bytes across change, same, growth, shrink and full fallback',()=>{
 let sender=null,receiver=null;
 const a=new Uint8Array(4096).fill(7),b=a.slice();b[123]=91;
 const cases=[a,a,b,new Uint8Array(4250).fill(3),new Uint8Array(3000).fill(3),new Uint8Array(),new Uint8Array(5).fill(8)];
 let saved=0;for(const bytes of cases){const p=encodeByteDelta(sender,bytes);receiver=decodeByteDelta(receiver,p.payload);assert.deepEqual(receiver.bytes,bytes);sender={sequence:p.sequence,bytes};saved+=bytes.length+24-p.payload.length;}
 assert.ok(saved>7000);
});
test('delta rejects wrong sequence/base, truncated ranges, trailing bytes and oversized targets',()=>{
 const a=new Uint8Array(2048).fill(9),full=encodeByteDelta(null,a),base=decodeByteDelta(null,full.payload),b=a.slice();b[2]=3;
 const p=encodeByteDelta(base,b).payload;assert.equal(p[4],1);
 assert.throws(()=>decodeByteDelta(null,p));
 const invalidBase=p.slice();new DataView(invalidBase.buffer).setUint32(12,123,true);assert.throws(()=>decodeByteDelta(base,invalidBase));
 assert.throws(()=>decodeByteDelta(base,p.slice(0,-1)));
 const badRange=p.slice();new DataView(badRange.buffer).setUint32(24,99999,true);assert.throws(()=>decodeByteDelta(base,badRange));
 const extra=new Uint8Array(p.length+1);extra.set(p);assert.throws(()=>decodeByteDelta(base,extra));
 const tooLarge=full.payload.slice();new DataView(tooLarge.buffer).setUint32(16,BYTE_LIMIT+1,true);assert.throws(()=>decodeByteDelta(null,tooLarge));
});
test('deterministic mixed edits and changing lengths roundtrip without base drift',()=>{
 let seed=7;const rand=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;};
 let sender=null,receiver=null,bytes=new Uint8Array(4096);
 for(let i=0;i<120;i++){
  if(i%17===0){const next=new Uint8Array(rand()%8000);next.set(bytes.subarray(0,next.length));bytes=next;}else bytes=bytes.slice();
  for(let j=0;j<12&&bytes.length;j++)bytes[rand()%bytes.length]=rand()%256;
  const p=encodeByteDelta(sender,bytes);receiver=decodeByteDelta(receiver,p.payload);assert.deepEqual(receiver.bytes,bytes);sender={sequence:p.sequence,bytes};
 }
});

test('aligned metadata keeps exact scene bytes and capacity across header growth/shrink',async()=>{
 const {readFile}=await import('node:fs/promises');
 const {encodePacket,decodePacket}=await import('../tools/scene-observer/protocol.mjs');
 const wire=new Uint8Array(await readFile(new URL('fixtures/native-scene/initial.nwr',import.meta.url)));
 let previous=null,receiver=null,capacity=0;const sizes=[];
 for(const score of [9,10,9,100]){
  const meta={session:'test',sampledAt:1,mode:'live',state:{score}};
  const packet=encodePacket(meta,wire,capacity);const next=new DataView(packet.buffer).getUint32(0,true);assert.ok(next>=capacity);capacity=next;
  const delta=encodeByteDelta(previous,packet);receiver=decodeByteDelta(receiver,delta.payload);
  const decoded=decodePacket(receiver.bytes);assert.deepEqual(decoded.meta,meta);assert.deepEqual(decoded.wire,wire);
  sizes.push(delta.payload.length);previous={sequence:delta.sequence,bytes:packet};
 }
 assert.ok(sizes.slice(1).every(n=>n<200));
 const meta={session:'test',sampledAt:1,mode:'live',state:{text:'x'.repeat(65000)}};
 assert.deepEqual(decodePacket(encodePacket(meta,wire,65536)).meta,meta);
 assert.throws(()=>encodePacket(meta,wire,65537),/capacity/);
});
