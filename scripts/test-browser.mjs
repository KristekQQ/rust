import puppeteer from 'puppeteer';
import { verifyCutting } from '../tests/cutting-browser.mjs';
import { verifyFox } from '../tests/fox-browser.mjs';
import { verifyProjectiles } from '../tests/projectiles-browser.mjs';
import { browserArgs } from './browser-options.mjs';
import { parseShard, partitionCases, suiteTimeout } from './browser-suite-plan.mjs';
import { createHash } from 'node:crypto';
import { spawn, spawnSync, execFileSync } from 'node:child_process';
import { mkdir, writeFile, readFile, readdir, realpath } from 'node:fs/promises';
import path from 'node:path';
import { root, assertFreshBuild } from './build-fingerprint.mjs';

const localOption = key => {
    try { return execFileSync('git', ['config', '--local', '--get', key], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); }
    catch { return undefined; }
};
// Keep machine-specific runtime paths in local Git config, never in repository code.
const browserNode = process.env.BROWSER_NODE || localOption('verification.node');
if (browserNode && await realpath(browserNode) !== await realpath(process.execPath)) {
    const child = spawnSync(browserNode, process.argv.slice(1), { stdio: 'inherit' });
    if (child.error) console.error(child.error.message);
    process.exit(child.status ?? 1);
}

const suites = [
    {name:'rim-three-stress',path:'examples/rim-three/?verify=1',status:'#three-stress-result',backends:['webgl'],softwareTimeoutMs:600000},
    ...['basic','culling','cutting','cynatar','fox-control','gltf','lights','mirror','monkey','neon-arena','neon-enemies','neon-font-preview','neon-robot','postprocessing','projectiles','rim','shadows','webxr','forest-arena','crystal-cave','monster-room','table-tennis'].map(scene=>({name:'info-'+scene,path:'tests/runtime-info-example.html?scene='+scene,status:'#result',backends:['webgl'],xrReport:true})),
    {name:"runtime-info-xr",path:"tests/runtime-info-xr.html",status:"#result",backends:["webgl"],xrReport:true},
    {name:"neon-arena-entry",path:"tests/neon-arena-entry.html",interaction:"arena-entry",backends:["webgl"]},
    {name:"basic",path:"examples/basic/",interaction:"example-smoke"},
    {name:"shadows",path:"examples/shadows/",interaction:"example-smoke"},
    {name:"mirror",path:"examples/mirror/",interaction:"example-smoke"},
    {name:"lights",path:"examples/lights/",interaction:"example-smoke"},
    {name:"gltf",path:"examples/gltf/",interaction:"example-smoke"},
    {name:"forest-arena",path:"examples/forest-arena/?verify=1",status:"#result",button:"#verify"},
    {name:"forest-arena-xr",path:"tests/forest-arena-xr.html",status:"#result",backends:["webgl"],xrReport:true},
    {name:"crystal-cave",path:"examples/crystal-cave/?verify=1",status:"#result",button:"#verify"},
    {name:"crystal-cave-xr",path:"tests/crystal-cave-xr.html",status:"#result",backends:["webgl"],xrReport:true},
    {name:"monster-room",path:"examples/monster-room/?verify=1",status:"#result",button:"#verify"},
    {name:"monster-room-xr",path:"tests/monster-room-xr.html",status:"#result",backends:["webgl"],xrReport:true,softwareTimeoutMs:600000},
    {name:"table-tennis",path:"examples/table-tennis/?verify=1",status:"#result",button:"#verify"},
    {name:"table-tennis-xr",path:"tests/table-tennis-xr.html",status:"#result",backends:["webgl"],xrReport:true},
    {name:"monster-combat",path:"tests/monster-combat.html",status:"#result",backends:["webgl"],xrReport:true},
    {name:"gltf-batching",path:"tests/gltf-batching.html",status:"#result"},
    {name:"gltf-opaque-batching",path:"tests/gltf-opaque-batching.html",status:"#result"},
    {name:"gltf-light-classes",path:"tests/gltf-light-classes.html",status:"#result"},
    {name:"gltf-gpu-transforms",path:"tests/gltf-gpu-transforms.html",status:"#result"},
    {name:"gltf-visibility",path:"tests/gltf-visibility.html",status:"#result"},
    {name:"scene-appearance",path:"tests/scene-appearance.html",status:"#result"},
    {name:"antialias",path:"tests/antialias.html",status:"#result"},
    {name:"rim-refresh",path:"examples/rim/",interaction:"rim-refresh"},
    {name:"rim-device-logs",path:"tests/rim-device-logs.html",status:"#device-result",xrReport:true,softwareTimeoutMs:600000},
    {name:"rim-gameplay-120",path:"tests/rim-gameplay.html?hz=120",status:"#result",xrReport:true,softwareTimeoutMs:600000},
    {name:"rim-cut-scheduler-control",path:"tests/rim-gameplay.html?omitXRHook=1",status:"#result",backends:["webgl"],xrReport:true},
    {name:"rim-stress-xr",path:"tests/rim-gameplay.html?stress=32",status:"#result",xrReport:true,softwareTimeoutMs:600000},
    {name:"rim-spatial-batches-xr",path:"tests/rim-gameplay.html?stress=256",status:"#result",xrReport:true,softwareTimeoutMs:600000},
    {name:"rim-gameplay",path:"tests/rim-gameplay.html",status:"#result",xrReport:true,softwareTimeoutMs:600000},
    {name:'rim',path:'examples/rim/?verify=1',status:'#result',button:'#verify'},
    { name: 'shadow-cache', path: 'tests/shadow-cache.html', status: '#result' },
    { name: 'gltf-material-cache', path: 'tests/gltf-material-cache.html', status: '#result' },
    { name: 'neon-enemies', path: 'examples/neon-enemies/?verify=1', status: '#result', button: '#verify', softwareTimeoutMs:600000 },
    { name: 'neon-robot', path: 'examples/neon-robot/?verify=1', status: '#result', button: '#verify' },
    { name: 'postprocess-fusion', path: 'tests/postprocess-fusion.html', status: '#result' },
    { name: 'cynatar', path: 'examples/cynatar/?verify=1', status: '#result', button: '#verify' },
    { name: 'monkey', path: 'examples/monkey/?verify=1', status: '#result', button: '#verify' },
    { name: 'arena-minute', path: 'tests/arena-minute.html', status: '#result', backends:['webgl'], xrReport:true, softwareTimeoutMs:600000 },
    { name: 'arena-presets', path: 'tests/arena-presets.html', status: '#result', backends:['webgl'] },
    { name: 'arena-waves', path: 'tests/arena-waves.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-arena-benchmark', path: 'tests/neon-arena-benchmark.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-font-preview', path: 'examples/neon-font-preview/', status: '#result', button: '#verify' },
    { name: 'frame-graph', path: 'tests/frame-graph.html', status: '#result', button: '#check', backends: ['webgpu'] },
    { name: 'neon-arena-gpu-profile', path: 'tests/webxr-webgpu.html?profile=1', status: '#result', backends:['webgpu'], xrReport:true },
    { name: 'neon-arena-profile', path: 'tests/neon-arena-profile.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-arena-xr', path: 'tests/neon-arena-xr.html', status: '#result', backends:['webgl'], xrReport:true, softwareTimeoutMs:1200000 },
    { name: 'neon-arena', path: 'examples/neon-arena/', status: '#result', button: '#verify', softwareTimeoutMs:600000 },
    { name: 'webxr-contract', path: 'tests/webxr-contract.html', status: '#result', backends: ['webgl'], xrReport: true },
    { name: 'webxr-simulation', path: 'tests/webxr-simulation.html', status: '#result', backends: ['webgl'], xrReport: true, softwareTimeoutMs:1200000 },
    { name: 'webxr-preview', path: 'examples/webxr/', interaction: 'webxr-preview', backends: ['webgl'] },
    { name: 'webxr', path: 'tests/webxr.html', status: '#result', xrReport: true },
    { name: 'webxr-webgpu', path: 'tests/webxr-webgpu.html', status: '#result', backends: ['webgpu'], xrReport: true },
    { name: 'projectiles', path: 'examples/projectiles/', interaction: 'projectiles' },
    { name: 'cutting', path: 'examples/cutting/', interaction: 'cutting' },
    { name: 'cutting-compute', path: 'tests/cutting-compute.html', status: '#result' },
    { name: 'cutting-resident', path: 'tests/cutting-resident.html', status: '#result' },
    { name: 'scene-sdk', path: 'tests/scene-sdk.html', status: '#result' },
    { name: 'lifecycle', path: 'tests/lifecycle.html', status: '#result' },
    { name: 'gltf-sdk', path: 'tests/gltf-sdk.html', status: '#result' },
    { name: 'gltf-full', path: 'tests/gltf-full.html', status: '#result' },
    { name: 'render-mirrored', path: 'tests/render-mirrored.html', status: '#result' },
    { name: 'render-target', path: 'tests/render-target.html', status: '#result', button: '#check' },
    { name: 'render-effects', path: 'tests/render-effects.html', status: '#status', button: '#verify' },
    { name: 'postprocessing', path: 'examples/postprocessing/', status: '#ui [role="status"]', button: '#verify' },
    { name: 'fox-control', path: 'examples/fox-control/', status: '#status', interaction: 'fox' },
    { name: 'culling', path: 'examples/culling/', interaction: 'culling' },
];
let xrSlow = false, arenaLegacyStress = false, productionRoot = null;
let shard = null, timeoutExplicit = false;
let backends = ['webgpu', 'webgl'], selected = suites, software = false, headed = false, capture = false, timeout = 90000;
for (let i = 2; i < process.argv.length; i++) {
    const arg = process.argv[i];
    if (arg === '--production-root') productionRoot = path.resolve(process.argv[++i] || 'output/production');
    else if (arg === '--arena-legacy-stress') arenaLegacyStress = true;
    else if (arg === '--xr-slow') xrSlow = true;
    else if (arg === '--software') software = true;
    else if (arg === '--capture') capture = true;
    else if (arg === '--headed') headed = true;
    else if (arg === '--shard') shard = parseShard(process.argv[++i]);
    else if (arg === '--backend') {
        const value = process.argv[++i];
        if (!['all', 'webgpu', 'webgl'].includes(value)) throw new Error('--backend requires all, webgpu or webgl');
        backends = value === 'all' ? ['webgpu', 'webgl'] : [value];
    } else if (arg === '--page') {
        const names = (process.argv[++i] || '').split(',');
        if (names.some(name => !suites.some(suite => suite.name === name))) throw new Error(`Unknown suite. Choose: ${suites.map(suite => suite.name).join(', ')}`);
        selected = suites.filter(suite => names.includes(suite.name));
    } else if (arg === '--timeout') {
        timeoutExplicit = true;
        timeout = Number(process.argv[++i]);
        if (!Number.isFinite(timeout) || timeout < 1000) throw new Error('--timeout must be at least 1000 ms');
    } else if (arg === '--help') {
        console.log('node scripts/test-browser.mjs [--backend all|webgpu|webgl] [--page suite,suite] [--shard index/count] [--software] [--headed] [--capture] [--xr-slow] [--arena-legacy-stress] [--timeout ms]');
        process.exit(0);
    } else throw new Error(`Unknown option ${arg}`);
}

const executionPlan = partitionCases(backends.flatMap(backend => selected
    .filter(suite => !suite.backends || suite.backends.includes(backend))
    .map(suite => ({backend, suite}))), shard);
const timeoutOptions = {software, timeout, timeoutExplicit};
if (productionRoot) process.env.VERIFY_WASM_PROFILE = 'release';
let browser, server;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function verifyHud(page, simpleScene = false) {
    const stats = () => page.evaluate(async () => (await import('./main.js')).engine.debugStats());
    const initial = await stats();
    if (initial.enabled || initial.hudFrames || initial.hudRefreshes) throw new Error('HUD must start disabled without work');
    const corner = () => page.evaluate(async () => {
        const { capturePixels } = await import('../../tests/pixels.js');
        const image = await capturePixels(document.querySelector('#gpu-canvas'));
        let bright = 0;
        for (let y = 12; y < Math.min(85, image.height); y++) {
            for (let x = 12; x < Math.min(320, image.width); x++) {
                const offset = (y * image.width + x) * 4;
                if (image.data[offset] > 190 && image.data[offset + 1] > 210 && image.data[offset + 2] > 220) bright++;
            }
        }
        return bright;
    });
    const before = await corner();
    await page.keyboard.press('F3');
    await page.waitForFunction(async () => (await import('./main.js')).engine.debugStats().hudFrames > 3);
    const visible = await corner();
    if (visible < 150 || visible <= before + 100) throw new Error(`Rust HUD glyph pixels missing: ${before} -> ${visible}`);
    const first = await stats();
    if (!Number.isInteger(first.drawCalls) || first.mainDrawCalls < 1 || first.drawCalls < first.mainDrawCalls + 1) throw new Error('Actual draw counters missing from HUD stats');
    if (simpleScene && (first.mainDrawCalls !== 1 || first.drawCalls !== 2)) throw new Error(`Instanced cubes + HUD should be 1 + 1 draws: ${JSON.stringify(first)}`);
    const drawRow = await page.evaluate(async () => {
        const { capturePixels } = await import('../../tests/pixels.js');
        const image = await capturePixels(document.querySelector('#gpu-canvas'));let bright = 0;
        for(let y=92;y<109;y++)for(let x=12;x<320;x++){const i=(y*image.width+x)*4;if(image.data[i]>190&&image.data[i+1]>210&&image.data[i+2]>220)bright++;}
        return bright;
    });
    if(drawRow<100)throw new Error('Draw-call HUD row has no glyph pixels');
    // Cache cadence depends on native frame dt. SwiftShader can take >250ms per
    // frame, legitimately refreshing every frame. Render six real frames while
    // controlling only this short timing probe; keep pixel and draw checks real.
    const cadence = await page.evaluate(async () => {
        const {engine}=await import('./main.js');
        const original=Object.getOwnPropertyDescriptor(performance,'now');
        let time=performance.now();
        Object.defineProperty(performance,'now',{configurable:true,value:()=>time});
        try {
            const before=engine.debugStats();
            for(let frame=0;frame<6;frame++) {
                time+=16;
                await new Promise(requestAnimationFrame);
            }
            return {before,after:engine.debugStats()};
        } finally {
            if(original)Object.defineProperty(performance,'now',original);
            else delete performance.now;
        }
    });
    const {before:cacheBefore,after:cacheAfter}=cadence;
    if (cacheAfter.hudFrames <= cacheBefore.hudFrames || cacheAfter.hudRefreshes - cacheBefore.hudRefreshes > 1
        || cacheAfter.hudRefreshes >= cacheAfter.hudFrames) throw new Error('HUD text must be cached between refreshes: '+JSON.stringify(cadence));
    await page.evaluate(async () => (await import('./main.js')).engine.set_output_mode('texture'));
    if (await corner() < 150) throw new Error('HUD missing from texture output');
    if(simpleScene && (await stats()).drawCalls !== 3)throw new Error('Texture presentation must add exactly one draw');
    await page.evaluate(async () => {
        const { engine } = await import('./main.js');
        engine.resize(420, 300);
        engine.postprocessing.enable('godRays', { strength: 0.1 });
    });
    if (await corner() < 150) throw new Error('HUD missing after resize/postprocessing');
    if(simpleScene && (await stats()).drawCalls !== 4)throw new Error('One postprocessing effect must add exactly one draw');
    await page.evaluate(async () => {
        const { engine } = await import('./main.js');
        engine.postprocessing.disable('godRays');
        const canvas = document.querySelector('#gpu-canvas');
        engine.resize(canvas.clientWidth, canvas.clientHeight);
    });
    await page.keyboard.press('F3');
    const off = await stats();
    if (await corner() > before + 100) throw new Error('Disabled HUD pixels remain');
    await delay(350);
    const later = await stats();
    if (later.enabled || later.hudFrames !== off.hudFrames || later.hudRefreshes !== off.hudRefreshes) {
        throw new Error('Disabled HUD still draws or refreshes');
    }
    await page.evaluate(async () => (await import('./main.js')).engine.set_output_mode('canvas'));
}
async function verifyCulling(page) {
    await page.evaluate(async () => {
        const { engine } = await import('./main.js');
        document.documentElement.dataset.rendererBackend = engine.renderer_backend();
        engine.setCameraView([0, 0, 0], [0, 0, -1]);
    });
    await verifyHud(page, true);
    const bvh = await page.evaluate(async () => {
        const { engine } = await import('./main.js');
        engine.setDebugOverlay(true);
        engine.setCameraView([0.1, 0, 0], [0.1, 0, -1]);
        for (let i = 0; i < 6; i++) await new Promise(requestAnimationFrame);
        return engine.debugStats();
    });
    if (bvh.total !== 10000 || bvh.visible <= 0 || bvh.visible >= 10000
        || bvh.objectTests >= 7500 || bvh.bvhNodes <= 0) throw new Error(`BVH did not prune: ${JSON.stringify(bvh)}`);
    await page.evaluate(async () => (await import('./main.js')).engine.set_frustum_culling(false));
    await page.waitForFunction(async () => (await import('./main.js')).engine.renderer_stats().visible === 10000);
    await page.evaluate(async () => (await import('./main.js')).engine.set_frustum_culling(true));
    await page.waitForFunction(async () => (await import('./main.js')).engine.renderer_stats().visible < 10000);
    await delay(400);
    const final = await page.evaluate(async () => {
        for (let i = 0; i < 4; i++) await new Promise(requestAnimationFrame);
        return (await import('./main.js')).engine.debugStats();
    });
    if (final.fps <= 0 || final.frameMs <= 0) throw new Error(`Slow scene FPS missing: ${JSON.stringify(final)}`);
    return `PASS: 10k static BVH, ${bvh.visible} visible / ${bvh.objectTests} object tests; Rust HUD F3/cached/off/texture pixels`;
}
let interrupted = false;
const report = { started: new Date().toISOString(), revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), software, node: process.execPath, architecture: process.arch, shard, plan: executionPlan.map(({backend,suite}) => ({backend,suite:suite.name})), results: [] };
const output = path.join(root, 'output/verification');
async function saveProgress(active = null) {
    await writeFile(path.join(output, 'browser-progress.json'), JSON.stringify({
        revision: report.revision, shard, plan: report.plan, active,
        results: report.results.map(({suite,backend,status,error}) => ({suite,backend,status,error})),
    }, null, 2) + '\n');
}
async function applicationFingerprint() {
    const hash = createHash('sha256');
    async function visit(directory) {
        const entries = await readdir(path.join(root, directory), { withFileTypes: true });
        for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
            const relative = path.join(directory, entry.name);
            if (entry.isDirectory()) await visit(relative);
            else if (/\.(m?js|html|ts)$/.test(entry.name)) hash.update(relative).update('\0').update(await readFile(path.join(root, relative))).update('\0');
        }
    }
    for (const directory of ['js', 'tests', 'examples']) await visit(directory);
    return hash.digest('hex');
}
async function startServer() {
    const serverArgs = ['-u', 'scripts/serve-local.py', '0'];
    if (productionRoot) serverArgs.push('--production', '--directory', productionRoot, '--test-directory', path.join(root, 'tests'));
    server = spawn('python3', serverArgs, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    server.stderr.on('data', data => { log = (log + data).slice(-4000); });
    return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`HTTP server startup timeout: ${log}`)), 10000);
        server.once('exit', code => { clearTimeout(timer); reject(new Error(`HTTP server exited ${code}: ${log}`)); });
        let stdout = '';
        server.stdout.on('data', data => {
            stdout += data;
            const match = stdout.match(/http:\/\/127\.0\.0\.1:\d+/);
            if (match) { clearTimeout(timer); resolve(match[0] + '/'); }
        });
    });
}
async function cleanup() {
    if (browser) await browser.close().catch(() => {});
    if (server && server.exitCode === null) server.kill();
}
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => {
    interrupted = true;
    process.exitCode = signal === 'SIGINT' ? 130 : 143;
    void cleanup();
});
try {
    report.build = await assertFreshBuild();
    if (productionRoot) {
        const {assertProductionBuild} = await import('./build-production.mjs');
        report.production = await assertProductionBuild(productionRoot, {checkCurrentSource: true});
    }
    report.application = await applicationFingerprint();
    await mkdir(output, { recursive: true });
    const baseUrl = await startServer();
    const args = browserArgs({ software });
    report.sandboxDisabled = args.includes('--no-sandbox');
    report.browserExecutable = process.env.BROWSER_EXECUTABLE || localOption('verification.browser') || await puppeteer.executablePath();
    const protocolTimeout = Math.max(...executionPlan.map(({suite}) => suiteTimeout(suite, timeoutOptions)));
    browser = await puppeteer.launch({ headless: !headed, args, executablePath: report.browserExecutable, timeout: 30000, protocolTimeout, dumpio: process.env.DEBUG_BROWSER_STARTUP === '1' });
    report.browser = await browser.version();
    for (const {backend, suite} of executionPlan) {
        if (interrupted) throw new Error('Verification interrupted');
        await saveProgress({suite:suite.name,backend});
        const page = await browser.newPage();
        await page.setViewport({ width: 960, height: 640, deviceScaleFactor: 1 });
        const caseTimeout = suiteTimeout(suite, timeoutOptions);
        page.setDefaultTimeout(caseTimeout);
        page.setDefaultNavigationTimeout(caseTimeout);
        const errors = [], abortedRequests = [];
        page.on('pageerror', error => errors.push(`JS: ${error.message || String(error)}`));
        // Dawn reports validation errors as warnings, not page/console errors.
        // Observe the actual GPU error event before the engine requests a device.
        await page.evaluateOnNewDocument(() => {
            if (!globalThis.GPUAdapter) return;
            const requestDevice = GPUAdapter.prototype.requestDevice;
            GPUAdapter.prototype.requestDevice = async function (...args) {
                const device = await requestDevice.apply(this, args);
                device.addEventListener('uncapturederror', event => console.error(`GPU validation: ${event.error.message}`));
                return device;
            };
        });
        page.on('console', message => { if (message.type() === 'error') errors.push(`console: ${message.text()}`); });
        page.on('response', response => { if (response.status() >= 400 && !response.url().endsWith('/favicon.ico')) errors.push(`HTTP ${response.status()}: ${response.url()}`); });
        page.on('requestfailed', request => {
            const message = `request: ${request.url()} ${request.failure()?.errorText}`;
            // SDK scenarios intentionally abort loads; failures still surface through
            // awaited model imports/page assertions. Record cancellation separately.
            if (request.failure()?.errorText === 'net::ERR_ABORTED') abortedRequests.push(message);
            else errors.push(message);
        });
        const result = { suite: suite.name, backend, timeoutMs: caseTimeout, status: 'FAIL' };
        try {
            const url = new URL(suite.path, baseUrl);
            url.searchParams.set('backend', backend === 'webgl' ? 'webgl' : 'auto');
            if (arenaLegacyStress && suite.name === 'arena-minute') url.searchParams.set('legacyStress','1');
            if (xrSlow && suite.xrReport) url.searchParams.set('slow','1');
            result.url = url.href;
            await page.goto(url.href, { waitUntil: 'load' });
            if (suite.button) {
                await page.waitForFunction((selector, status) => {
                    const control = document.querySelector(selector);
                    return /FAIL/.test(document.querySelector(status)?.textContent || '') || (control && typeof control.onclick === 'function');
                }, { polling: 100 }, suite.button, suite.status);
                const failure = await page.$eval(suite.status, node => /FAIL/.test(node.textContent || ''));
                if(suite.name==='rim'){
                    result.verifyControl=await page.evaluate(selector=>{const node=document.querySelector(selector),r=node.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return {hit:hit?.id,hitElement:hit?.outerHTML.slice(0,700),stack:document.elementsFromPoint(r.x+r.width/2,r.y+r.height/2).slice(0,5).map(n=>({tag:n.tagName,id:n.id,cls:n.className})),rect:{x:r.x,y:r.y,w:r.width,h:r.height},expected:node.id,detailsOpen:document.querySelector('details').open,ready:document.documentElement.dataset.ready,visibility:document.visibilityState};},suite.button);
                    if(result.verifyControl.hit!==result.verifyControl.expected)throw Error('Verify button covered: '+JSON.stringify(result.verifyControl));
                }
                if (!failure) await page.click(suite.button);
            }
            if (suite.interaction === 'arena-entry') {
                await page.waitForFunction(() => window.arenaEntryTest?.ready);
                for (const viewport of [{width:390,height:740,deviceScaleFactor:1},{width:960,height:640,deviceScaleFactor:1}]) {
                    await page.setViewport(viewport);
                    await page.evaluate(async()=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);window.arenaEntryTest.before();});
                }
                await page.screenshot({path:path.join(output,'neon-arena-entry-before-vr.png')});
                await page.click('#enter');
                await page.waitForFunction(() => window.arenaEntryTest.presenting);
                await page.evaluate(()=>window.arenaEntryTest.inside());
                const stereo=await page.evaluate(()=>window.arenaEntryTest.stereoPng.split(',')[1]);
                await writeFile(path.join(output,'neon-arena-entry-spatial.png'),Buffer.from(stereo,'base64'));
                await page.evaluate(()=>window.arenaEntryTest.leave());
                await page.waitForFunction(()=>!document.querySelector('#enter').disabled);
                await page.click('#enter');
                await page.waitForFunction(() => window.arenaEntryTest.presenting);
                result.text=await page.evaluate(()=>window.arenaEntryTest.finish());
                result.entryChecks=await page.evaluate(()=>window.arenaEntryTest.checks);
            }
            else if (suite.interaction === 'rim-refresh') {
                await page.setCacheEnabled(true);
                const ready=()=>page.waitForFunction(()=>window.rimDemo&&window.rimCutTransportDiagnostics?.status==='ready');
                await ready();
                const before=await page.evaluate(()=>({build:document.documentElement.dataset.buildId,backend:window.rimDemo.engine.renderer_backend()}));
                const storageKey='verification-refresh-preference';
                await page.evaluate(key=>localStorage.setItem(key,'preserve'),storageKey);
                const responses=[];
                const observe=response=>{
                    const url=new URL(response.url());
                    if(url.origin===new URL(baseUrl).origin&&(/\.(?:js|wasm|glb|gltf|json)$/.test(url.pathname)||url.pathname==='/examples/rim/'))responses.push({path:url.pathname,cache:response.fromCache(),serviceWorker:response.fromServiceWorker(),status:response.status(),policy:response.headers()['cache-control']});
                };
                page.on('response',observe);
                const clean=new URL(page.url());clean.searchParams.delete('v');
                await page.goto(clean.href+'&v=old-link',{waitUntil:'load'});
                await ready();
                if(new URL(page.url()).searchParams.has('v'))throw Error('Legacy page version remains in address bar');
                responses.length=0;
                await page.reload({waitUntil:'load'});
                await ready();
                page.off('response',observe);
                const after=await page.evaluate(key=>({build:document.documentElement.dataset.buildId,backend:window.rimDemo.engine.renderer_backend(),preference:localStorage.getItem(key)}),storageKey);
                await page.evaluate(key=>localStorage.removeItem(key),storageKey);
                if(after.build!==before.build||after.backend!==before.backend||after.preference!=='preserve')throw Error('Refresh changed build/backend or erased preferences');
                if(responses.some(r=>r.cache||r.serviceWorker||r.status!==200||!r.policy?.includes('no-store')))throw Error('Refresh reused cached or invalid runtime resources: '+JSON.stringify(responses));
                for(const suffix of ['/examples/rim/','/examples/rim/main.js','/examples/rim/cut-worker.js','/pkg/webgpu_wasm_bg.wasm'])if(!responses.some(r=>r.path===suffix))throw Error('Missing fresh runtime request '+suffix);
                await page.select('#stress-load','32');
                const stressEvidence=await page.evaluate(async()=>{
                    const demo=window.rimDemo;
                    for(let i=0;i<4;i++)await new Promise(requestAnimationFrame);
                    const value={count:demo.stress.count,native:demo.engine.scene.rimState.stressModels,query:new URL(location.href).searchParams.get('stress'),select:document.querySelector('#stress-load').value};
                    if(value.count!==32||value.native!==32||value.query!=='32'||value.select!=='32')throw Error('Stress selection/state/URL disagree: '+JSON.stringify(value));
                    return value;
                });
                await page.reload({waitUntil:'load'});await ready();
                await page.evaluate(()=>{const d=window.rimDemo;if(d.stress.count!==32||d.engine.scene.rimState.stressModels!==32)throw Error('Stress query lost after refresh');d.setStress(0);if(new URL(location.href).searchParams.has('stress')||d.stress.objects.length)throw Error('Stress reset leaves geometry or URL state');});
                await page.setViewport({width:390,height:740,deviceScaleFactor:1});
                await page.evaluate(async()=>{
                    await new Promise(requestAnimationFrame);
                    for(const selector of ['#enter','#stress-load']){
                        const node=document.querySelector(selector),r=node.getBoundingClientRect();
                        if(r.width<=0||r.height<=0||r.top<0||r.bottom>innerHeight||r.left<0||r.right>innerWidth)throw Error('Stress/VR control offscreen: '+selector);
                        const top=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);if(top!==node&&!node.contains(top))throw Error('Stress/VR control obscured: '+selector);
                    }
                });
                // A discarded/cached document can receive input and XR-end
                // events after pagehide. Restoring its dead engine must reload.
                await page.evaluate(()=>{
                    window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true}));
                    window.dispatchEvent(new FocusEvent('blur'));
                    window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW'}));
                    window.rimDemo.engine.xr.dispatchEvent(new Event('end'));
                });
                const restoredNavigation=page.waitForNavigation({waitUntil:'load',timeout:10000});
                await page.evaluate(()=>{setTimeout(()=>window.dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true})),0);});
                await restoredNavigation;await ready();
                await page.click('#play');
                result.lifecycleEvidence=await page.evaluate(()=>({build:document.documentElement.dataset.buildId,active:window.rimDemo.engine.scene.rimState.active}));
                if(!result.lifecycleEvidence.active||result.lifecycleEvidence.build!==before.build)throw Error('Restored scene cannot start current gameplay');
                result.stressEvidence=stressEvidence;
                result.refreshEvidence={before,after,resources:responses};
                result.text='PASS: plain page reload with cache enabled fetched current modules, worker and WASM; canonical URL and preferences preserved';
            }
            else if (suite.interaction === 'example-smoke') {
                result.text = await page.evaluate(async () => {
                    // Await the authored scene bootstrap, then reuse its existing SDK owner.
                    await import('./main.js');
                    const {initEngine} = await import('../../js/engine.js');
                    const canvas = document.querySelector('#gpu-canvas');
                    if (!(canvas instanceof HTMLCanvasElement)) throw Error('Missing scene canvas');
                    const engine = await initEngine(canvas, {
                        backend: canvas.dataset.backend === 'webgl' ? 'webgl' : 'auto',
                        profiling: canvas.dataset.frameProfile === 'true',
                    });
                    for (let i=0; i<6; i++) await new Promise(requestAnimationFrame);
                    const {capturePixels,assertForeground} = await import('../../tests/pixels.js');
                    assertForeground(await capturePixels(canvas), 'Authored example geometry', 1000);
                    if (engine.scene.objectCount < 1) throw Error('Empty scene');
                    document.documentElement.dataset.rendererBackend = engine.renderer_backend();
                    return 'PASS: authored example boots with real Rust geometry and pixels';
                });
            }
            else if (suite.interaction === 'webxr-preview') {
                result.text = await page.evaluate(async () => {
                    const {engine} = await import('./main.js');
                    const {capturePixels, assertForeground} = await import('../../tests/pixels.js');
                    assertForeground(await capturePixels(document.querySelector('#gpu-canvas')), 'WebXR example preview');
                    if (engine.scene.objectCount !== 10 || engine.scene.lightCount !== 2) throw new Error('Incomplete WebXR example scene');
                    if (!document.querySelector('#status').textContent.includes('Meta Quest')) throw new Error('Missing unsupported-headset UI');
                    const {shootApple} = await import('./main.js');
                    const apple = shootApple();
                    if (!apple) throw new Error('Apple launcher failed');
                    const y = apple.meshInfo.position[1];
                    await new Promise((resolve, reject) => {
                        let frame;
                        const timer = setTimeout(() => {
                            cancelAnimationFrame(frame);
                            reject(new Error('Rust projectile did not rise within 15 seconds'));
                        }, 15000);
                        const sample = () => {
                            if (apple.exists && apple.meshInfo.position[1] > y + .3) {
                                clearTimeout(timer); resolve();
                            } else frame = requestAnimationFrame(sample);
                        };
                        frame = requestAnimationFrame(sample);
                    });
                    if (apple.meshInfo.position[1] <= y + 0.3) throw new Error('Rust projectile did not rise');
                    assertForeground(await capturePixels(document.querySelector('#gpu-canvas')), 'flying apple preview');
                    // This example deliberately requires WebGL, even when desktop WebGPU exists.
                    document.documentElement.dataset.rendererBackend = engine.renderer_backend();
                    return 'PASS: vlastní WebXR ukázka, skutečné pixely náhledu a UI bez headsetu';
                });
            }
            else if (suite.interaction === 'cutting') result.text = await verifyCutting(page);
            else if (suite.interaction === 'projectiles') result.text = await verifyProjectiles(page);
            else if (suite.interaction === 'fox') result.text = await verifyFox(page, verifyHud);
            else if (suite.interaction === 'culling') result.text = await verifyCulling(page);
            else {
                await page.waitForFunction(selector => /^(PASS|FAIL):/.test(document.querySelector(selector)?.textContent?.trim() || ''), { polling: suite.name.startsWith('rim-') ? 100 : suite.xrReport ? 'raf' : 100 }, suite.status);
                result.text = await page.$eval(suite.status, node => node.textContent.trim());
            }
            if (suite.xrReport) {
                const report = await page.evaluate(() => window.xrTestReport || null);
                if (!report) throw new Error('XR scenario did not publish a structured report');
                const {trace, ...summary} = report;
                if (summary.profiles) summary.profiles = summary.profiles.map(({trace, ...profile}) => profile);
                result.xrReport = `${suite.name}-report.json`;
                result.xrTrace = `${suite.name}-trace.json`;
                await writeFile(path.join(output,result.xrReport),JSON.stringify(summary,null,2)+'\n');
                await writeFile(path.join(output,result.xrTrace),JSON.stringify(trace,null,2)+'\n');
                if (report.status !== 'PASS') throw new Error(report.error || 'XR checks failed');
            }
            if (!result.text.startsWith('PASS:')) throw new Error(result.text);
            result.actualBackend = await page.evaluate(() => document.documentElement.dataset.rendererBackend || document.querySelector('[role="status"]')?.textContent?.match(/BrowserWebGpu|\bGl\b/)?.[0]);
            const expected = suite.name === 'rim-three-stress' ? 'ThreeWebGL2' : backend === 'webgpu' ? 'BrowserWebGpu' : 'Gl';
            if (result.actualBackend !== expected) throw new Error(`Requested ${backend}, observed ${result.actualBackend || 'unknown'}; fallback is not a pass`);
            if (errors.length) throw new Error(errors.join('\n'));
            if (suite.name === 'rim-gameplay' || suite.name === 'rim-gameplay-120') result.rimCutEvidence = await page.evaluate(() => ({halfVisibility:window.rimCutEvidence.halfVisibility,transport:window.rimCutEvidence.transport,completionMs:window.rimCutEvidence.completionMs}));
            if (suite.name === 'rim-device-logs') result.rimDeviceLogEvidence = await page.evaluate(() => window.rimDeviceLogEvidence);
            if (['rim-stress-xr','rim-spatial-batches-xr'].includes(suite.name)) result.rimStressEvidence = await page.evaluate(() => window.rimStressEvidence);
            if (suite.name === 'rim-three-stress') {
                result.threeStressEvidence = await page.evaluate(() => window.threeStressVerification);
                await page.select('#stress-count','32');
                await page.select('#stress-representation','objects');
                await page.waitForFunction(()=>cityDebug.stress.count===32&&cityDebug.stress.representation==='objects');
                await page.evaluate(()=>{const u=new URL(location.href);u.searchParams.delete('verify');history.replaceState(null,'',u);});
                await page.reload({waitUntil:'networkidle0'});
                await page.waitForFunction(()=>window.cityDebug?.stress?.count===32&&cityDebug.stress.representation==='objects');
                await page.setViewport({width:390,height:740,deviceScaleFactor:1});
                await page.evaluate(()=>{for(const id of ['vr','stress-count','stress-representation']){const n=document.getElementById(id),r=n.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);if(r.x<0||r.y<0||r.right>innerWidth||r.bottom>innerHeight||!(hit===n||n.contains(hit)))throw Error('Three mobile control covered/offscreen: '+id);}});
                await page.select('#stress-count','0');
                await page.waitForFunction(()=>cityDebug.stress.count===0&&cityDebug.stress.group.children.length===0);
                await page.setViewport({width:1280,height:720,deviceScaleFactor:1});
                await page.click('#walk');
                await page.waitForFunction(()=>document.body.classList.contains('walking'));
                const vrChecks=[];
                for(const viewport of [{width:1280,height:720,deviceScaleFactor:1},{width:960,height:540,deviceScaleFactor:1},{width:390,height:740,deviceScaleFactor:1}]){
                    await page.setViewport(viewport);
                    vrChecks.push(await page.evaluate(()=>{const n=document.getElementById('vr'),r=n.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);if(r.width<44||r.height<44||r.x<0||r.y<0||r.right>innerWidth||r.bottom>innerHeight||!(hit===n||n.contains(hit)))throw Error('Three VR entry hidden/covered after Walk: '+JSON.stringify({width:innerWidth,height:innerHeight,rect:{x:r.x,y:r.y,w:r.width,h:r.height},hit:hit?.id}));return {width:innerWidth,height:innerHeight,walking:true,visible:true,hit:true};}));
                }
                await page.evaluate(()=>{
                    window.threeVrRequestProbe=0;
                    Object.defineProperty(navigator,'xr',{configurable:true,value:{requestSession:async()=>{window.threeVrRequestProbe++;throw new Error('Controlled VR request rejection');}}});
                    document.getElementById('vr').disabled=false;
                });
                await page.click('#vr');
                await page.waitForFunction(()=>window.threeVrRequestProbe===1&&document.getElementById('status').textContent.includes('Controlled VR request rejection'));
                result.threeStressControls={refresh:true,mobileHitTargets:true,removal:true,walkingVrEntry:vrChecks,physicalClickReachedRequestSession:true,requestScope:'controlled rejected request only; not an immersive XR session'};
            }
            if (suite.name === 'shadow-cache') result.shadowCacheEvidence = await page.evaluate(() => window.shadowCacheEvidence);
            if (suite.name === 'gltf-material-cache') result.materialCacheEvidence = await page.evaluate(() => window.materialCacheEvidence);
            if (suite.name === 'gltf-light-classes') result.lightClassEvidence = await page.evaluate(() => window.lightClassEvidence);
            if (suite.name === 'postprocess-fusion') {
                result.pixelEvidence = await page.evaluate(() => window.fusionEvidence.map(({png,...item})=>item));
                if (capture) for (const item of await page.evaluate(() => window.fusionEvidence)) {
                    await writeFile(path.join(output,`fusion-${backend}-${item.name}.png`),Buffer.from(item.png,'base64'));
                }
            }
            if (capture && suite.name === 'rim') {
                for (const stage of await page.evaluate(() => window.rimVisualEvidence??[])) {
                    await writeFile(path.join(output, `rim-${backend}-${stage.label}.png`), Buffer.from(stage.png.split(',')[1], 'base64'));
                }
            }
            if (capture && suite.xrReport) {
                result.xrScreenshot = `${suite.name}-report.png`;
                await page.screenshot({path:path.join(output,result.xrScreenshot),fullPage:true});
            }
            if (capture && suite.name === 'webxr-simulation') {
                result.simulationScreenshot = 'webxr-simulation-stereo.png';
                await page.screenshot({path:path.join(output,result.simulationScreenshot),fullPage:true});
                for (const phase of ['before','after','score','physics','jam','slice-start','slice-open','recut']) {
                    const pixels = await page.$eval(`#${phase}`, canvas => canvas.toDataURL('image/png').split(',')[1]);
                    await writeFile(path.join(output,`webxr-simulation-${phase}.png`),Buffer.from(pixels,'base64'));
                }
            }
            if (capture) {
                result.screenshot = `${suite.name}-${backend}.png`;
                const png = await page.evaluate(() => new Promise((resolve, reject) => requestAnimationFrame(() => {
                    try {
                        const canvas = document.querySelector('#gpu-canvas, #world');
                        const copy = document.createElement('canvas');
                        copy.width = canvas.width; copy.height = canvas.height;
                        copy.getContext('2d').drawImage(canvas, 0, 0);
                        resolve(copy.toDataURL('image/png').split(',')[1]);
                    } catch (error) { reject(error); }
                })));
                await writeFile(path.join(output, result.screenshot), Buffer.from(png, 'base64'));
                const fontFrame = await page.evaluate(() => {
                    const evidence=document.querySelector('#font-evidence');
                    return evidence instanceof HTMLCanvasElement ? evidence.toDataURL('image/png').split(',')[1] : null;
                });
                if(fontFrame){
                    result.fontScreenshot=`${suite.name}-${backend}-font.png`;
                    await writeFile(path.join(output,result.fontScreenshot),Buffer.from(fontFrame,'base64'));
                }
                const cutFrame = await page.evaluate(() => {
                    const evidence=document.querySelector('#cut-evidence');
                    return evidence instanceof HTMLCanvasElement ? evidence.toDataURL('image/png').split(',')[1] : null;
                });
                if(cutFrame){
                    result.cutScreenshot=`${suite.name}-${backend}-cut.png`;
                    await writeFile(path.join(output,result.cutScreenshot),Buffer.from(cutFrame,'base64'));
                }
                const reflectionFrame = await page.evaluate(() => {
                    const evidence=document.querySelector('#reflection-evidence');
                    return evidence instanceof HTMLCanvasElement ? evidence.toDataURL('image/png').split(',')[1] : null;
                });
                if(reflectionFrame){
                    result.reflectionScreenshot=`${suite.name}-${backend}-reflection.png`;
                    await writeFile(path.join(output,result.reflectionScreenshot),Buffer.from(reflectionFrame,'base64'));
                }
            }
            result.status = 'PASS';
        } catch (error) {
            result.error = error.message;
            if(suite.status) result.statusText = await page.$eval(suite.status, node => node.textContent).catch(() => null);
            result.errors = errors;
            result.screenshot = `${suite.name}-${backend}-failure.png`;
            await page.screenshot({ path: path.join(output, result.screenshot) }).catch(() => {});
            process.exitCode = 1;
        } finally {
            result.abortedRequests = abortedRequests;
            report.results.push(result);
            await saveProgress();
            console.log(`${result.status} ${suite.name} ${backend}${result.error ? ': ' + result.error : ''}`);
            await page.close();
        }
    }
    await assertFreshBuild();
    if (report.application !== await applicationFingerprint()) throw new Error('SDK/tests/examples changed during verification; rerun after integration');
} catch (error) {
    report.error = error.message;
    console.error(`FAIL: ${error.message}`);
    process.exitCode ||= 1;
} finally {
    await cleanup();
    await mkdir(output, { recursive: true });
    report.finished = new Date().toISOString();
    await writeFile(path.join(output, 'browser-report.json'), JSON.stringify(report, null, 2) + '\n');
    console.log(`Report: ${path.join(output, 'browser-report.json')}`);
}
