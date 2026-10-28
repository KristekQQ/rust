# Batch7 — GPU diagnosis; both production experiments rejected

Status: complete locally; final restored release verified, evidence archived, owned checkpoint. No production speedup accepted. Only batch7, isolated local work; no push/merge/deployment/server restart.

## Actual diagnostic work

Frozen instrumented unoptimized release D; each row8s warmup/25s sample,1280×720 unless specified, DPR1.16 runs, alternating backend order, stable input assets/camera/Idle@0. Single-axis exploratory observations, not three-pair performance proofs. Repeated production anchors are exact same-backend pixels. WebGPU load2048 and WebGL1024 are different loads: compare axes **within** a backend, not the two columns as a renderer ranking.

| Axis | WebGPU FPS / callback ms | WebGL FPS / callback ms |
|---|---:|---:|
| anchor | 37.74 / 16.40 | 42.29 / 9.06 |
| low | 41.01 / 14.54 | 57.44 / 7.83 |
| high | 34.26 / 17.01 | 33.82 / 9.88 |
| cheap | 38.44 / 15.21 | 60.00 / 7.92 |
| shadow-off | 37.80 / 15.94 | 50.60 / 8.62 |
| fxaa-off | 37.83 / 8.81 | 43.45 / 6.36 |
| depth | 37.66 / 16.05 | 45.88 / 8.85 |
| anchor-repeat | 37.76 / 15.88 | 42.63 / 9.00 |

Resolution controls are640×360 and1920×1080; all other rows1280×720. Every WebGPU row submits307 glTF commands/39,067,116 triangles per profiler frame; every WebGL row240/20,714,924. Geometry work is unchanged across axes. Native render_stats.drawCalls is a requested/unbatched primitive estimate and is **not** the actual command count. Full distributions/counters/quality snapshots in matrix-summary.json and raw report archives.

## Cost boundaries and inference

WebGPU scene.main elapsed26.15ms at anchor; low24.20/high28.69; glTF-only cheap25.64; shadows off26.01; depth26.00. FXAA3.19ms at anchor/1.32 low/5.48 high. These sampled pass spans cannot be mechanically added to callback or frame interval. Valid mapped async timestamps, stale/nonfinite checks, no sync GPU wait. Separate present CPU scope exists; direct canvas presentation has no independent GPU timestamp, compositor latency/utilization unavailable.

WebGL application-command interval54.50ms anchor,36.95 low/71.97 high,18.93 cheap,43.02 shadows off,45.43 FXAA off,49.39 depth. Valid asynchronous EXT queries; this interval includes potential CPU submission gaps/driver waits and excludes compositor/worker/between-RAF work. It is **not** a pure GPU busy-time equivalent of WebGPU scene.main. Profiling overhead excluded from later clean runs.

Inference: WebGL is strongly sensitive to pixel/shader cost; WebGPU at the doubled geometry load is much less sensitive to the cheap fragment or shadow switch, so main-pass geometry/vertex/throughput cost remains plausible. These axes do not isolate exact ALU/cache/bandwidth utilization. FXAA scales with resolution, but removing it did not materially improve WebGPU frame interval. Do not lower quality as a “fix”.

CPU sparse observed-history overlay selection8.48ms/2048 WebGPU and4.58ms/1024 WebGL. It repeatedly scans immutable primitive arrays/active-node HashSets despite already-published B3 classifications. Scope histories are sparse changes, not complete per-frame scope histograms. Complete RAF wall distributions are separate; they include driver waits and benchmark sampler overhead.

## Decisions

- Reject depth-order prototype: WebGL exploratory42.29→45.88FPS is accompanied by304 changed pixels/max102; WebGPU4533/max101. Repeated original anchors have zero differences. Small fixtures passed, real-scene oracle failed. Removed from C; frozen D/rejected patch preserve the experiment. No tolerance/reference update.
- Reject cheap shader, missing shadows/FXAA and lower resolution as final optimizations: they change intended quality. They remain explicit diagnostics; cheap mode replaces glTF material lighting, while non-glTF meshes and postprocessing stay on their existing paths.
- Reject published-count reuse C despite substantial CPU callback savings: stress FPS is lower in all three pairs on both backends, with non-overlapping ranges. WG baseline37.64–37.80 vs C37.44–37.50; GL42.55–42.68 vs41.97–42.12. Foreign GPU load does not justify attributing away the regression. Restore the exact original scan/ownership, keep C and its evidence unchanged. No selective fallback is accepted without separate evidence. See DESIGN.md.
- Keep only default-off diagnostic controls, profiler scopes, maintained measurement/Three controls and correctness assertions. No quality reduction, further batching or accepted production optimization in this batch.

## Three reference

Pinned r170 and existing per-part InstancedMesh factory, not clone-per-model substitute. Limited shared-GLB base-color control and original production configuration are reported separately; see THREE-CONTROL.md. Preflight64 sends86 commands/1,372,924 triangles in both, paired captures visually verified. Full PBR/AA/gameplay parity is not established.

## Final verification

Strict WASM clippy, typecheck and release build passed.38 maintained performance helper tests passed. Targeted both-backend gltf-plan-cache/motion passed, including exact original shader restoration, OPAQUE/MASK silhouette, independent cache-off pixels, active scene3→1→3 and instance6→3, mutations/negative transforms/auxiliary passes/lifecycle. No explicit stored-depth-buffer oracle or injected internal malformed-pose error; original entrypoints and C’s failed-update fallback audited independently; the candidate flag/fallback is absent from R. Browser/simulated XR are not physical Quest evidence. Final restored-runtime full gate passed:428native (2ignored manual timings),147browser, strict WASM/SDK/harness/XR/types. Candidate full gate passed, but is superseded by the source rollback.

## Rejected C: paired results

D is the instrumented unchanged-production control. C is the now-rejected published-count candidate. Each row is median of three run means; complete per-run mean/median/p95/p99/max remain in RESULTS.json and raw archives.48clean runs/63,403RAF rows;24/24 strict zero-difference image pairs. All runs8s warmup/25s sample, AB/BA/AB, profiler verified off. C reduced callback wall work but did **not** improve stress throughput. Callback time includes driver/sampler work and is not pure CPU compute.

| Workload | D→C FPS | D→C callback mean ms | D→C callback p99 ms |
|---|---:|---:|---:|
| Rome stress2048 WG |37.68→37.48|14.93→4.15|31.7→6.3|
| Rome stress1024 GL |42.57→42.00|8.32→3.88|17.7→5.4|
| Rome one-moving1024 WG |60.00→60.00|12.49→7.68|23.2→9.9|
| Rome one-moving1024 GL |42.44→42.44|13.81→8.83|24.1→10.8|
| Ordinary Rome WG |60.00→60.00|.748→.710|1.6→1.5|
| Ordinary Rome GL |60.00→60.00|1.394→1.346|3.0→2.6|
| Neon WG |60.00→60.00|.729→.728|1.5→1.5|
| Neon GL |60.00→60.00|1.200→1.170|2.1→2.1|

Adverse tails are retained: stressWG frame maximum50→66.7ms; ordinaryWG callback maximum2.8→3.7ms; NeonWG3.5→4ms. Stress frame p95/p99 remain33.4ms. C profile observations: WG scene.main26.20ms/FXAA3.35ms, GL command interval53.58ms; overlay selection observed mean.139/.076ms, respectively. Sparse scope histories are not invocation distributions. Whole API warm diagnostics found zero new buffers/binds (WG113frames/GL126frames), whole API242GL/308WG commands versus glTF240/307. GL whole-API triangle reconstruction is incomplete after the bounded8192-buffer metadata cap; native glTF counters remain exact. This count-only experiment does not claim resource-allocation savings.

## Actual pinned Three timings — experimental C, not final R

The following18clean runs compare the **rejected C runtime**, not the final restored renderer. Native limited control uses explicit cheap-fragment diagnostic mode. These data are retained for diagnosis and must not be reused as a current shipping-renderer ranking.

| Profile/load | Native FPS / callback ms | Three GL FPS / callback ms | actual native/Three commands |
|---|---:|---:|---:|
| Limited common2048, nativeWG |42.65 /3.73|36.64 /.557|287/86|
| Limited common2048, nativeGL |38.22 /5.06|36.72 /.597|287/86|
| Unmatched production1024, nativeGL |42.17 /3.93|45.03 /2.76|240/108|

Common2048 native submits39,061,900 triangles versus Three42,009,212 (7.02% fewer submitted triangles, not an independently proven culling-rate improvement). Different conservative visibility/batch policies remain. CommonWG adverse frame maximum83.3ms vs Three33.5ms. A separate profiled pair has valid native pass queries and Three GL EXT interval, with incomparable boundaries; it is excluded from the clean table. Original production Three actually has antialias=true/SAMPLES4, ACES/exposure1.25, sRGB, cached2048PCFSoft shadows and fog; native has FXAA/custom PBR. Same authored GLBs/camera/pose/load are controlled, full visual/gameplay parity is unavailable. The limited base-color profile is the fairer reduced control, still without cross-engine exact pixel equivalence. See THREE-CONTROL.md.

## Limits and disposition

Apple M1 desktop, persistent foreign GPU process left untouched. Pre-run power snapshot: AC attached70%,not charging; post-timing snapshot: battery67%,discharging. Transition time was not continuously recorded; power is not proven controlled throughout. Device GPU utilization, compositor latency and physical Quest timings are unavailable. Power change and contention further limit timing attribution. Causality of the small throughput change is not established; the observed consistent regression is sufficient to reject C under the user's acceptance rule. Neither CPU savings nor fewer draw calls alone establish a renderer win. Inherited B4 crowded-moving WebGPU strict image instability remains open. No push, main merge, deployment, shared8443 restart or remote CI. Batch8 requires a separate request.

## Separate CPU preparation/encoding/presentation scopes

Frozen D anchors, sparse observed-history means in milliseconds (different stress loads). These are nested/partial scopes, not a complete additive CPU budget. Main clean distributions use every RAF callback.

| CPU wall scope | WG2048 | GL1024 |
|---|---:|---:|
| scene.prepare |.021|.020|
| gltf.queue |.072|.046|
| render.encode.color |.373|.444|
| render.overlays.select |8.483|4.582|
| render.submit |.028|.527|
| output.present.cpu |.007|.024|

Warm exact prepared-plan hits bypass fine preparation scopes, so missing scope observations are unavailable/not invoked, not fabricated zero durations. GPU timing of direct canvas/compositor presentation is unavailable; the optional output.present GPU scope measures only an already-existing offscreen-copy pass when used. queue.submit wall time is not GPU time. Cutting handlers/installation stay outside these RAF scopes; existing maintained worker/cut scenarios are correctness coverage, not new B7 worker latency measurements.

## Restored final R — independent restoration check

Exact original B6 draw_count restored; candidate validity flag removed. R release source `da7117cc33917296e0a73bd94007b3e73c7a882e744a8dc2e8b7c09b6a447715`, WASM `a5ada8fb48ad0fddbc457a7a251f2743d52e262ca988eec5ba672f2c3449ea22`. Diagnostic SDK/pipeline modes0–1 remain explicit/default off; no depth-sort prototype or count reuse in final source.

Predeclared12clean D/R runs,3pairs/backend,8s/25s,11,011RAF rows. Six strict image pairs exact. Maintained regression CLI default5% median threshold passes; this is not a claim of improved tails or perfect equality of timings. Median-of-three:WG37.4966→37.4965FPS/callback15.125→15.233ms;GL42.5217→42.4417FPS/callback8.440→8.450ms. Callback p99 WG33.4→35.9ms,GL17.6→18.7ms; adverse max frameWG66.7→83.3ms,GL callback34.8→45.6ms retained. Contended desktop measurements do not resolve small differences. No accepted production speedup, including in R.

Independent audit:98main diagnostic/paired/profile runs,114,498rawRAF rows,10,084frozen files across B6/D/C/R; recomputed mean/median/p95/p99/max, capture hashes, actually served runtime/harness hashes, clean profiler-off assertions and current src/js/pkg equality to R all pass. Pilots/calibrations/failed attempts are retained separately and excluded from those counts. Whole-API allocation diagnostic pages are separate from the primary RAF rows. Native triangle counters are explicitly glTF-only; a zero for Neon does not mean zero total scene geometry. Three rows use renderer.info submitted main work; whole-API GL topology reconstruction is incomplete and not substituted.

## Checkpoint and evidence

Final R `VERIFY_WASM_PROFILE=release npm run verify:push` PASS,428native/147browser,2ignored manual timing tests;38helper units PASS. Generated all_sources.txt included in the local checkpoint. Content-bound receipt recheck passes and post-gate src/js/pkg still equal frozen R; no stale build/fallback accepted. C full gate is preserved separately as superseded evidence, not substituted for R. Independent final read-only review confirms exact B6 count restoration and no material report contradictions.

[Executed commands](COMMANDS.md), [machine results](RESULTS.json), [build/environment manifest](MANIFEST.json), [verification](VERIFICATION.json), [compressed evidence](evidence.tar.gz). Archive44,318,633B,1744named files,195,379,843logical bytes, SHA256 `83c759e8930c39d7e70636ab6dedebcf2b47195044e74b062a32e9ad9071b0ce`; every TAR entry/hardlink read back and hash verified. Exact-content dedup preserves named captures and adverse/failed attempts. Private hardware IDs/full foreign process command lines excluded. Full src/js/pkg/scripts/tests snapshots and inventories included; unchanged authored assets/vendor/original example files require parent checkout and manifest hash checks. No next batch starts automatically.
