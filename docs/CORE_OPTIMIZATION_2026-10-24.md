# Core renderer optimization — 2026-10-24

## Outcome and scope

Implemented and locally verified on `codex/core-render-optimization`, based on `fb986b4822e4889cdf4fd6dcb6b657cdf28b8182`. The Rome workload spends substantially less CPU time deforming and uploading glTF geometry. Rendering quality and gameplay were retained. This is a local development result, not a Quest performance result or a deployed release.

### Changes

- Immutable rigid vertices stay on the GPU; model and normal transforms run in the shader.
- GPU skinning supports up to 128 joints and eight nonzero influences per vertex, subject to device limits. Morph targets, larger rigs and unsupported devices retain the exact CPU path. No weights are silently discarded.
- Geometry, indices and material resources are shared by asset identity. `ModelObject.createInstance()` creates an independent instance backed by the same Rust asset. Rome now uses this for repeated enemies and the second sword.
- Cached conservative bounds replace per-vertex deformation for supported render-bound updates. Public exact geometry/bounds queries and cutting remain intact.
- Unchanged uniforms are not uploaded; consecutive redundant pipeline/material bindings are skipped. This does not introduce glTF draw instancing.
- Already-empty dynamic shadow layers are not repeatedly cleared. Sampling omits known-empty layers while preserving the same per-texel minimum, 3×3 PCF, bias and resolution. Static-only sampling uses nine comparisons instead of eighteen.
- Added upload/resource/draw counters, animation/bounds/update scopes, CPU p99/max statistics and a repeatable benchmark runner.

## Method

Apple M1; Chrome 155.0.8059.85; hardware ANGLE Metal, Apple driver 26.6.1. Headless browser, 960×640, DPR 1, two-second warmup and five-second sampling. RAF was capped near 60 Hz. CPU values measure summed application RAF callback work, not full display latency or GPU completion. Upload MB/s counts API buffer writes, not measured physical bus traffic.

Baseline WASM and changed JavaScript were frozen in `output/core-optimization/baseline/`. Comparisons retain scene content, resolution and effects. The rigid fixture has six helmets; skin has six Fox instances; morph exercises the CPU fallback; Rome samples its first wave. Baseline and repeat samples are pooled where available; final Rome is repeated. Morph WebGL2 was repeated after a noisy first result.

Final release WASM SHA-256: `4a922a45d95a205c7bf0866fb448e18b74b7d9c756bc06e6006cef096e86a51c`.
Source fingerprint: `f789fbbc2c156df1e32c9648363bd64de114248f283a7ae8d1b99a2a263b5fa6`.
Rust 1.99.0, release, `--cfg=web_sys_unstable_apis`.

## Before → after

| Backend / scene | CPU mean ms | CPU p95 ms | Upload MB/s |
|---|---:|---:|---:|
| WebGPU / six rigid models | 9.41 → 0.64 | 9.70 → 0.80 | 503.13 → 0.108 |
| WebGPU / six skinned models | 4.57 → 0.73 | 5.50 → 1.10 | 59.78 → 0.574 |
| WebGPU / morph fallback | 0.64 → 0.58 | 1.00 → 0.90 | 0.108 → 0.090 |
| **WebGPU / Rome** | **14.19 → 1.86** | **14.60 → 2.20** | **556.19 → 5.90** |
| WebGL2 / six rigid models | 7.26 → 1.03 | 7.60 → 1.40 | 503.13 → 0.108 |
| WebGL2 / six skinned models | 3.98 → 1.15 | 5.40 → 1.70 | 59.78 → 0.574 |
| WebGL2 / morph fallback | 0.89 → 0.76 | 1.30 → 1.30 | 0.108 → 0.090 |
| **WebGL2 / Rome** | **14.35 → 3.28** | **15.10 → 3.80** | **555.24 → 5.90** |

Rome CPU work fell approximately 87% on WebGPU and 77% on WebGL2. Upload volume fell approximately 99%. FPS remained near 60 before and after: this demonstrates CPU headroom, not a measured multiple of uncapped FPS. Morph timing ranges overlap; no reliable morph speedup is claimed.

| Requested Rome metric | Before | After / interpretation |
|---|---|---|
| Whole GPU frame | Not established | Not established; existing asynchronous profiler measures passes |
| WebGPU pass GPU times | Invalid Rome samples | Main 3.23 ms; FXAA 2.66 ms; do not sum potentially overlapping intervals |
| Shadow GPU time | Empty-pass query invalid | Steady-state generation omitted: static cached, dynamic empty; no invented zero-duration measurement |
| Draw calls | Sample mean ≈451; populated frame 463 | Sample mean ≈451; populated frame 463 |
| GPU-update CPU scope, WebGPU | 9.48 ms | 0.59 ms |
| GPU-update CPU scope, WebGL2 | 6.80 ms | 0.28 ms |
| glTF update CPU | Not separately isolated | WebGPU 0.54 ms; WebGL2 0.25 ms |
| glTF animation / bounds CPU | Not separately isolated | WebGPU 0.125 / 0.048 ms; WebGL2 0.076 / 0.031 ms |
| Simulation CPU, WebGPU / WebGL2 | 4.10 / 4.23 ms | 0.23 / 0.14 ms |
| FPS | ≈60, capped | ≈60, capped |

GPU-update includes mesh synchronization; nested scopes must not be summed. Sparse profiler histories retain old initialization samples for cached passes, so `render.encode.depth` is not interpreted as steady-state per-frame work. Raw Rome baseline query pairs had nonzero start timestamps and zero end timestamps for the empty depth pass. Its GPU duration cannot be recovered from this evidence.

Skinning moves work onto the GPU: the small skin fixture main pass measured approximately 0.285 → 0.305 ms while CPU time fell substantially. Rigid GPU timings varied between runs. Neither is evidence for a precise general GPU speedup. Profiling-disabled Rome controls measured CPU 1.73 ms WebGPU and 3.47 ms WebGL2, still around 60 FPS.

## Three.js control

A separate control used the available original Three.js Rome distribution with both renderers at 960×640/DPR 1, camera at the same starting position, first wave, shadows and AA disabled on both. Native FXAA was disabled only for this control. Authored lighting/tone mapping still differ; submitted draw counts differ. These controls used the optimized build before the final shadow sampling change, with shadows disabled.

| Renderer | CPU mean ms | CPU p95 ms | Draws | FPS |
|---|---:|---:|---:|---:|
| Three.js WebGL2 | 2.44 | 3.50 | 343 | 59.61 |
| Rust WebGL2 | 3.62 | 3.90 | 462 | 60.00 |
| Rust WebGPU | 1.57 | 2.10 | 462 | 60.00 |

Three.js still uses less CPU than our WebGL2 path in this control; our WebGPU path uses less CPU. Different submitted work prevents a universal renderer verdict. Investigating the draw-count difference is a sensible next task; its cause is not established here. This does not support leaving Rust/WASM, nor does it establish Quest performance.

## Correctness evidence

- Release WASM build and wasm32 Clippy with warnings denied passed; build fingerprint freshness confirmed.
- Native library tests: 344 passed, two ignored. Five affected layout tests passed after the final shadow uniform change.
- Type checking passed; SDK tests: 29 passed.
- Integrated browser coverage passed 26 cases across applicable WebGPU/WebGL2 paths, including Rome gameplay, cutting, lifecycle, glTF and simulated XR. Following the final shadow change, 15 affected cases passed again. Expanded eight-influence GPU and twelve-influence CPU fallback transform tests then passed on both backends.
- GPU and CPU reference transforms cover negative/nonuniform scales, normals/tangents and multiple poses. Observed image differences are only 1–2 channel levels. Fixed benchmark rigid/skin images differed by at most one channel level; morph was exact.
- Shadow-cache images before and after the final sampling change are bit-identical on both backends. Tests cover initial clearing, cached empty layers, a new caster, removal of the last caster and subsequent noncaster movement.
- Shared-resource lifecycle tests cover independent playback, removing the source, removing the final instance and stale handles.
- Independent implementation review and shadow follow-up review found no remaining material issues.

Browser and simulated XR evidence is not physical-headset acceptance. The final `npm run verify:push` publication gate subsequently passed, including the full native/WASM/SDK/browser suite. CI, production packaging and deployment have not been verified for this task. The primary checkout and servers were preserved.

## Remaining boundaries and evidence files

Morph deformation and unsupported rigs remain on CPU. No new global caster revision system, glTF draw instancing, render bundles or frame-graph fusion was introduced. Conservative render bounds can admit additional objects near frustum edges. Existing exact geometry remains available for cutting.

Raw reports and screenshots are under `output/core-optimization/`: `baseline`, `baseline-repeat`, `final`, `final-repeat`, `final-no-profile`, morph repeats, `three-control`, `webgpu-control`, `timestamps`, `pixels.json` and aggregate `summary.json`. These local generated artifacts are not versioned. The browser runner overwrites its latest report; coverage above records successive integrated checks, not one combined report.

Reproduce with the repository Node/browser setup and `scripts/benchmark-core.mjs <label> <pkg-directory>`. Environment options: `SCENES`, `BACKENDS`, `PROFILING=0`, `BROWSER_EXECUTABLE`, optional `THREE_ROOT`. Frozen baseline JavaScript is served from the sibling `source` directory when present. Do not benchmark concurrent builds or compare development and release packages.
