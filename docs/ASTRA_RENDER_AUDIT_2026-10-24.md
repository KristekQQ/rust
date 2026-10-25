# Astra consultation: render preparation audit

Scope: read-only source/design/acceptance consultation with GPT-6 Astra Medium, explicitly requested by the user; implementation and serialized builds remain with the parent task. Checkout `rim-publish/rust`, branch `codex/core-render-optimization`, base `088fdb34` plus preserved owned follow-ups. This is a scoped audit, not proof of the absence of all engine defects.

## Concrete criticism and disposition

| Finding or evidence limit | Action | State |
|---|---|---|
| Previous instancing reduced commands but `prepare_draws` still allocated nested eligible/group lists, a tree per asset run and temporary payloads every frame. | Reuse seven flat index/value vectors, group ranges and payload capacity; retain GPU resource ownership and batching semantics. | Implemented; measured separately below. |
| A straightforward ordinal loop could add work for almost entirely culled models. | Jump to the next eligible ordinal instead of scanning every absent ordinal; add a sparse-culling control. | Implemented; control measured below. |
| My initial gap regression checked monotonic order but could pass after losing some visible items. | Compare every `(node, model)` pair against independently imported, non-instanced assets and compare their pixels; verify camera changes actually change the visible set. | Fixed; both-backend acceptance. |
| A first pixel-reference attempt included live FPS/draw-count text, causing real image differences unrelated to geometry. | Capture scene pixels with the debug HUD disabled; collect ordered diagnostics in separate enabled frames. Keep strict scene pixel thresholds. | Fixed; the failed attempt is not reported as renderer evidence. |
| My benchmark's `PROFILING=0` omitted the URL parameter while the example wrapper defaulted to profiling on. | Always send `profile=0/1` and assert actual `frameProfile().enabled`; invalidate the mislabeled controls, retain their data with an explicit reason. | Fixed. Ordinary profile-on pairs remain valid. |
| A single short control suggested a large slowdown, while later opposite-order runs reversed it. | Keep all valid short results, add independent longer 8-second-warmup/15-second-sample alternating pairs, and avoid choosing only favorable runs. | Final data below distinguish protocols and scope. |
| Stable scratch capacities could be mistaken for zero allocation. | Explicitly label the two diagnostics as vector-capacity growth and retained storage. Models, prepared GPU handles, diagnostic records and stable blend-sort storage may still allocate. | Documented and tested. |
| Checking all disjoint roots is O(N²); the 1024 draw-group cap does not cap this search. | Preserve current semantics in this isolated experiment. Investigate a bounded spatial method separately, preserving overlap ordering. | Open scaling candidate; no asymptotic improvement claimed. |
| Per-primitive update repeats inverse/determinant for primitives of the same node. | Consider a per-node transform cache as a separate measured change. | Open; not mixed into this experiment. |
| Animation clones a transactional pose and rebuilds hierarchy data. | Investigate reusable candidate pose/precomputed hierarchy, retaining validation and atomic rollback. | Open; higher correctness risk. |
| Existing draw-count and RAF CPU improvements cannot establish total GPU cost or Quest FPS. | Preserve frozen builds, alternating pairs, profile-off controls and actual device evidence boundaries. | No Quest performance claim. |
| Rust death halves are not full Three.js UV/PBR/cap/Explode parity, and some installation/physics work remains on the main Rust frame. | Retain the earlier disclosure; do not describe the whole physics/render pipeline as worker-owned or visually identical. | Open gameplay/visual-parity work, outside queue optimization. |

## Design and external inspiration

Three.js also pools render-list items and resets their active cursor each frame: [r170 WebGLRenderLists source](https://github.com/mrdoob/three.js/blob/r170/src/renderers/webgl/WebGLRenderLists.js). The currently served local Three reference reports revision170; do not confuse it with a separate historical benchmark using another revision. This supports the reuse design; it does not prove a Rust speedup.

The Rust change retains the previous compatibility checks, overlapping-root behavior, stable transparent distance sorting, 1024 group boundary, shaders, GPU layouts and independent eye/pass instance pages. Scratch owns indices/values only; prepared draws still own cloned GPU handles. Capacity is released when model-dependent caches are cleared after model removal.

The merge visits visible ordinals in ascending order and models in their prior order. It skips empty ordinal ranges, but remains proportional to models × distinct visible ordinals for an asset run. Its purpose is lower temporary allocation and CPU preparation work, not a universal complexity claim.

## Measurement and delivery

[Versioned raw samples and identities](benchmarks/astra-queue-2026-10-24.json) retain all46 local reports, including unfavorable controls and explicitly invalid mislabeled profile-off runs. Compact row/sample field schemas are included; original reports and frozen packages remain under ignored `output/astra-queue/` and `output/core-optimization/astra-*`.

Apple M1, hardware ANGLE Metal, Chrome155.0.8059.85, 960×640/DPR1, unchanged content/effects and about60Hz capped RAF. Baseline WASM `594f1a1eaed1ca184ea0b5151ff6b246d690d44a757d00132c91470209eace60`; candidate/final WASM `24b0fc9c853423f23e34710491acce0bdee7ea4555294ce00d724748da4611a8`.

| Rome CPU metric | Baseline | Candidate | Evidence scope |
|---|---:|---:|---|
| WebGPU glTF queue | 0.403ms | 0.350ms | Three paired profile-on runs; all pairs lower. |
| WebGL2 glTF queue | 0.308ms | 0.259ms | Three paired profile-on runs; all pairs lower. |
| WebGPU whole RAF callback CPU, short profile-on | 1.700ms | 1.622ms | Descriptive mean; not every pair improves. |
| WebGL2 whole RAF callback CPU, short profile-on | 2.634ms | 2.592ms | Descriptive mean; not every pair improves. |
| WebGPU whole RAF callback CPU, long actual profile-off | 1.976ms | 1.820ms | Two opposite-order pairs; both lower. |
| WebGL2 whole RAF callback CPU, long actual profile-off | 3.372ms | 2.842ms | Two opposite-order pairs; both lower. |

Short profile-on pairs use2s warmup/5s samples in AB/BA/AB order per backend. Longer independent checks use8s warmup/15s samples, actual profiling disabled and asserted, in candidate/baseline then baseline/candidate order. The longer active-game sample covers different gameplay phases and must not be compared directly to the shorter numbers. Draw/upload samples show comparable load within each protocol (long uploads39.55–39.64KB/frame). CPU excludes worker-message/timer-handler work. Overlapping60-frame scope histories are not independent repetitions. No confidence interval, whole-GPU speedup, uncapped throughput or Quest FPS claim is made.

The first valid short profile-off controls were worse on the candidate; two additional opposite-order pairs produced mixed results. These remain in the data. The longer protocol was selected for both builds before its results were known. It supports retaining the optimization, but does not establish a universal percentage gain. The candidate's second long WebGL run had a6.9ms maximum versus4.5ms baseline, despite a slightly lower p99 (4.0 versus4.1ms); removal of all hitches is not claimed.

A single initial sparse control showed a large apparent slowdown. Three alternating sparse pairs then measured total means WebGPU0.739→0.770ms and WebGL1.029→1.040ms, with mixed individual outcomes. Queue means were0.308→0.323ms on WebGPU and0.309→0.294ms on WebGL. This is a small possible sparse-path cost, not an arena-wide improvement. Small rigid controls also varied; they are regression controls, not a second headline speedup.

The seven retained scratch vectors did not grow during any warm Rome sample. Their live retained capacity was10.5–25.9KB depending on warmup history, and `scene.clear()` releases it. `queueScratchGrowths` counts vectors whose capacity increased in a preparation call, not allocator calls; `queueScratchBytes` reports their current retained capacity. New memory for models/prepared handles/optional records and stable sorting is outside those counters.

Static Rome and neon-arena images are bit-identical before/after on **both** backends: four comparisons, zero differing pixels. Static actual commands stay172 for Rome and13 for arena; the optimization changes preparation, not submitted geometry. Active Rome remains around126.5 main commands and39KB/frame uploads. These fixtures do not establish full material/death-animation parity with Three.js.

## Acceptance and deployment

Astra's final scoped source/evidence review found no remaining blocker, conditioned on the production checks below. Its review independently checked the source and interpreted parent-run data; it did not itself run tests. Final production results and served-byte verification are recorded below after the gate completes.


Final release/minified production-root acceptance: **26 applicable cases PASS**, zero failures, on WebGPU and WebGL2. Coverage includes rigid/skin GPU transforms, glTF culling/instancing/materials/SDK/full scene, mirrored rendering, targets, shadow caches, Rome worker-cut gameplay, neon arena and simulated XR. New queue regressions cover independent-reference completeness/pixels, unequal column gaps/camera changes, stable blend ties, the1025→1024+1 boundary and scratch cleanup. WASM release build, wasm32 Clippy with warnings denied, typecheck and diff whitespace checks passed. No native code path changed in this iteration; native tests do not compile this renderer and were not rerun as a substitute.

HTTPS8443 serves the exact final24b0fc9c WASM and sampled production SDK/shared/Rome modules (verified HTTP200 and SHA-256 equality). The existing Three.js8878 service is preserved. Source changes remain uncommitted on the owned branch; no new push, CI result or physical Quest acceptance. The consultation remains Astra Medium for continuation of this optimization work, with compact problem/delta/build/measurement/limit packets rather than full-history copies.
