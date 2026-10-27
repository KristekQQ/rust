# Batch 2 — exact prepared opaque plans

## Result and evidence boundary

Accepted: unchanged opaque plans now bypass eligible primitive selection, group scheduling, common-basis proofs, instance-payload packing and prepared-row construction. Actual rendering still runs every frame. Existing pose/active-primitive and GPU page/stream caches are reused. Transparent preparation preserves its ordered uncached path. No shader, scene content, resolution, image quality, simulation or submission boundary changed.

Only batch2 is complete. This is a local isolated checkpoint, not a push, merge or deployment. The shared8443 package was not changed. Motion/VR fine invalidation (batch3) is unstarted. Head movement uses exact keys and misses; these static results do not prove an active-game or physical-Quest speedup. No fresh Three comparison was requested or performed in this batch.

## Frozen protocol and identity

Isolated `/private/tmp/rust-next-render-cycle-2`, branch `codex/next-render-cycle-2`, parent40b2a4ccc (completed batch1). The reused baseline runtime is b7de3be56 with verified release WASM `b5cc8eba529f7c3990b1a4eacbc16c13786ac774b8a366ad15eba6943340987a`. A separately frozen pre-cache instrumented release measures the six stages. Candidate release WASM is `af2905cc7d71af63b731c586198bd201917d63375821e80d1a70e7a99f1ea088`. Full source/bindings/SDK/tool identities and snapshot hashes are in [MANIFEST.json](MANIFEST.json) and the raw manifests.

Main clean experiment: actual authored Rome plus2048 shared static drone models, original gate camera and unchanged appearance/effects,1280×720,DPR1,seed1337,spatial order. A fixed-load baseline-only8s/25s calibration confirmed2048 over budget on both backends; configuration was then reused exactly. No busy-wait, readback or GPU completion wait occurs inside timing. Three AB/BA/AB pairs per backend, each8s warmup and25s sample. Each raw run has472–886 frame records;12 primary runs contain7807 records. Profiling is asserted disabled in both native boundaries and the GL timer; zero timing queries and no primary GPU times. Full raw distributions were independently recalculated and match reported means/medians/p95/p99/max.

All6 independent pair captures have byte-identical pixels (max channel delta0). Every primary run submits exactly307 glTF commands,126823 instance-events and39067116 triangles per RAF. These count primitive-instance events, not unique model handles. Both revisions already have zero warm glTF buffer uploads; the new improvement removes CPU preparation before the old byte-comparison/upload check. Main/compositor FXAA work outside glTF is unchanged.

## Clean whole-frame results

Numbers below are medians of the three per-run means. FPS is1000/mean RAF interval, not1000/median interval. Callback CPU means elapsed wall time in the application RAF, including driver waits/descheduling; it is not isolated on-core CPU computation.

| Backend | Callback wall ms, baseline → candidate | RAF interval mean ms | Observed mean-cadence FPS |
|---|---:|---:|---:|
| webgpu | 43.24 → 17.36 | 43.48 → 28.29 | 23.00 → 35.35 |
| webgl | 47.98 → 20.82 | 48.20 → 37.37 | 20.75 → 26.76 |

Median paired ratio of the *run-mean* callback wall times falls59.8% on WebGPU and55.2% on WebGL2. Ratio of the two group medians gives59.8% and56.6%; these estimators must not be mixed. All three pairs improve mean cadence, with median paired FPS gains53.7%/25.7% respectively. Frame-median quantization is material: two WebGPU pairs retain almost identical frame medians (~33.4→33.3ms), while their mix of16.7/33.3/50ms intervals and mean cadence improves. The driver’s original frame-median ratios are preserved in RESULTS.json.

Every run and unfavorable tail is retained:

| Backend | Pair/order | Revision | FPS from mean | Callback mean | Callback p95/p99/max ms | RAF median/p95/p99/max ms |
|---|---|---|---:|---:|---:|---:|
| webgpu | 1/AB | baseline | 18.80 | 52.83 | 69.2/70.5/71.8 | 50.1/66.7/83.4/100.0 |
| webgpu | 1/AB | candidate | 35.38 | 16.94 | 37.1/40.0/42.2 | 33.3/33.4/50.0/50.1 |
| webgpu | 2/BA | candidate | 35.23 | 17.56 | 39.1/44.2/47.1 | 33.3/49.9/50.0/50.1 |
| webgpu | 2/BA | baseline | 23.67 | 41.99 | 63.3/65.6/70.4 | 33.4/66.7/66.8/83.4 |
| webgpu | 3/AB | baseline | 23.00 | 43.24 | 65.5/69.0/72.0 | 33.4/66.7/66.8/83.4 |
| webgpu | 3/AB | candidate | 35.35 | 17.36 | 37.8/40.4/44.1 | 33.3/33.4/50.0/50.1 |
| webgl | 1/AB | baseline | 21.44 | 46.44 | 68.5/72.3/74.4 | 33.5/66.7/83.2/83.3 |
| webgl | 1/AB | candidate | 26.76 | 20.82 | 39.9/44.5/47.1 | 33.4/50.0/50.1/50.1 |
| webgl | 2/BA | candidate | 26.78 | 21.90 | 45.4/51.8/53.4 | 33.4/50.0/50.1/50.1 |
| webgl | 2/BA | baseline | 20.75 | 47.98 | 70.3/74.3/87.1 | 49.9/66.7/83.3/100.0 |
| webgl | 3/AB | baseline | 19.67 | 50.57 | 75.5/79.3/84.1 | 50.0/83.3/83.4/100.0 |
| webgl | 3/AB | candidate | 24.73 | 20.71 | 39.3/42.7/45.9 | 33.4/50.1/50.1/66.6 |

## Separate preparation diagnostics

Single4s/10s profiler-enabled1024-model pilots before/after the cache are diagnostics, not paired clean throughput evidence. Stage values are cumulative exclusive wall totals divided by measured RAF count. Instrumentation itself adds overhead, particularly inner-loop clock reads. The six stages isolate the requested five areas plus resource/pipeline/upload setup; resource API work is not attributed to PreparedDraw allocation.

| Stage | Pre-cache WebGPU ms/RAF | Cached WebGPU | Pre-cache WebGL2 ms/RAF | Cached WebGL2 |
|---|---:|---:|---:|---:|
| eligible | 0.7232 | 0 | 0.8371 | 0 |
| groups | 5.9669 | 0 | 6.7066 | 0 |
| basis | 2.8785 | 0 | 3.7124 | 0 |
| payload | 1.3478 | 0 | 1.6787 | 0 |
| records | 0.0739 | 0 | 0.0982 | 0 |
| resources | 0.6783 | 0 | 0.7459 | 0 |

The1024-model diagnostic records11.67/13.78ms of pre-cache exclusive rebuild phases per RAF. Warm candidate counters prove zero eligible/group/payload/record rebuilds, not merely zero uploads. Lookup still validates the exact visible-model list and creates its temporary model-reference Vec; this cost stays in parent `gltf.queue`. Sparse changed-history parent-queue samples average11.87→0.044ms(WebGPU),14.23→0.066ms(WebGL2). Those sparse samples are not per-invocation histograms or a clean-frame CPU subtraction. `prepareTimedCalls` counts full timed rebuilds; hit paths publish zero rebuild-stage history so cold timings are not silently reused. Disabled profiling performs no stage clock reads.

Adverse diagnostic preserved: the single WebGL1024 pilot has lower observed FPS after the cache (39.30→36.60) despite shorter callback wall time (23.79→11.21ms). It is an unpaired instrumented, GPU-contended observation; it is not discarded or presented as throughput improvement. The third clean WebGL candidate also drops to24.75FPS versus26.76/26.82 in earlier runs and remains in the primary table.

The largest remaining changed-history CPU wall scope in these candidate diagnostics is `render.encode.overlays`5.24/5.37ms(WebGPU/WebGL2), followed by `scene.gpuUpdates`1.45/1.52ms. These inclusive sparse scopes locate remaining work but do not isolate its cause. They are recorded, not refactored in this batch. Candidate WebGPU `scene.main` timestamps average16.34ms(n19) versus15.68ms(n17) before the cache; FXAA3.13/3.12ms. WebGL RAF query intervals average62.24ms(n13) versus50.92ms(n13). These adverse/no-GPU-gain diagnostic values are retained; the cache is a CPU-plan optimization, not a demonstrated reduction in GPU rendering work.

Warm primary candidate: exactly1 hit/RAF,0 misses,0 eligible/group/payload/prepared-record rebuilds,0 buffer uploads or new instance GPU pages/buffers/bind groups. Accounted retained plan storage102520 bytes (~100.1KiB), bounded by16MiB and64 fixed pass/view slots. Memory is conservative CPU element/active-entry accounting, not exact allocator/OS residency; fixed slot metadata/allocator overhead and existing referenced GPU allocations are separate. No new camera-matrix map or cached framebuffer image exists.

## GPU timing and environment limits

Primary GPU times are unavailable by protocol: profiling is off. Separate WebGPU diagnostic samples are valid asynchronous named pass timestamps; WebGL lacks native pass timestamps but has bounded asynchronous EXT query intervals around application RAF commands. Those GL intervals include command gaps/preemption and are not pure GPU busy time. Exact valid samples, rejected/pending query reasons and scope definitions remain in RESULTS.json/raw reports. `queue.submit()` CPU wall time is never treated as GPU time. CPU/GPU scopes are not mechanically added.

Environment is not isolated. The persistent browser GPU process30816 remains active (~24–28% process CPU at recorded boundaries), along with another preserved GPU process. Aggregate GPU snapshots across boundaries are mixed totals (~46–91%); immediate post-run statistics can include our recently completed work and must not be attributed solely to other tasks. No foreign process was terminated. The large repeated static work removal and observed timings are documented; isolated capacity, small gains or general backend superiority are not established. Hardware/browser metadata and timestamped process/GPU snapshots are archived.

Candidate-only ordinary stationary Rome controls (8s/25s, no profile) both reach60.00FPS; callback mean0.675ms(WebGPU)/1.158ms(WebGL2). They verify ordinary configuration and the cap, not paired improvement or headroom. No historical drifting batch1 FPS is used as the current candidate comparator.

## Validity, lifecycle and verification

Cache keys include exact camera/VP bits, culling, full format/depth/winding/clipping/mask/shadow/light-class pipeline state, scene revision, actual model-update generation, ordered model identities/generations/reorder policy, diagnostics and mutable glTF buffer-content epoch. Geometry/material assets are immutable per uploaded generation; replacements invalidate. Every glTF write_buffer increments the content epoch. Input mutation/removal/clear invalidates retained references; an empty cache does not rescan64 slots for each moving model. Resource handles alone never validate content. Every actual draw and existing uniform/submission/eye ownership remains active. See [DESIGN.md](DESIGN.md).

New both-backend test:27 cache-off exact-pixel oracles per backend cover warm zero rebuilds; independent root transform/basis/mirroring/options; tiny camera movement and left/right-like positions; culling/light class; add/remove; separately replaced material and changed vertex geometry; actual mirror rendering/shadows; SSR/bloom auxiliary targets; transparent depth reorder; morph/animation/skin palettes/active scenes; clear/dispose/reinit. Existing render-target/mirrored/effect tests exercise supported target formats; no public arbitrary-format hot-swap API exists. Direct WebGPU and WebGL simulated XR checks use real eye attachment pixels, prove both eye hits, and test left-only projection changes without corrupting the right eye. This is not a physical headset test.

One final `VERIFY_WASM_PROFILE=release npm run verify:push` exited0:419 native PASS,2 existing manual timing tests ignored;139/139 browser scenarios PASS;32 SDK and59 XR-platform units plus harness/type/strict native+wasm checks PASS.42 separate performance helper units PASS. Existing Rome worker-cut/spawn/wave and Neon arena regressions passed within the maintained gate; no artificial worker timing is relabelled RAF CPU. No worker-handler scheduling code changed. One independent static implementation/test review found no material defects; it did not run tests. Detailed identities/check logs: [VERIFICATION.json](VERIFICATION.json).

Failures were repaired or environmental and retained: initial instrumentation macro recursion and compile errors; sandboxed wasm tool-cache failure; release metadata first recorded as dev despite a real release build (corrected to release using unchanged WASM bytes); Node native abort in a loopback unit when sandbox denied binding (identical units pass with local server permission). No assertions/reference pixels were weakened, no failed primary run was dropped.

## Reproduction and storage

Exact argv/cwd/start/end/exit codes: `measurement-commands.json` in the evidence archive. Representative actual commands:

```sh
export VERIFY_WASM_PROFILE=release
wasm-pack build --target web --release -- --locked
node scripts/freeze-render-build.mjs output/next-render-cycle-2/candidate
node scripts/render-performance.mjs calibrate --baseline-root output/next-render-cycle-2/baseline --backend webgpu --family rim-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --no-profile --out output/next-render-cycle-2/calibrate-webgpu
node scripts/render-performance.mjs compare --baseline-root output/next-render-cycle-2/baseline --candidate-root output/next-render-cycle-2/candidate --config output/next-render-cycle-2/calibrate-webgpu/config.json --backend webgpu --family rim-stress --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --no-profile --out output/next-render-cycle-2/pairs-webgpu
node scripts/test-browser.mjs --backend all --page gltf-plan-cache,webxr,webxr-webgpu
npm run verify:push
```

The actual build additionally sets `RUSTFLAGS=--cfg=web_sys_unstable_apis`; commands use the configured bundled arm64Node. WebGL commands use the separately calibrated WebGL config. Source/pkg/SDK and every requested served payload/common-harness hash were rechecked. All2461 baseline,2473 instrumented-baseline and2475 candidate snapshot files were checked against their frozen manifests; actual counts are authoritative in RESULTS.json if this prose is revised. Immutable runtime/assets remain in local `output/next-render-cycle-2/{baseline,instrumented-baseline,candidate}`. Raw frames, captures, manifests, adverse runs, environment snapshots, gate/XR reports and logs are stored in [evidence.tar.gz](evidence.tar.gz), with hash/inventory in MANIFEST.json. Versioned RESULTS.json is compact; detailed scopes/work counters are archived outside STATE.md.

Next action: stop. Only an explicit batch3 instruction may start fine motion/VR invalidation.
