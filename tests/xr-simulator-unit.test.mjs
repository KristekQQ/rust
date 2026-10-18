import test from 'node:test';
import assert from 'node:assert/strict';
import {matrixFromPose,multiply,inverse} from './xr/poses.js';
import {installXRSimulator} from './xr/simulator.js';
import {swordGeometry} from '../examples/webxr/geometry.js';

test('katana flat follows grip palm normal and cutter covers visible blade',()=>{
    const {positions}=swordGeometry();
    // First box is the blade. Grip X is the palm normal; -Z is the thumb axis.
    const blade=Array.from({length:8},(_,i)=>positions.slice(i*3,i*3+3));
    const extent=axis=>Math.max(...blade.map(p=>p[axis]))-Math.min(...blade.map(p=>p[axis]));
    assert.ok(extent(0)<extent(1)/4,'flat must face grip X, not grip Y');
    assert.ok(Math.abs(Math.min(...blade.map(p=>p[2]))+.95)<1e-8);
    assert.ok(Math.abs(Math.max(...blade.map(p=>p[2]))+.1)<1e-8);
});

const close=(actual,expected)=>{assert.equal(actual.length,expected.length);actual.forEach((value,i)=>assert.ok(Math.abs(value-expected[i])<1e-5,`${i}: ${value} != ${expected[i]}`));};
function environment(run) {
    const descriptor=Object.getOwnPropertyDescriptor(globalThis,'navigator');
    const xr={sentinel:true};const navigator={};Object.defineProperty(navigator,'xr',{configurable:true,get:()=>xr});
    Object.defineProperty(globalThis,'navigator',{configurable:true,value:navigator});
    return Promise.resolve().then(()=>run(navigator)).finally(()=>descriptor?Object.defineProperty(globalThis,'navigator',descriptor):delete globalThis.navigator);
}
async function start(options) {const simulator=installXRSimulator(options);await navigator.xr.requestSession('immersive-vr');return simulator;}

test('pose composition and inverse preserve full six degree transforms',()=>{
    const pose=matrixFromPose({position:[2,3,4],orientation:[0,Math.sin(Math.PI/4),0,Math.cos(Math.PI/4)]});
    close(multiply(pose,inverse(pose)),matrixFromPose());
    close(multiply(pose,matrixFromPose({position:[1,0,0]})).slice(12,15),[2,3,3]);
    assert.throws(()=>matrixFromPose({orientation:[0,0,0,0]}),/orientation/);
    assert.throws(()=>inverse(new Float32Array(16)),/Singular/);
});

test('rotated stereo eyes, projection depth and local-floor offset math',()=>environment(async()=>{
    const sim=await start({floorAvailable:false});
    try {
        const local=await sim.session.requestReferenceSpace('local');
        await assert.rejects(sim.session.requestReferenceSpace('local-floor'),{name:'NotSupportedError'});
        const floor=local.getOffsetReferenceSpace(new XRRigidTransform({y:-1.6}));
        sim.session.updateRenderState({depthNear:.2,depthFar:20});
        let pose,localPose;
        sim.session.requestAnimationFrame((_,frame)=>{pose=frame.getViewerPose(floor);localPose=frame.getViewerPose(local);});
        sim.frame({head:{position:[1,.4,3],orientation:[0,Math.sin(Math.PI/4),0,Math.cos(Math.PI/4)]}});
        close(pose.transform.matrix.slice(12,15),[1,2,3]);
        close(localPose.transform.matrix.slice(12,15),[1,.4,3]);
        close(pose.views[0].transform.matrix.slice(12,15),[1,2,3.032]);
        close(pose.views[1].transform.matrix.slice(12,15),[1,2,2.968]);
        assert.ok(Math.abs(pose.views[0].projectionMatrix[14]+2*20*.2/19.8)<1e-5);
        let reset=0;floor.addEventListener('reset',()=>reset++);sim.resetReferenceSpace();assert.equal(reset,1);
    } finally {sim.dispose();}
}));

test('deterministic timer order, cancellation, hidden RAF and input identities',()=>environment(async()=>{
    const sim=await start();
    try {
        const calls=[];const interval=setInterval(()=>calls.push(sim.time),5);
        const canceled=setTimeout(()=>assert.fail('Canceled timer fired'),1);clearTimeout(canceled);clearTimeout(canceled);
        setTimeout(()=>calls.push('once'),7);
        let rendered=0,changes=0;sim.session.addEventListener('inputsourceschange',()=>changes++);
        sim.session.requestAnimationFrame(()=>rendered++);
        assert.equal(sim.session.pendingFrameCount,1);
        sim.setVisibility('hidden');sim.frame({dt:10,controllers:{right:{pressed:true}}});
        assert.equal(rendered,0);assert.deepEqual(calls,[5,'once',10]);
        const source=sim.session.inputSources[0],grip=source.gripSpace;
        sim.setVisibility('visible');sim.frame({dt:0,controllers:{right:{position:[1,2,3]}}});
        assert.equal(rendered,1);assert.equal(changes,1);assert.equal(sim.session.inputSources[0],source);assert.equal(source.gripSpace,grip);
        sim.frame({dt:0,controllers:{right:null}});assert.equal(changes,2);
        clearInterval(interval);assert.equal(sim.timerCount,0);
        assert.throws(()=>sim.frame({dt:NaN}),/dt/);assert.throws(()=>sim.frame({dt:-1}),/dt/);assert.throws(()=>sim.frame({dt:10001}),/dt/);
        assert.throws(()=>sim.frame({head:{matrix:[1]}}),/matrix/);
    } finally {sim.dispose();}
}));

test('recording replays JSON poses and restores descriptors and timer functions',()=>environment(async navigator=>{
    const originalXR=Object.getOwnPropertyDescriptor(navigator,'xr'),originalTimer=globalThis.setTimeout,originalTransform=Object.getOwnPropertyDescriptor(globalThis,'XRRigidTransform');
    const sim=await start();let trace;
    sim.frame({dt:10,head:{position:[1,2,3]},controllers:{left:{pressed:true,axes:[.2,-.4]}}});
    trace=JSON.parse(JSON.stringify(sim.recording()));sim.dispose();sim.dispose();
    assert.deepEqual(Object.getOwnPropertyDescriptor(navigator,'xr'),originalXR);assert.equal(globalThis.setTimeout,originalTimer);assert.deepEqual(Object.getOwnPropertyDescriptor(globalThis,'XRRigidTransform'),originalTransform);
    const replay=await start();
    try {const seen=[];await replay.replay(trace,{afterFrame:index=>seen.push(index)});assert.deepEqual(seen,[0]);assert.deepEqual(replay.recording(),trace);}
    finally {replay.dispose();}
}));

test('denied, deferred and unsupported session requests',()=>environment(async()=>{
    const sim=installXRSimulator();
    try {
        sim.denyNextRequest();await assert.rejects(navigator.xr.requestSession('immersive-vr'),{name:'NotAllowedError'});
        const release=sim.deferNextRequest();let resolved=false;const request=navigator.xr.requestSession('immersive-vr').then(()=>resolved=true);
        await Promise.resolve();assert.equal(resolved,false);release();await request;assert.equal(resolved,true);
        await sim.session.end();assert.equal(sim.session.ended,true);
    } finally {sim.dispose();}
    const unsupported=installXRSimulator({supported:false});
    try {assert.equal(await navigator.xr.isSessionSupported('immersive-vr'),false);await assert.rejects(navigator.xr.requestSession('immersive-vr'),{name:'NotSupportedError'});}
    finally {unsupported.dispose();}
}));

test('layer pixels flip rows, crop eyes, restore GL state and release resources',()=>environment(async()=>{
    const originalGL=Object.getOwnPropertyDescriptor(globalThis,'WebGL2RenderingContext');
    const originalLayer=Object.getOwnPropertyDescriptor(globalThis,'XRWebGLLayer');
    const originalImage=Object.getOwnPropertyDescriptor(globalThis,'ImageData');
    const compatibility=()=> 'original';
    class FakeGL {
        constructor(){Object.assign(this,{TEXTURE_2D:1,TEXTURE_BINDING_2D:2,FRAMEBUFFER_BINDING:3,RENDERBUFFER_BINDING:4,FRAMEBUFFER:5,RENDERBUFFER:6,RGBA:7,UNSIGNED_BYTE:8,COLOR_ATTACHMENT0:9,DEPTH_ATTACHMENT:10,DEPTH_COMPONENT16:11,FRAMEBUFFER_COMPLETE:12});this.bindings=new Map([[2,'texture-before'],[3,'framebuffer-before'],[4,'renderbuffer-before']]);this.deleted=[];}
        makeXRCompatible=undefined;
        getParameter(key){return this.bindings.get(key);}
        createTexture(){return {kind:'texture'};}
        createFramebuffer(){return {kind:'framebuffer'};}
        createRenderbuffer(){return {kind:'depth'};}
        bindTexture(_,value){this.bindings.set(2,value);}
        bindFramebuffer(_,value){this.bindings.set(3,value);}
        bindRenderbuffer(_,value){this.bindings.set(4,value);}
        texParameteri(){} texImage2D(){} framebufferTexture2D(){} renderbufferStorage(){} framebufferRenderbuffer(){}
        checkFramebufferStatus(){return this.FRAMEBUFFER_COMPLETE;}
        readPixels(x,y,width,height,format,type,bytes){this.lastRead={x,width,height};for(let row=0;row<height;row++)bytes.fill(row+1,row*width*4,(row+1)*width*4);}
        deleteTexture(value){this.deleted.push(value.kind);}
        deleteFramebuffer(value){this.deleted.push(value.kind);}
        deleteRenderbuffer(value){this.deleted.push(value.kind);}
    }
    FakeGL.prototype.makeXRCompatible=compatibility;
    Object.defineProperty(globalThis,'WebGL2RenderingContext',{configurable:true,value:FakeGL});
    Object.defineProperty(globalThis,'ImageData',{configurable:true,value:class{constructor(data,width,height){Object.assign(this,{data,width,height});}}});
    let sim;
    try {
        sim=await start({profile:{eyeWidth:2,eyeHeight:2}});
        const gl=new FakeGL();delete gl.makeXRCompatible;
        assert.notEqual(gl.makeXRCompatible,compatibility);
        const layer=new XRWebGLLayer(sim.session,gl);assert.equal(layer.framebufferWidth,4);
        assert.equal(gl.getParameter(gl.FRAMEBUFFER_BINDING),'framebuffer-before');
        assert.equal(gl.getParameter(gl.TEXTURE_BINDING_2D),'texture-before');
        assert.equal(gl.getParameter(gl.RENDERBUFFER_BINDING),'renderbuffer-before');
        const pixels=sim.pixels('right');assert.equal(pixels.width,2);assert.deepEqual(gl.lastRead,{x:2,width:2,height:2});assert.equal(pixels.data[0],2);assert.equal(pixels.data[8],1);
        assert.equal(gl.getParameter(gl.FRAMEBUFFER_BINDING),'framebuffer-before');
        sim.dispose();sim.dispose();assert.deepEqual(gl.deleted.sort(),['depth','framebuffer','texture']);
        assert.equal(FakeGL.prototype.makeXRCompatible,compatibility);
        assert.deepEqual(Object.getOwnPropertyDescriptor(globalThis,'XRWebGLLayer'),originalLayer);
    } finally {
        sim?.dispose();
        for(const [key,descriptor] of [['WebGL2RenderingContext',originalGL],['ImageData',originalImage]])if(descriptor)Object.defineProperty(globalThis,key,descriptor);else delete globalThis[key];
    }
}));

test('tracking persists per source and grip identities survive temporary grip absence',()=>environment(async()=>{
    const sim=await start({clockTimers:false});
    try {
        const floor=await sim.session.requestReferenceSpace('local-floor');let observed;
        const observe=()=>sim.session.requestAnimationFrame((_,frame)=>observed=frame);
        observe();sim.frame({controllers:{right:{tracked:false,targetRayMatrix:matrixFromPose({position:[1,2,3]})}}});
        const source=sim.session.inputSources[0],grip=source.gripSpace;
        assert.equal(observed.getPose(grip,floor),null);
        observe();sim.frame({controllers:{right:{tracked:true,grip:false}},head:{tracked:false}});
        assert.equal(source.gripSpace,null);assert.equal(observed.getViewerPose(floor),null);
        close(observed.getPose(source.targetRaySpace,floor).transform.matrix.slice(12,15),[1,2,3]);
        observe();sim.frame({controllers:{right:{grip:true}}});assert.equal(source.gripSpace,grip);assert.equal(observed.getViewerPose(floor),null);
    } finally {sim.dispose();}
}));

test('native wait resolves while virtual timers and frame time remain frozen',()=>environment(async()=>{
    const sim=await start();
    try {
        let fired=false;const timer=setTimeout(()=>fired=true,1);
        await sim.wait(5);assert.equal(sim.time,0);assert.equal(fired,false);assert.equal(sim.timerCount,1);
        sim.frame({dt:1});assert.equal(fired,true);clearTimeout(timer);
        assert.throws(()=>sim.wait(Infinity),/wait/);assert.throws(()=>sim.wait(-1),/wait/);
    } finally {sim.dispose();}
}));

test('recording and replay preserve visibility, resets and explicit session boundaries',()=>environment(async()=>{
    const source=await start();let trace;
    try {
        source.setVisibility('visible-blurred');source.frame({dt:1});
        source.setVisibility('hidden');source.frame({dt:2});
        source.resetReferenceSpace();source.frame({dt:3});
        source.setVisibility('visible');source.frame({dt:4});
        await source.session.end();await navigator.xr.requestSession('immersive-vr');
        source.resetReferenceSpace();source.frame({dt:5});
        trace=source.recording();assert.equal(trace.floorAvailable,true);
        assert.deepEqual(trace.frames.map(frame=>frame.sessionIndex),[0,0,0,0,1]);
        assert.deepEqual(trace.frames.map(frame=>frame.referenceReset),[false,false,true,false,true]);
        const detached=source.recording();detached.profile.hz=1;detached.frames[0].head.matrix[0]=99;
        assert.equal(source.recording().profile.hz,72);assert.equal(source.recording().frames[0].head.matrix[0],1);
    } finally {source.dispose();}
    const replay=await start(),events=[];let oldResets=0;
    try {
        const original=replay.session;
        const space=await original.requestReferenceSpace('local-floor');space.addEventListener('reset',()=>{oldResets++;events.push('reset-0');});
        original.addEventListener('visibilitychange',()=>events.push(original.visibilityState));
        await assert.rejects(replay.replay(trace),/requires onSession/);assert.equal(replay.time,0);
        await assert.rejects(replay.replay(trace,{onSession:async()=>{}}),/must end/);
        // The rejected callback happened after session 0 frames, so use a fresh platform below.
    } finally {replay.dispose();}
    const clean=await start();
    try {
        const states=[],changes=[];let resets=0;
        async function listen(){const current=clean.session;current.addEventListener('visibilitychange',()=>changes.push(current.visibilityState));const space=await current.requestReferenceSpace('local-floor');space.addEventListener('reset',()=>resets++);}
        await listen();
        await clean.replay(trace,{afterFrame:()=>states.push(clean.session.visibilityState),onSession:async index=>{assert.equal(index,1);await clean.session.end();await navigator.xr.requestSession('immersive-vr');await listen();}});
        assert.deepEqual(states,['visible-blurred','hidden','hidden','visible','visible']);
        assert.deepEqual(changes,['visible-blurred','hidden','visible']);assert.equal(resets,2);
        assert.deepEqual(clean.recording(),trace);
    } finally {clean.dispose();}
    assert.equal(oldResets,1);assert.deepEqual(events,['visible-blurred','hidden','reset-0','visible']);
}));

test('replay rejects mismatched rendering profile and floor mode before frames',()=>environment(async()=>{
    const sim=await start();
    try {
        sim.frame({dt:1});const trace=sim.recording();
        for(const key of ['hz','ipd','eyeWidth','eyeHeight']) {
            const changed=JSON.parse(JSON.stringify(trace));changed.profile[key]+=1;
            await assert.rejects(sim.replay(changed),new RegExp(`profile mismatch: ${key}`));
        }
        await assert.rejects(sim.replay({...trace,floorAvailable:false}),/floorAvailable mismatch/);assert.equal(sim.time,1);
        const legacy={version:1,profile:trace.profile,frames:trace.frames.map(({visibilityState,referenceReset,sessionIndex,...frame})=>frame)};
        await sim.replay(legacy);assert.equal(sim.time,2);
    } finally {sim.dispose();}
}));
