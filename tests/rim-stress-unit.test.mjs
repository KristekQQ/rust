import test from 'node:test';
import assert from 'node:assert/strict';
import {createStressLoad, MAX_STRESS_MODELS, STRESS_LEVELS, stressPosition} from '../examples/rim/stress.js';

function fixture({failId = 0, failCommandCount = null} = {}) {
    let nextId = 1;
    const calls = [];
    const handles = [];
    const source = {
        id: 'source',
        createInstance() {
            const id = nextId++;
            const handle = {id, position: null, scale: null, removed: false};
            const record = (name, value) => {
                calls.push([id, name, value]);
                if (id === failId && name === 'renderOptions') throw new Error('initialization failed');
                return model;
            };
            const model = {
                id,
                setPosition: value => { handle.position = value.slice(); return record('position', value); },
                setScale: value => { handle.scale = value.slice(); return record('scale', value); },
                playAnimation: (...args) => record('play', args),
                pauseAnimation: (...args) => record('pause', args),
                seekAnimation: (...args) => record('seek', args),
                setRenderOptions: value => record('renderOptions', value),
                remove() { handle.removed = true; calls.push([id, 'remove']); },
            };
            handles.push(handle);
            return model;
        },
    };
    const commands = [];
    const scene = {rimInput: value => {
        commands.push(value);
        if (value.stress_count === failCommandCount) throw new Error('command failed');
    }};
    const content = {characters: [{model: source, spec: {slug: 'drone-orbiter', root: {scale: [.298, .298, .298]}}}]};
    return {load: createStressLoad(scene, content), source, handles, calls, commands};
}

test('limits, validated atomic counts, stable incremental instances and static paused poses', () => {
    assert.equal(MAX_STRESS_MODELS, 2048);
    assert.deepEqual(STRESS_LEVELS, [0, 16, 32, 64, 128, 256, 512, 1024, 2048]);
    const f = fixture();
    for (const invalid of [-1, 2049, .5, NaN, Infinity, '16']) {
        assert.throws(() => f.load.setCount(invalid), RangeError);
        assert.equal(f.load.count, 0);
        assert.equal(f.commands.length, 0);
    }
    f.load.setCount(16);
    const first = f.load.objects;
    const firstPoses = first.map(object => f.handles.find(handle => handle.id === object.id).position);
    f.load.setCount(32);
    assert.deepEqual(f.load.objects.slice(0, 16), first);
    assert.deepEqual(first.map(object => f.handles.find(handle => handle.id === object.id).position), firstPoses);
    for (let index = 0; index < 32; index++) assert.deepEqual(f.handles[index].position, stressPosition(index));
    assert.equal(f.load.count, 32);
    assert.deepEqual(f.commands, [{stress_count: 16}, {stress_count: 32}]);
    for (const handle of f.handles) {
        if (handle.id > 32) continue;
        assert(handle.position.every(Number.isFinite));
        assert(handle.scale.every(Number.isFinite));
        assert(handle.position[1] >= .35);
        const options = f.calls.find(call => call[0] === handle.id && call[1] === 'renderOptions')[2];
        assert.deepEqual(options, {castsShadow: false, receivesShadow: true, isStatic: true, opaqueReorderable: true});
        assert(f.calls.some(call => call[0] === handle.id && call[1] === 'pause' && call[2].length === 0));
        assert.deepEqual(f.calls.find(call => call[0] === handle.id && call[1] === 'seek')[2], [0]);
    }
    const exposed = f.load.objects;
    exposed.pop();
    assert.equal(f.load.count, 32);
    assert.equal(f.source.id, 'source');
});

test('stress positions are pure deterministic values and reject invalid indices', () => {
    for (const index of [0, 7, 8, 63, 64, 1024, MAX_STRESS_MODELS - 1]) {
        const position = stressPosition(index);
        assert.equal(position.length, 3);
        assert(position.every(Number.isFinite));
        assert.deepEqual(stressPosition(index), position);
    }
    for (const index of [-1, MAX_STRESS_MODELS, .5, NaN, Infinity]) {
        assert.throws(() => stressPosition(index), RangeError);
    }
});

test('growth initialization failure rolls back new handles and leaves count unchanged', () => {
    const f = fixture({failId: 3});
    f.load.setCount(2);
    assert.throws(() => f.load.setCount(4), /initialization failed/);
    assert.equal(f.load.count, 2);
    assert.deepEqual(f.commands, [{stress_count: 2}]);
    assert.deepEqual(f.handles.map(handle => handle.removed), [false, false, true]);
    assert.deepEqual(f.load.objects.map(object => object.id), [1, 2]);
});

test('shrink and idempotent disposal remove only owned instances and signal zero', () => {
    const f = fixture();
    f.load.setCount(5);
    const owned = f.load.objects;
    f.load.setCount(2);
    assert.deepEqual(f.load.objects, owned.slice(0, 2));
    assert.deepEqual(f.handles.map(handle => handle.removed), [false, false, true, true, true]);
    f.load.dispose();
    f.load.dispose();
    assert.equal(f.load.count, 0);
    assert.deepEqual(f.handles.map(handle => handle.removed), [true, true, true, true, true]);
    assert.deepEqual(f.commands, [{stress_count: 5}, {stress_count: 2}, {stress_count: 0}]);
    assert.equal(f.calls.filter(call => call[0] === 'source' && call[1] === 'remove').length, 0);
    assert.throws(() => f.load.setCount(1), /ukončen/);
});

test('command failures preserve live counts and disposal still cleans owned handles', () => {
    const grow = fixture({failCommandCount: 4});
    grow.load.setCount(2);
    assert.throws(() => grow.load.setCount(4), /command failed/);
    assert.equal(grow.load.count, 2);
    assert.deepEqual(grow.handles.map(handle => handle.removed), [false, false, true, true]);

    const shrink = fixture({failCommandCount: 2});
    shrink.load.setCount(4);
    assert.throws(() => shrink.load.setCount(2), /command failed/);
    assert.equal(shrink.load.count, 4);
    assert(shrink.handles.every(handle => !handle.removed));
    assert.doesNotThrow(() => shrink.load.dispose());
    assert.equal(shrink.load.count, 0);
    assert(shrink.handles.every(handle => handle.removed));
});
