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
    const code = source.replace("'./webxr.js'", JSON.stringify(new URL('../js/webxr.js', import.meta.url).href)).replace('await import(`../pkg/webgpu_wasm.js?v=${version}`)', 'await globalThis.__sdkImport()')
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

test('cutting commands preserve handle ownership, typed buffers and Rust piece IDs', async () => {
    const {Scene, SceneObject} = await sdk();
    const calls = [];
    const wasm = {
        add_triangle_mesh(positions, indices, ...color) { calls.push(['mesh', positions, indices, color]); return 10; },
        set_cuttable(...args) { calls.push(['cuttable', ...args]); },
        set_cutter(...args) { calls.push(['cutter', ...args]); },
        set_blade_physics(...args) { calls.push(['bladePhysics', ...args]); },
        set_blade_contact_response(...args) { calls.push(['bladeContactResponse', ...args]); },
        set_cut_assist(...args) { calls.push(['cutAssist', ...args]); },
        triangle_mesh_info: () => JSON.stringify({triangles: 12, volume: 8, closed: true, cuttable: true}),
        triangle_mesh_gpu_snapshot: async id => {calls.push(['gpuSnapshot',id]);return JSON.stringify({positions:[[1,2,3]],normals:[[0,1,0]],colors:[[1,0,0]]});},
        cut_with(...args) { calls.push(['cut', ...args]); return JSON.stringify({removed:[10],pieces:[12,13],objectCandidates:1,bvhNodes:9,triangleTests:8}); },
        cut_with_scatter(...args) { calls.push(['scatter', ...args]); return JSON.stringify({removed:[10],pieces:[14,15]}); },
        object_exists: id => id !== 10,
        aim_cutter(...args) {calls.push(['aim',...args]);return new Float32Array([0,-3,0]);},
        add_colored_triangle_mesh(...args) {calls.push(['colored',...args]);return 16;},
        scene_object_count:()=>2,
        async cut_with_async(...args) {calls.push(['async',...args]);return JSON.stringify({removed:[10],pieces:[18,19],execution:'webgpu-compute',gpuMeshes:1,totalMs:2,computeMs:1,cpuMs:1,fallbackReason:null});},
    };
    const scene = new Scene(wasm);
    const mesh = scene.createMesh({positions:[0,0,0,1,0,0,0,1,0,0,0,1],indices:[0,1,2]});
    mesh.setCuttable(true, {capColor:[1,.2,.1]});
    const blade = new SceneObject(wasm, 11).setCutter({base:[0,0,-1],tip:[0,0,1]});
    const result = scene.cutWith(blade, {motion:[0,-3,0]});
    assert(calls[0][1] instanceof Float32Array); assert(calls[0][2] instanceof Uint32Array);
    assert.equal(calls[0][3].at(-1),false,'Meshes default to flat shading');
    assert.deepEqual(calls[1], ['cuttable',10,true,1,.2,.1]);
    assert.deepEqual(Array.from(calls[2][3]), [0,0,-1,0,0,1]);
    assert.deepEqual(calls[3], ['cut',11,0,-3,0,.12]);
    assert.deepEqual(result.pieces.map(p => p.id), [12,13]);
    assert(result.pieces.every(p => p instanceof SceneObject && p.exists));
    assert.equal(result.pieces[0].meshInfo.closed, true);
    assert.deepEqual((await result.pieces[0].gpuMeshSnapshot()).positions,[[1,2,3]]);
    assert.deepEqual(calls.at(-1),['gpuSnapshot',12]);
    const animated=scene.cutWith(blade,{motion:[0,-3,0],scatter:{floorY:.2}});
    assert.deepEqual(animated.pieces.map(p=>p.id),[14,15]);
    assert.deepEqual(calls.at(-1),['scatter',11,new Float32Array([0,-3,0]),.12,.2,.8,.6]);
    assert.throws(() => scene.cutWith(new SceneObject({}, 11), {motion:[1,0,0]}), /Čepel/);
    for (const indices of [[-1,0,1],[.5,0,1],[NaN,0,1],[4294967296,0,1]]) {
        assert.throws(() => scene.createMesh({positions:[0,0,0], indices}), /indexy/);
    }
    blade.setCutter(null);
    assert.deepEqual(calls.at(-1), ['cutter',11,false,new Float32Array()]);
    const guide=new SceneObject(wasm,17);
    assert.deepEqual(scene.aimCutter(blade,{center:[1,1.15,2],guides:[guide]}),[0,-3,0]);
    assert.deepEqual(Array.from(calls.at(-1)[2]),[17]);
    assert.throws(()=>scene.aimCutter(blade,{center:[0,0,0],guides:[new SceneObject({},17)]}),/scéně/);
    scene.createMesh({positions:[0,0,0,1,0,0,0,1,0,0,0,1],indices:[0,1,2],triangleColors:[1,0,0],smooth:true});
    assert.equal(calls.at(-1).at(-1),true,'Smooth normals are explicitly requested from Rust');
    assert(calls.at(-1)[3] instanceof Float32Array);
    assert.throws(()=>scene.createMesh({positions:[0,0,0],indices:[0,1,2],triangleColors:[1]}),/RGB/);
    const computed=await scene.cutWithAsync(blade,{motion:[0,-3,0],execution:'gpu',scatter:{floorY:.2}});
    assert.deepEqual(calls.at(-1),['async',11,new Float32Array([0,-3,0]),.12,new Float32Array([.2,.8,.6]),'gpu']);
    assert.equal(computed.execution,'webgpu-compute');assert.deepEqual(computed.pieces.map(p=>p.id),[18,19]);
    await assert.rejects(()=>scene.cutWithAsync(new SceneObject({},11),{motion:[0,-3,0]}),/Čepel/);
    assert.equal(blade.setCutAssist(.35), blade);
    assert.deepEqual(calls.at(-1), ['cutAssist',11,.35]);
    assert.equal(blade.setBladeContactResponse(true), blade);
    assert.deepEqual(calls.at(-1), ['bladeContactResponse',11,true]);
    blade.setBladeContactResponse(false);
    assert.deepEqual(calls.at(-1), ['bladeContactResponse',11,false]);
    blade.setCutAssist(0);
    assert.deepEqual(calls.at(-1), ['cutAssist',11,0]);
    blade.setBladePhysics({edge:[0,-1,0]});
    assert.deepEqual(calls.at(-1),['bladePhysics',11,new Float32Array([0,-1,0]),.0225,.004]);
    blade.setBladePhysics(null);
    assert.equal(calls.at(-1)[2].length,0);
});

test('orbit and world/screen commands forward to Rust, including lifecycle guards',async t=>{
    const canvas=environment(t),calls=[];
    globalThis.__sdkImport=async()=>wasmMock({set_orbit_view:(...p)=>{calls.push(p);return true;},adjust_orbit:(...p)=>{calls.push(p);return true;},camera_view_state:()=>JSON.stringify({position:[1,2,3],orbitTarget:[0,0,0]}),world_to_screen:()=>new Float32Array([.25,.75]),dispose(){}});
    const {initEngine}=await sdk();const engine=await initEngine(canvas);
    engine.setOrbitView([1,2,3],[0,0,0]);engine.adjustOrbit(10,-5,20);
    assert.deepEqual(calls,[[1,2,3,0,0,0],[10,-5,20]]);assert.deepEqual(engine.worldToScreen([0,0,0]),[.25,.75]);assert.deepEqual(engine.cameraViewState().position,[1,2,3]);
    engine.dispose();assert.throws(()=>engine.adjustOrbit(0,0,1));assert.throws(()=>engine.worldToScreen([0,0,0]));
});
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

test('XR controller bindings enforce engine handle ownership and disposal', async t => {
    const canvas = environment(t);
    const calls = [];
    globalThis.__sdkImport = async () => wasmMock({
        add_cube: () => 42,
        xr_bind_controller: (...args) => calls.push(args),
        dispose() {},
    });
    const {initEngine, SceneObject} = await sdk();
    const engine = await initEngine(canvas);
    const sword = engine.scene.createCube();
    engine.xr.bindController('right', sword, {cutting:true});
    engine.xr.bindController('left', null);
    assert.deepEqual(calls, [[1,42,true],[0,0xffffffff,false]]);
    assert.throws(() => engine.xr.bindController('left', new SceneObject({},42)), /tohoto enginu/);
    assert.throws(() => engine.xr.bindController('invalid', sword), /left\/right/);
    engine.xr.bindButton(sword);
    assert.throws(()=>engine.xr.bindButton(new SceneObject({},42)),/tomuto enginu/);
    engine.xr.bindButton(null);
    engine.dispose();
    assert.throws(() => engine.xr.bindController('right', sword), /ukončen/);
    assert.throws(()=>engine.xr.bindButton(sword),/tomuto enginu/);
});


test('environment maps validate upload shape and intensity before WASM', async t => {
    const canvas = environment(t);
    const uploads = [], intensities = [];
    let clears = 0;
    globalThis.__sdkImport = async () => wasmMock({
        start: async () => {}, dispose() {},
        set_environment_cube_map: (...args) => { uploads.push(args); return true; },
        set_environment_intensity: value => { intensities.push(value); return true; },
        clear_environment_cube_map: () => { clears++; return true; },
    });
    const engine = await (await sdk()).initEngine(canvas);
    const color = new Uint8Array(24), emission = new Uint8Array(24);
    engine.postprocessing.setEnvironmentCubeMap({ size: 1, color, emission });
    assert.deepEqual(uploads[0], [1, color, emission, 1.2, new Float32Array()]);
    engine.postprocessing.setEnvironmentIntensity(0);
    engine.postprocessing.setEnvironmentIntensity(8);
    assert.deepEqual(intensities, [0, 8]);
    for (const invalid of [{ size: 0 }, { size: 1.5 }, { size: 2049 },
        { color: new Uint8Array(7) }, { emission: [] }, { intensity: NaN }, { intensity: 9 },
        ...[{}, { center: [0, 0], halfExtents: [1, 1, 1] },
            { center: new Array(3), halfExtents: [1, 1, 1] },
            { center: [0, 0, 0], halfExtents: new Array(3) },
            { center: [0, Infinity, 0], halfExtents: [1, 1, 1] },
            { center: [1e7, 0, 0], halfExtents: [1, 1, 1] },
            { center: [0, 0, 0], halfExtents: [1, 0, 1] },
            { center: [0, 0, 0], halfExtents: [1, NaN, 1] },
            { center: [0, 0, 0], halfExtents: [1, 1e7, 1] }].map(projection => ({ projection }))]) {
        assert.throws(() => engine.postprocessing.setEnvironmentCubeMap({ size: 1, color, emission, ...invalid }), /Invalid environment/);
    }
    for (const invalid of [-1, Infinity, NaN, 9]) assert.throws(() => engine.postprocessing.setEnvironmentIntensity(invalid), /Invalid environment/);
    assert.equal(uploads.length, 1);
    engine.postprocessing.setEnvironmentCubeMap(null);
    assert.equal(clears, 1);
    engine.postprocessing.setEnvironmentCubeMap({ size: 1, color, emission, intensity: 2 });
    assert.deepEqual(uploads[1], [1, color, emission, 2, new Float32Array()]);
    engine.postprocessing.setEnvironmentCubeMap({ size: 1, color, emission, projection: { center: [0, 1.65, 0], halfExtents: [7, 10.35, 7] } });
    assert.deepEqual(uploads[2], [1, color, emission, 1.2, new Float32Array([0, 1.65, 0, 7, 10.35, 7])]);
    engine.postprocessing.setEnvironmentCubeMap({ size: 1, color, emission, projection: null });
    assert.deepEqual(uploads[3], [1, color, emission, 1.2, new Float32Array()]);
    engine.dispose();
    assert.throws(() => engine.postprocessing.setEnvironmentIntensity(1), /ukončen/);
    assert.throws(() => engine.postprocessing.setEnvironmentCubeMap(null), /ukončen/);
});


test('world bitmap panels forward static inscriptions and diagnostic kind to Rust', async () => {
    const {Scene, SceneObject} = await sdk();
    const calls = [];
    const scene = new Scene({
        add_text_panel(...args) { calls.push(['text', ...args]); return 71; },
        add_info_panel(...args) { calls.push(['info', ...args]); return 72; },
        set_object_position(...args) { calls.push(['position', ...args]); return true; },
    });
    const panel = scene.createTextPanel({title:'CODEX', lines:['POSTAVENO SPOLECNE', 'S HONZOU'], position:[1,2,3], width:1.6, height:.7});
    assert(panel instanceof SceneObject);
    assert.equal(panel.id,71);
    assert.deepEqual(calls[0],['text','CODEX','["POSTAVENO SPOLECNE","S HONZOU"]',1.6,.7]);
    assert.deepEqual(calls[1],['position',71,1,2,3]);
    assert.equal(scene.createInfoPanel('diagnostics').id,72);
    assert.deepEqual(calls[2],['info','diagnostics',1.2,.72]);
    scene.createTextPanel({title:'TITLE',lines:[]});
    assert.deepEqual(calls[4],['text','TITLE','[]',1.2,.72]);
});


test('color cutout forwards validation to Rust and remains chainable', async () => {
    const { SceneObject } = await sdk();
    const calls = [];
    const object = new SceneObject({
        set_object_color_cutout(...args) { calls.push(args); return Number.isFinite(args[1]) && args[1] >= 0 && args[1] <= 1; },
    }, 17);
    assert.equal(object.setColorCutout(0.25), object);
    assert.equal(object.setColorCutout(0), object);
    assert.deepEqual(calls, [[17, 0.25], [17, 0]]);
    for (const invalid of [NaN, Infinity, -0.1, 1.1]) assert.throws(() => object.setColorCutout(invalid), /odmítl/);
});

test('cut feedback and blade trail are thin ownership-checked native commands', async () => {
    const {Scene, SceneObject} = await sdk();
    const calls = [];
    const wasm = {
        play_cut_feedback_facing(ids, mirrorText, size) { calls.push(['feedback', Array.from(ids), mirrorText, size]); return JSON.stringify({objects:[40,41],duration:.65}); },
        clear_cut_feedback() { calls.push(['clearFeedback']); },
        set_blade_trail(id, enabled) { calls.push(['trail',id,enabled]); },
    };
    const scene = new Scene(wasm);
    const pieces = [new SceneObject(wasm,12), new SceneObject(wasm,13)];
    const result = scene.playCutFeedback(pieces);
    assert.deepEqual(result.objects.map(p=>p.id),[40,41]);
    assert.equal(result.duration,.65);
    assert.equal(pieces[0].setBladeTrail(true),pieces[0]);
    scene.clearCutFeedback();
    assert.deepEqual(calls,[['feedback',[12,13],false,1],['trail',12,true],['clearFeedback']]);
    scene.playCutFeedback(pieces,{mirrorText:true,size:.15});
    assert.deepEqual(calls[3],['feedback',[12,13],true,.15]);
    assert.throws(()=>scene.playCutFeedback([pieces[0],new SceneObject({},14)]),/scéně/);
    assert.equal(calls.length,4,'Foreign handles never reach native mutation');
});

test('XRPlayer exposes native tracking and generic inventory without weapon assumptions', async () => {
    const {XRPlayer,SceneObject}=await sdk();
    let alive=true;
    const calls=[];
    const pose={tracked:true,matrix:Array(16).fill(0),position:[1,2,3],orientation:[0,0,0,1],speed:.5,object:12};
    const state={head:{...pose,object:null},leftHand:{...pose,object:null},rightHand:pose,inventory:[12]};
    const wasm={
        xr_player_state:()=>JSON.stringify(state),
        xr_bind_head:id=>calls.push(['head',id]),
        xr_add_item:id=>calls.push(['add',id]),
        xr_remove_item:id=>calls.push(['remove',id]),
    };
    const tool=new SceneObject(wasm,12),head=new SceneObject(wasm,13);
    const host={bindController:(hand,object,options)=>calls.push(['equip',hand,object?.id??null,options])};
    const player=new XRPlayer(wasm,host,object=>object===tool||object===head,id=>new SceneObject(wasm,id),()=>alive);
    assert.equal(player.rightHand.speed,.5);
    assert.deepEqual(player.head.position,[1,2,3]);
    assert.deepEqual(player.inventory.map(object=>object.id),[12]);
    player.addItem(tool).bindHead(head).equip('right',tool).unequip('left').removeItem(tool);
    assert.deepEqual(calls,[['add',12],['head',13],['equip','right',12,{}],['equip','left',null,{}],['equip','right',null,{}],['remove',12]]);
    assert.throws(()=>player.equip('left',new SceneObject({},12)),/patřit/);
    assert.throws(()=>player.bindHead(new SceneObject({},13)),/patřit/);
    alive=false;
    assert.throws(()=>player.state,/ukončen/);
    assert.throws(()=>player.equip('right',tool),/ukončen/);
});
