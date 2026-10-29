# Nine-batch renderer cycle — final integration evidence

**Evidence collection/report complete. Unconditional exact-image integration signoff is BLOCKED:** one candidate-only camera capture differs6pixels/max2; the historical B4 image issue also remains open. No renderer change was made in B9. No push, main merge, deployment or shared-server restart. The series stops here.

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


## Protocol and environment

Apple M1 desktop, Chrome156.0.8078.48 hardware rendering,1280×720/DPR1, seed1337/spatial ordering. Release WASM and exact frozen SDK bytes, not a newly deployed/minified Quest package. Primary protocol:8s warmup/25s sample,3AB/BA/AB pairs independently per backend. Actual2048-drone stress was calibrated on original B1 before candidate observations; no busy wait, quality/content reduction or cached final image. Dynamic64 samples60s; pool256 samples120s with one pair/backend (memory/lifetime soak, not a three-repeat speed claim). [Fixed protocol](batch-9/PROTOCOL.md), [actual commands/reproduction](batch-9/COMMANDS.md).

Original ordinary/static/camera/cluster/Neon primaries were on battery; after the user resumed on AC, dynamic/pool/Three/worker/diagnostics ran on AC. The pause was between whole cases. Host per-run endpoints showed no within-pair source inconsistency, so the predeclared bounded confirmation rule requested zero reruns. Endpoints are not continuous power monitoring. Sandbox/host pmset disagreement and the retracted physical-transition inference are retained. A foreign Chrome GPU process was preserved: this is not an isolated GPU/thermal experiment, and process%CPU is not GPU utilization. Small deltas cannot establish general causal gains.

Clean primary/Three runs assert actual profiling:false, zero native GPU samples and disabled GL queries; cold pre-init observers separately observed zero query creations. Profiling/API runs are separate pages and excluded from clean results. CPU callback wall time includes driver/browser waits and observer work, not pure CPU computation. Native pass timestamps and GL elapsed command intervals have different boundaries; neither is compositor latency/pure GPU busy time. Do not add them to CPU times.

## Fresh native results

88clean original-B1/final runs contain142,721RAF records. Including28instrumented and18clean Three runs:134runs/193,076records. Every full-series mean/median/p95/p99/max and capture hash was independently recomputed with zero metadata errors. [Each run and all tails](batch-9/RUNS.md), [identities/quality/counters/per-run results](batch-9/RESULTS.json), [GPU/API/pool/worker/cold details](batch-9/DIAGNOSTICS.json).

The following values are medians of per-run mean-cadence FPS and callback statistics; maxima are across runs. Ratios of frame medians are not substituted for FPS. Pool rows have only one run/revision/backend. GPU time is unavailable in these deliberately unprofiled runs.

| Family/load | Backend | FPS B1→final | Callback mean ms | Callback p95/p99 ms | Callback max ms |
|---|---|---:|---:|---|---:|
| rim-static/1 | webgpu | 60.00→60.00 | 0.745→0.683 | 1.10/1.30→1.20/1.40 | 3.00→2.50 |
| rim-static/1 | webgl | 60.00→60.00 | 1.086→1.165 | 1.60/1.80→1.70/2.00 | 4.80→2.90 |
| rim-active/1 | webgpu | 60.00→60.00 | 2.144→2.118 | 2.90/3.10→3.00/3.30 | 12.60→5.50 |
| rim-active/1 | webgl | 60.00→60.00 | 2.836→2.907 | 3.80/4.30→3.90/4.30 | 5.00→5.40 |
| rim-stress/2048 | webgpu | 27.56→37.66 | 36.044→14.953 | 51.00/57.80→30.80/32.50 | 65.20→41.00 |
| rim-stress/2048 | webgl | 26.66→29.02 | 37.313→17.357 | 52.70/57.90→30.90/34.20 | 86.50→46.90 |
| rim-camera-stress/2048 | webgpu | 30.00→30.00 | 23.573→19.722 | 35.40/37.60→31.30/34.10 | 49.00→74.00 |
| rim-camera-stress/2048 | webgl | 30.00→30.00 | 24.784→20.795 | 36.80/39.00→32.70/35.00 | 46.10→68.80 |
| rim-clustered/1024 | webgpu | 25.86→25.52 | 38.441→38.941 | 47.50/50.40→47.40/50.80 | 60.50→61.80 |
| rim-clustered/1024 | webgl | 24.62→24.75 | 40.416→40.248 | 49.50/51.70→49.30/54.20 | 64.70→66.20 |
| dynamic/64 | webgpu | 60.00→60.00 | 0.862→0.984 | 1.40/1.70→1.60/2.10 | 10.40→10.90 |
| dynamic/64 | webgl | 60.00→60.00 | 1.236→1.376 | 1.80/2.30→2.20/2.80 | 8.40→7.00 |
| rim-pool-cycle/256 | webgpu | 60.00→60.00 | 3.174→2.325 | 6.70/8.60→6.50/11.90 | 24.40→23.30 |
| rim-pool-cycle/256 | webgl | 60.00→60.00 | 3.666→2.932 | 7.10/10.30→6.90/12.40 | 31.40→27.90 |
| arena-active/1 | webgpu | 30.00→30.00 | 1.023→1.222 | 1.50/1.80→2.10/2.40 | 4.10→5.20 |
| arena-active/1 | webgl | 30.00→30.00 | 2.340→2.454 | 3.60/4.20→3.70/4.20 | 6.50→13.30 |

Static stress improves observed cadence, with exactly307native glTF commands and39,067,116submitted triangles/RAF on both builds/backends. Both already upload zero glTF instance bytes when static; the cache removes repeated CPU construction rather than only uploads. Final warm group/payload rebuilds are zero in this workload. Global/non-glTF uniforms are a different scope, so this is not zero total renderer upload.

Camera stress retains roughly267prepared rows/RAF, but still rebuilds view-dependent groups/payloads. WebGPU instance upload increases from about1,330,282 to1,487,989B/RAF: reservations and a union dirty span include unchanged gaps. Instance bytes overlap uniform-byte counters; do not sum them. Fully moving clustered1024 is approximately neutral (including adverse WG cadence); dynamic64 stays60Hz but final callback means/p99 are higher. Static cache success is not a solution for active-game/VR throughput.

The ordinary native gameplay control has six genuinely moving orbiters, advancing simulation/visual frames and a stationary player. No attacks are scripted; projectile-fired/kills remain zero in these samples. Native playing camera overrides the JS requested orbit: this is a native-gameplay-camera control. Separate clustered models have real animated drone geometry but no extra AI. Dynamic64 is an explicit native closed-mesh cut/spawn/despawn scenario, not the worker/AI workload. Its12minutes of samples perform372verified cuts, each with two pieces and matching removals.

Neon clean battery runs show30Hz cadence, while later AC instrumented controls show60Hz. No explicit30Hz application cap was established. Different power/profiling conditions prevent attributing this to a build or extrapolating GPU headroom. Neon keeps original glow/SSR and has no glTF draw work: zero glTF counters never mean zero scene rendering.

## Three reference — controlled scope, not blanket superiority

Pinned vendored Three r170 SHA256 `ce1fa418de16a19495a9f72495580e3015d7745c296d3ce0485897f902ddedfb`; existing67-part InstancedMesh representation is used, not naive clones. Common2048 uses identical city/drone GLBs, Idle@0, camera/scale/viewport, AAoff/lights0/shadows0/fogoff/no-tone/postoff/sRGB. Native uses its explicit diagnostic base-color variant. This is a limited common profile, not full PBR/gameplay parity. Actual submission differs: native39,061,900triangles/287main commands versus Three42,009,212/86. Do not normalize this into an invented equal-work win.

| Control | Native FPS / callback mean ms | Three GL FPS / callback mean ms | Native/Three main commands |
|---|---|---|---:|
| three-common-webgpu | 41.519 / 7.123 | 35.465 / 0.553 | 287/86 |
| three-common-webgl | 36.655 / 8.602 | 35.185 / 0.564 | 287/86 |
| three-production-webgl | 41.014 / 9.193 | 42.722 / 3.099 | 240/108 |

Each clean row has3AB/BA/AB pairs. Native CPU wall overhead remains much higher even where common-profile cadence is higher. The production1024control is intentionally unmatched: Three uses MSAA4/ACES exposure1.25/PCFSoft shadows/fog versus native FXAA/custom PBR/shadows. It cannot quantify a remaining full-quality renderer gap. Cross-renderer pixel comparisons are diagnostics with pass:null, never native regression PASS; zero-tolerance differences are separately recomputed in PIXEL_AUDIT.json.

Actual captures visually reviewed: [common native](batch-9/common-native-webgpu.png), [common Three](batch-9/common-three.png), [production native](batch-9/production-native-webgl.png), [production Three](batch-9/production-three.png), [ordinary native enemies/swords/HP](batch-9/active-native-webgpu.png). Common layout/content match visibly, but edge pixels differ. Production shading/AA differences are visible; no claimed perceptual equality or headset comfort result.

## Costs observed after integration

Separate instrumented stress2048 WG observations: glTF queue/preparation changed-history mean22.716→0.093ms; render.overlays.select remains8.557ms in final. Main GPU pass27.743→27.078ms, FXAA4.162→3.391ms. This supports removal of CPU preparation, not a proven GPU speedup. Scope histories are sparse changes sampled around100ms, not complete invocation distributions or additive components. Full callback tails come from all RAF records.

WebGL stress asynchronous elapsed command interval is50.748→80.733ms, an adverse observation retained despite higher clean final FPS. Valid elapsed queries include submission gaps/scheduling and cannot be treated as pure GPU busy time. Current common-profile native WG scene pass26.710ms and Three GL interval40.020ms have incompatible boundaries; native GL/Three intervals42.037/40.166ms are separate instrumented observations, not the clean table.

Neon WG final sampled scene.main0.933ms, SSR trace1.486ms, fused final/post/overlays4.790ms. Full API observer sees48commands/17,274triangles per RAF, while native main snapshot is46and glTFdraws0. Whole-API warm GPU buffer creations remain194baseline/192final over25s: non-glTF work still allocates. GPU pass coverage is not complete GPU utilization, and these AC observations cannot decompose earlier battery30Hz. All valid sample counts/tails/discards and unavailable fields remain in DIAGNOSTICS.json.

## Resource retention, actual allocations and geometry

The120s pool256 cycle repeats many→few16→none→many at0.5s phases (241transitions per run). Both final backends retain exactly622,512known buffer-capacity bytes across all four30s quarters; fine-cache bytes range0–83,448. After8s warmup, final creates zero instance buffers/bind groups versus B1 8,100/8,040 per backend. This is bounded engine-accounted retention, not physical VRAM/process/WASM memory. Inactive pool64MiB/4096handle policy excludes unavoidable active/pinned working sets and driver overhead. Clear/reload/dispose/pressure/layout/XR lifecycle are covered by the fresh maintained gate.

Pool mean callbacks improve, but p99 worsens: WG8.6→11.9ms; GL10.3→12.4ms. Maxima improve only in these single-pair soaks (24.4→23.3WG;31.4→27.9GL). No general p99/stutter gain is established. Separate actual API allocations and frames with/without allocations are associated observations; visibility/group changes co-occur, so they are not isolated allocation-cost experiments.

| Pool25s separate API observer | RAFs | Actual createBuffer | Actual createBindGroup | Allocation-frame callback p99 ms |
|---|---:|---:|---:|---:|
| webgpu/baseline | 1500 | 1620 | 1608 | 22 |
| webgpu/candidate | 1500 | 0 | 0 | no observed allocation frame |
| webgl/baseline | 1501 | 1620 | N/A GL API | 21.299999952316284 |
| webgl/candidate | 1500 | 0 | N/A GL API | no observed allocation frame |

WebGL has no browser createBindGroup API; native wgpu logical creations remain separately counted. Missing API/triangle proof is null/unavailable, not zero. Some bounded GL index-shadow observers become incomplete; their partial triangle totals must not be quoted as exact. Native glTF submitted counts remain available. glTF buffer upload bytes/calls and whole-API upload call counts are reported separately; full-engine texture/upload-byte traffic is unavailable. Requested allocation capacity is not measured transfer traffic.

Historical isolated B5/B6 evidence: Rome258eligible primitives save16B exactly-white attribute per vertex (9,417,848→5,650,760requested rigid vertex bytes) and index bytes1,479,384→739,692. Combined initial rigid vertex/index payload10,897,232→6,390,452B; CPU cutting geometry is retained. Current final counters confirm these layouts/capacities. They do not imply proportional FPS gains. [Intermediate accepted/rejected evidence and checkpoints](batch-9/INTERMEDIATE.md).

## Cold variants and real worker publication

Repaired instrumented cold probe12/12PASS (3AB/BA/AB fresh pages/backend), actual profiling:false and pre-init query observers zero. Original7/12capture-tool failures remain retained; repair copies immediately after the actual engine callback outside timing, without changing threshold/scene. Browser/driver cache is reused; second variant shares an engine; assets can draw before load resolves. These are load/first-post-load/warm callbacks, not guaranteed first-ever shader compilation or driver-cold GPU timings. Zeros reflect timer resolution, not zero cost.

| Variant/backend | Load median ms B1→final | First post-load callback ms | Warm callback median ms |
|---|---:|---:|---:|
| white-helmet/webgpu | 1610.60→1614.60 | 0.00→0.00 | 0.10→0.10 |
| colored-rgba/webgpu | 3.00→2.50 | 0.00→0.00 | 0.10→0.10 |
| white-helmet/webgl | 1668.90→1696.50 | 129.30→124.50 | 0.20→0.10 |
| colored-rgba/webgl | 3.60→3.40 | 17.20→13.90 | 0.10→0.10 |

All12real-worker simulated-XR replays PASS with64stress models,72Hz simulation and320×320eyes. Each build/backend has3replays, not three alternating throughput pairs. Request→publication runs from first nonempty rimTakeCutJob through the first frame exposing uploaded halves; completion-from-kill has different observation boundaries. Neither includes GPU completion/compositor; worker internal compute is unavailable. Nine staged installation samples/build/backend and three handler maxima make tails descriptive small-n observations.

| Backend | Request→publish median ms B1→final | Worst observed request→publish ms | Handler max ms B1→final |
|---|---:|---:|---:|
| webgpu | 172.4→174.8 | 174.5→176.4 | 0.4→0.3 |
| webgl | 193.2→196.2 | 193.7→207.4 | 0.3→0.4 |

No publication-latency improvement is established. Install/accept/job-serialization/frame costs are separate in DIAGNOSTICS.json. A deferred effect is not labelled improved response, and these are actual Rust cuts in a worker, not parallel rigid-body physics/Quest measurements.

## Exact pixels and acceptance blockers

Fresh native A/B comparisons:39/40exact,16ordinary-gameplay/Neon pairs explicitly lack a deterministic capture oracle;11Three pairs are non-gating cross-renderer diagnostics. Independent PNG decoding reproduces every recorded pixel statistic, with zero metadata errors. The earlier initial24/24prose was wrong: original report, backup and committed RESULTS-PARTIAL.json already contained the camera failure. Data/tolerances were never altered. [Pixel audit](batch-9/PIXEL_AUDIT.json), [explicit correction](batch-9/PIXEL_CORRECTION.json).

**Camera2048-WebGPU pair1:**6pixels/max2/255 differ only in candidate pair1; all3baseline and candidate pairs0/2 have identical decoded RGBA. Baseline noise is not established here. Independent targeted source review found no specific omitted dependency. [Coordinates/source audit](batch-9/camera-mismatch-audit.md). This remains an unresolved candidate-only history-dependent capture difference; no universal image-equivalence claim.

A bounded stronger oracle with16deterministic camera steps/three fresh sessions compared cached→cache+culling-off→restored and consecutive captures. Consecutive/restored images were exact; combined-off differs5,014pixels/max79 in all three sessions. Follow-up axes isolate it: cache-off alone is exact; culling-off differs5,014/max79 in both original B1 and final. Original/final are exact for matching on/on, off/off and restored settings. Thus this wider difference predates the new caches and is not evidence of their regression. Culling also changes light-frustum/shadow selection, so this test does not isolate main-camera visibility. Its underlying culling/shadow/numerical cause remains unproven; it is still a failed broader equivalence control, not a PASS. It is disjoint from the six original pixels. [Oracle records](batch-9/camera-oracle.json), [separated axes](batch-9/camera-isolation.json), [source/spatial review](batch-9/culling-oracle-review.md).

Historical B4 moving1024-WebGPU8pixels/max2 and independent1/3exact remain separately open; later tests do not close it or prove the same cause. No tolerances/reference pixels were relaxed. The final code gate passes, but unconditional exact-image signoff is BLOCKED. No new renderer candidate was introduced in B9 to accept/revert; the inherited final build is retained for investigation, not represented as unconditionally ready to ship.

## Fresh verification and evidence

`VERIFY_WASM_PROFILE=release npm run verify:push` ran once on unchanged final code, exit0:428Rust PASS/2existing manual timing tests ignored,32SDK,71XR-platform,14Node+13Python harness,149/149browser PASS (55WebGPU/94WebGL). Strict native/WASM Clippy, formatting/types and source dump were included. Separate49performance helper units, new-script syntax and skill validation PASS. Browser was hardware Chrome156, sandbox not disabled. These are fresh counts, not historical418/137. [Actual verification](batch-9/VERIFICATION.json).

Final assertFreshBuild() passed for original/final/current; final=current source/WASM/bindings/configuration. All296/297frozen runtime files and297current runtime files match. Accepted served/driver/worker/oracle inputs were rehashed:2,377checks/142distinct files/zero errors. The content-bound all PASS receipt was rechecked without rerunning tests; docs/evidence updates do not change code inputs. Source dump regenerated and byte-identical to parent. No production package/server replacement is implied. [Source identity](batch-9/final-source-audit.json), [runtime audit](batch-9/final-frozen-runtime-audit.json), [current runtime](batch-9/current-runtime-audit.json), [served audit](batch-9/FINAL_INPUT_AUDIT.json).

Compressed deduplicated raw/config/log/failed-attempt/runtime/source/harness evidence: [archive](batch-9/EVIDENCE.tar.gz), [path→content-hash index](batch-9/EVIDENCE_INDEX.json); archive identity is in MANIFEST.json. Every unique object and alias is reverified after writing. No recursive embedding of older batch archives. Historical reports retain separate archives. Original failed cold-capture source bytes were not frozen (hash/results retained); one overwritten initial1024pilot raw series is unavailable/excluded. The new output-refusal regression prevents overwriting primary evidence.

## Nine-batch disposition

| Batch | Local result | Remaining limit |
|---|---|---|
|1|Accepted baseline/measurement infrastructure|No speedup claim; original report/identity retained.|
|2|Accepted exact opaque plan cache|Static preparation benefit; not global/game/VR proof.|
|3|Accepted bounded fine invalidation/fallback|Moving-camera/sparse benefit; broad image issues remain open.|
|4|Accepted bounded inactive resource retention|Allocation proof; general p99/stutter gain unproven.|
|5|Accepted exact-white attribute omission; varying removal rejected|Memory/upload savings; original interpolation retained.|
|6|Accepted portable Uint16 and valid point/line shadow bias|Memory/upload savings; no isolated FPS proof.|
|7|Production experiments rejected; diagnostics accepted|Depth reorder changed pixels; count reuse repeatedly regressed cadence.|
|8|Explicit opt-in prototype, device-unverified|Canvas-transfer default; ordinary symmetric callback regression; no Quest signoff.|
|9|Evidence/report/checkpoint delivered; unconditional visual integration signoff BLOCKED|Camera6pixels and inherited image limits; no new renderer optimization.|

Every batch remains unverified on physical Quest for the final build. Real opaque headset FBO/context-loss behavior, comfort/FPS, full production-quality Three equivalence, physical VRAM/process/WASM memory plateau and compositor/pure GPU-busy latency are unavailable. CI, push, merge, production deployment and shared8443 refresh were not performed. Desktop/simulated XR evidence does not supply those facts.

## Local delivery and next priorities

All accepted B1–B8 checkpoints remain in the branch chain; [stage checkpoints/evidence](batch-9/INTERMEDIATE.md) and MANIFEST.json identify them. B9 stable-code recovery checkpoint `0fcf7b1589bcf323a5cc65eb5c41289d9db03123`; final evidence checkpoint is the commit containing this report (parent0fcf7b1). The isolated diff owns only B9 runner/guard/cold probe/tests/docs/skill/evidence, not another task or production runtime. Source Rust/SDK/gameplay changes in B9: none. Main dirty work and foreign processes were preserved.

Next separately authorized priorities: (1) reproduce/isolate the six camera pixels with capture-time dependencies and retain the separate legacy culling/shadow oracle issue; (2) remove demonstrated overlay selection CPU work under fresh exact-pixel/throughput proof, without blindly restoring rejected count reuse; (3) investigate remaining non-glTF Neon warm allocations and camera union-span upload overhead; (4) validate matched settings on physical Quest before enabling shared-GL by default. No next task starts automatically.

Final evidence archive:102651442 bytes, SHA256 `9748a3d8962c01f8c32f532fe45916b6b381c8c1dee479fb74f20f15b4922aa9`;1285 unique objects/2142 aliases verified, all2142 restored independent files rehashed without error. [Archive verification](batch-9/ARCHIVE_VERIFICATION.json). Full foreign-process command lines remain local; exported role-only records retain original-input hashes. [Independent report review](batch-9/final-report-review.md) found no material numerical/causal/scope defect in its documented inspection; archive validation was performed afterward separately.
