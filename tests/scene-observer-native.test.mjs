import test from 'node:test';
import assert from 'node:assert/strict';
import {once} from 'node:events';
import dgram from 'node:dgram';
import {WebSocket} from 'ws';
import {createObserverServer} from '../tools/scene-observer/server.mjs';
import {decodeNativePose} from '../tools/scene-observer/native-poses.mjs';
import {startNativePoseFixture} from './scene-observer-native-fixture.mjs';
const token='1234567890abcdef1234567890abcdef';
const until=async(fn)=>{const end=Date.now()+4000;while(!await fn()){if(Date.now()>end)throw Error('Native bridge timeout');await new Promise(resolve=>setTimeout(resolve,10));}};
test('authenticated UDP tracking normalizes device clocks and coalesces while viewer is busy',async()=>{
    const relay=await createObserverServer({port:0,nativePoses:{port:0,host:'127.0.0.1',token}});let source,viewer;
    try{
        source=await startNativePoseFixture({port:relay.nativeBridge.port,token,clockOffsetMs:31000});
        await until(()=>relay.nativeBridge.latest);
        assert.equal(relay.nativeBridge.latest.state.swords,2);assert.ok(Math.abs(Date.now()-relay.nativeBridge.latest.sampledAt)<100);
        viewer=new WebSocket(relay.url.replace('http:','ws:')+'/observer?role=viewer&source=native');
        const messages=[];viewer.on('message',bytes=>messages.push(JSON.parse(bytes.toString())));await once(viewer,'open');
        await until(()=>messages.some(m=>m.type==='session'));viewer.send(JSON.stringify({type:'ready'}));
        await until(()=>messages.some(m=>m.type==='pose-frame'));
        const first=messages.find(m=>m.type==='pose-frame');assert.equal(first.meta.native,true);assert.equal(first.meta.state.head.position.length,3);
        await new Promise(resolve=>setTimeout(resolve,120));assert.equal(messages.filter(m=>m.type==='pose-frame').length,1);
        viewer.send(JSON.stringify({type:'ack'}));await until(()=>messages.filter(m=>m.type==='pose-frame').length===2);
        const second=messages.filter(m=>m.type==='pose-frame')[1];assert.ok(BigInt(second.meta.sequence)>BigInt(first.meta.sequence)+1n);
        assert.ok(Date.now()-second.meta.sampledAt<100);
        const invalid=dgram.createSocket('udp4');invalid.send(Buffer.alloc(120),relay.nativeBridge.port,'127.0.0.1');await until(()=>relay.nativeBridge.status().rejected>0);invalid.close();
    }finally{viewer?.terminate();await source?.close();await relay.close();}
});
test('native packet decoder rejects malformed or nonfinite transforms before rendering',()=>{
    const packet=Buffer.alloc(120);packet.write('NPO1');Buffer.from(token,'hex').copy(packet,4);packet.writeBigUInt64LE(1n,20);packet.writeBigUInt64LE(1n,28);packet.writeBigUInt64LE(1000n,36);packet.writeUInt32LE(7,44);
    assert.equal(decodeNativePose(packet,Buffer.from(token,'hex')).poses.length,3);
    packet.writeFloatLE(NaN,48);assert.throws(()=>decodeNativePose(packet,Buffer.from(token,'hex')),/transform/);
    assert.throws(()=>decodeNativePose(packet,Buffer.alloc(16)),/packet/);
});
