import assert from 'node:assert/strict';
import test from 'node:test';
import {playbackDecision} from '../assets/playback-policy.js';
const one = (current, start, end) => playbackDecision(current, [{start, end}]);

test('ordinary Wi-Fi bursts do not repeatedly discard frames', () => {
  for (const reserve of [0.15, 0.3, 0.46, 0.6, 0.8, 1.2]) {
    const result = one(10, 8, 10 + reserve);
    assert.equal(result.seek, null);
    assert.ok(result.rate >= 0.97 && result.rate <= 1.03);
  }
});
test('small reserve is rebuilt and moderate backlog drains gradually', () => {
  assert.ok(one(10, 8, 10.1).rate < 1);
  assert.equal(one(10, 8, 10.35).rate, 1);
  assert.ok(one(10, 8, 10.8).rate > 1);
});
test('late join or long interruption recovers inside an actual buffered range', () => {
  assert.equal(one(0, 123, 123.033).seek, 123);
  const delayed = one(10, 8, 13);
  assert.ok(delayed.seek >= 8 && delayed.seek < 13);
  assert.ok(13 - delayed.seek >= 0.3);
  assert.equal(delayed.rate, 1);
});
test('a future range does not discard still-playable video', () => {
  const ranges = [{start:0, end:10}, {start:10.1, end:10.2}];
  assert.equal(playbackDecision(9.8, ranges).seek, null);
  assert.equal(playbackDecision(10, ranges).seek, 10.1);
  assert.equal(playbackDecision(10.2, ranges).seek, null);
  assert.equal(playbackDecision(10, []).seek, null);
});
