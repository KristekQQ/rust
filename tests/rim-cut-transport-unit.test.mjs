import test from 'node:test';
import assert from 'node:assert/strict';
import {installCutTransport} from '../examples/rim/cut-transport.js';

test('XR jobs survive suspended document RAF, lost replies and late tokens',()=>{
 const saved=new Map(),replace=(name,value)=>{saved.set(name,Object.getOwnPropertyDescriptor(globalThis,name));Object.defineProperty(globalThis,name,{configurable:true,value});};
 let now=0,nextFrame=0,worker,failures=0,accepted=0;
 const raf=new Map(),jobs=[1,2,3,4,5];
 class WorkerStub {
  sent=[];
  constructor(){worker=this;}
  postMessage(buffer){this.sent.push(new DataView(buffer).getUint32(0,true));}
  terminate(){this.terminated=true;}
 }
 const xr=new EventTarget();xr.isPresenting=false;
 const scene={rimCutFailed:()=>failures++,rimAcceptCut:()=>accepted++,rimTakeCutJob:()=>{
  if(!jobs.length)return new Uint8Array();
  const bytes=new Uint8Array(4);new DataView(bytes.buffer).setUint32(0,jobs.shift(),true);return bytes;
 }};
 let stop;
 try {
  replace('window',{});replace('Worker',WorkerStub);replace('performance',{now:()=>now});
  replace('requestAnimationFrame',fn=>{raf.set(++nextFrame,fn);return nextFrame;});replace('cancelAnimationFrame',id=>raf.delete(id));
  stop=installCutTransport(scene,xr);
  assert.equal(raf.size,1);
  xr.isPresenting=true;xr.dispatchEvent(new Event('start'));
  assert.equal(raf.size,0,'entry cancels desktop scheduling');
  for(let i=0;i<3;i++)xr.dispatchEvent(new Event('frame'));
  assert.deepEqual(worker.sent,[1,2],'XR drives bounded jobs without document frames');
  now=10001;xr.dispatchEvent(new Event('frame'));
  assert.deepEqual(worker.sent,[1,2,3],'lost replies cannot permanently consume all slots');
  const reply=token=>{const bytes=new Uint8Array(12);new DataView(bytes.buffer).setUint32(0,token,true);worker.onmessage({data:{result:bytes.buffer}});};
  reply(1);assert.equal(accepted,0,'expired reply never publishes');
  xr.dispatchEvent(new Event('frame'));assert.deepEqual(worker.sent,[1,2,3,4]);
  reply(2);xr.dispatchEvent(new Event('frame'));assert.equal(worker.sent.length,4,'late old reply does not release a current slot');
  reply(3);assert.equal(accepted,1);xr.dispatchEvent(new Event('frame'));assert.deepEqual(worker.sent,[1,2,3,4,5]);
  assert.equal(window.rimCutTransportDiagnostics.expired,2);
  xr.isPresenting=false;xr.dispatchEvent(new Event('end'));assert.equal(raf.size,1,'exit restarts desktop transport');
  stop();stop=null;assert.equal(worker.terminated,true);assert.equal(raf.size,0);assert.equal(failures,1);
  xr.dispatchEvent(new Event('frame'));assert.equal(worker.sent.length,5,'dispose removes XR listener');
 }finally {stop?.();for(const [name,descriptor]of saved)descriptor?Object.defineProperty(globalThis,name,descriptor):delete globalThis[name];}
});
