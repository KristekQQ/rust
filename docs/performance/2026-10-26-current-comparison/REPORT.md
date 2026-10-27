# Current native versus Three FPS — 2026-10-26

The current native WebGPU reaches the observed 60Hz ceiling in this static Rome view. Native WebGL2 remains below Three at high resolution: **38.682 versus 60.002 FPS**, a **35.53% lower observed FPS**. No extra headroom can be ranked when implementations are capped at60.

| Renderer | 2880×1920 FPS, short control | 4096×2730 FPS, median of3 | High-resolution callback CPU median, ms |
|---|---:|---:|---:|
| Native Rust/WASM WebGPU | 60.0 | 60.0 | 0.7 |
| Native Rust/WASM WebGL2 | 60.0 | 38.7 | 1.2 |
| Original Three r170 WebGL2 | 60.0 | 60.0 | 3.2 |

## What was measured

Apple M1, Chromium/ANGLE Metal as recorded in each raw report, headless desktop, real native WASM renderer and original Three application. Same authored city, fixed gate camera, pixel resolution and time0 original character poses. Actual renderer/backend and profiler-disabled state were asserted. No source optimization, shader/quality change or live-package replacement occurred during measurement.

Current release WASM `6c4bc5dd9e4e98c0ad42ebbc52aec25034511fb7ba75068c07fcc414b597219e`, source `fd8bbe73edfdc4922ae41eb8c66870e0e6c7751d6b3666098b8df3ade7ef113f`, deployed app `de4fef8a1a`. The production package still matches current source and the same WASM. Benchmark runtime JS uses unminified versions of those source modules; production minifies them. All30 current original Three app files matched the frozen reference. Served vendor/runtime files and driver identities are hashed in the reports.

High-resolution GL-versus-Three:3 AB/BA/AB pairs,8s warmup/25s sample per run. Native WebGPU:3 separate8s/25s runs after the GL/Three series, not a three-way interleaving. Native GL individual FPS: 38.682, 38.736, 38.522. Native WebGPU: 60.003, 60.002, 60.002. Three: 60.002, 60.002, 60.003.

2880×1920 is one short3s/8s control per implementation. Every version reaches approximately60FPS; this is a cap observation, not a capacity/ordering result. FPS is1000 divided by mean RAF interval per run, then the median of those run values; it is not1000 divided by median frame interval.

Native GL submits39main draws and67,292 glTF triangles per application frame versus original Three41draws/67,326triangles. Its lower callback CPU (1.2ms versus3.2ms) does not locate a specific GPU/driver bottleneck. Native WebGPU callback CPU median is0.7ms. These scopes are not summed into frame time and GPU timings were intentionally unavailable with profiling off.

## Limits

- **Static scene diagnostic:** the view is not an active fight and does not exercise new moving NPC blobs, cuts or full combat. It provides no physical Quest/VR FPS evidence.
- **Equal graphics is unresolved:** original Three uses hardware MSAA4, native FXAA; PBR shading, shadow policies and runtime differ. Cross-engine image comparisons remain diagnostics rather than a passing pixel-equivalence gate.
- Latest blob changes did not change the retained a54 renderer kernels. The older separate Three diagnostic had36.081 nativeGL versus60.002FPS; the fresh38.682 result is **not an attributed optimization gain** against it. Session/environment variation and separate measurement series must not be conflated with a matched before/after experiment.
- Driver hashes stayed unchanged, server errors were empty, every browser/server cleanup completed normally.12 final samples passed across the six commands; calibration probes are separate. Live servers8443/8878 were preserved. No commit/push/CI was requested.

[Machine-readable summary](summary.json) · [GL/Three repeated results](evidence/gl-vs-three/report.json) · [WebGPU0](evidence/webgpu-0/report.json) · [WebGPU1](evidence/webgpu-1/report.json) · [WebGPU2](evidence/webgpu-2/report.json) · [normal GL/Three](evidence/normal-gl-three/report.json) · [normal WebGPU](evidence/normal-webgpu/report.json) · [current original-source identity](evidence/reference-current.json). Each evidence directory retains raw compressed frame records and served manifests. PNG captures remain in the original immutable local output directories. [Follow-up command sequence](followups.sh), [summary collector and validation](collect.mjs).
