import {initEngine} from '/js/engine.js';
import {installXRSimulator} from '/tests/xr/simulator.js';

const q = new URLSearchParams(location.search);
const route = q.get('route'), workload = q.get('workload');
const load = Number(q.get('load')), eyeWidth = Number(q.get('eyeWidth')), eyeHeight = Number(q.get('eyeHeight'));
const diagnostics = q.get('diagnostics') === 'true';
if (!['canvas-transfer', 'shared-framebuffer'].includes(route) || !['rome', 'neon'].includes(workload)) throw Error('Invalid benchmark workload/route');
if (!Number.isInteger(load) || load < 0 || load > 2048 || (workload === 'neon' && load !== 0)) throw Error('Invalid stress count');
if (![eyeWidth, eyeHeight].every(n => Number.isInteger(n) && n >= 64 && n <= 2048)) throw Error('Invalid eye dimensions');
// Deliberately different offsets and sizes exercise both target/viewport paths.
const asymmetric=q.get('asymmetric')!=='false';
const viewports = asymmetric ? [{x: 0, y: 8, width: eyeWidth, height: eyeHeight},
    {x: eyeWidth + 11, y: 0, width: eyeWidth - 16, height: eyeHeight - 16}] : [{x:0,y:0,width:eyeWidth,height:eyeHeight},{x:eyeWidth,y:0,width:eyeWidth,height:eyeHeight}];
const hz = 72;
const sim = installXRSimulator({clockTimers: false, profile: {id: 'presentation-asymmetric-72', hz, eyeWidth, eyeHeight, viewports}});
const queryPrototype = WebGL2RenderingContext.prototype;
const originalCreateQuery = queryPrototype.createQuery;
let createQueryCalls = 0;
queryPrototype.createQuery = function(...args) {
    createQueryCalls++;
    return originalCreateQuery.apply(this, args);
};
const canvas = document.querySelector('#gpu-canvas');
const engine = await initEngine(canvas, {backend: 'webgl', profiling: false});
if (engine.renderer_backend() !== 'Gl') throw Error('WebGL backend required');
engine.resize(eyeWidth, eyeHeight);
engine.setDebugOverlay(false); engine.set_camera_mode('free');
engine.set_grid_visible(false); engine.set_light_helpers_visible(false);
engine.set_simulation_paused(true);
let sceneMetadata, stress;
if (workload === 'rome') {
    const {createScene, createGameplay} = await import('/examples/rim/scene.js');
    const {configureAppearance} = await import('/examples/rim/appearance.js');
    const {createStressLoad} = await import('/examples/rim/stress.js');
    configureAppearance(engine);
    const content = await createScene(engine.scene);
    const game = await createGameplay(engine, content);
    engine.scene.rimInput({pause: true, reset: true});
    for (const {model} of [...content.characters, ...game.enemies]) model.playAnimation('Idle', {loop: true}).pauseAnimation().seekAnimation(0);
    stress = createStressLoad(engine.scene, content); stress.setCount(load);
    sceneMetadata = {source: 'actual Rome createScene/createGameplay/configureAppearance', pose: 'paused Idle clip time 0', stressModels: stress.count,
        expectedWorldHead: [0, 1.7, 42], head: {position: [0, 1.7, 0], orientation: [0, 0, 0, 1]}};
} else {
    const m = await import('/examples/neon-arena/scene.js');
    const world = await m.createScene(engine.scene);
    engine.set_render_effects(false, false); engine.setBackgroundColor([0, 0, 0]);
    engine.setNeonGlow(m.NEON_GLOW);
    engine.postprocessing.configureSSR({resolutionScale: .5, receiverBounds: m.FLOOR_SSR_BOUNDS});
    engine.postprocessing.enable('ssr', m.FLOOR_SSR);
    if (world.arenaFont.selected !== 'cynatar') world.arenaFont.set('cynatar');
    world.galleryIcons.forEach((icon, i) => icon.setColorCutout(0).setOpacity(m.FRUIT_OPACITY,
        {opaqueAbove: .25, variation: m.FRUIT_OPACITY_VARIATION, coverage: world.galleryPatterns[i].coverage ?? .5}));
    engine.xr.configureComboDisplay(world.comboDisplay);
    engine.xr.player.equip('right', world.sword, {cutting: true, requireTrigger: false, haptics: true});
    engine.xr.player.equip('left', world.leftSword, {cutting: true, requireTrigger: false, haptics: true});
    sceneMetadata = {source: 'actual Neon createScene; authored XR panel orientation', glow: m.NEON_GLOW,
        ssr: {...m.FLOOR_SSR, resolutionScale: .5, receiverBounds: m.FLOOR_SSR_BOUNDS}, paused: true,
        head: {position: [0, 1.75, 1.2], orientation: [0, 0, 0, 1]}};
}
let runtimeSettings = null;
const errors = [], fallbacks = [];
engine.xr.addEventListener('firstframe', event => { runtimeSettings = event.detail; });
engine.xr.addEventListener('presentationfallback', event => fallbacks.push(event.detail));
engine.xr.addEventListener('error', event => errors.push(String(event.detail?.message ?? event.detail ?? 'XR error')));
engine.xr.addEventListener('gpuerror', event => errors.push(String(event.detail?.message ?? 'GPU error')));
await engine.xr.enter({resolutionScale: 1, webglPresentation: route, presentationDiagnostics: diagnostics});
if (engine.xr.presentationMode !== route || fallbacks.length) throw Error('Requested presentation route was not selected');
let pumping = false, frameIndex = 0;
const cleanProof = () => {
    const engineProfilingEnabled = engine.frameProfile().enabled;
    if (engineProfilingEnabled !== false) throw Error('Engine profiling must remain disabled');
    if (!diagnostics && createQueryCalls !== 0) throw Error('Clean run allocated WebGL timer queries');
    return {createQueryCalls, engineProfilingEnabled, presentationDiagnostics: diagnostics,
        scope: 'actual WebGL2 createQuery calls intercepted before initEngine, all contexts'};
};
cleanProof();
const boundary = () => ({cleanProof: cleanProof(), atMs: performance.now(), frameIndex, profileFrame: diagnostics ? engine.xr.presentationProfile.frameCount : null,
    api: window.renderApiCounters ? structuredClone(window.renderApiCounters) : null, renderer: engine.renderer_stats(), debug: engine.debugStats(), postprocessing: engine.postprocessing.stats(),
    actualWorldHead: engine.xr.player.head.position, nativeRim: workload === 'rome' ? engine.scene.rimState : null});
const check = () => {
    if (!diagnostics && createQueryCalls !== 0) throw Error('Clean run allocated WebGL timer queries');
    if (engine.xr.lastError) throw Error(String(engine.xr.lastError));
    if (!engine.xr.isPresenting || sim.layer?.gl.isContextLost()) throw Error('XR stopped or WebGL context lost');
    if (errors.length || fallbacks.length || window.renderRuntimeLosses?.length) throw Error('Runtime error/loss/fallback detected');
};
window.xrPresentationBenchmark = {
    metadata: {workload, route, diagnostics, engineProfiling: false, configuredSimHz: hz,
        clock: 'real DOM RAF; simulator advances fixed 1000/72ms per callback, not headset cadence',
        viewports, eyeWidth, eyeHeight, asymmetric, scene: sceneMetadata, userAgent: navigator.userAgent},
    boundary, check, cleanProof,
    get runtimeSettings() { return runtimeSettings; },
    get errors() { return [...errors]; },
    get fallbacks() { return [...fallbacks]; },
    run(ms, measure = false) {
        if (pumping || !Number.isFinite(ms) || ms <= 0 || ms > 300000) throw Error('Invalid/overlapping measurement');
        pumping = true;
        return new Promise((resolve, reject) => {
            const records = []; let epoch = null;
            const tick = stamp => {
                const start = performance.now();
                try {
                    epoch ??= start;
                    if (!sim.session?.pendingFrameCount) throw Error('XR callback loop stopped');
                    sim.frame({dt: 1000 / hz, head: sceneMetadata.head, controllers: {left: null, right: null}});
                    frameIndex++;
                    check();
                    const end = performance.now();
                    if (measure) records.push({index: frameIndex - 1, stamp, startMs: start, endMs: end, cpuMs: end - start, simTimeMs: sim.time});
                    if (end - epoch >= ms) { pumping = false; resolve({frames: records, elapsedMs: end - epoch}); }
                    else requestAnimationFrame(tick);
                } catch (error) { pumping = false; reject(error); }
            };
            requestAnimationFrame(tick);
        });
    },
    profile(start, end) {
        if (!diagnostics) return null;
        const profile = engine.xr.presentationProfile;
        const frames = profile.frames.filter(f => f.index >= start && f.index < end);
        return {...profile, frames, sampleBoundary: {startInclusive: start, endExclusive: end,
            expectedFrames: end - start, retainedFrames: frames.length, complete: frames.length === end - start},
            counterScope: 'pending, validGpuSamples, dropped counts and reasons cover entire profile lifetime; frames filtered to sample'};
    },
    capture() {
        if (pumping) throw Error('Readback is forbidden while timing');
        check();
        const image = sim.pixels('stereo'), target = document.createElement('canvas');
        target.width = image.width; target.height = image.height;
        target.getContext('2d').putImageData(image, 0, 0);
        const eyeEvidence = viewports.map(region => {
            // ImageData is top-down; viewports use GL bottom-up Y. Reference the
            // eye's own corner, so unused framebuffer gutters cannot count as geometry.
            const top = image.height - region.y - region.height;
            const first = (top * image.width + region.x) * 4;
            const background = [...image.data.slice(first, first + 3)];
            let foreground = 0, minimum = 255, maximum = 0;
            const unique = new Set();
            for (let y = top; y < top + region.height; y++) {
                for (let x = region.x; x < region.x + region.width; x++) {
                    const i = (y * image.width + x) * 4;
                    const rgb = image.data.subarray(i, i + 3);
                    if (Math.abs(rgb[0] - background[0]) + Math.abs(rgb[1] - background[1]) + Math.abs(rgb[2] - background[2]) > 20) foreground++;
                    for (const c of rgb) { minimum = Math.min(minimum, c); maximum = Math.max(maximum, c); }
                    if (unique.size < 32) unique.add(rgb.join(','));
                }
            }
            if (foreground < 100 || unique.size < 2) throw Error('XR eye is missing rendered geometry');
            return {foregroundPixels: foreground, background, uniqueRgbCapped32: unique.size, channelRange: [minimum, maximum]};
        });
        return {png: target.toDataURL('image/png'), width: image.width, height: image.height,
            foregroundPixels: eyeEvidence.reduce((n, e) => n + e.foregroundPixels, 0), eyeEvidence};
    },
    async dispose() { if (pumping) throw Error('Stop timing before disposal'); await engine.xr.exit(); stress?.dispose(); sim.dispose(); engine.dispose(); queryPrototype.createQuery = originalCreateQuery; },
};
