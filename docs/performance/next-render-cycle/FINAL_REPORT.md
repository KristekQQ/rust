# Nine-batch renderer cycle — final integration

**DRAFT / power hold: batch9 performance evidence is incomplete. The owned scheduler is held after actual host battery reached11%/21min remaining; current case may finish. Stable-code gate passed; this file does not assert completion of missing performance evidence.**

## Scope and identities

Only final integration of the separately authorized nine-batch series. Isolated `codex/next-render-cycle-9`, parent `813a5abb266609cceea644e4b1086129a89ca535`; primary dirty work and shared services are preserved. No push, main merge, production deployment or port8443 restart. B9 changes measurement tooling/regression coverage/documentation, not renderer behavior. Final runtime deliberately reuses the byte-verified B8 release.

| Identity | Original B1 baseline A | Final B8/B9 runtime B |
|---|---|---|
| Origin | main `b7de3be56ad2a8a79a30e7b4fe927fa6822e33c7` | completed B8 checkpoint `813a5abb266609cceea644e4b1086129a89ca535` |
| Rust/WGSL/build-source SHA256 | `845a83b79156a8b6d30151f17629b07cc966d0dc152de58b6fed36726d8a0963` | `54c6cde834bd3ad2e5363aabece753cd77cf4aa4537d378147405f6a814797f1` |
| Release WASM SHA256 | `b5cc8eba529f7c3990b1a4eacbc16c13786ac774b8a366ad15eba6943340987a` | `f14c4beb5f3e5b59f16cae9ae90ddccba88b152ed10f07f3ce999d5a765a12de` |
| Generated bindings SHA256 | `e248ba204254d2f9dcdaf67eb8dc4ec0438bb9f5ad34417caf2b0cdd421804e8` | `d359800638f899ae7df46d50ab453a75b477044dec2d538d8ed39d91d24d44ba` |

All296 baseline and297 final frozen runtime files were checked before measurements; authored examples/assets match. Current B9 Rust/pkg identity matches final B. The maintained B9 measurement harness is deliberately common to both frozen runtimes; actual served hashes and driver identities are recorded separately. A frozen package, current release source identity, SDK bytes, a served workload and a shared production service are distinct facts. The unchanged shared service is not represented as this deployment.

## Integrated decisions

- B2: bounded exact opaque prepared plans skip repeated selection/grouping/common-basis proof/payload packing/prepared rows when all dependencies are unchanged. Rendering still occurs each frame; transparent order remains independent.
- B3: fine compatibility/data/view reuse preserves immutable rows under camera/sparse motion; fresh visibility/depth decisions, exact transforms and dynamic fallback remain. No global camera-matrix map or rounded views.
- B4: per-view/pass/layout instance pages/streams/material binds retain bounded inactive capacity, global64MiB/4096handle policy, leases/eviction/clear semantics. Active/pinned working sets can exceed the inactive budget rather than drop geometry. Engine lease release is not immediate physical GPU destruction.
- B5: exactly finite-white effective rigid RGBA omits the16B vertex color attribute while preserving material/texture/alpha factors and CPU geometry. Original color interpolator/multiply remains after removing it failed strict XR pixels. Unsupported deformation paths keep the original layout.
- B6: actual normalized max index≤65534 uses Uint16;65535 and larger use Uint32. Explicit format/strip pipeline identity and upload padding/logical-count separation survive sharing/cache/lifecycle. Point/line shadow depth bias is zero to satisfy wgpu validation; triangle shadow settings are unchanged. CPU cutting indices remain intact.
- B7: runtime depth reordering and published-count reuse are **rejected and removed**. Depth order changed pixels; count reuse reduced callback wall time but worsened repeated throughput. Only default-off diagnostics/scopes remain; production experiment mode0.
- B8: public pinned-wgpu shared-GL/external-framebuffer XR prototype is **explicit opt-in**. Default remains canvas-transfer; WebGPU owned-eye staging/final stereo submit is unchanged. Asymmetric stress improved in B8, ordinary symmetric callback wall time regressed; no automatic rollout or general transfer-only claim.

Independent read-only integration review found no material defect in inspected keys/content epochs/layouts/index formats/resource leases/default switches or rejected-code removal. Its follow-up found no material defect in new B9 measurement tooling. These are source reviews, not extra test runs.

## Measurement boundaries

See [fixed protocol](batch-9/PROTOCOL.md) and [commands](batch-9/COMMANDS.md). Desktop1280×720/DPR1, seed1337/spatial ordering, unchanged production scene content/effects,8s warmup/25s sample and3AB/BA/AB pairs/backend. Dynamic cut/spawn/despawn samples60s; pool256 soak samples120s with one pair/backend, explicitly not a three-repeat speed claim. Baseline-only bounded calibration selects actual work; capped ordinary controls do not measure throughput. All primary profiling and GL timer queries are off; named scopes/API diagnostics are separate pages/runs. CPU callback wall time includes driver waits/browser scheduling; it is not pure CPU computation. GPU timestamps and elapsed-query intervals are not compositor/Quest latency or isolated GPU busy time, and are not added to CPU wall time.

Actual browser/hardware/power and preserved foreign GPU process evidence are retained. The machine is not GPU-isolated; battery/thermal state is recorded per run. Small differences are observations, not causal/general speedup proof. Existing active foreign processes are neither stopped nor attributed to another task without evidence. A provisional AC-transition inference was retracted after repeated adjacent sandbox/host calls disagreed (sandbox AC, host battery); runner snapshots use the host. Preserve that observation and counterevidence. Discrete endpoints do not establish continuous power stability.

## Correctness and known limits

The inherited B4 moving1024 WebGPU strict-image discrepancy remains **open**: its primary8pixels/max2 and independent1/3exact evidence cannot be dismissed as harmless baseline instability. Later passing fixtures/full gates do not close it. B9 keeps exact tolerances and every mismatch. Native active-gameplay captures lack a deterministic reset and are functional evidence, not an invented pixel oracle. In rim-active the playing Rust game overrides JS setCameraView: actual images show native gate/player view, visible enemies/HP, both swords and HUD. Treat this as a native-gameplay-camera control, not the JS requested orbit path; source/phase/image audit retained. Original color-layout/Uint32 and cache-off+culling-off references in the maintained gate remain independent controls.

Cold probe initial7/12 failures are retained as capture-tooling failures; capture was repaired to copy immediately after the actual engine callback, outside timing, without changing scene/threshold. Repaired12/12 probes render foreground with actual profiling false and observed zero query creations. They separate asset-load callbacks, first post-load callback and next120callbacks. Models may draw during async loading; browser/driver caches are shared and the second variant reuses an engine. This is not guaranteed first-draw/driver-cold compilation timing or automatic A/B pixel equality.

A calibration output-label mistake overwrote one initial short1024 WebGPU pilot. Its raw series is unavailable and excluded. Fresh2048 calibration is retained. The maintained driver now refuses existing output directories and a registered regression test verifies sentinel preservation before browser/server activity. No primary result is discarded or overwritten.

Physical Quest testing, real opaque headset framebuffer behavior, headset comfort/FPS, physical VRAM/process-memory plateau, production deployment, push and CI are not established by desktop/simulated-XR evidence. ADB is unavailable on the current PATH; no physical-device test is claimed.

## Partial fresh results — evidence is incomplete

72 clean primary runs /70,716 full RAF records have been completed. Independent recomputation matches every stored callback/frame mean,median,p95,p99,max and capture hash. All24 available deterministic primary pairs are exact (zero channel/pixel tolerance). Native gameplay/Neon are deliberately not assigned a false deterministic pixel PASS. This does not close the historical B4 issue. [Every completed run and tails](batch-9/RUNS-PARTIAL.md), [machine-readable partial results](batch-9/RESULTS-PARTIAL.json), [actual active-game image](batch-9/active-native-webgpu.png).

Values below are medians of three per-run mean-cadence FPS and callback means, not a ratio of frame medians. These are observed desktop results under contention/battery operation; ordinary60Hz controls do not establish throughput/headroom or causal small gains. All original individual rows, including adverse callbacks, remain in the linked results.

| Family / load | Backend | Baseline→final FPS | Callback wall mean ms A→B | Callback p95 / p99 ms A→B |
|---|---|---:|---:|---|
| arena-active / 1 | webgl | 30.00→30.00 | 2.340→2.454 | 3.60/4.20→3.70/4.20 |
| arena-active / 1 | webgpu | 30.00→30.00 | 1.023→1.222 | 1.50/1.80→2.10/2.40 |
| rim-active / 1 | webgl | 60.00→60.00 | 2.836→2.907 | 3.80/4.30→3.90/4.30 |
| rim-active / 1 | webgpu | 60.00→60.00 | 2.144→2.118 | 2.90/3.10→3.00/3.30 |
| rim-camera-stress / 2048 | webgl | 30.00→30.00 | 24.784→20.795 | 36.80/39.00→32.70/35.00 |
| rim-camera-stress / 2048 | webgpu | 30.00→30.00 | 23.573→19.722 | 35.40/37.60→31.30/34.10 |
| rim-clustered / 1024 | webgl | 24.62→24.75 | 40.416→40.248 | 49.50/51.70→49.30/54.20 |
| rim-clustered / 1024 | webgpu | 25.86→25.52 | 38.441→38.941 | 47.50/50.40→47.40/50.80 |
| rim-static / 1 | webgl | 60.00→60.00 | 1.086→1.165 | 1.60/1.80→1.70/2.00 |
| rim-static / 1 | webgpu | 60.00→60.00 | 0.745→0.683 | 1.10/1.30→1.20/1.40 |
| rim-stress / 2048 | webgl | 26.66→29.02 | 37.313→17.357 | 52.70/57.90→30.90/34.20 |
| rim-stress / 2048 | webgpu | 27.56→37.66 | 36.044→14.953 | 51.00/57.80→30.80/32.50 |

Camera-motion retained CPU rows do not imply zero GPU upload. Current per-RAF stream counters retain cases where final uploaded bytes exceed B1; B3 reservations/one union dirty span can include unchanged gaps. Do not turn cache hits into an upload/FPS claim. Fully moving clustered scenes are approximately neutral; do not label static gains a solution for dynamic gameplay or VR. Main command/instance/triangle/upload/native allocation counters and retained capacities are in the partial JSON; glTF counters do not count all custom-mesh Neon work.

Primary GPU timings are unavailable by design. Fresh separate profile/API diagnostics, long dynamic64 cut/spawn/despawn,120s pool256 soaks, real-worker latency replays and final-native/Three controls **have not run yet**. Therefore final memory plateau, cold-vs-warm allocation attribution, current Three gap and GPU bottleneck breakdown are not established. Cold12/12 instrumented probes are separate evidence; they do not replace these missing measurements. Historical B2–B8 reports retain their original protocols/identities, not fresh B9 results or a current Three ranking.

## Nine-batch status

| Batch | Local disposition | Limit |
|---|---|---|
|1|Accepted baseline/infrastructure|No renderer speedup claimed; [original report](REPORT.md).|
|2|Accepted exact opaque plan cache|Static CPU preparation benefit; [B2](batch-2/REPORT.md).|
|3|Accepted bounded fine invalidation/fallback|Motion/sparse benefits; historical crowded strict-image investigation open; [B3](batch-3/REPORT.md).|
|4|Accepted bounded resource lifetime/reuse|No universal FPS/stutter gain; same strict-image limit; [B4](batch-4/REPORT.md).|
|5|Accepted lossless white attribute omission; varying removal rejected|GPU geometry bytes saved, no established general FPS gain; [B5](batch-5/REPORT.md).|
|6|Accepted lossless portable Uint16 plus valid point/line shadow bias|Index bytes saved, no established general FPS gain; [B6](batch-6/REPORT.md).|
|7|Production candidates rejected; diagnostics accepted|Depth order changes pixels; count reuse worsened throughput; original scan restored; [B7](batch-7/REPORT.md).|
|8|Opt-in prototype only; device unverified|Canvas-transfer default; symmetric callback regression; no Quest signoff; [B8](batch-8/REPORT.md).|
|9|INCOMPLETE / external power constraint|Partial measurements and stable-code verification; missing work listed above. No completion claim.|

Every batch remains unverified on the physical Quest for this final build. No push, merge, deployment or CI is claimed. Fresh `VERIFY_WASM_PROFILE=release npm run verify:push` completed once with exit0:428native PASS,2existing manual timing tests ignored,32SDK,71XR-platform,14Node harness+13Python harness PASS;149/149browser PASS (55WebGPU,94WebGL), actual Chrome156.0.8078.48/hardware rendering/no sandbox disable. Separate49performance helper units, syntax checks and skill validation PASS. Release source/WASM/bindings remain exactly the hashes above; content-bound PASS receipt rechecked without rerunning tests. Source dump regenerated (Rust unchanged). [Verification record](batch-9/VERIFICATION.json).

These counts are freshly executed B9 evidence, not historical418/137 or inherited B8 counts. A passing source gate cannot substitute for missing performance evidence. B9 is a local incomplete evidence checkpoint, not final integration/performance signoff.

## Recovery / next exact action

Own scheduler PID84701 was held with SIGSTOP after the current case; its browser cleanup completed. Verify PID/command before any SIGCONT. Do not restart primary.py blindly: immutable existing output labels intentionally fail. Continue the remaining original primary cases after power is available, then remaining.py (fresh Three,worker/profile/API controls), then bounded actual-host-power confirmations. Keep all original data.

Emergency durable backup: `/Users/Honza/WebstormProjects/rust/output/next-render-cycle-9-power-backup`, unique task-owned ignored output, containing own source copies/patch, completed raw measurement records and both complete frozen runtime packages. It is not a production deployment or another task's package. The isolated worktree/branch and B1–B8 local checkpoints remain; no B9 completion checkpoint is asserted yet.
