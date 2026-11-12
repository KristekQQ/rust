import test from 'node:test';
import assert from 'node:assert/strict';
import {AudioMixer,AudioClip} from '../js/audio.js';
const param=()=>({value:0,setValueAtTime(){},setTargetAtTime(v){this.value=v;}});
function context(){const node=()=>({connect(){},disconnect(){},gain:param(),threshold:param(),knee:param(),ratio:param(),attack:param(),release:param(),playbackRate:param(),positionX:param(),positionY:param(),positionZ:param(),start(){},stop(){}});return {state:'running',currentTime:0,listener:{setPosition(){},setOrientation(){}},destination:{},createGain:node,createDynamicsCompressor:node,createBufferSource:node,createPanner:node,close:async()=>{},suspend:async()=>{},resume:async()=>{}};}
const setup=()=>{const m=new AudioMixer({context:context()});m.groups=[{prefix:'blade_swing',limit:2}];for(const id of ['fruit_cut_soft_1','blade_swing_1','combo_triple'])m.clips.set(id,new AudioClip(id,{}));return m;};
const ev=(id,at=0)=>({id,at,clip:'fruit_cut_soft_1',position:[0,0,-1],gain:.6,pitch:1,priority:2});
const snapshot=(events,clock=.1,epoch=1)=>({epoch,clock,latest:events.at(-1)?.id??0,events});
test('coalesced journal exactly once, late delivery expires',()=>{const m=setup();const s=snapshot([ev(1),ev(2,.05)]);m.ingest(s);m.ingest(s);assert.equal(m.stats.played,2);m.ingest(snapshot([ev(3,.1)],.2),{transportAge:.4});assert.equal(m.stats.played,2);assert.equal(m.stats.expired,1);});
test('cold observer/reconnect suppresses history, new round plays',()=>{const m=setup();m.ingest(snapshot([ev(1)]),{suppressInitial:true});assert.equal(m.stats.played,0);m.ingest(snapshot([ev(1),ev(2)]),{suppressInitial:true});assert.equal(m.stats.played,1);m.ingest(snapshot([ev(1)],.1,2),{suppressInitial:true});assert.equal(m.stats.played,2);m.reset();m.ingest(snapshot([ev(1),ev(2)],.1,2),{suppressInitial:true});assert.equal(m.stats.played,2);});
test('voice limits, invalid values, cleanup',()=>{const m=setup();for(let i=0;i<50;i++)m.play('blade_swing_1');assert.equal(m.voices.size,2);assert.equal(m.play('fruit_cut_soft_1',{position:[NaN,0,0]}),null);assert.equal(m.play('fruit_cut_soft_1',{gain:NaN}),null);m.stopAll();assert.equal(m.voices.size,0);});
test('expired/paused journal cannot replay after resume',()=>{const m=setup();m.context.state='suspended';m.ingest(snapshot([ev(1)]));m.context.state='running';m.ingest(snapshot([ev(1)]));assert.equal(m.stats.played,0);m.ingest(snapshot([ev(2)],2));assert.equal(m.stats.played,0);});
test('out of order publications and malformed metadata ignored',()=>{const m=setup();m.ingest(snapshot([ev(4)]));m.ingest(snapshot([ev(3)]));m.ingest({epoch:1,latest:NaN,clock:0,events:[]});assert.equal(m.stats.played,1);assert.equal(m.latest,4);assert.equal(m.stats.gaps,3);});

test('hidden output consumes journal without new voices',()=>{const m=setup();m.setOutputEnabled(false);m.ingest(snapshot([ev(1)]));assert.equal(m.stats.played,0);m.setOutputEnabled(true);m.ingest(snapshot([ev(1),ev(2)]));assert.equal(m.stats.played,1);});

test('Rust policy master and empty-prefix group apply across clips',()=>{const m=setup();m.ingest({...snapshot([]),policy:{maxVoices:24,master:0,groups:[{prefix:'',limit:2}]}});m.play('fruit_cut_soft_1');m.play('blade_swing_1');m.play('combo_triple');assert.equal(m.master.gain.value,0);assert.equal(m.voices.size,2);});

test('higher priority survives overload, low priority cannot displace it',()=>{
 const m=setup();m.maxVoices=2;
 const low=m.play('blade_swing_1',{priority:0});m.play('fruit_cut_soft_1',{priority:2});
 const reward=m.play('combo_triple',{priority:3});
 assert.ok(reward);assert.equal(m.voices.size,2);assert.ok(low.stopped);
 assert.equal(m.play('blade_swing_1',{priority:0}),null);assert.ok(m.voices.has(reward));
});
test('user volume remains proportional across policy changes and mute',()=>{
 const m=setup();m.setVolume(.5);
 const policy={maxVoices:24,master:.4,groups:[]};m.ingest({...snapshot([]),policy});
 assert.equal(m.master.gain.value,.2);m.setVolume(0);
 m.ingest({...snapshot([]),policy:{...policy,master:.8}});assert.equal(m.master.gain.value,0);
});
test('delayed scheduling is cancelled on reset and disposal prevents playback',async()=>{
 const m=setup();const source=m.play('combo_triple',{delay:.2});m.reset();
 assert.equal(source.stopped,true);assert.equal(m.voices.size,0);
 await m.dispose();assert.equal(m.clips.size,0);assert.equal(m.play('combo_triple'),null);
});
