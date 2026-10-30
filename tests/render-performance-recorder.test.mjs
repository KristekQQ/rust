import test from 'node:test';
import assert from 'node:assert/strict';
import {installFrameRecorder} from '../scripts/render-performance-recorder.mjs';
import {parseArgs, summarize} from '../scripts/render-performance-lib.mjs';

test('minimal recorder is bounded, joins callbacks and never invokes profiling', () => {
  const previous = globalThis.window, queued = [];
  globalThis.window = {requestAnimationFrame: fn => queued.push(fn),
    renderGlRafTimer: {beginFrame() {throw Error('Profiler called');}}};
  try {
    installFrameRecorder('minimal', 2);
    const m = window.perfMeasure, stamps = m.stamps, cpu = m.cpu;
    m.active = true;
    let calls = 0;
    for (let i = 0; i < 4; i++) window.requestAnimationFrame(() => calls++);
    queued[0](10); queued[1](10); queued[2](20); queued[3](30);
    assert.equal(calls, 4);
    assert.equal(m.count, 2); assert.equal(m.overflow, true);
    assert.equal(m.stamps, stamps); assert.equal(m.cpu, cpu);
    assert.throws(() => m.snapshot(), /during timing/);
    m.active = false;
    assert.deepEqual(m.snapshot().map(r => r.stamp), [10, 20]);
    assert.ok(m.snapshot().every(r => r.cpu >= 0));
    m.reset(); assert.equal(m.overflow, false); assert.equal(m.count, 0);
  } finally {globalThis.window = previous;}
});

test('minimal measurement rejects profiler/frame polling and retains boundary counters', () => {
  assert.throws(() => parseArgs(['smoke', '--measurement', 'minimal']), /no-profile/);
  assert.throws(() => parseArgs(['smoke', '--no-profile', '--measurement', 'minimal', '--frame-context', '1']), /frame-context/);
  const o = parseArgs(['compare', '--no-profile', '--candidate-measurement', 'minimal']);
  assert.equal(o.measurement, 'sampled'); assert.equal(o.candidateMeasurement, 'minimal');
  const r = summarize({frames:[{stamp:0,cpu:1},{stamp:16,cpu:2}],gpu:[],samples:[],
    boundaryEnd:{renderer:{drawCalls:7},debug:{mainDrawCalls:7}},profiling:false}, 1000/60);
  assert.equal(r.renderer.drawCalls, 7); assert.equal(r.debug.mainDrawCalls, 7);
  assert.equal(r.gpuAvailable, false); assert.equal(r.cpuScopes, null);
});
