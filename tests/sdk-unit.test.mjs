import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = await readFile(new URL('../js/engine.js', import.meta.url), 'utf8');
const key = Symbol.for('webgpu_wasm.engine.initialization');
let moduleId = 0;
const deferred = () => {
    let resolve, reject;
    const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; });
    return { promise, resolve, reject };
};

async function sdk() {
    // Replace only the generated-WASM import: test production SDK with a mocked boundary.
    const code = source.replace('await import(`../pkg/webgpu_wasm.js?v=${version}`)', 'await globalThis.__sdkImport()')
        .replaceAll('import.meta.url', JSON.stringify('https://example.test/js/engine.js'));
    assert.notEqual(code, source, 'The WASM import test seam must match');
    return import(`data:text/javascript;base64,${Buffer.from(`${code}\n// test module ${moduleId++}`).toString('base64')}`);
}

function environment(t) {
    const names = ['HTMLCanvasElement', 'document', 'window', 'location', 'fetch', '__sdkImport'];
    const descriptors = new Map(names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
    delete globalThis[key];
    class Canvas {
        id = 'gpu-canvas'; dataset = {}; isConnected = true;
        constructor() { this.ownerDocument = globalThis.document; }
    }
    globalThis.HTMLCanvasElement = Canvas;
    globalThis.document = { getElementById: () => canvas };
    globalThis.window = { innerWidth: 640, innerHeight: 360 };
    globalThis.location = { href: 'https://example.test/page.html' };
    const canvas = new Canvas();
    t.after(() => {
        delete globalThis[key];
        for (const [name, descriptor] of descriptors) {
            if (descriptor) Object.defineProperty(globalThis, name, descriptor);
            else delete globalThis[name];
        }
    });
    return canvas;
}

function wasmMock(overrides = {}) {
    return { default: async () => {}, start: async () => {}, clear_all() {}, scene_object_count: () => 0,
        gltf_resource_uris: () => JSON.stringify({ buffers: [], images: [] }),
        load_gltf_model: () => 1, ...overrides };
}
function response(bytes = new Uint8Array([1]), url = '') {
    return { ok: true, url, headers: new Headers(), body: new ReadableStream({
        start(controller) { controller.enqueue(bytes); controller.close(); },
    }) };
}

test('character command resolves clip names, validates parameters and preserves runtime ownership', async t => {
    const canvas = environment(t);
    const calls = [];
    globalThis.fetch = async () => response();
    globalThis.__sdkImport = async () => wasmMock({
        model_info: () => JSON.stringify({ animations: [{name:'Survey'}, {name:'Walk'}, {name:'Run'}] }),
        control_character: (...args) => { calls.push(args); return true; },
        configure_character_physics: (...args) => { calls.push(['physics', ...args]); return true; },
        set_box_collider: (...args) => { calls.push(['collider', ...args]); return true; },
        remove_collider: id => { calls.push(['remove-collider', id]); return true; },
        character_state: () => JSON.stringify({id:1, position:[0,0,0], moving:false, running:false}),
        dispose() {},
    });
    const { initEngine } = await sdk();
    const engine = await initEngine(canvas);
    const fox = await engine.scene.loadModel('fox.glb');
    engine.controlCharacter(fox, {speed:2, runSpeed:4, animations:{idle:'Survey', walk:'Walk', run:'Run'}});
    assert.deepEqual(calls, [[1,2,4,0,0,1,2]]);
    assert.deepEqual(engine.characterState().position, [0,0,0]);
    assert.throws(() => engine.controlCharacter(fox, {speed:NaN}), /Rychlosti/);
    assert.throws(() => engine.controlCharacter(fox, {runSpeed:0}), /Rychlosti/);
    assert.throws(() => engine.controlCharacter(fox, {animations:{walk:'Missing'}}), /Missing/);
    assert.equal(calls.length, 1);
    assert.throws(() => engine.controlCharacter(fox, {physics:{gravity:0}}), /Rozměry/);
    assert.throws(() => engine.controlCharacter(fox, {physics:{halfExtents:[0,1,1]}}), /Rozměry/);
    assert.throws(() => engine.controlCharacter(fox, {physics:{jumpSpeed:Infinity}}), /Rozměry/);
    assert.equal(calls.length, 1, 'Invalid physics cannot replace an active controller');
    engine.controlCharacter(fox, {physics:{halfExtents:[0.4,0.8,0.8],gravity:18,jumpSpeed:7}});
    assert.deepEqual(calls.at(-1), ['physics',0.4,0.8,0.8,18,7]);
    fox.setCollider({halfExtents:[0.5,0.2,0.5],offset:[0,-0.2,0]}).removeCollider();
    assert.deepEqual(calls.slice(-2), [['collider',1,0.5,0.2,0.5,0,-0.2,0],['remove-collider',1]]);
    engine.dispose();
    const replacement = await initEngine(canvas);
    assert.throws(() => replacement.controlCharacter(fox), /tohoto enginu/);
    assert.throws(() => engine.characterState(), /ukončen/);
    replacement.dispose();
});

test('HUD is opt-in and only forwards commands and explicit diagnostic queries', async t => {
    const canvas = environment(t);
    const calls = [];
    globalThis.__sdkImport = async () => wasmMock({
        set_debug_overlay: enabled => calls.push(enabled),
        debug_stats: () => '{"enabled":false,"hudFrames":0}',
        dispose() {},
    });
    const { initEngine } = await sdk();
    const engine = await initEngine(canvas);
    assert.deepEqual(calls, []);
    assert.equal(engine.debugStats().hudFrames, 0);
    engine.setDebugOverlay(true);
    engine.setDebugOverlay(false);
    assert.deepEqual(calls, [true, false]);
    engine.dispose();
    assert.throws(() => engine.setDebugOverlay(true), /ukončen/);
    assert.throws(() => engine.debugStats(), /ukončen/);
});

test('concurrent/repeated init and cache-busted SDK imports start one runtime', async t => {
    const canvas = environment(t);
    const gate = deferred();
    let imports = 0, starts = 0;
    globalThis.__sdkImport = async () => {
        imports++;
        return wasmMock({ start: async () => { starts++; await gate.promise; } });
    };
    const firstSdk = await sdk();
    const secondSdk = await sdk();
    const first = firstSdk.initEngine(canvas);
    const second = secondSdk.initEngine(canvas);
    gate.resolve();
    const [a, b] = await Promise.all([first, second]);
    assert.equal(a, b);
    canvas.width = 123;
    assert.equal(await firstSdk.initEngine(canvas), a);
    assert.equal(canvas.width, 123, 'Repeated init must not reset the drawing buffer');
    assert.equal(imports, 1); assert.equal(starts, 1);
});

test('initialization failures can be retried', async t => {
    const canvas = environment(t);
    let starts = 0;
    globalThis.__sdkImport = async () => wasmMock({ start: async () => {
        if (++starts === 1) throw new Error('device unavailable');
    } });
    const { initEngine } = await sdk();
    await assert.rejects(initEngine(canvas), /device unavailable/);
    const engine = await initEngine(canvas);
    assert.ok(engine.scene); assert.equal(starts, 2);
});

test('init rejects detached/foreign/duplicate canvas and invalid or conflicting backend', async t => {
    const canvas = environment(t);
    let imports = 0;
    globalThis.__sdkImport = async () => { imports++; return wasmMock(); };
    const { initEngine } = await sdk();
    canvas.isConnected = false;
    await assert.rejects(initEngine(canvas), /připojený/);
    canvas.isConnected = true;
    canvas.ownerDocument = {};
    await assert.rejects(initEngine(canvas), /připojený/);
    canvas.ownerDocument = document;
    const duplicate = new HTMLCanvasElement();
    await assert.rejects(initEngine(duplicate), /připojený/);
    await assert.rejects(initEngine(canvas, { backend: 'typo' }), /backend/);
    assert.equal(imports, 0);
    await initEngine(canvas, { backend: 'webgl' });
    await assert.rejects(initEngine(canvas), /jiný canvas či backend/);
    document.getElementById = () => duplicate;
    await assert.rejects(initEngine(duplicate, { backend: 'webgl' }), /jiný canvas či backend/);
    assert.equal(imports, 1);
});

test('resize rejects values that WASM u32 coercion would silently corrupt', async t => {
    const canvas = environment(t);
    const calls = [];
    globalThis.__sdkImport = async () => wasmMock({ resize: (...args) => calls.push(args) });
    const engine = await (await sdk()).initEngine(canvas);
    for (const width of [NaN, Infinity, -1, 0, 1.5, 2 ** 32, '10']) {
        assert.throws(() => engine.resize(width, 20), /Rozměry/);
        assert.throws(() => engine.resize(20, width), /Rozměry/);
    }
    engine.resize(800, 600);
    assert.deepEqual(calls, [[800, 600]]);
});

test('clear aborts all pending fetches and permits subsequent imports', async t => {
    environment(t);
    const signals = [];
    globalThis.fetch = async (_url, { signal }) => {
        signals.push(signal);
        return new Promise((resolve, reject) => signal.addEventListener('abort', () => reject(signal.reason), { once: true }));
    };
    let imports = 0, clears = 0;
    const { Scene } = await sdk();
    const scene = new Scene(wasmMock({ load_gltf_model: () => ++imports, clear_all: () => clears++ }));
    const pending = [scene.loadModel('first.glb'), scene.loadModel('second.glb')];
    const checks = pending.map(p => assert.rejects(p, /zrušeno/));
    scene.clear();
    await Promise.all(checks);
    assert.ok(signals.every(s => s.aborted));
    assert.equal(imports, 0); assert.equal(clears, 1);
    globalThis.fetch = async () => response();
    assert.equal((await scene.loadModel('fresh.glb')).id, 1);
});

test('clear cancels an active body read even for a custom stream', async t => {
    environment(t);
    const reading = deferred();
    let cancels = 0, imports = 0;
    const body = new ReadableStream({ pull() { reading.resolve(); }, cancel() { cancels++; } });
    globalThis.fetch = async () => ({ ok: true, headers: new Headers(), body });
    const scene = new (await sdk()).Scene(wasmMock({ load_gltf_model: () => ++imports }));
    const pending = scene.loadModel('slow.glb');
    const check = assert.rejects(pending, /zrušeno/);
    await reading.promise;
    scene.clear();
    await check;
    assert.equal(cancels, 1); assert.equal(imports, 0); assert.equal(body.locked, false);
});

test('clear aborts external resource fetches after root parsing', async t => {
    environment(t);
    const fetching = deferred();
    let externalSignal, imports = 0;
    globalThis.fetch = async (url, { signal }) => {
        if (url.pathname.endsWith('root.gltf')) return response();
        externalSignal = signal; fetching.resolve();
        return new Promise((_resolve, reject) => signal.addEventListener('abort', () => reject(signal.reason)));
    };
    const scene = new (await sdk()).Scene(wasmMock({
        gltf_resource_uris: () => JSON.stringify({ buffers: ['mesh.bin'], images: [] }),
        load_gltf_model: () => ++imports,
    }));
    const pending = scene.loadModel('root.gltf');
    const check = assert.rejects(pending, /zrušeno/);
    await fetching.promise; scene.clear(); await check;
    assert.equal(externalSignal.aborted, true); assert.equal(imports, 0);
});

test('completed loads release cancellation tracking and resolve redirected resources', async t => {
    environment(t);
    const resources = [], signals = [];
    globalThis.fetch = async (url, { signal }) => {
        resources.push(url.href); signals.push(signal);
        return response(new Uint8Array([1]), resources.length === 1 ? 'https://cdn.test/model/root.gltf' : '');
    };
    const scene = new (await sdk()).Scene(wasmMock({
        gltf_resource_uris: () => JSON.stringify({ buffers: ['mesh.bin'], images: ['texture.png'] }),
    }));
    assert.equal((await scene.loadModel('root.gltf')).id, 1);
    scene.clear();
    assert.ok(signals.every(s => !s.aborted));
    assert.deepEqual(resources, ['https://example.test/root.gltf', 'https://cdn.test/model/mesh.bin', 'https://cdn.test/model/texture.png']);
});

test('stream failure preserves original error when cancellation fails and releases lock', async t => {
    environment(t);
    let releases = 0;
    const original = new Error('network disconnected');
    globalThis.fetch = async () => ({ ok: true, headers: new Headers(), body: { getReader: () => ({
        read: async () => { throw original; }, cancel: async () => { throw new Error('cancel failed'); },
        releaseLock: () => releases++,
    }) } });
    const scene = new (await sdk()).Scene(wasmMock());
    await assert.rejects(scene.loadModel('bad.glb'), error => error === original);
    assert.equal(releases, 1);
});

test('dispose aborts transport, releases ownership, and rejects every old command after reinit', async t => {
    const canvas = environment(t);
    let disposals = 0, positions = 0, starts = 0;
    globalThis.__sdkImport = async () => wasmMock({
        start: async () => { starts++; }, dispose: () => { disposals++; },
        add_cube: () => 1, object_exists: () => true,
        set_object_position: () => { positions++; return true; },
        set_postprocess: () => true, resize() {},
    });
    const { initEngine } = await sdk();
    const old = await initEngine(canvas);
    const handle = old.scene.createCube();
    let canceled = false;
    const reading = deferred();
    globalThis.fetch = async () => ({ ok: true, headers: new Headers(), body: new ReadableStream({
        pull() { reading.resolve(); }, cancel() { canceled = true; },
    }) });
    const load = old.scene.loadModel('/pending.glb');
    const rejection = assert.rejects(load, /zrušeno/);
    await reading.promise;
    old.dispose(); old.dispose();
    await rejection;
    assert.equal(canceled, true);
    assert.equal(disposals, 1);
    assert.equal(globalThis[key], undefined);
    const replacement = await initEngine(canvas);
    assert.notEqual(old, replacement);
    assert.equal(starts, 2);
    const next = replacement.scene.createCube();
    for (const command of [() => handle.setPosition([1, 2, 3]), () => handle.exists,
        () => old.scene.createCube(), () => old.add_cube(0, 0, 0), () => old.resize(10, 10),
        () => old.postprocessing.enable('ssr')]) {
        assert.throws(command, /ukončen/);
    }
    next.setPosition([1, 2, 3]);
    assert.equal(positions, 1);
    assert.throws(() => replacement.scene.setMirror(handle), /odmítl/);
    await assert.rejects(old.scene.loadModel('/never-fetch.glb'), /ukončen/);
    old.dispose();
    assert.equal(disposals, 1, 'Old dispose must not release the replacement');
    assert.equal(await initEngine(canvas), replacement);
    replacement.dispose();
    assert.equal(disposals, 2);
});
