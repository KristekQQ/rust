import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {SceneInbox} from '../tools/scene-observer/scene-inbox.mjs';
import {WireCache} from '../tools/scene-observer/protocol.mjs';
const fixture=async name=>new Uint8Array(await readFile(new URL(`fixtures/native-scene/${name}.nwr`,import.meta.url)));
test('unrendered cut publications retain introduced assets and only the latest state',async()=>{
    const inbox=new SceneInbox(),mirror=new WireCache();
    inbox.receive({meta:{state:'initial'},wire:await fixture('initial')});
    inbox.receive({meta:{state:'cut'},wire:await fixture('cut')});
    const packet=inbox.take();assert.equal(packet.meta.state,'cut');mirror.ingest(packet.wire);
    const ids=JSON.parse(await readFile(new URL('fixtures/native-scene/ids.json',import.meta.url)));
    assert.deepEqual([...mirror.assets[0].keys()],ids.halves);
    assert.equal(inbox.take(),null);
    inbox.receive({meta:{state:'removed'},wire:await fixture('removed')});mirror.ingest(inbox.take().wire);
    assert.equal(mirror.assets[0].size,0);
});
test('new transport rebases consecutive sequence and all renderer invalidation counters',async()=>{
    const revisions=[19n,29n,39n,49n],inbox=new SceneInbox({sequence:7n,revisions});
    const initial=await fixture('initial'),cut=await fixture('cut');
    inbox.receive({meta:{},wire:initial});const first=inbox.take();
    const h=new DataView(first.wire.buffer);assert.equal(h.getBigUint64(8,true),8n);
    [16,24,32,40].forEach((at,i)=>assert.equal(h.getBigUint64(at,true),revisions[i]+1n));
    inbox.receive({meta:{},wire:cut});const second=inbox.take();
    const next=new DataView(second.wire.buffer),a=new DataView(initial.buffer),b=new DataView(cut.buffer);
    assert.equal(next.getBigUint64(8,true),9n);
    [16,24,32,40].forEach((at,i)=>assert.equal(next.getBigUint64(at,true),BigInt.asUintN(64,revisions[i]+1n+b.getBigUint64(at,true)-a.getBigUint64(at,true))));
});

test('buffered playback preserves every topology boundary and rebases ordered deltas',async()=>{
 const inbox=new SceneInbox({buffered:true,sequence:20n}),mirror=new WireCache();
 for(const name of ['initial','cut','removed'])inbox.receive({meta:{name},wire:await fixture(name)});
 let sequence=21n;
 for(const name of ['initial','cut','removed']){
  const frame=inbox.take();assert.equal(frame.meta.name,name);
  assert.equal(new DataView(frame.wire.buffer).getBigUint64(8,true),sequence++);
  mirror.ingest(frame.wire);
  assert.equal(mirror.assets[0].size,{initial:0,cut:2,removed:0}[name]);
 }
 assert.equal(inbox.take(),null);assert.equal(inbox.queuedBytes,0);
});
test('buffered backlog fails explicitly rather than discarding required assets',async()=>{
 const frame={meta:{},wire:await fixture('initial')};
 const bytes=new SceneInbox({buffered:true,maxBytes:1});
 assert.throws(()=>bytes.receive(frame),/backlog/);assert.equal(bytes.take(),null);
 const count=new SceneInbox({buffered:true,maxFrames:1});count.receive(frame);
 const cut=await fixture('cut');assert.throws(()=>count.receive({meta:{},wire:cut}),/backlog/);
 assert.ok(count.take());assert.equal(count.take(),null);
});
