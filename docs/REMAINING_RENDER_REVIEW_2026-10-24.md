# Remaining renderer review: implementation and measured scope

This completes the remaining review follow-up on `codex/core-render-optimization`, based on `088fdb34` plus the already-owned uncommitted follow-ups. The work is local and served on HTTPS 8443; it is not a new commit, push, CI result or physical Quest acceptance. Primary-checkout work was preserved.

## Review disposition

| Recommendation | Current result |
|---|---|
| Avoid WebGL packed-depth output for FXAA/bloom only | Completed in the [earlier follow-up](CORE_REVIEW_OPTIMIZATION_2026-10-24.md). SSR restores depth resources; normal depth testing and composition remain. |
| Bound preparation and installation of cut geometry | Earlier follow-up: fixed arrays, strict triangle batches, staged hidden geometry, actual GPU-upload acknowledgement and atomic publication. The Rust/WASM worker still computes the cut; final installation/upload has bounded main-thread work. |
| Shared opaque glTF queue and actual draw diagnostics | Earlier follow-up introduced the shared queue. This change adds actual instanced-command and primitive-culling counters. Transparent ordering remains separate. |
| True rigid glTF instancing | Implemented and measured below. Shared geometry now also reduces compatible draw commands. |
| Conservative per-primitive frustum culling | Implemented using cached rigid/joint bounds and the current eye/pass frustum. Unsupported or unsafe bounds fail open. |
| Shadow-caster-specific invalidation | Completed earlier; the final browser checks retain caster/noncaster and empty-map regression coverage. |
| Batch small uploads | Instance transforms use cached per-view pages; immutable material data has a separate buffer and skips unchanged writes. The earlier general aligned-uniform arena was measured, rejected and remains removed. |
| Material/attribute shader specialization | Unused map UV inputs/varyings are omitted. The shared 96-byte vertex storage, derivative TBN and `128/255` fallback normal remain to preserve material semantics. Full vertex repacking and removal of the tangent calculation are not claimed. No isolated GPU-time gain is established for the UV change. |
| Reproducible timing, cut events and accessible raw samples | Earlier cut timing separates worker acceptance, preparation and staging; this report adds versioned raw paired runs and deterministic scene pixels. Short profiler p99 and sparse GPU samples are not treated as long-run hitch evidence. |

## Instancing, culling and lifecycle

Compatible opaque rigid draws use one indexed command with multiple instances. The portable instance record is 112 bytes (model matrix, normal matrix and winding sign), with seven attributes at locations 9–15. Devices without the required attribute/buffer limits retain independent draws. Skins, morphs and blended primitives retain their existing paths.

Batching is deliberately conservative: consecutive shared-asset roots must have disjoint conservative world bounds, matching primitive/material/options and winding. Primitive order within each model is retained; overlapping roots remain separate. Groups are capped at 1024. This is not arbitrary global material sorting or instancing of independently skinned actors.

Separate pages for each view/pipeline protect left/right-eye, shadow and reflection data. Unchanged instance transforms and material data skip buffer writes; removal clears cached pages, including inactive views. Optional diagnostic draw records identify all objects in a batched command. Existing `render_stats().drawCalls` remains the legacy estimate; actual submitted commands are exposed by `debugStats().mainDrawCalls` and the shared runtime information panel.

Primitive bounds combine root and animated node transforms. Cached joint bounds remain conservative; invalid, unsupported and morph cases bypass the fine test. Camera-only changes and each eye use their own frustum. This avoids recalculating all deformed vertices just to cull a primitive.

## Measurements

[Versioned raw samples, build identities and final checks](benchmarks/remaining-review-2026-10-24.json). Apple M1, hardware ANGLE Metal, Chrome 155.0.8059.85, release WASM, 960×640/DPR1, unchanged effects and scene content. Three sequential before/after pairs per backend; two-second warmup and five-second sample. RAF remained capped near 60 Hz.

| Rome metric, mean of three run means | Before | After |
|---|---:|---:|
| WebGPU RAF callback CPU | 1.787 ms | 1.659 ms (−7.2%) |
| WebGL2 RAF callback CPU | 4.053 ms | 2.833 ms (−30.1%) |
| Actual main draw commands, both backends | about 450 | about 127 |
| Buffer upload bytes/frame | about 98 KB | about 39 KB |
| glTF buffer-write calls/frame | about 402 | about 90 |

CPU means sum measured RAF callback work. They exclude worker-message and timer-handler CPU; they are not total CPU, GPU time or uncapped throughput. Sparse asynchronous GPU timings varied even in unchanged FXAA, so no GPU speedup percentage is asserted. Small rigid/skin controls did not improve uniformly; this result targets repeated rigid work in Rome. Neon arena uses the shared renderer changes when applicable, but no arena FPS gain is established.

Baseline WASM SHA-256: `eda0b9ee6dea71eec71dc99334231f7dd29df397f5c91589809577ab3b48d195`.
Paired measurement candidate: `c01296491726a6857df96ef1f716ccd0c6006de7613336991dfce99c646a2b4e`.
Final served build: `594f1a1eaed1ca184ea0b5151ff6b246d690d44a757d00132c91470209eace60`.

The final build restores the legacy stats estimate without changing the render implementation. A final single-run confirmation of that exact build measured WebGPU 1.621 ms and WebGL2 2.630 ms CPU, about 39 KB/frame uploads, no errors and the requested backends. These confirmation runs are not substituted into the three-pair averages.

## Image preservation

The deterministic scene comparison freezes camera, simulation and assets, with resolution/effects fixed. Only changing diagnostics text is hidden. Rome WebGL2 is bit-identical; neon arena is bit-identical on both backends. Rome WebGPU has a mean absolute channel difference of 0.000769, maximum 32/255, and 23 of 614,400 pixels above the comparison threshold (RGB absolute-difference sum >12). Side-by-side inspection shows the same composition and models; these small GPU rounding differences are disclosed, rather than described as exact equality. Controlled static Rome draw commands fall from 441 to 172; its paused arrangement differs from the active-game timing sample.

Dedicated GPU/CPU pixel tests additionally cover rigid/skin transforms, negative/nonuniform scale, overlap ordering and fallback paths. The existing shared five-UV vertex storage and nonneutral fallback-normal semantics were retained.

## Verification and delivery

- Native library: 352 passed, two ignored; includes eight bounds tests, animated joints, combined transforms and unsupported-input fallbacks.
- Release WASM build/fingerprint, wasm32 Clippy with warnings denied, SDK/type checks and 14 simulator unit tests passed.
- Final production-root browser batch: **40 applicable cases passed**, zero failures, both backends; actual instancing, glTF/material/lifecycle, mirrored rendering, effects/targets/shadows, Rome gameplay/cut-worker halves, neon arena/enemies and simulated XR. Each-eye actual instanced commands, repeat pixels, camera isolation and cleanup are covered. Native/build results alone are not used as visual evidence.
- VR test timer assertions now count the intended director/launcher delay separately from the shared 250 ms diagnostics sampler. Product timing was not changed to satisfy these fixtures.
- Independent source review covered bounds, instancing, ordering, page lifetimes and canonical per-page material data; no outstanding scoped finding.
- HTTPS `192.168.0.17:8443` returns the exact final production WASM, SDK and sampled shared/Rome modules. The production package is release/minified. Three.js on 8878 is preserved.

Refresh [Rome WebGPU](https://192.168.0.17:8443/examples/rim/?backend=webgpu) or [Rome WebGL2](https://192.168.0.17:8443/examples/rim/?backend=webgl). Browser and simulated XR evidence do not establish Quest FPS or comfort, nor a new matched Three.js performance verdict. No new commit/push was requested for this follow-up.

Local reproduction: `SCENES=rim node scripts/benchmark-core.mjs <label> <frozen-pkg-directory>`; a sibling `source/` holds the frozen SDK/example inputs. `scripts/compare-render-optimization.mjs` compares two frozen package directories and records the actual served input hashes. Final acceptance and local images are also retained under ignored `output/remaining-review/`.
