#!/usr/bin/env node
// Desktop/simulated XR only. One frozen runtime, two presentation routes.
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import puppeteer from 'puppeteer';
import {PNG} from 'pngjs';
import {serveFrozen, stats, hash, pixelDifference, boundedCleanup} from './render-performance-lib.mjs';
import {browserArgs} from './browser-options.mjs';
import {installApiCounters,installRuntimeLossMonitor} from './render-performance-api.mjs';

const options = {frozenRoot: '', out: '', warmup: 8000, sample: 25000, pairs: 3, workload: 'rome', load: 0,
    eyeWidth: 320, eyeHeight: 320, asymmetric: true, diagnostics: false, routes: 'both'};
for (let i = 2; i < process.argv.length; i += 2) {
    const key = process.argv[i].replace(/^--/, ''), value = process.argv[i + 1];
    if (!process.argv[i].startsWith('--') || !(key in options) || value === undefined) throw Error('Expected supported --option value');
    if (['diagnostics','asymmetric'].includes(key)) {
        if (!['true', 'false'].includes(value)) throw Error('--diagnostics must be true or false');
        options[key] = value === 'true';
    } else options[key] = typeof options[key] === 'number' ? Number(value) : value;
}
if (!options.frozenRoot || !options.out) throw Error('Required: --frozenRoot PATH --out NEW_DIRECTORY');
if (!['baseline', 'both'].includes(options.routes)) throw Error('--routes must be baseline or both');
if (!['rome', 'neon'].includes(options.workload)) throw Error('--workload must be rome or neon');
for (const key of ['warmup', 'sample']) if (!Number.isFinite(options[key]) || options[key] < 100 || options[key] > 60000) throw Error('Duration bounds: 100..60000 ms');
for (const key of ['pairs', 'load', 'eyeWidth', 'eyeHeight']) if (!Number.isInteger(options[key])) throw Error('Integer required: ' + key);
if (options.pairs < 1 || options.pairs > 10 || options.load < 0 || options.load > 2048) throw Error('Pairs/load bounds exceeded');
if (options.workload === 'neon' && options.load !== 0) throw Error('Neon is a fixed scene: --load 0 required');
if (![options.eyeWidth, options.eyeHeight].every(n => n >= 64 && n <= 2048)) throw Error('Eye dimensions: 64..2048');
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
options.frozenRoot = await fs.realpath(options.frozenRoot);
options.out = path.resolve(options.out);
try { await fs.stat(options.out); throw Error('Refusing existing output directory'); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
await fs.mkdir(options.out, {recursive: true});
const json = (file, value) => fs.writeFile(path.join(options.out, file), JSON.stringify(value, null, 2) + '\n');
const power = () => {
    try { return {at: new Date().toISOString(), command: 'pmset -g batt; pmset -g therm', battery: execFileSync('pmset', ['-g', 'batt'], {encoding: 'utf8', timeout: 5000}),
        thermal: execFileSync('pmset', ['-g', 'therm'], {encoding: 'utf8', timeout: 5000})}; }
    catch (error) { return {at: new Date().toISOString(), unavailable: String(error)}; }
};
const driverPaths = ['scripts/benchmark-xr-presentation.mjs', 'scripts/render-performance-lib.mjs', 'scripts/browser-options.mjs',
    'scripts/render-performance-api.mjs', 'tests/xr/presentation-benchmark.html', 'tests/xr/presentation-benchmark.js',
    'tests/xr/simulator.js', 'tests/xr/poses.js'];
const identity = async () => Object.fromEntries(await Promise.all(driverPaths.map(async name => [name, hash(await fs.readFile(path.join(repo, name)))])));
const report = {version: 1, options, startedAt: new Date().toISOString(), status: 'RUNNING',
    environment: {platform: process.platform, arch: process.arch, release: os.release(), cpus: os.cpus().map(c => c.model), node: process.version},
    scope: 'desktop DOM-RAF-driven simulated XR; configured 72Hz is simulation time, not observed cadence; no physical headset/compositor claim',
    diagnosticMode: options.diagnostics ? 'separate opt-in presentation timer queries; engine profiling false' : 'clean presentation timing; no presentation or engine profiling',
    driverFiles: await identity(), runs: [], comparisons: [], errors: [], cleanup: []};
let server, browser;
const routes = options.routes === 'baseline' ? ['canvas-transfer'] : ['canvas-transfer', 'shared-framebuffer'];
function workDelta(start, end) {
    const result = {};
    for (const key of ['drawCalls', 'trianglesDrawn', 'instancesDrawn', 'instancedDrawCalls', 'primitiveTests', 'primitiveCulled']) {
        const a = start.debug?.gltf?.[key], b = end.debug?.gltf?.[key];
        result[key] = {start: a ?? null, end: b ?? null, delta: Number.isFinite(a) && Number.isFinite(b) ? b - a : null};
    }
    return {scope: 'native glTF counters; renderer_stats boundary snapshots also retained; not whole-renderer GPU work', gltf: result};
}
try {
    server = await serveFrozen(options.frozenRoot, repo);
    const executablePath = process.env.BROWSER_EXECUTABLE || execFileSync('git', ['config', '--local', '--get', 'verification.browser'], {cwd: repo, encoding: 'utf8'}).trim();
    browser = await puppeteer.launch({headless: true, executablePath, args: browserArgs(), protocolTimeout: 180000});
    const cdp = await browser.target().createCDPSession();
    report.environment.browser = await browser.version();
    const {gpu} = await cdp.send('SystemInfo.getInfo');
    // Deliberately discard auxAttributes, machineModel, commandLine and IDs/paths.
    const safeText = value => typeof value === 'string' ? value
        .replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, '[redacted-id]')
        .replace(/(?:[A-Z]:\\|\/)[^\s,;]+/gi, '[redacted-path]').slice(0, 256) : null;
    report.environment.gpu = {devices: gpu.devices.map(device => ({vendorId: device.vendorId,
        deviceId: device.deviceId, vendor: safeText(device.vendorString), device: safeText(device.deviceString),
        driverVendor: safeText(device.driverVendor), driverVersion: safeText(device.driverVersion)})),
        features: Object.fromEntries(Object.entries(gpu.featureStatus ?? {}).map(([key, value]) => [key, safeText(value)]))};
    await cdp.detach();
    for (let pair = 0; pair < options.pairs; pair++) {
        const order = pair % 2 ? [...routes].reverse() : routes;
        const pairImages = new Map();
        for (const route of order) {
            const id = `pair-${pair + 1}-${route}`, record = {id, pair: pair + 1, route, status: 'RUNNING', powerBefore: power(), errors: [], consoleErrors: [], requestFailures: [], abortedRequests: []};
            report.runs.push(record);
            await json(id + '.json', record);
            await json('report.json', report);
            const page = await browser.newPage();
            try {
                await page.setViewport({width: options.eyeWidth * 2 + 20, height: options.eyeHeight + 20, deviceScaleFactor: 1});
                await page.evaluateOnNewDocument(installRuntimeLossMonitor);
                if(options.diagnostics)await page.evaluateOnNewDocument(installApiCounters);
                page.on('pageerror', error => record.errors.push(String(error.stack || error)));
                page.on('console', message => { if (message.type() === 'error') record.consoleErrors.push(message.text()); });
                page.on('requestfailed', request => {
                    const entry = {url: request.url(), error: request.failure()};
                    if (entry.error?.errorText === 'net::ERR_ABORTED') record.abortedRequests.push(entry);
                    else record.requestFailures.push(entry);
                });
                const query = new URLSearchParams(Object.entries({...options, route}).filter(([key]) => !['frozenRoot', 'out'].includes(key)).map(([key, value]) => [key, String(value)]));
                await page.goto(`${server.url}/tests/xr/presentation-benchmark.html?${query}`, {waitUntil: 'networkidle0', timeout: 120000});
                await page.waitForFunction(() => Boolean(window.xrPresentationBenchmark), {timeout: 120000});
                await page.evaluate(ms => window.xrPresentationBenchmark.run(ms), options.warmup);
                record.metadata = await page.evaluate(() => ({...window.xrPresentationBenchmark.metadata, runtimeSettings: window.xrPresentationBenchmark.runtimeSettings}));
                record.boundaryStart = await page.evaluate(() => window.xrPresentationBenchmark.boundary());
                record.sample = await page.evaluate(ms => window.xrPresentationBenchmark.run(ms, true), options.sample);
                record.boundaryEnd = await page.evaluate(() => window.xrPresentationBenchmark.boundary());
                // Resolve delayed queries after the timed interval; filter by its exact frame indices.
                if (options.diagnostics) await page.evaluate(() => window.xrPresentationBenchmark.run(100));
                record.presentationProfile = await page.evaluate(({start, end}) => window.xrPresentationBenchmark.profile(start, end),
                    {start: record.boundaryStart.profileFrame, end: record.boundaryEnd.profileFrame});
                const capture = await page.evaluate(() => window.xrPresentationBenchmark.capture());
                const png = Buffer.from(capture.png.split(',')[1], 'base64');
                await fs.writeFile(path.join(options.out, id + '.png'), png);
                record.capture = {file: id + '.png', sha256: hash(png), width: capture.width, height: capture.height, foregroundPixels: capture.foregroundPixels, eyeEvidence: capture.eyeEvidence,
                    timing: 'exact simulator stereo framebuffer PNG; readPixels after sample and outside RAF callback'};
                pairImages.set(route, PNG.sync.read(png));
                record.cleanProof = await page.evaluate(() => window.xrPresentationBenchmark.cleanProof());
                record.losses = await page.evaluate(() => window.renderRuntimeLosses ?? []);
                await page.evaluate(() => window.xrPresentationBenchmark.check());
                const frames = record.sample.frames, intervals = frames.slice(1).map((f, i) => f.stamp - frames[i].stamp);
                if (frames.length < 2) throw Error('Too few sample callbacks');
                if (!record.metadata.runtimeSettings || record.metadata.runtimeSettings.presentation !== route) throw Error('Missing/mismatched actual runtime settings');
                if (record.errors.length || record.consoleErrors.length || record.requestFailures.length || record.losses.length) throw Error('Runtime error or resource failure');
                record.summary = {callbackCpuMs: stats(frames.map(f => f.cpuMs)), desktopRafIntervalMs: stats(intervals),
                    observedDesktopHz: 1000 / stats(intervals).mean, configuredSimHz: 72,
                    work: workDelta(record.boundaryStart, record.boundaryEnd), profileComplete: record.presentationProfile?.sampleBoundary.complete ?? null};
                record.status = 'PASS';
                await page.evaluate(() => window.xrPresentationBenchmark.dispose());
            } catch (error) { record.status = 'FAIL'; record.failure = String(error.stack || error); throw error; }
            finally {
                record.powerAfter = power();
                record.manifest = server.manifest();
                record.serverErrors = server.errors();
                record.cleanup = await boundedCleanup(() => page.close(), {label: id + ' page', timeoutMs: 5000});
                if (!record.cleanup.complete || record.cleanup.forced) {
                    record.status = 'FAIL'; record.failure = 'Page cleanup failed or timed out';
                }
                await json(id + '.json', record);
                await json('report.json', report);
            }
            if (record.status !== 'PASS') throw Error(record.failure);
            console.log(JSON.stringify({id, status: record.status, ...record.summary}));
        }
        if (routes.length === 1) continue; // Baseline-only calibration never visits the candidate.
        const comparison = pixelDifference(pairImages.get(routes[0]), pairImages.get(routes[1]), {channelTolerance: 0, pixelFractionTolerance: 0});
        report.comparisons.push({pair: pair + 1, ...comparison});
        await json('report.json', report);
        if (!comparison.exact) throw Error(`Pair ${pair + 1}: stereo pixels differ between presentation routes`);
    }
    if (server.errors().length) throw Error('Frozen server reported missing/changed files');
    report.driverFilesAfter = await identity();
    if (JSON.stringify(report.driverFiles) !== JSON.stringify(report.driverFilesAfter)) throw Error('Driver/harness changed during benchmark');
    report.status = 'PASS';
} catch (error) {
    report.status = 'FAIL'; report.errors.push(String(error.stack || error)); process.exitCode = 1;
} finally {
    report.manifest = server?.manifest() ?? [];
    report.manifestSha256 = hash(JSON.stringify(report.manifest));
    report.serverErrors = server?.errors() ?? [];
    if (browser) report.cleanup.push(await boundedCleanup(() => browser.close(), {label: 'browser', force: () => browser.process()?.kill('SIGKILL')}));
    if (server) report.cleanup.push(await boundedCleanup(() => server.close(), {label: 'frozen server', force: () => server.forceClose()}));
    const failedCleanup = report.cleanup.filter(result => !result.complete || result.forced);
    if (failedCleanup.length) {
        report.status = 'FAIL'; process.exitCode = 1;
        report.errors.push('Owned browser/server cleanup failed or timed out');
        for (const run of report.runs) if (run.status === 'PASS') {
            run.status = 'FAIL'; run.failure = 'Owned browser/server cleanup failed or timed out';
            await json(run.id + '.json', run);
        }
    }
    report.finishedAt = new Date().toISOString();
    await json('report.json', report);
    console.log(JSON.stringify({status: report.status, output: options.out, runs: report.runs.length, errors: report.errors, cleanup: report.cleanup}));
}
