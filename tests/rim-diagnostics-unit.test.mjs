import test from 'node:test';
import assert from 'node:assert/strict';
import {installRimDiagnostics} from '../examples/rim/diagnostics.js';

function fixture(extra={}){
 let time=0,reads=0;
 const records=[],xr=new EventTarget(),target=new EventTarget();xr.isPresenting=true;
 const state={health:83,kills:2,wave:3,player:{position:[9,9,9]},blade:{contactSamples:25,hits:3,slowSamples:6,discontinuities:2,maxSpeedForFrame:1.5,position:[8,8,8]},enemies:[{id:4,hp:12,phase:3,position:[1,2,3]}],cuts:{queued:1,pending:2,staging:3,commits:4,errors:5,motion:[{direction:[9,9,9]}]}};
 const engine={xr,scene:{get rimState(){reads++;return state;}},renderer_backend:()=> 'BrowserWebGpu'};
 const dispose=installRimDiagnostics(engine,{build:'app-123',wasmBuild:'wasm-456',now:()=>time,eventTarget:target,userAgent:'Quest test',send:record=>records.push(record),getTransport:()=>({status:'ready',jobs:7,inflight:2}),...extra});
 const event=(name,detail)=>{const event=new Event(name);Object.assign(event,{detail});xr.dispatchEvent(event);};
 return {records,xr,target,engine,state,dispose,event,setTime:value=>time=value,reads:()=>reads};
}

test('XR samples use actual frames at 1 Hz, include IDs and cut phases, omit spatial data',()=>{
 const f=fixture();f.event('start');f.event('frame');
 for(let i=1;i<1000;i++){f.setTime(i);f.event('frame');}
 assert.equal(f.reads(),1);
 f.setTime(1000);f.event('frame');assert.equal(f.reads(),2);
 const sample=f.records.filter(r=>r.stage==='rim-xr-sample').at(-1);
 assert.equal(sample.build,'app-123');assert.equal(sample.wasmBuild,'wasm-456');assert.equal(sample.backend,'BrowserWebGpu');assert.equal(sample.userAgent,'Quest test');
 assert.deepEqual(sample.details.cuts,{queued:1,pending:2,staging:3,errors:5,commits:4});
 assert.deepEqual(sample.details.enemies,[{id:4,hp:12,phase:3}]);
 assert.deepEqual(sample.details.transport,{status:'ready',jobs:7,inflight:2});
 assert.deepEqual(sample.details.blade,{contactSamples:25,hits:3,slowSamples:6,discontinuities:2,maxSpeedForFrame:1.5});
 assert.equal(JSON.stringify(sample).includes('position'),false);
 assert.equal(JSON.stringify(sample).includes('direction'),false);
 f.xr.isPresenting=false;f.setTime(2000);f.event('frame');assert.equal(f.reads(),2);f.dispose();
});

test('sample cap retains lifecycle/errors and resets on session reentry; disposal removes listeners',()=>{
 const f=fixture({maxSamples:2});f.event('start');
 for(let i=0;i<8;i++){f.setTime(i*1000);f.event('frame');}
 assert.equal(f.reads(),2);f.event('performance',{fps:72,headForward:[1,0,0]});
 f.xr.lastError=new Error('render failed');f.event('error');f.event('error');
 f.event('firstframe',{presentation:'stereo-final-submit-v1'});f.event('end');
 assert.equal(f.records.filter(r=>r.stage==='rim-xr-error').length,1);
 assert.equal(f.records.filter(r=>r.stage==='rim-xr-firstframe').length,1);
 assert.equal(f.records.at(-1).stage,'rim-xr-end');
 f.event('start');f.event('frame');assert.equal(f.reads(),3);
 const count=f.records.length;f.dispose();f.dispose();
 f.setTime(20000);for(const name of ['start','frame','error','end','performance','firstframe'])f.event(name);
 const error=new Event('error');Object.assign(error,{error:new Error('after dispose')});f.target.dispatchEvent(error);
 assert.equal(f.records.length,count);assert.equal(f.reads(),3);
});

test('snapshot and bounded global errors are recorded without escaping into XR',()=>{
 const f=fixture();Object.defineProperty(f.engine.scene,'rimState',{get(){throw Error('snapshot broke');}});
 f.event('start');assert.doesNotThrow(()=>f.event('frame'));
 assert.equal(f.records.at(-1).stage,'rim-snapshot-error');
 for(let i=0;i<40;i++){const event=new Event('error');Object.assign(event,{error:new Error('failure '+i)});f.target.dispatchEvent(event);}
 assert.equal(f.records.filter(r=>r.stage==='rim-uncaught').length,19);f.dispose();
});

test('rejected or synchronously failing sender never disrupts rendering',async()=>{
 for(const send of [()=>{throw Error('offline');},()=>Promise.reject(Error('offline'))]){
  const f=fixture({send});assert.doesNotThrow(()=>{f.event('start');f.event('frame');f.event('end');});
  await new Promise(resolve=>setImmediate(resolve));f.dispose();
 }
});

test('XR runtime resolution and rates retain only bounded numeric metadata',()=>{
 const f=fixture();
 const rates=[72,90,120,'144',Infinity,NaN,-1,0,60,80,100,110,130,140];
 f.event('performance',{eyeViewports:[{eye:'left',width:1832,height:1920,pose:{position:[1,2,3]}},{eye:'right',width:1832,height:1920,extra:'omit'},{eye:'left',width:999,height:999}],supportedFrameRates:rates,headForward:[1,0,0]});
 assert.deepEqual(f.records.at(-1).details.eyeViewports,[{eye:'left',width:1832,height:1920},{eye:'right',width:1832,height:1920}]);
 assert.deepEqual(f.records.at(-1).details.supportedFrameRates,[72,90,120,60,80,100,110,130]);
 f.event('firstframe',{eyes:[{eye:'left',viewport:{width:1024,height:1080,x:4},color:{format:'private'}},{eye:'right',viewport:{width:1024,height:1080},transform:{position:[1,2,3]}}],supportedFrameRates:[90]});
 assert.deepEqual(f.records.at(-1).details,{supportedFrameRates:[90],eyeViewports:[{eye:'left',width:1024,height:1080},{eye:'right',width:1024,height:1080}]});
 f.event('performance',{eyeViewports:[{eye:'none',width:1,height:1},{eye:'left',width:Infinity,height:1},{eye:'right',width:1,height:0},{eye:'left',width:'1',height:1},null],supportedFrameRates:[{},null,'90']});
 assert.deepEqual(f.records.at(-1).details,{supportedFrameRates:[],eyeViewports:[]});
 assert.equal(JSON.stringify(f.records).includes('position'),false);assert.equal(JSON.stringify(f.records).includes('headForward'),false);f.dispose();
});

test('default sender posts same-origin JSON bytes with build IDs and safe fetch failure',async()=>{
 const saved=Object.getOwnPropertyDescriptor(globalThis,'fetch'),requests=[];
 Object.defineProperty(globalThis,'fetch',{configurable:true,value:(url,options)=>{requests.push({url,...options});return Promise.reject(Error('offline'));}});
 try{
  const f=fixture({send:undefined});f.event('start');f.event('frame');
  await new Promise(resolve=>setImmediate(resolve));
  const request=requests.find(r=>JSON.parse(r.body).stage==='rim-xr-sample');
  assert.equal(request.url,'/__diagnostics');assert.equal(request.method,'POST');assert.equal(request.headers['Content-Type'],'application/json');assert.equal(request.keepalive,true);
  assert.equal(JSON.parse(request.body).wasmBuild,'wasm-456');assert.ok(Buffer.byteLength(request.body)<32768);f.dispose();
 }finally {saved?Object.defineProperty(globalThis,'fetch',saved):delete globalThis.fetch;}
});
