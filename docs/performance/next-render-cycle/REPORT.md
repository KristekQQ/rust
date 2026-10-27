# Batch 1 — frozen baseline and measurement audit

## Scope and frozen inputs

This is a baseline-only report for batch 1. The 48 primary runs use the reused, verified release frozen at `output/next-render-cycle/baseline` (WASM `b5cc8eba…0987a`), 1280×720, DPR 1, 8 s warmup, 25 s sample, three runs per case/backend, engine profiling disabled. `audit/primary-acceptance.json` records all 48 accepted, profiler-off, cleanup-complete runs and the frozen WASM identity. No renderer source/runtime change or new release-WASM build is part of baseline preparation; the final gate compiles/checks native and wasm targets while reusing the verified release bytes. Exact invocations and exit codes are in `output/next-render-cycle/commands.json`; primary series use the maintained `render-performance.mjs smoke --baseline-root … --family … --backend … --load … --width 1280 --height 720 --warmup 8000 --sample 25000 --no-profile` path. The ten separate `profile-*` runs and five corrected `api64-*` captures are diagnostics, not throughput rows.

Post-timing immutable-view captures: [WebGPU](baseline-webgpu.png), [WebGL2](baseline-webgl.png). These preserve the measured1280×720 viewport; neither is a Three reference nor a cross-backend pixel-equality assertion.

## Primary cadence and callbacks

FPS below is sampled per-run RAF cadence, capped by the observed 60 Hz desktop presentation. Values cannot rank headroom where all runs cap at 60. Each cell gives the three-run range and median FPS; pooled RAF and application-RAF callback CPU tails follow as p95/p99/max milliseconds.

| Case | WebGL FPS range / median | RAF p95/p99/max | callback CPU p95/p99/max | WebGPU FPS range / median | RAF p95/p99/max | callback CPU p95/p99/max |
|---|---:|---:|---:|---:|---:|---:|
| A stationary Rome | 59.88–60.00 / 60.00 | 16.7/16.8/49.9 | 1.9/2.4/5.6 | 60.00 / 60.00 | 16.7/16.8/16.8 | 1.2/1.5/3.0 |
| B camera motion | 59.76–60.00 / 60.00 | 16.8/16.8/33.4 | 1.8/2.0/2.7 | 60.00 / 60.00 | 16.7/16.8/16.8 | 1.2/1.4/2.5 |
| C actual-drone clustering/visibility | 46.29–60.00 / 56.44 | 33.3/33.4/50.1 | 4.3/5.1/7.7 | 60.00 / 60.00 | 16.7/16.8/16.8 | 3.7/4.3/5.8 |
| C native gameplay | 52.40–60.00 / 52.40 | 33.3/33.4/50.1 | 4.5/5.4/11.9 | 54.96–60.00 / 60.00 | 16.8/33.3/50.0 | 3.6/5.2/10.5 |
| D synthetic cut lifecycle | 60.00 / 60.00 | 16.7/16.8/16.8 | 2.8/3.5/5.3 | 60.00 / 60.00 | 16.8/16.8/16.8 | 1.9/2.2/4.1 |
| E neon arena | 58.36–60.00 / 60.00 | 16.8/16.8/33.4 | 2.5/2.8/3.6 | 60.00 / 60.00 | 16.7/16.8/16.8 | 1.4/1.7/2.5 |
| Static 1024-drone stress | 7.38–41.61 / 7.88 | 150.0/183.3/216.7 | 103.6/112.6/127.8 | 9.74–54.24 / 12.52 | 116.6/133.3/200.0 | 108.0/122.4/146.9 |
| Moving 1024-drone stress | 4.91–24.82 / 5.84 | 216.6/233.4/316.7 | 208.9/235.6/252.5 | 4.89–24.88 / 6.49 | 216.6/233.3/266.7 | 207.1/229.0/236.8 |

Callback “CPU” is elapsed wall time inside the application RAF callback, including possible descheduling/driver waits; it is not measured on-core computation. Timer/message handlers are outside that scope. `queue.submit()` wall time is never used as a GPU duration. Raw frame tails are retained per run under each primary output; pooled rows are descriptive and frames within a run are not independent repetitions. GPU driver state was not isolated. The static stress runs varied sharply across repetitions, while the moving stress case was also deeply over budget. These data do not establish a clean capacity threshold, small backend advantage, or general backend superiority.

## Work counters and diagnostic scope

Actual glTF cumulative event-counter differences are reported at measurement boundaries; they are not renderer-wide totals. Counts sum submitted primitive-instance events across observed native glTF passes/views; instances are not unique model handles. Stationary Rome records 39 observed glTF draw/instance events, about 67,292 triangles/application RAF, with zero measured buffer-upload calls, instance-upload bytes, page creations, or buffer creations. In the 1024 moving-drone WebGL stress, medians are 203 all-observed-pass draw events, 52,264 submitted primitive-instance events and 16.19 million triangles/RAF; about 165 upload calls and 836 kB instance-upload bytes/RAF, with 1.82 page/buffer creations per RAF. These are workload-specific observed deltas, not memory totals. Dynamic synthetic lifecycle records about 13.7 glTF draws, 126.7 instances, 253 triangles, 12.7 upload calls and 12.9 kB instance upload per RAF. Renderer draw gauges include one additional FXAA compose where applicable. Full per-case counters and resource gauges remain in `summary.json`; do not treat a last-frame snapshot as cumulative work.

Ten separate profile runs exercise existing CPU scopes and asynchronous GPU timing. Scope freshness is derived from changed profiler histories; stale `lastMs` values with no history changes are explicitly unproven. `gltf.queue` combines grouping and instance packing, while `scene.prepare` is broader; neither isolates those subcosts. GPU values are asynchronous WebGPU named passes; WebGL EXT values are application-RAF command intervals, not isolated GPU busy time. Example: the stress WebGPU diagnostic observed `scene.main` mean 55.64 ms (10 fresh samples), while matching clean throughput rows remain profiler-off. These diagnostic values do not substitute for clean-run timing.

The five corrected 64 MiB WebGL API instrumentation captures are separate diagnostics and have no complete whole-API upload-byte total. Whole-API submitted draw/triangle proof is complete for static Rome, arena-active and the small synthetic dynamic case. Clustered and 1024-drone stress whole-API counters remain incomplete: the bounded index metadata table hit 8,192 entries and recorded about 62,000 overflow events, despite retaining only 27,254,736 bytes of its 64 MiB index-byte budget. Native glTF event draw/triangle counters remain available separately. Preserve the earlier 16 MiB prototype overflow in the journal; increasing the byte bound does not resolve the metadata-entry cap. Instrumentation output is not a memory-ownership census.

Whole-API warm diagnostics (direct calls/frame; counts span all wrapped contexts; not unique scene objects):

| Case | WebGPU draws / submitted instances / triangles | WebGL2 draws / submitted instances / triangles |
|---|---:|---:|
| Stationary Rome |40 /40 /67,293|41 /41 /67,294|
| Static1024 |241 /66,784 /20,714,925|242 /66,785 /unavailable (metadata proof overflow)|
| Neon arena |48 /56 /17,274|49 /57 /17,275|

Neon uses custom mesh paths, so a native glTF triangle counter of0 must not be read as an empty scene. Native draw gauges, glTF cumulative submissions and all-context API commands have different coverage; the observed extra WebGL command must not be silently dropped to align counts. These are submitted primitives before clipping/rasterization, not visible pixels. Full cold and warm API event counts, glTF upload-byte/call deltas and resource gauges are in RESULTS/raw evidence.

## Most expensive observed areas; hypotheses kept separate

Sparse, freshness-filtered CPU last-sample medians in the **separate profiled diagnostics**:

| Scope / case | WebGPU ms | WebGL2 ms | Interpretation |
|---|---:|---:|---|
| `gltf.queue`, static1024 |42.5|19.6|Grouping, instance packing and submission preparation are combined; their individual costs remain unavailable.|
| `gltf.update`, moving1024 |54.2|59.9|Motion/update path is an observed major CPU-wall scope.|
| `scene.gpuUpdates`, moving1024 |57.2|63.0|Parent includes glTF update; never add these two rows.|
| `render.encode.color`, static1024 |43.1|20.5|Parent includes queue work; never add to `gltf.queue`.|
| `render.encode.depth` |unproven|unproven|Unchanged historical scope cannot prove a recurring shadow render.|

These are sparse observed history changes, **not per-frame or per-invocation scope distributions**. Full callback/RAF raw distributions above are separate. WebGPU static-stress GPU `scene.main` mean55.642ms (10 samples), FXAA11.753ms (10); moving-stress scene21.987ms (5), FXAA3.486ms (5). WebGL EXT static-stress command interval mean285.303ms (7), p95/max439.218ms, includes CPU submission gaps/driver waiting/preemption and is not pure GPU busy time. Sparse GPU tail estimates are weak; no synthetic whole-frame GPU total is formed.

Warm API windows show static-stress buffer/texture/pipeline creations0 on both backends; WebGPU command encoders about3/frame are separate API events. Moving1024 shows WebGPU createBuffer/createBindGroup about2.79/frame and WebGL createBuffer3.72/frame in their separate short diagnostic windows; whole-API stress primitive proof remains incomplete on GL. Native clean moving-case page/buffer/bind-group creations around1.6–1.8/applicationRAF identify changing-page churn. These different windows/counters must not be substituted for one another.

**Untested hypotheses:** retaining prepared groups could reduce repeated static `gltf.queue`; finer pose/view invalidation could reduce moving `gltf.update`; bounded resource reuse could reduce moving page churn. They motivate the already ordered batches2–4. No implementation or speedup is attributed to any of them. High GPU main-pass times also require the separate isolation stage; a draw-count reduction alone does not establish its cause.

## Remaining limits and next work

Twelve accepted real Rust/WASM worker replays completed:3 each on WebGPU/WebGL2 at0 and1024 retained stress models. Both actual halves change pixels in both simulated eyes, separate, evaporate, disappear, and survive XR exit/reentry lifecycle checks. The simulator uses72Hz/320×320 eyes; its frame CPU is a diagnostic scope, not primary1280×720 throughput or physicalQuest performance.

| Actual worker replay | request→publication median / range ms (3 replays) | handler max ms | prepare call p95/max ms (18 samples) | simulated-frame CPU p95 / max ms |
|---|---:|---:|---:|---:|
| WebGPU normal |379.6 / 331.7–546.7|0.7|3.5/3.5|7.8/9.0|
| WebGL2 normal |181.2 / 180.2–238.6|0.6|2.7/2.7|32.2/58.7|
| WebGPU1024 |924.4 / 829.6–1119.3|0.7|3.4/3.4|96.1/98.7|
| WebGL2 1024 |985.1 / 937.7–1016.8|0.7|3.3/3.3|217.0/250.9|

`request→publication` includes worker scheduling, transfer, asynchronous installation and observation through the first simulated frame exposing uploaded pieces; it excludes GPU completion/compositor. It is **not worker-internal compute time**, which is unavailable. Message-handler last/max aggregates are separate from RAF; only3 replay aggregates (6 worker jobs/messages per replay) exist, so their apparent p95/p99 are not a meaningful per-message latency distribution. All72 accepted worker jobs returned72 messages with0 errors/expired/late/inflight at publication. Native acceptance, serialization/prepare, per-frame upload staging and pre-death/death frame records remain distinct in raw evidence. Already before death, stress frames have p95 about85.5ms(WG)/250.9ms(GL); the heavy rendering stall must not all be attributed to cutting. Last-frame upload-byte snapshots are gauges, not summed transferred bytes.

The first1024-stress attempt is retained as3 harness failures: the old correctness scenario removed the fleet before the cut; the new native-count guard correctly rejected observed0. Timing mode now retains all actual handles/count through publication; ordinary removal/pixel assertions remain unchanged. Distinct `stress-v2` labels preserve both attempts, and140 cut input-file hashes were checked against their exact test revisions. No renderer fix is claimed. Two completed browser replays left an owned Chrome crashpad process holding their diagnostic pipe; only the proven owned orphan was terminated after the browser had closed. The earlier failed Node process was terminated after its complete FAIL report; the command journal retains its signal.

Dedicated group-preparation versus instance-packing duration and whole-API upload bytes remain unavailable. WebGL XR context-copy has simulator coverage only. This cycle makes no physicalQuest claim, no Three.js parity claim, and no renderer optimization/speedup claim. Batch2 remains unstarted pending a separate instruction.

## Identity, environment and reproduction

The task's87447ea5 is7 commits behind the observed GitHub-main baselineb7de3be56. The primary checkout is olderffa2e6bb and dirty; its final status listing matches the initial listing. The repository inventory records21 worktrees and coordination-entry hashes. All edits/builds and ephemeral loopback test servers stayed in the isolated `codex/next-render-cycle-1` checkout. After all performance samples finished, an APFS copy-on-write clone of the prior completed target cache seeded this checkout’s own `target`; subsequent gate writes were isolated. During office measurements, shared8443 PID12350 bound192.168.0.17 while the Mac used192.168.11.134, so actualHTTPS bytes were initially unreachable. After the user resumed at home (192.168.0.17), the same server’s unchanged manifest and7 real WASM/SDK/Rome/worker payloads passed TLS/hash/no-store checks. Actual served releaseba920dd931/WASMc7091506 is DIFFERENT from baselineb5cc8eba. Nothing was replaced/restarted; the initial limitation and subsequent evidence are both retained.

AppleM1/7GPU cores/16GB, macOS26.6.1, bundled Node24.22.1arm64, Chrome156.0.8078.48, release rustc1.99.0. Aggregate device GPU utilization remained39–45% after every owned primary browser had closed; foreign Chrome GPU process30816 remained untouched. CPU process percent is not GPU busy percent, and no per-process GPU attribution is available. AC power was recorded, thermal state unavailable. The broad stress drift is retained rather than averaged into an optimization claim.

Actual historical phase commands (cwd isolated checkout; exact child argv/timestamps/statuses in archived `commands.json`). For reproduction, use a fresh checkout/output destination and the explicitly archived primary or diagnostic harness revision; do not rerun these hardcoded historical destinations over retained raw evidence:

```sh
export PATH="/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH"
export VERIFY_WASM_PROFILE=release
node scripts/freeze-render-build.mjs output/next-render-cycle/baseline
node output/next-render-cycle/run-cycle.mjs pilots
node output/next-render-cycle/run-cycle.mjs calibrate
node output/next-render-cycle/run-cycle.mjs primary
node output/next-render-cycle/run-cycle.mjs profiles
node output/next-render-cycle/run-cycle.mjs api64
node output/next-render-cycle/run-cycle.mjs cuts
node output/next-render-cycle/run-cycle.mjs cuts-v2
node --test scripts/render-performance.test.mjs scripts/render-performance-api.test.mjs
node output/next-render-cycle/summarize-cycle.mjs
python3 output/next-render-cycle/compact-results.py
npm run verify:push
```

The initial direct camera pilot (`pilot-camera-wg`) and direct API64 arena retry predate their orchestration phases; their report settings/logs are retained separately rather than inventing missing exact argv. Calibrations probe128/512/1024 with1s/2.5s pilots.1024 is the first completed over-budget probe for both, then frozen for primary stress; this is not a sustained-capacity proof for512. The failed first `cuts` phase is preserved; corrected stress uses distinct `stress-v2` labels. Initial helper default-Node abort and16MiB API overflows are retained as environmental/tool findings, not product speed results.

Large evidence lives in the compressed archive named/hashes in MANIFEST.json: all primary/profile/API/cut reports, raw.gz, images, command journal, audits, exact primary/diagnostic/final common-tool revisions, frozen hash manifest and release package. Frozen runtime source/assets reconstruct from baselineGit commit plus the full manifest; the complete2461-file local snapshot also remains under the owned output directory. No raw run is overwritten to make a gate pass. [RESULTS.json](RESULTS.json) is derived descriptive data, including unavailable values and completeness flags; raw artifacts remain authoritative. Final gate and checkpoint status are recorded in [STATE.md](STATE.md) and MANIFEST.json.

## Final verification and checkpoint

`npm run verify:push` exited0:419 native tests PASS (2 intentionally ignored manual timing benchmarks),137/137 browser scenarios PASS, SDK/harness/XR-platform/types and strict native/wasm checks PASS; source dump regenerated without a Rust-source delta.42 separate performance helper units PASS. The existing release-WASM bytes were reused, not rebuilt. No newCI or physicalQuest proof is claimed.

A subsequent direct-Node receipt check falsely missed: `verificationKey` serializes relevant `process.env` entries in insertion order. Direct order is `[BROWSER_USE_ACCESS_VERIFYING_IDENTITY,RUST_LOG]`; npm reverses it. Node version and hashed values are identical. Actual `npm exec --offline -- node scripts/verification-cache.mjs check all` returnsPASS with original receipt9508e96f… (direct-order key39061625…). Audits retain both orders/keys/value hashes. The cache implementation was not changed and no tests were repeated or evidence forced. This is a known tooling false miss; use the proven matching invocation for this checkpoint.

The evidence archive losslessly deduplicates identical payloads using standard tar hardlinks; every unique archived payload and every link’s target/hash/size was verified. All raw runs, failures and post-timing images are preserved. The local checkpoint belongs only to `codex/next-render-cycle-1`; no push, merge, deployment or subsequent batch is included.
