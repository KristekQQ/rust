import test from 'node:test';
import assert from 'node:assert/strict';
import {deflateSync} from 'node:zlib';
import {readFile} from 'node:fs/promises';
import {unwrapNativePacket} from '../tools/scene-observer/native-compression.mjs';
import {encodeByteDelta,decodeByteDelta} from '../tools/scene-observer/byte-delta.mjs';
const limit=32*1024*1024;
function wrap(bytes){const h=Buffer.alloc(8);h.write('NZP1');h.writeUInt32LE(bytes.length,4);return Buffer.concat([h,deflateSync(bytes,{level:1})]);}
test('compressed real cut geometry and subsequent delta remain byte exact',async()=>{
 let source,receiver;
 for(const name of ['initial','cut','motion','removed']){
  const bytes=new Uint8Array(await readFile(new URL(`./fixtures/native-scene/${name}.nwr`,import.meta.url)));
  const encoded=encodeByteDelta(source,bytes),compressed=wrap(encoded.payload);
  assert.deepEqual(new Uint8Array(unwrapNativePacket(compressed,limit)),encoded.payload);
  receiver=decodeByteDelta(receiver,unwrapNativePacket(compressed,limit));
  assert.deepEqual(receiver.bytes,bytes);source={sequence:encoded.sequence,bytes};
 }
});
test('compressed envelope rejects corruption, trailing data, nested payload and excessive expansion',()=>{
 const raw=encodeByteDelta(null,new Uint8Array(20000)).payload,good=wrap(raw);
 assert.equal(unwrapNativePacket(raw,limit),raw);
 const alias=Buffer.from(good);for(let i=0;i<4;i++)alias[i]|=128;assert.equal(unwrapNativePacket(alias,limit),alias);
 const innerAlias=Buffer.from(raw);for(let i=0;i<4;i++)innerAlias[i]|=128;assert.throws(()=>unwrapNativePacket(wrap(innerAlias),limit));
 const bad=Buffer.from(good);bad[bad.length-1]^=1;
 for(const value of [bad,good.subarray(0,good.length-1),Buffer.concat([good,Buffer.from([0])]),wrap(good)])assert.throws(()=>unwrapNativePacket(value,limit));
 const small=Buffer.from(good);small.writeUInt32LE(24,4);assert.throws(()=>unwrapNativePacket(small,limit));
 const big=Buffer.from(good);big.writeUInt32LE(limit+25,4);assert.throws(()=>unwrapNativePacket(big,limit));
 assert.throws(()=>unwrapNativePacket(good,100));
});
