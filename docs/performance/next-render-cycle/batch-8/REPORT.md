# Batch8 — WebGL XR presentation, local prototype

2026-10-29. Parent0711e024e48fc61b418831cd645aee92b27677ba, isolated `/private/tmp/rust-next-render-cycle-8`, branch `codex/next-render-cycle-8`. No push/main merge/deployment/shared8443 restart/foreign process termination. Stop before9.

## Decision

Deliver the bounded **opt-in** `shared-framebuffer` prototype and diagnostics. **Keep `canvas-transfer` as the default.** Shared-context rendering removes actual cross-context uploads and passes exact independent pixels, but is **not a generally faster replacement**: symmetric ordinary eyes consistently increase callback wall time. No physical Quest performance/comfort or real opaque-framebuffer acceptance is established. No new Three timing/ranking is made in this batch; the pinned r170 same-context XR implementation and differing authored AA/layer settings were source-audited in DESIGN.md.

Public interop exists in the pinned wgpu/hal/core/types30.0.1. No backend fork, private handle layout or raw JS framebuffer hack is needed. See [design/API audit](DESIGN.md). Rust owns two sRGB/depth eye stages; one backend-controlled final pass copies both eyes into the browser FBO. Actual layer/context AA must be false. Browser FBO is render-only, frame-local; no attachment inspection/change/deletion. WebGPU owned-eye staging/final stereo files remain unchanged.

## Clean paired results

Same frozen B8 release/SDK for both routes; same native scene, poses, effects and exact eye viewport sizes within each pair.8s warmup/25s sample,3AB/BA/AB pairs per workload,24runs/**32,856 complete callback records**. Engine profiler disabled, presentation diagnostics disabled, **actual createQuery calls0 at both boundaries for every clean run**. Every one of12full-stereo pairs is **RGBA-exact**, including nonzero/asymmetric viewport gaps. Workload names below denote explicit simulated viewports, not real Quest eye sizes.

Numbers are medians of the three run means; CPU is callback **wall duration**, including driver effects/simulator overhead, not isolated CPU compute.

| Workload | Original CPU ms | Prototype CPU ms | Original FPS | Prototype FPS |
|---|---:|---:|---:|---:|
| Rome ordinary, asymmetric320/304px eyes |4.838|3.636|60.0|60.0|
| Rome512 extra drones, asymmetric960x1008/944x992 |30.595|21.571|32.42|45.44|
| Neon authored scene/effects, asymmetric320/304 |4.782|3.738|60.0|60.0|
| Rome ordinary, symmetric320x320 both eyes |2.009|3.192|60.0|60.0|

Stress improves observed cadence **39.0–40.2% across individual pairs**, callback wall decreases28.9–29.6%. Its CPU p95 ranges47.4–50.4→34.6–37.7ms, p99 56.1–56.9→42.2–43.5ms, maxima58.3–66.8→45.9–52.8ms. Retain all runs/tails. Symmetric callback regression is58.8% using the ratio of median run means; per-pair ratios and tails are in [results](RESULTS.json). Capped60Hz cases do not prove throughput gains. This static presentation study does not establish active-game/VR performance.

Native glTF work per stereo frame is identical within every pair. Ordinary Rome:76glTF commands/134,296 triangles; stress344/21,107,544. Neon uses authored mesh primitives, so glTF counters alone are not whole-scene work. Separate actual GL API diagnostics below include presentation triangles and all geometry.

## What work was actually removed

Separate8s/25s diagnostic A/B pages, engine profiler still false, presentation queries/API counters enabled; excluded from clean timing. Six diagnostic runs/**7,959 callback records**, all three stereo pairs exact; full query/profile histories retained.

| Actual calls per stereo frame | Rome stress A→B | Symmetric Rome A→B | Neon A→B |
|---|---:|---:|---:|
| `texSubImage2D(canvas)` |2→0|2→0|2→0|
| `createTexture` |8→4|0→0|12→8|
| `createFramebuffer` |2→0|0→0|2→0|
| All submitted GL draw commands |350→348|82→80|28→26|
| All submitted triangles |21,107,550→21,107,548|134,302→134,300|10,324→10,322|

Two removed triangles/commands are presentation work, not missing scene geometry. Asymmetric A repeatedly resizes the renderer canvas; B retains independent eye targets. **Do not attribute all route-level improvement to cross-context copying.** B still recreates some shared postprocessing resources when eye dimensions differ; this batch does not claim to eliminate that separate churn.

Exclusive diagnostic CPU means(ms), labels exclude enclosing/full callback:

| Case/route | Left eye | Right eye | Left transfer | Right transfer | Left/right blit or stereo present |
|---|---:|---:|---:|---:|---:|
| Stress A |14.098|16.229|0.020|0.021|0.003/0.003|
| Stress B |5.742|4.621|—|—|10.793 stereo|
| Symmetric A |1.109|0.697|0.034|0.022|0.006/0.004|
| Symmetric B |1.011|0.621|—|—|1.127 stereo|
| Neon A |2.023|2.595|0.017|0.013|0.004/0.002|
| Neon B |1.016|0.569|—|—|1.462 stereo|

Driver work/waits can move into final submission; short CPU upload calls are not proof of cheap total GPU transfer. Do not sum these CPU scopes with GPU times.

## GPU interval evidence and its limits

EXT timer queries are asynchronous, bounded/reused, result read only after availability and rejected for disjoint/context-lost/nonfinite/nonpositive values. No `gl.finish` or timed `readPixels`. Phase samples and whole-stereo samples alternate to avoid nested queries. All sampled API-valid values/statuses, p95/p99/max and pending reasons are preserved in RESULTS/raw evidence; there is no substitution of unavailable times with zero.

Examples of returned **elapsed command intervals**, medians(ms): symmetric original left/right eye2.266/2.570, transfers0.809/0.870, blits0.862/0.064; prototype eyes1.238/1.755, final stereo0.812. Whole query4.299(original XR context only) versus3.609(shared context) is **not an equivalent GPU-work comparison**. Fallback uses two timing domains; its XR-context interval need not bracket all renderer-context work.

Stress intervals are especially unsuitable for GPU-busy attribution: original XR-context whole44.277ms, shared whole556.837ms despite clean45.44FPS cadence; eye phase medians263ms in shared route. They are API-valid returned elapsed intervals, retained as adverse/surprising evidence, **not claimed as shader execution time or a proven GPU cost reduction**. Scheduling/queueing/query perturbation is not isolated by this experiment. Complete cross-context stereo GPU time, GPU busy time and compositor time remain **unavailable**. No GPU bottleneck verdict follows from these values.

## Configuration, identity and environment

[Manifest](MANIFEST.json), [fixed protocol/calibration repairs](PROTOCOL.md), [commands](COMMANDS.md), [review](REVIEW.md), [verification](VERIFICATION.json), [raw archive](evidence.tar.gz). The19.1MB archive has297indexed files/250unique contents; every stored content/alias was re-read and hash-verified. It contains all run records/failures, actual primary/diagnostic served runtime/harness bytes, both release pkg snapshots and gate logs. Transient pilot harness versions with matching recorded hashes are retained where reconstructed; raw pilot manifests remain explicit.

- B7 frozen ancestry release: source`da7117cc33917296e0a73bd94007b3e73c7a882e744a8dc2e8b7c09b6a447715`, WASM`a5ada8fb48ad0fddbc457a7a251f2743d52e262ca988eec5ba672f2c3449ea22`.
- B8 measured release source`54c6cde834bd3ad2e5363aabece753cd77cf4aa4537d378147405f6a814797f1`, WASM`f14c4beb5f3e5b59f16cae9ae90ddccba88b152ed10f07f3ce999d5a765a12de`, bindings`d359800638f899ae7df46d50ab453a75b477044dec2d538d8ed39d91d24d44ba`.
- Each report retains actual served URL hashes plus separately hashed common harness, browser/GPU/toolchain and per-run power/thermal snapshots. Pilot harness fixes are explicit; no runtime/driver edits within the accepted primary/diagnostic series.
- Desktop Chrome156.0.8078.48 on Apple M1; simulator configured72Hz, actual delivered DOM cadence measured separately. XRWebGLLayer, resolutionScale1, actual AAfalse, fixedFoveation0, requested layer depthfalse/alphaopaque. Renderer targets rgba8unorm-srgb/depth32float/sample1. Ordinary asymmetric layer635x328 with left(0,8,320,320), right(331,0,304,304); stress left(0,8,960,1008), right(971,0,944,992), layer1915x1016. Symmetric layer640x320; both320x320. Simulator storage is ordinary RGBA8 plus unused depth16, not a real opaque headset FBO.
- Every primary run's before/after power snapshot reports Battery Power. External snapshots around the series also reported AC while discharging; no frequency/power pinning or continuous power trace. Preexisting browser/GPU processes preserved; background GPU load not isolated. Broad consistent observations are reported, not small causal claims or physical-device guarantees.

## Correctness and safe limits

Six exact independent original-route stereo states: opaque color/orientation/depth, blending/near clipping, shadows+FXAA+bloom, SSR, shared glTF assets and clear/reload. Opaque attachment guards reject forbidden operations. Desktop restoration, repeated entry/exit, private-context loss preserving renderer, shared-context loss requiring explicit disposal/reinitialization and fresh-engine reentry pass. Actual Rome tracked strikes, worker serialization/cutting/upload/atomic publication, visible opposing halves, evaporation and encounter persistence pass in the shared route. Final `VERIFY_WASM_PROFILE=release npm run verify:push` passed428native tests(2manual timings ignored),149browser scenarios,32SDK/71XR platform tests,13Node harness tests and2+11Python harness checks, strict WASM/type checks. Post-gate source/pkg identity and content-bound PASS receipt match. Existing WebGPU/default-route regressions are covered; source `xr_output.rs`/`xr_present.rs` unchanged.

Real opaque FBOs and real `makeXRCompatible()` context migration/loss are not reproduced by the simulator. Shared mode requires the renderer's current RGBA8-sRGB GLES device, a live single-sampled layer/context and one/two valid nonoverlapping eye rectangles within device limits. Layer has no external depth; internal depth/postprocessing preserved. Capability-probe failure reports fallback; runtime/import/AA failure safely ends session, leaving explicit retry with default available. Shared context loss needs engine reinit. Do not enable universally without physical acceptance and a workload/device policy supported by further data.

Initial failed UV flip, asset path, Node mock/TypeScript header issues and pilot classification/camera repairs remain in evidence. Independent scoped source review found no additional material defect. No default production optimization is accepted merely because the stress case improves. Batch9 is not started.
