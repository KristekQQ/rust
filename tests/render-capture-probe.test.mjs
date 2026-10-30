import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

const source=fs.readFileSync(new URL('../scripts/render-capture-probe.js',import.meta.url),'utf8');
function fixture(){
 class GPUBuffer { constructor(d){this.size=d.size;} }
 class GPUDevice { createBuffer(d){return new GPUBuffer(d);} createCommandEncoder(){return new GPUCommandEncoder();} }
 class GPUCommandEncoder { copyBufferToBuffer(){} finish(){return {};} }
 class GPUQueue { writeBuffer(){} submit(){} }
 const context={GPUBuffer,GPUDevice,GPUCommandEncoder,GPUQueue,TextEncoder,Uint8Array,WeakRef,requestAnimationFrame:()=>0};
 context.window=context;
 vm.runInNewContext(source,context);
 return {probe:context.captureProbe,device:new GPUDevice(),queue:new GPUQueue(),GPUBuffer};
}
test('capture probe keeps known buffer-copy shadows valid',()=>{
 const {probe,device,queue}=fixture();
 const a=device.createBuffer({size:16}),b=device.createBuffer({size:16});
 queue.writeBuffer(a,0,new Uint8Array(16).fill(7));
 const encoder=device.createCommandEncoder();encoder.copyBufferToBuffer(a,0,b,0,16);queue.submit([encoder.finish()]);
 assert.equal(probe.state().limitations.length,0);
});
test('capture probe rejects unknown copy content instead of claiming a valid shadow',()=>{
 const {probe,device,queue,GPUBuffer}=fixture();
 const unknown=new GPUBuffer({size:16}),dst=device.createBuffer({size:16});
 const encoder=device.createCommandEncoder();encoder.copyBufferToBuffer(unknown,0,dst,0,16);queue.submit([encoder.finish()]);
 assert.ok(probe.state().limitations.includes('copy references unavailable buffer content'));
});
