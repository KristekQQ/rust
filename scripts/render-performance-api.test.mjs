import test from 'node:test';
import assert from 'node:assert/strict';
import {installApiCounters} from './render-performance-api.mjs';

const names=['window','GPUDevice','GPUQueue','GPUCommandEncoder','GPURenderPassEncoder','WebGL2RenderingContext'];
async function globals(run){const saved=new Map(names.map(name=>[name,Object.getOwnPropertyDescriptor(globalThis,name)]));try{return await run();}finally{for(const [name,descriptor]of saved)if(descriptor)Object.defineProperty(globalThis,name,descriptor);else delete globalThis[name];}}

test('WebGPU direct draws count known pipeline topology and instances, including successful async creation',async()=>globals(async()=>{
 globalThis.window={};
 globalThis.GPURenderPassEncoder=class{setPipeline(p){this.pipeline=p;}draw(){}drawIndexed(){}drawIndirect(){}executeBundles(){}};
 globalThis.GPUCommandEncoder=class{beginRenderPass(){return new GPURenderPassEncoder();}finish(){return {};}};
 let resolveAsync;globalThis.GPUDevice=class{createRenderPipeline(descriptor){return {descriptor};}createRenderPipelineAsync(descriptor,reject=false){if(reject)return Promise.reject(Error('pipeline rejected'));if(descriptor.defer)return new Promise(resolve=>{resolveAsync=()=>resolve({descriptor});});return Promise.resolve({descriptor});}createCommandEncoder(){return new GPUCommandEncoder();}};
 globalThis.GPUQueue=class{submit(){}};
 installApiCounters();const device=new GPUDevice(),encoder=device.createCommandEncoder(),pass=encoder.beginRenderPass();
 const triangles=device.createRenderPipeline({primitive:{topology:'triangle-list'}});pass.setPipeline(triangles);pass.draw(6,2);
 const lines=await device.createRenderPipelineAsync({primitive:{topology:'line-strip'}});pass.setPipeline(lines);pass.draw(5,3);
 const defaults=await device.createRenderPipelineAsync({});pass.setPipeline(defaults);pass.drawIndexed(6,2);
 const descriptor={primitive:{topology:'triangle-list'},defer:true},pending=device.createRenderPipelineAsync(descriptor);descriptor.primitive.topology='line-strip';resolveAsync();pass.setPipeline(await pending);pass.draw(3);
 const c=window.renderApiCounters.webgpu;assert.deepEqual({drawCalls:c.submitted.drawCalls,instances:c.submitted.instances,primitives:c.submitted.primitives,triangles:c.submitted.triangles,lines:c.submitted.lines,complete:c.submitted.complete},{drawCalls:4,instances:8,primitives:21,triangles:9,lines:12,complete:true});
 assert.equal(c.createRenderPipeline,1);assert.equal(c.createRenderPipelineAsync,3);assert.equal(c.draw,3);assert.equal(c.drawIndexed,1);
 await assert.rejects(device.createRenderPipelineAsync({},true),/pipeline rejected/);assert.equal(c.createRenderPipelineAsync,3);
}));

test('unknown WebGPU topology and indirect or bundle draws mark totals incomplete without invented primitives',()=>globals(()=>{
 globalThis.window={};globalThis.GPURenderPassEncoder=class{setPipeline(){}draw(){}drawIndexed(){}drawIndirect(){}drawIndexedIndirect(){}executeBundles(){}};
 globalThis.GPUCommandEncoder=class{};globalThis.GPUDevice=class{createRenderPipeline(d){return {descriptor:d};}};
 installApiCounters();const device=new GPUDevice(),pass=new GPURenderPassEncoder();pass.setPipeline(device.createRenderPipeline({primitive:{topology:'quad-list'}}));pass.draw(6,2);pass.setPipeline(device.createRenderPipeline({primitive:{topology:'triangle-strip'}}));pass.drawIndexed(6,2);pass.drawIndirect();pass.executeBundles([]);
 const s=window.renderApiCounters.webgpu.submitted;assert.equal(s.drawCalls,2);assert.equal(s.instances,4);assert.equal(s.triangles,0);assert.equal(s.complete,false);assert.ok(s.incompleteReasons.some(reason=>reason.includes('unknown topology')));assert.ok(s.incompleteReasons.includes('indexed strip may contain primitive-restart indices'));assert.ok(s.incompleteReasons.includes('indirect WebGPU draw'));assert.ok(s.incompleteReasons.includes('WebGPU render bundle execution'));
}));

function installWebGL(){globalThis.window={};globalThis.WebGL2RenderingContext=class{bindBuffer(){}bindVertexArray(){}bufferData(){}bufferSubData(){}copyBufferSubData(){}deleteBuffer(){}deleteVertexArray(){}drawArrays(){}drawElements(){}drawArraysInstanced(){}drawElementsInstanced(){}};Object.assign(WebGL2RenderingContext.prototype,{POINTS:0,LINES:1,LINE_LOOP:2,LINE_STRIP:3,TRIANGLES:4,TRIANGLE_STRIP:5,TRIANGLE_FAN:6,ARRAY_BUFFER:0x8892,ELEMENT_ARRAY_BUFFER:0x8893,COPY_READ_BUFFER:0x8f36,COPY_WRITE_BUFFER:0x8f37,UNSIGNED_BYTE:0x1401,UNSIGNED_SHORT:0x1403,UNSIGNED_INT:0x1405,STATIC_DRAW:0x88e4});installApiCounters();return new WebGL2RenderingContext();}

test('WebGL2 direct modes and instanced variants count submitted primitives after restart-safe index proof',()=>globals(()=>{
 const gl=installWebGL(),indices={};gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,indices);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2,3,4,5]),gl.STATIC_DRAW);gl.drawArrays(gl.TRIANGLES,0,6);gl.drawArraysInstanced(gl.TRIANGLE_STRIP,0,5,2);gl.drawElements(gl.TRIANGLE_FAN,6,gl.UNSIGNED_SHORT,0);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2,3,4]),gl.STATIC_DRAW);gl.drawElementsInstanced(gl.LINES,5,gl.UNSIGNED_SHORT,0,3);gl.drawArraysInstanced(gl.POINTS,0,4,2);
 const s=window.renderApiCounters.webgl.submitted;assert.deepEqual({drawCalls:s.drawCalls,instances:s.instances,primitives:s.primitives,triangles:s.triangles,lines:s.lines,points:s.points,complete:s.complete},{drawCalls:5,instances:9,primitives:26,triangles:12,lines:6,points:8,complete:true});
}));

test('WebGL restart indices and unrecorded bytes leave indexed primitive totals incomplete',()=>globals(()=>{
 const gl=installWebGL(),buffer={};gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,buffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2,0xffff]),gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLE_STRIP,4,gl.UNSIGNED_SHORT,0);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,8,gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_SHORT,0);
 const s=window.renderApiCounters.webgl.submitted;assert.equal(s.drawCalls,2);assert.equal(s.instances,2);assert.equal(s.triangles,0);assert.equal(s.complete,false);assert.ok(s.incompleteReasons.includes('WebGL indexed byte range contains a fixed primitive-restart index'));assert.ok(s.incompleteReasons.includes('WebGL indexed byte range contains unknown uploaded bytes'));
}));

test('WebGL zero-length and omitted-length upload overloads capture the remaining source bytes',()=>globals(()=>{
 const gl=installWebGL(),buffer={},source=new Uint16Array([77,0,1,2,0xffff]);gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,buffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,source,gl.STATIC_DRAW,1,0);gl.drawElements(gl.TRIANGLE_STRIP,4,gl.UNSIGNED_SHORT,0);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,source,gl.STATIC_DRAW,1);gl.drawElements(gl.TRIANGLE_STRIP,4,gl.UNSIGNED_SHORT,0);
 const s=window.renderApiCounters.webgl.submitted;assert.equal(s.triangles,0);assert.equal(s.complete,false);assert.equal(s.incompleteReasons.filter(reason=>reason==='WebGL indexed byte range contains a fixed primitive-restart index').length,1);assert.equal(s.drawCalls,2);
}));

test('WebGL u32 fixed restart marker and shadow overflow are explicit incomplete cases',()=>globals(()=>{
 const gl=installWebGL(),buffer={};gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,buffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint32Array([0,1,0xffffffff]),gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_INT,0);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,window.renderApiCounters.webgl.indexShadow.byteLimit/2+1,gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_INT,0);
 const c=window.renderApiCounters.webgl,s=c.submitted;assert.equal(s.drawCalls,2);assert.equal(s.triangles,0);assert.equal(s.complete,false);assert.ok(s.incompleteReasons.includes('WebGL indexed byte range contains a fixed primitive-restart index'));assert.ok(s.incompleteReasons.includes('WebGL element buffer shadow exceeded bounded memory'));assert.ok(c.indexShadow.overflowEvents>0);assert.equal(c.indexShadow.retainedBytes,0);
}));

test('WebGL only proves uploaded index subranges and tracks element bindings per VAO',()=>globals(()=>{
 const gl=installWebGL(),vaoA={},vaoB={},restartBuffer={},validBuffer={};gl.bindVertexArray(vaoA);gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,restartBuffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2,0xffff]),gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLE_STRIP,4,gl.UNSIGNED_SHORT,0);
 gl.bindVertexArray(vaoB);gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,validBuffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0xffff,0,1,2,0xffff]),gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_SHORT,2);
 const s=window.renderApiCounters.webgl.submitted;assert.equal(s.complete,false);assert.equal(s.drawCalls,2);assert.equal(s.triangles,1);assert.ok(s.incompleteReasons.includes('WebGL indexed byte range contains a fixed primitive-restart index'));
}));

test('WebGL bufferSubData invalidates cached restart proofs',()=>globals(()=>{
 const gl=installWebGL(),buffer={};gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,buffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2,3]),gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLE_STRIP,3,gl.UNSIGNED_SHORT,0);gl.bufferSubData(gl.ELEMENT_ARRAY_BUFFER,6,new Uint16Array([0xffff]));gl.drawElements(gl.TRIANGLE_STRIP,4,gl.UNSIGNED_SHORT,0);
 const s=window.renderApiCounters.webgl.submitted;assert.equal(s.drawCalls,2);assert.equal(s.triangles,1);assert.equal(s.complete,false);assert.ok(s.incompleteReasons.includes('WebGL indexed byte range contains a fixed primitive-restart index'));
}));

test('WebGL copyBufferSubData invalidates cached destination index proof',()=>globals(()=>{
 const gl=installWebGL(),destination={},source={};gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,destination);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2]),gl.STATIC_DRAW);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_SHORT,0);
 gl.bindBuffer(gl.COPY_READ_BUFFER,source);gl.bufferData(gl.COPY_READ_BUFFER,new Uint16Array([0,1,2]),gl.STATIC_DRAW);gl.bindBuffer(gl.COPY_WRITE_BUFFER,destination);gl.copyBufferSubData(gl.COPY_READ_BUFFER,gl.COPY_WRITE_BUFFER,0,0,6);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_SHORT,0);
 const c=window.renderApiCounters.webgl,s=c.submitted;assert.equal(c.copyBufferSubData,1);assert.equal(s.drawCalls,2);assert.equal(s.triangles,1);assert.equal(s.complete,false);assert.ok(s.incompleteReasons.includes('WebGL indexed byte range contains unknown uploaded bytes'));
}));

test('WebGL deleteBuffer and deleteVertexArray release retained diagnostic references',()=>globals(()=>{
 const gl=installWebGL(),vao={},buffer={};gl.bindVertexArray(vao);gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER,buffer);gl.bufferData(gl.ELEMENT_ARRAY_BUFFER,new Uint16Array([0,1,2]),gl.STATIC_DRAW);const shadow=window.renderApiCounters.webgl.indexShadow;assert.equal(shadow.trackedBuffers,1);assert.equal(shadow.trackedVaos,1);assert.equal(shadow.retainedBytes,12);
 gl.deleteVertexArray(vao);assert.equal(shadow.trackedVaos,0);assert.equal(shadow.retainedBytes,12);gl.deleteBuffer(buffer);assert.equal(shadow.trackedBuffers,0);assert.equal(shadow.retainedBytes,0);gl.drawElements(gl.TRIANGLES,3,gl.UNSIGNED_SHORT,0);assert.equal(window.renderApiCounters.webgl.submitted.complete,false);
}));

test('API return and throw behavior is preserved and failed draw calls are not counted',()=>globals(()=>{
 globalThis.window={};const marker={};globalThis.GPURenderPassEncoder=class{setPipeline(){}draw(){return marker;}drawIndexed(){throw Error('original draw failure');}};
 globalThis.GPUCommandEncoder=class{};globalThis.GPUDevice=class{createRenderPipeline(){return {}};};installApiCounters();const pass=new GPURenderPassEncoder();assert.equal(pass.draw(3),marker);assert.throws(()=>pass.drawIndexed(3),/original draw failure/);const c=window.renderApiCounters.webgpu;assert.equal(c.draw,1);assert.equal(c.drawIndexed,0);assert.equal(c.submitted.drawCalls,1);assert.equal(c.submitted.complete,false);
}));
