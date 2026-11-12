import test from 'node:test';import assert from 'node:assert/strict';
import {InterpolationCadence} from '../tools/scene-observer/interpolation-policy.mjs';
test('4Hz source selects full250ms interval without repeatedly resetting interpolation',()=>{
 const p=new InterpolationCadence();assert.deepEqual([0,250,500,750,1000,1250].map(t=>p.sample(t)),[null,null,null,250,null,null]);
});
test('20Hz jitter and isolated source pauses do not reset active trajectories',()=>{
 const p=new InterpolationCadence();for(const t of [0,50,102,149,201,252,2200,2250,2301,2352])assert.equal(p.sample(t),null);assert.equal(p.duration,60);
});
test('sustained cadence changes adapt both ways while rejecting invalid samples',()=>{
 const p=new InterpolationCadence();for(const t of [0,250,500,750])p.sample(t);assert.equal(p.duration,250);
 for(const t of [800,850,900,950,1000,1050,1100])p.sample(t);assert.equal(p.duration,55);assert.equal(p.sample(NaN),null);
});
