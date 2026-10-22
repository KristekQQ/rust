import puppeteer from 'puppeteer';
import { verifyCutting } from '../tests/cutting-browser.mjs';
import { verifyProjectiles } from '../tests/projectiles-browser.mjs';
import { browserArgs } from './browser-options.mjs';
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
    { name: 'monkey', path: 'examples/monkey/?verify=1', status: '#result', button: '#verify' },
    { name: 'arena-minute', path: 'tests/arena-minute.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'arena-presets', path: 'tests/arena-presets.html', status: '#result', backends:['webgl'] },
    { name: 'arena-waves', path: 'tests/arena-waves.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-arena-benchmark', path: 'tests/neon-arena-benchmark.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-font-preview', path: 'examples/neon-font-preview/', status: '#result', button: '#verify' },
    { name: 'frame-graph', path: 'tests/frame-graph.html', status: '#result', button: '#check', backends: ['webgpu'] },
    { name: 'neon-arena-gpu-profile', path: 'tests/webxr-webgpu.html?profile=1', status: '#result', backends:['webgpu'], xrReport:true },
    { name: 'neon-arena-profile', path: 'tests/neon-arena-profile.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-arena-xr', path: 'tests/neon-arena-xr.html', status: '#result', backends:['webgl'], xrReport:true },
    { name: 'neon-arena', path: 'examples/neon-arena/', status: '#result', button: '#verify' },
    { name: 'webxr-contract', path: 'tests/webxr-contract.html', status: '#result', backends: ['webgl'], xrReport: true },
    { name: 'webxr-simulation', path: 'tests/webxr-simulation.html', status: '#result', backends: ['webgl'], xrReport: true },
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
let backends = ['webgpu', 'webgl'], selected = suites, software = false, headed = false, capture = false, timeout = 90000;
for (let i = 2; i < process.argv.length; i++) {
    const arg = process.argv[i];
    if (arg === '--production-root') productionRoot = path.resolve(process.argv[++i] || 'output/production');
    else if (arg === '--arena-legacy-stress') arenaLegacyStress = true;
    else if (arg === '--xr-slow') xrSlow = true;
    else if (arg === '--software') software = true;
    else if (arg === '--capture') capture = true;
    else if (arg === '--headed') headed = true;
    else if (arg === '--backend') {
        const value = process.argv[++i];
        if (!['all', 'webgpu', 'webgl'].includes(value)) throw new Error('--backend requires all, webgpu or webgl');
        backends = value === 'all' ? ['webgpu', 'webgl'] : [value];
    } else if (arg === '--page') {
        const names = (process.argv[++i] || '').split(',');
        if (names.some(name => !suites.some(suite => suite.name === name))) throw new Error(`Unknown suite. Choose: ${suites.map(suite => suite.name).join(', ')}`);
        selected = suites.filter(suite => names.includes(suite.name));
    } else if (arg === '--timeout') {
        timeout = Number(process.argv[++i]);
        if (!Number.isFinite(timeout) || timeout < 1000) throw new Error('--timeout must be at least 1000 ms');
    } else if (arg === '--help') {
        console.log('node scripts/test-browser.mjs [--backend all|webgpu|webgl] [--page suite,suite] [--software] [--headed] [--capture] [--xr-slow] [--arena-legacy-stress] [--timeout ms]');
        process.exit(0);
    } else throw new Error(`Unknown option ${arg}`);
}

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
    await delay(800);
    const second = await stats();
    if (second.hudFrames <= first.hudFrames || second.hudRefreshes - first.hudRefreshes > 4
        || second.hudRefreshes >= second.hudFrames) throw new Error('HUD text must be cached between refreshes');
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
async function verifyFox(page) {
    await page.waitForFunction(() => document.documentElement.dataset.rendererBackend);
    await verifyHud(page);
    const read = () => page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        const info = fox.info;
        return { ...engine.characterState(), clip: info.animations[info.playback?.clip]?.name };
    });
    const assert = (condition, message) => { if (!condition) throw new Error(message); };
    const frameHash = () => page.evaluate(async () => {
        const { capturePixels, assertForeground } = await import('../../tests/pixels.js');
        const image = await capturePixels(document.querySelector('#gpu-canvas'));
        assertForeground(image, 'Fox example');
        let hash = 2166136261;
        for (const value of image.data) hash = Math.imul(hash ^ value, 16777619);
        return hash;
    });
    const distance = (a, b) => Math.hypot(b.position[0] - a.position[0], b.position[2] - a.position[2]);
    const canvas = await page.$('#gpu-canvas');
    const waitGameState = async (playing, label) => {
        try {
            await page.waitForFunction(active => active
                ? document.pointerLockElement === document.querySelector('#gpu-canvas') && document.fullscreenElement === document.querySelector('#gpu-canvas')
                : !document.pointerLockElement && !document.fullscreenElement,
            { timeout: 10000 }, playing);
        } catch {
            const state = await page.evaluate(() => ({
                lock: document.pointerLockElement?.id, fullscreen: document.fullscreenElement?.id,
                focused: document.hasFocus(), status: document.querySelector('#status')?.textContent,
            }));
            throw new Error(`${label}: ${JSON.stringify(state)}`);
        }
    };
    const enterGame = async () => {
        const bounds = await canvas.boundingBox();
        await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
        await waitGameState(true, 'Click enters fullscreen and pointer lock');
        await delay(120);
    };
    await enterGame();
    const box = await canvas.boundingBox();
    const spawn = await read();
    assert(spawn.clip === 'Survey' && !spawn.moving && spawn.grounded, 'Fox starts idle on solid ground');
    await page.keyboard.down('w');
    await delay(750);
    const blocked = await read();
    assert(blocked.position[2] >= 1.1 && blocked.position[2] <= 1.201, 'Solid platform blocks forward movement at its front face');
    await delay(250);
    assert(distance(blocked, await read()) < 0.01, 'Holding W cannot walk through a platform');
    await page.keyboard.down('Space');
    await delay(220);
    const airborne = await read();
    assert(airborne.position[1] > 0.5 && !airborne.grounded && airborne.verticalVelocity > 0, 'Space produces an airborne upward jump');
    await delay(800);
    const landed = await read();
    assert(Math.abs(landed.position[1] - 0.6) < 0.01 && landed.grounded, 'Fox lands on first platform instead of passing through it');
    await delay(250);
    assert(Math.abs((await read()).position[1] - 0.6) < 0.01, 'Holding Space does not auto-jump on landing');
    await page.evaluate(async () => (await import('./main.js')).engine.set_simulation_paused(true));
    await delay(120);
    await page.evaluate(async () => (await import('./main.js')).engine.set_simulation_paused(false));
    await page.keyboard.down('Space'); // Trusted repeat=true: this key is still held.
    await delay(220);
    const resumed = await read();
    assert(Math.abs(resumed.position[1] - 0.6) < 0.01 && resumed.grounded, 'Space held across pause cannot become a fresh jump');
    await page.keyboard.up('Space');
    await page.keyboard.down('w');
    await page.keyboard.down('d');
    await delay(250);
    const sliding = await read();
    assert(sliding.position[0] - landed.position[0] > 0.2 && sliding.position[2] <= 3.901, 'Diagonal movement slides along the next blocked platform');
    await page.keyboard.up('d');
    await page.keyboard.press('Space');
    await delay(1100);
    const higher = await read();
    assert(Math.abs(higher.position[1] - 1.2) < 0.01 && higher.grounded, 'A fresh jump reaches the higher second platform');
    await page.keyboard.up('w');
    // Reset through the same keyboard command as the player, then use an open lane.
    await page.keyboard.press('r');
    await delay(120);
    const reset = await read();
    assert(distance(reset, spawn) < 0.01 && Math.abs(reset.position[1]) < 0.01, 'R resets player and physics to the spawn');
    await page.keyboard.down('a');
    await delay(1200);
    await page.keyboard.up('a');
    await delay(120);
    const initial = await read();
    const initialFrame = await frameHash();
    await page.keyboard.down('w');
    await delay(500);
    const walking = await read();
    await page.keyboard.up('w');
    await delay(120);
    assert(distance(initial, walking) > 0.3 && walking.clip === 'Walk' && walking.moving, 'W moves real model and selects Walk');
    const stopped = await read();
    await delay(180);
    const idle = await read();
    assert(distance(stopped, idle) < 0.01 && idle.clip === 'Survey', 'Key release stops motion and selects idle');
    await page.keyboard.down('Shift');
    await page.keyboard.down('w');
    await delay(500);
    const running = await read();
    await page.keyboard.up('w');
    await page.keyboard.up('Shift');
    await delay(120);
    assert(running.running && running.clip === 'Run' && distance(idle, running) > distance(initial, walking) * 1.3, 'Shift runs faster and selects Run');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.move(box.x + box.width / 2 + 160, box.y + box.height / 2, { steps: 8 });
    const turned = await read();
    await page.keyboard.down('w');
    await delay(450);
    const afterTurn = await read();
    assert(afterTurn.position[0] - turned.position[0] > 0.1, 'Mouse moving right turns right without holding any button');
    assert(await frameHash() !== initialFrame, 'Real canvas pixels change after movement and mouse steering');
    await page.evaluate(() => window.dispatchEvent(new Event('blur')));
    await waitGameState(false, 'Blur releases fullscreen and pointer lock');
    await delay(120);
    const blurred = await read();
    await delay(180);
    assert(distance(blurred, await read()) < 0.01, 'Blur releases held movement keys');
    await page.keyboard.up('w');
    await enterGame();
    await page.keyboard.down('w');
    await delay(160);
    await page.evaluate(async () => {
        const { engine } = await import('./main.js');
        engine.set_camera_mode('free');
    });
    await delay(120);
    const released = await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        const info = fox.info;
        return { controller: engine.characterState(), clip: info.animations[info.playback?.clip]?.name };
    });
    assert(released.controller === null && released.clip === 'Survey', 'Changing camera releases control and the Walk animation');
    await page.keyboard.up('w');
    // Leave the example in its intended chase-camera mode for capture.
    await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        engine.controlCharacter(fox, { animations: { idle: 'Survey', walk: 'Walk', run: 'Run' } });
    });
    await page.keyboard.down('w');
    await delay(160);
    await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        engine.set_camera_mode('free');
        fox.playAnimation('Walk', { speed: 0.5 });
    });
    await delay(120);
    const manual = await page.evaluate(async () => {
        const { fox } = await import('./main.js');
        const info = fox.info;
        return { clip: info.animations[info.playback?.clip]?.name, speed: info.playback?.speed };
    });
    assert(manual.clip === 'Walk' && manual.speed === 0.5, 'Detached controller preserves an explicit same-clip animation command');
    await page.keyboard.up('w');
    await page.evaluate(async () => {
        const { engine, fox } = await import('./main.js');
        engine.controlCharacter(fox, { animations: { idle: 'Survey', walk: 'Walk', run: 'Run' } });
    });
    await delay(120);
    await page.keyboard.press('Escape');
    await waitGameState(false, 'Escape releases fullscreen and pointer lock');
    const escaped = await read();
    await page.keyboard.down('w');
    await delay(180);
    assert(distance(escaped, await read()) < 0.01, 'Escape pauses movement while cursor is released');
    await page.keyboard.up('w');
    await delay(600);
    await enterGame();
    await page.keyboard.press('r');
    await delay(120);
    return 'PASS: solid obstacles, wall sliding, gravity, jump/landing on two platforms, R reset, fullscreen/mouse/Escape, clips and lifecycle';
}
let interrupted = false;
const report = { started: new Date().toISOString(), revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), software, node: process.execPath, architecture: process.arch, results: [] };
const output = path.join(root, 'output/verification');
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
    browser = await puppeteer.launch({ headless: !headed, args, executablePath: report.browserExecutable, timeout: 30000, protocolTimeout: timeout, dumpio: process.env.DEBUG_BROWSER_STARTUP === '1' });
    report.browser = await browser.version();
    for (const backend of backends) for (const suite of selected) {
        if (suite.backends && !suite.backends.includes(backend)) continue;
        if (interrupted) throw new Error('Verification interrupted');
        const page = await browser.newPage();
        await page.setViewport({ width: 960, height: 640, deviceScaleFactor: 1 });
        page.setDefaultTimeout(timeout);
        page.setDefaultNavigationTimeout(timeout);
        const errors = [], abortedRequests = [];
        page.on('pageerror', error => errors.push(`JS: ${error.message || String(error)}`));
        page.on('console', message => { if (message.type() === 'error') errors.push(`console: ${message.text()}`); });
        page.on('response', response => { if (response.status() >= 400 && !response.url().endsWith('/favicon.ico')) errors.push(`HTTP ${response.status()}: ${response.url()}`); });
        page.on('requestfailed', request => {
            const message = `request: ${request.url()} ${request.failure()?.errorText}`;
            // SDK scenarios intentionally abort loads; failures still surface through
            // awaited model imports/page assertions. Record cancellation separately.
            if (request.failure()?.errorText === 'net::ERR_ABORTED') abortedRequests.push(message);
            else errors.push(message);
        });
        const result = { suite: suite.name, backend, status: 'FAIL' };
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
                if (!failure) await page.click(suite.button);
            }
            if (suite.interaction === 'webxr-preview') {
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
                    await new Promise(resolve => setTimeout(resolve, 250));
                    if (apple.meshInfo.position[1] <= y + 0.3) throw new Error('Rust projectile did not rise');
                    assertForeground(await capturePixels(document.querySelector('#gpu-canvas')), 'flying apple preview');
                    // This example deliberately requires WebGL, even when desktop WebGPU exists.
                    document.documentElement.dataset.rendererBackend = engine.renderer_backend();
                    return 'PASS: vlastní WebXR ukázka, skutečné pixely náhledu a UI bez headsetu';
                });
            }
            else if (suite.interaction === 'cutting') result.text = await verifyCutting(page);
            else if (suite.interaction === 'projectiles') result.text = await verifyProjectiles(page);
            else if (suite.interaction === 'fox') result.text = await verifyFox(page);
            else if (suite.interaction === 'culling') result.text = await verifyCulling(page);
            else {
                await page.waitForFunction(selector => /^(PASS|FAIL):/.test(document.querySelector(selector)?.textContent?.trim() || ''), { polling: suite.xrReport ? 'raf' : 100 }, suite.status);
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
            const expected = backend === 'webgpu' ? 'BrowserWebGpu' : 'Gl';
            if (result.actualBackend !== expected) throw new Error(`Requested ${backend}, observed ${result.actualBackend || 'unknown'}; fallback is not a pass`);
            if (errors.length) throw new Error(errors.join('\n'));
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
                        const canvas = document.querySelector('#gpu-canvas');
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
