if (['smoke','calibrate','compare','capacity','soak','regression','--help'].includes(process.argv[2])) { await import('./render-performance.mjs'); process.exit(process.exitCode || 0); }
// Headless release benchmark; GPU times come only from the existing asynchronous profiler.
import fs from "node:fs/promises";
import path from "node:path";
import http from "node:http";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
const repo = process.cwd(), label = process.argv[2] || "baseline", pkg = path.resolve(process.argv[3] || "pkg"), out = path.join(repo, "output/core-optimization", label);
const localOption = (key) => {
  try {
    return execFileSync("git", ["config", "--local", "--get", key], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return void 0;
  }
};
const profiling = process.env.PROFILING !== "0";
const warmupMs = Number(process.env.WARMUP_MS || 2000), sampleMs = Number(process.env.SAMPLE_MS || 5000);
if (![warmupMs, sampleMs].every(v => Number.isFinite(v) && v >= 1000)) throw Error("Warmup/sample duration must be at least1000ms");
const threeRoot = process.env.THREE_ROOT ? path.resolve(process.env.THREE_ROOT) : null;
const sourceOverlay = path.join(path.dirname(pkg), "source");
const { default: puppeteer } = await import(path.join(repo, "node_modules/puppeteer/lib/puppeteer/puppeteer.js"));
const { browserArgs } = await import(path.join(repo, "scripts/browser-options.mjs"));
await fs.mkdir(out, { recursive: true });
const server = http.createServer(async (req, res) => {
  try {
    const name = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    if (name === "/favicon.ico") {
      res.writeHead(204);
      return res.end();
    }
    const isThree = name.startsWith("/three/") && threeRoot;
    const base = isThree ? threeRoot : name.startsWith("/pkg/") ? pkg : repo;
    const rel = isThree ? name.slice(6) : name.startsWith("/pkg/") ? name.slice(4) : name;
    let p = path.resolve(base, "." + rel + (name.endsWith("/") ? "index.html" : ""));
    if (!p.startsWith(base + "/")) throw Error("outside");
    if (name === "/core-benchmark.html") p = path.join(repo, "tests/core-benchmark.html");
    if (!name.startsWith("/pkg/")) {
      const overlay = path.resolve(sourceOverlay, "." + name);
      if (overlay.startsWith(sourceOverlay + "/")) try {
        if ((await fs.stat(overlay)).isFile()) p = overlay;
      } catch {
      }
    }
    res.setHeader("Content-Type", { ".js": "text/javascript", ".html": "text/html", ".wasm": "application/wasm", ".json": "application/json", ".gltf": "model/gltf+json" }[path.extname(p)] || "application/octet-stream");
    res.setHeader("Cache-Control", "no-store");
    res.end(await fs.readFile(p));
  } catch (e) {
    res.writeHead(404);
    res.end(String(e));
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const url = "http://127.0.0.1:" + server.address().port;
const browser = await puppeteer.launch({ headless: true, executablePath: process.env.BROWSER_EXECUTABLE || localOption("verification.browser"), args: browserArgs(), protocolTimeout: 12e4 });
const summary = (a) => {
  a = a.filter(Number.isFinite);
  if (!a.length) return null;
  const s = a.toSorted((a2, b) => a2 - b);
  return { mean: a.reduce((a2, b) => a2 + b, 0) / a.length, p95: s[Math.ceil(a.length * 0.95) - 1], p99: s[Math.ceil(a.length * 0.99) - 1], max: s.at(-1) };
};
let gpuDevice = null;
try {
  const session = await browser.target().createCDPSession();
  const info = await session.send("SystemInfo.getInfo");
  gpuDevice = info.gpu.devices.map(({vendorString,deviceString,driverVendor,driverVersion}) => ({vendorString,deviceString,driverVendor,driverVersion}));
  await session.detach();
} catch { /* Hardware detail is optional; backend acceptance remains mandatory. */ }
const report = { gpuDevice, sourceOverlay, profiling, warmupMs, sampleMs, threeRoot, comparison: !!threeRoot, label, date: (/* @__PURE__ */ new Date()).toISOString(), browser: await browser.version(), pkg, wasmHash: createHash("sha256").update(await fs.readFile(path.join(pkg, "webgpu_wasm_bg.wasm"))).digest("hex"), results: [] };
try {
  for (const backend of (process.env.BACKENDS || "webgpu,webgl").split(",")) for (const kind of (process.env.SCENES || "rigid,skin,morph,rim").split(",")) {
    console.log("RUN", label, backend, kind);
    const page = await browser.newPage();
    await page.setViewport({ width: 960, height: 640, deviceScaleFactor: 1 });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text());
    });
    await page.evaluateOnNewDocument(() => {
      const get = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function(kind2, options) {
        return get.call(this, kind2, kind2 === "webgl2" ? { ...options, antialias: false } : options);
      };
      const raf = requestAnimationFrame.bind(window);
      window.measure = { active: false, rows: [], before: null };
      window.requestAnimationFrame = (fn) => raf((t) => {
        if (!measure.active) return fn(t);
        let r = measure.rows.at(-1);
        if (!r || r.stamp !== t) {
          r = { stamp: t, cpu: 0 };
          measure.rows.push(r);
        }
        const begin = performance.now();
        try {
          return fn(t);
        } finally {
          r.cpu += performance.now() - begin;
        }
      });
    });
    await page.goto(url + (kind === "three" ? "/three/" : kind === "rim" ? "/examples/rim/?backend=" + backend + "&profile=" + Number(profiling) : "/core-benchmark.html?kind=" + kind + "&backend=" + backend + "&profile=" + Number(profiling)), { waitUntil: "networkidle0" });
    await page.waitForFunction(kind === "three" ? "!!window.cityDebug" : kind === "rim" ? "!!window.rimDemo" : "!!window.bench", { timeout: 12e4 });
    if (kind === "rim") await page.evaluate(() => {
      window.bench = { engine: rimDemo.engine, start() {
        rimDemo.play();
      }, stop() {
        rimDemo.engine.scene.rimInput({ pause: true });
        rimDemo.view("gate");
      } };
    });
    if (kind === "three") await page.evaluate(async () => {
      await cityDebug.combat.ready;
      cityDebug.renderer.shadowMap.enabled = false;
      cityDebug.renderer.setPixelRatio(1);
      cityDebug.renderer.setSize(960, 640);
      cityDebug.teleport(0, 42);
      window.bench = { engine: { renderer_backend: () => "ThreeWebGL2", renderer_stats: () => ({ drawCalls: cityDebug.renderer.info.render.calls, triangles: cityDebug.renderer.info.render.triangles, uploadBytes: null }), debugStats: () => ({ drawCalls: cityDebug.renderer.info.render.calls, triangles: cityDebug.renderer.info.render.triangles }), frameProfile: () => ({ enabled: false, cpu: {}, gpu: { available: false, supported: false } }) }, start() {
        cityDebug.setManualTime(false);
        cityDebug.startWalk();
      }, stop() {
        cityDebug.setManualTime(true);
      } };
    });
    if (threeRoot && kind === "rim") await page.evaluate(() => {
      rimDemo.engine.set_render_effects(false, false);
      rimDemo.engine.postprocessing.disable("fxaa");
    });
    if (kind !== "three") {
      const actualProfiling = await page.evaluate(() => bench.engine.frameProfile().enabled);
      if (actualProfiling !== profiling) throw Error("Requested/actual profiling mismatch: " + profiling + "/" + actualProfiling);
    }
    await page.evaluate(() => bench.start());
    await new Promise((r) => setTimeout(r, warmupMs));
    const data = await page.evaluate(async (measureMs) => {
      measure.rows = [];
      measure.active = true;
      const samples = [], gpu = [], start = performance.now();
      let lastGpu = -1;
      while (performance.now() - start < measureMs) {
        await new Promise((r) => setTimeout(r, 100));
        const e = bench.engine, p = e.frameProfile();
        samples.push({ stats: e.renderer_stats(), debug: e.debugStats(), cpu: p.cpu });
        if (p.gpu.available && p.gpu.sampleFrame !== lastGpu) {
          gpu.push(p.gpu);
          lastGpu = p.gpu.sampleFrame;
        }
      }
      measure.active = false;
      bench.stop();
      return { rows: measure.rows, samples, gpu, backend: bench.engine.renderer_backend(), profile: bench.engine.frameProfile() };
    }, sampleMs);
    await new Promise((r) => setTimeout(r, 150));
    const png = await page.evaluate(async () => {
      const { capturePixels } = await import("/tests/pixels.js");
      const im = await capturePixels(document.querySelector("canvas"));
      const c = document.createElement("canvas");
      c.width = im.width;
      c.height = im.height;
      c.getContext("2d").putImageData(im, 0, 0);
      return c.toDataURL().split(",")[1];
    });
    await fs.writeFile(path.join(out, backend + "-" + kind + ".png"), Buffer.from(png, "base64"));
    const intervals = data.rows.slice(1).map((r, i) => r.stamp - data.rows[i].stamp), cpu = data.rows.map((r) => r.cpu);
    // Optional expensive breakdown is collected after timing and pixel capture.
    const diagnostics = process.env.DRAW_DIAGNOSTICS === '1' && kind !== 'three' ? await page.evaluate(async () => {
      bench.engine.setDebugOverlay(true);
      await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
      const value=bench.engine.debugStats();bench.engine.setDebugOverlay(false);return value;
    }) : undefined;
    const result = { diagnostics, kind, requestedBackend: backend, ...data, errors, cpu: summary(cpu), interval: summary(intervals), fps: 1e3 / summary(intervals).mean };
    report.results.push(result);
    await fs.writeFile(path.join(out, "report.json"), JSON.stringify(report, null, 2));
    console.log(JSON.stringify({ kind, backend, cpu: result.cpu, fps: result.fps, uploads: summary(data.samples.map((s) => s.stats.uploadBytes)), gpu: data.gpu.at(-1), errors }));
    if (errors.length || data.backend !== (kind === "three" ? "ThreeWebGL2" : backend === "webgl" ? "Gl" : "BrowserWebGpu")) throw Error("Browser errors or backend fallback");
    await page.close();
  }
} finally {
  await browser.close();
  await new Promise((r) => server.close(r));
}
