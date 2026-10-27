# Reproducible renderer performance

Use [the project skill](../.agents/skills/wasm-render-performance/SKILL.md), then [current state](performance/STATE.md) and [experiment journal](performance/EXPERIMENTS_2026-10-25.md). The separate cut replay uses fixed simulated XR72Hz inputs and actual native combat, workers and staged publication; it is not a physical Quest measurement. Its onmessage timing is main-thread handler CPU, not internal worker compute duration. The driver is `scripts/render-performance.mjs`; `scripts/benchmark-core.mjs` keeps its old interface and forwards the new mode names. Run `node scripts/render-performance.mjs --help` for all bounds and workload families.

## Freeze and repeat

Use a clean owned worktree for substantial changes. Build a verified release using the repository Rust toolchain and existing development workflow. `VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/perf/baseline` refuses an existing destination, checks source/WASM/SDK identity, copies runtime/assets/source/tools and verifies copied hashes and input stability. Freeze baseline before optimization, candidate after its release build. Do not regenerate another task's live production package. Run all commands below serially, with no builds or other GPU runners in parallel.

```sh
# A short actual-render smoke. This is not performance acceptance.
node scripts/render-performance.mjs smoke --baseline-root output/perf/baseline --family batching --backend webgpu --load 64 --warmup 500 --sample 2500 --out output/perf/smoke
# Baseline-only bounded calibration: light/near/over, fixed camera/seed/quality.
node scripts/render-performance.mjs calibrate --baseline-root output/perf/baseline --family batching --backend webgpu --max-load 6144 --warmup 1000 --sample 2500 --out output/perf/calibration
# The primary comparison: three AB/BA/AB pairs; actual profiling disabled/asserted.
node scripts/render-performance.mjs compare --baseline-root output/perf/baseline --candidate-root output/perf/candidate --config output/perf/calibration/config.json --level over --max-load 6144 --pairs 3 --no-profile --warmup 8000 --sample 25000 --out output/perf/paired-off
# Profiled scopes and a separate API-instrumented cold/warm page.
node scripts/render-performance.mjs compare --baseline-root output/perf/baseline --candidate-root output/perf/candidate --config output/perf/calibration/config.json --level over --max-load 6144 --pairs 3 --warmup 4000 --sample 10000 --diagnostics --out output/perf/profiled
# Same exact calibrated load list and criterion on both versions. Short runs are exploratory.
node scripts/render-performance.mjs capacity --baseline-root output/perf/baseline --candidate-root output/perf/candidate --config output/perf/calibration/config.json --max-load 6144 --pairs 3 --warmup 8000 --sample 25000 --max-time 7200 --out output/perf/capacity
# A long repeated-cut/clustering run, using its own dynamic-family calibration.
node scripts/render-performance.mjs soak --baseline-root output/perf/candidate --config output/perf/dynamic-calibration/config.json --max-load 512 --warmup 2000 --sample 120000 --out output/perf/soak
# Regression checks: pixel tolerance plus median cadence/CPU threshold (defaults5%).
node scripts/render-performance.mjs regression --baseline-root output/perf/baseline --candidate-root output/perf/candidate --config output/perf/calibration/config.json --level over --max-load 6144 --pairs 3 --no-profile --out output/perf/regression
# Independent real cut worker/prepare/installation/commit and visible-halves evidence.
node scripts/benchmark-rim-cuts.mjs candidate-cuts output/perf/candidate/pkg webgpu
```

These are executable usage examples; they are not a claim that each example command above was executed with those paths. The versioned result archive contains the exact commands actually run. Repeat calibration with `--backend webgl`, then reuse that backend's configuration. Changing a workload/frozen baseline/harness requires new calibration. Comparing a different candidate does not. `--no-profile` is an explicit control override, recorded separately from the profiled calibration. A missing overload level means the bounded calibration never observed one; do not manufacture it.

## Workloads and validity

- `rim-static`, `arena-static`: actual imported scene modules with authored appearance/settings and paused simulation; fixed authored load. `rim-active` drives native gameplay input/camera by elapsed seconds. `arena-active` enables native simulation and drives authored fruit poses by elapsed seconds. These control paths are not a complete deterministic human combat replay.
- `batching`: actual visible generated glTF quad instances, shared asset, fixed6×4 grid and camera,1024-instance pages. `batching-heterogeneous` alternates two distinct asset/material identities. `primitives`/`heterogeneous` cover other command paths. Requested instance submission/zero culls are asserted for batching.
- `rigid`: shared DamagedHelmet instances; `skin`: real Fox GPU skinning/seek; `morph`: actual morph fallback fixture. Counter reports distinguish submitted/culling work.
- `fragment`, `effects`: explicit overlapping geometry/fragment stress, effects enabled only for the effects family. This diagnostic overdraw is not normal gameplay.
- `dynamic`: elapsed clustering/dispersing changes instancing compatibility, plus real closed-mesh spawn/cut/despawn every2s with one removed target/two pieces asserted. A separate X drift (`4.5*sin(seconds*0.35)`) crosses the frustum edge. Phase boundaries and glTF primitive culls are recorded; top-level root culls can occur earlier and must be inspected separately. The current harness is freshly calibrated.
- `--order spatial|shuffled|alternating`, `--seed`, `--overlap` change insertion/spatial organization reproducibly. They are calibration identity fields.

Viewport960×640 DPR1, backend/AA/quality retained. Load and timing limits are declared in the report. The memory abort measures JS heap only; total GPU/process allocation is unavailable. Startup uses DOMContentLoaded plus explicit readiness with a single deadline. Driver and actual requested bytes are hashed, and mutations during a run fail it. Page errors, fallback backend, runtime losses, unsupported workload results and extreme frame times are failures with retained diagnostics. A native test/build is not rendered-content acceptance.

## Timing and counters

`report.json` contains per-run median/mean/p95/p99/max, tolerance-aware and exact overruns, missed intervals, actual backend/profile state, work deltas, gauges, hashes, cold readiness, errors and image differences. `raw.json.gz` retains actual RAF records and distinct sampled CPU/GPU snapshots. Adjacent frames are not independent experiment repetitions. CPU scopes are existing rolling profiler samples taken roughly10Hz, not per-frame phase histograms. Native histories can retain cold-only values indefinitely. Use `cpuScopeEvidence` to retain only observed history changes relative to the window boundary; unchanged histories have unproven freshness, not zero cost. Identical remeasurements and untouched histories are indistinguishable, and intermediate invocations are unavailable; even filtered values are sparse last-sample changes, not per-invocation histograms. RAF CPU includes application RAF callbacks, not timer/message/worker handlers. Cold readiness includes page/network/setup and excludes neither OS nor persistent driver caches.

WebGPU GPU values are valid asynchronous sampled named pass intervals. They do not establish whole GPU frame latency. Desktop WebGL native pass profiling is unavailable; the actual supported EXT timer records a separate application RAF GPU command interval, which can include CPU submission gaps/driver waiting, excludes compositor/between-RAF/worker commands and is not pure GPU busy time. Queries are bounded, never force completion and reject stale/nonfinite/disjoint samples. Profiler-off runs assert disabled engine profiling and no GL timer queries. `gpuAvailable`/`gpuSupported`/`gpuSamples` refer to the native named-pass profiler; WebGL EXT support/samples are separate under `glRafGpu`/`glRafGpuMs`. Missing timing is null. CPU, GPU, RAF cadence and cut-completion latency have different scopes and must never be added mechanically.

Actual glTF cumulative event counters are differenced at measurement boundaries: uploads/calls/bytes, primitives/culls, submitted draw commands/instances/triangles, overlap tests and page/resource creations. Unsupported old-baseline fields remain null. Resource gauges and retained scratch bytes are not event counts. `instancePagesActive` sums last active counts per view/pass key. Separate API diagnostics count actual whole API calls, including successfully resolved async pipeline creation, and are excluded from primary timing. Cold counters cover initialization+1s. A distinct warm window starts after4s and differences counters over3s; its actual elapsed time and engine-frame delta are recorded. Counts describe wrapped API events, not physical driver allocations. Derived whole-API primitive counts carry completeness flags: indirect/bundle/unknown index ranges remain incomplete. WebGL proves index ranges from bounded CPU upload shadows (64MiB data+mask cap; separate metadata/proof bounds), never GPU readback. No wrapper claims complete driver/GPU memory ownership.

Pixel readback occurs after timing. Same-renderer static captures gate regressions; active gameplay has no deterministic reset and explicitly reports an unavailable pixel gate. Maintained browser tests supply independent unbatched pixels, overlap/ordering and simulated-XR/cut correctness. Synthetic benchmark capture alone is insufficient.

## Three.js and XR

A local r170 module is frozen under a separate reference root. `compare --three-root ROOT` requires `batching`, `webgl`, and `--no-profile`; it uses the exact visible quad geometry/load/camera/grid/elapsed poses and verifies actual calls/triangles. `--three-representation instanced` uses pre-established 1024-instance groups; `objects` creates independent root/mesh pairs and normally issues one draw per root. Both modes assert actual commands/triangles. Shading/lighting and engine/SDK responsibilities differ; Three pixel differences are diagnostic, not a false parity PASS. This does not prove equivalent full Rome gameplay or a universal20% renderer gap.

Desktop and simulated XR are separate from physical Quest. Record actual eye sizes, Hz, AA, foveation, layer and intermediate targets before any matched headset comparison. WebGL second-context canvas upload/per-eye blit and WebGPU owned-eye staging remain separate investigations; do not discard correctness fixes or mix their effects into a desktop batching result.

Historical assumptions and earlier timings remain in [the dated legacy note](history/RENDER_PERFORMANCE-2026-10-25-legacy.md).
