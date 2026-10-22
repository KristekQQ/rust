import test from 'node:test';
import assert from 'node:assert/strict';
import {PerformanceTest,PERFORMANCE_PHASES} from '../examples/neon-arena/performance-test.js';

const packet=(extra={})=>({frames:180,frameMs:10,cpuSubmitMs:2,sessionId:123,
    eyeViewports:[{eye:'left',width:1024,height:1024},{eye:'right',width:1024,height:1024}],...extra});
function controller(){
    const applied=[],updates=[],finished=[];
    const run=new PerformanceTest({applyPhase:phase=>applied.push(phase),onUpdate:state=>updates.push(state),onFinish:state=>finished.push(state)});
    return {run,applied,updates,finished};
}

test('six phases apply in order, discard two windows each, and finish only once',()=>{
    const {run,applied,updates,finished}=controller();
    assert.equal(run.start(),true);assert.equal(run.start(),false);
    for(let phase=0;phase<6;phase++){
        run.addSample(packet({frameMs:100}));run.addSample(packet({frameMs:200}));
        assert.equal(run.snapshot().sampleCount,0);
        for(let sample=0;sample<3;sample++)run.addSample(packet());
    }
    assert.deepEqual(applied.map(phase=>phase.id),['base-start','glow-only','ssr-half','ssr-full','both-half','base-end']);
    assert.deepEqual(applied.map(({ssr,glow,resolutionScale})=>[ssr,glow,resolutionScale]),
        [[false,false,.5],[false,true,.5],[true,false,.5],[true,false,1],[true,true,.5],[false,false,.5]]);
    assert.equal(updates.length,31,'one start update and one per accepted packet');
    assert.equal(finished.length,1);assert.equal(finished[0].status,'complete');
    assert.equal(run.running,false);assert.equal(run.addSample(packet()),false);assert.equal(run.cancel(),false);
    assert.ok(finished[0].results.every(result=>result.fps===100&&result.windows.length===3));
    assert.equal(PERFORMANCE_PHASES.length,6);
});

test('weights duration and CPU by frames and preserves per-window metadata',()=>{
    const {run}=controller();run.start();run.addSample(packet());run.addSample(packet());
    run.addSample(packet({frames:100,frameMs:10,cpuSubmitMs:1,medianFrameMs:9}));
    run.addSample(packet({frames:300,frameMs:20,cpuSubmitMs:5,p95FrameMs:23}));
    const detail=packet({frames:100,frameMs:40,cpuSubmitMs:3,p95FrameMs:45,frameRate:90,renderer:'BrowserWebGpu',headPose:{position:[0,1.7,0]}});
    run.addSample(detail);detail.headPose.position[0]=99;
    const result=run.snapshot().results[0];
    assert.equal(result.frames,500);assert.equal(result.fps,500/11000*1000);
    assert.notEqual(result.fps,(100+50+25)/3);
    assert.equal(result.frameMs,22);assert.equal(result.cpuSubmitMs,3.8);
    assert.deepEqual(result.windowFps,{median:50,min:25});
    assert.equal(result.windows[0].medianFrameMs,9);assert.equal(result.windows[1].p95FrameMs,23);
    assert.equal(result.windows[2].p95FrameMs,45);assert.equal(result.metadata.p95FrameMs,undefined);
    assert.equal(result.metadata.frameRate,90);assert.equal(result.metadata.renderer,'BrowserWebGpu');
    assert.deepEqual(result.metadata.headPose.position,[0,1.7,0]);
    result.windows.length=0;assert.equal(run.snapshot().results[0].windows.length,3);
});

test('invalid packets do not consume warmup or sample slots; cancellation finishes once',()=>{
    const {run,finished,updates}=controller();run.start();
    for(const invalid of [null,{},packet({frames:0}),packet({frameMs:NaN}),packet({cpuSubmitMs:-1}),packet({eyeViewports:[]})]){
        assert.equal(run.addSample(invalid),false);
    }
    assert.equal(run.snapshot().settleCount,0);assert.equal(updates.length,1);
    run.addSample(packet());run.cancel('user');run.cancel('again');run.addSample(packet());
    assert.equal(finished.length,1);assert.equal(finished[0].reason,'user');assert.equal(finished[0].status,'cancelled');
});

for(const [reason,change]of [
    ['session-changed',{sessionId:456}],
    ['viewport-changed',{eyeViewports:[{eye:'left',width:1024,height:1024},{eye:'right',width:1024,height:900}]}],
])test(`incompatible ${reason} cancels even during warmup`,()=>{
    const {run,finished,updates}=controller();run.start();run.addSample(packet());run.addSample(packet(change));
    assert.equal(run.running,false);assert.equal(finished.length,1);assert.equal(finished[0].reason,reason);
    assert.equal(updates.length,3);assert.equal(finished[0].results.length,0);
});

test('identity stays checked across phases and eye ordering does not change dimensions',()=>{
    const {run}=controller();run.start();
    for(let i=0;i<5;i++)run.addSample(packet({eyeViewports:packet().eyeViewports.reverse()}));
    assert.equal(run.snapshot().phase.id,'glow-only');
    run.addSample(packet({sessionId:789}));assert.equal(run.snapshot().reason,'session-changed');
});
