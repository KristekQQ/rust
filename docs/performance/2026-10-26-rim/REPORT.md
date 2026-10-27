# Actual Rome renderer progress — 2026-10-26

The real authored `examples/rim` does not yet demonstrate a substantial improvement. Small CPU changes are mixed across repetitions. The large improvements on thousands of synthetic instances do not transfer automatically to this scene.

## Matched primary measurements

| Backend / workload | CPU medians before (ms) | CPU medians after (ms) | Median paired after/before ratio |
|---|---|---|---|
| WebGPU / static | 0.60 / 0.70 / 0.70 | 0.70 / 0.70 / 0.70 | 1.0000 |
| WebGPU / moving enemies | 2.20 / 2.20 / 2.20 | 2.10 / 2.10 / 2.20 | 0.9545 |
| WebGL2 / static | 1.40 / 1.30 / 1.50 | 1.30 / 1.30 / 1.40 | 0.9333 |
| WebGL2 / moving enemies | 3.50 / 3.40 / 3.50 | 3.10 / 3.50 / 3.30 | 0.9429 |

Three AB/BA/AB pairs per backend/workload; 8 s warmup and 25 s measurement, profiler off. Identical 960 × 640 DPR 1 viewport, 15 runtime/asset hashes, authored appearance and native gameplay source. CPU covers application RAF work, including simulation and render preparation; it excludes between-RAF timers/workers. The capped 60 Hz cadence stayed unchanged. No capacity or physical Quest gain is established.

Predeclared meaningful-improvement criterion: at least 5% and 0.2 ms CPU median reduction, with all three pairs improving. **None of the four scenarios passes all criteria.** Preserve the adverse WebGPU static pair and WebGL2 moving-enemy pair.

## Actual workload and image evidence

Static comparisons: all six pixel gates pass. All three WebGPU pairs and one WebGL2 pair are exact; the other two WebGL2 pairs differ by at most 1/255 in a channel, with no pixels exceeding the original tolerance of 8.

Active comparisons: native frame counters advance and six enemies remain in motion. Recorded fired-projectile deltas are zero; beam activity is not separately counted. This is a movement/animation trace, not a firefight, sword or cut benchmark. Identical time-based input does not guarantee identical simulation/capture state, so active image parity is unavailable. Real images are shown without an invented parity result.

The fixture imports actual `scene.js`, `appearance.js` and native gameplay, excluding the desktop DOM controls in `main.js`. Resolution, assets and quality were preserved. The old glTF triangle counter is unavailable, not zero.

## Separate GPU diagnostics

| Backend / workload | Before (ms) | After (ms) | Scope |
|---|---:|---:|---|
| WebGPU / static | 2.53 | 2.66 | WebGPU scene.main pass |
| WebGPU / moving enemies | 3.03 | 3.17 | WebGPU scene.main pass |
| WebGL2 / static | 5.03 | 5.74 | WebGL EXT command interval |
| WebGL2 / moving enemies | 7.00 | 6.31 | WebGL EXT command interval |

One separate profiled 2 s/8 s run per build/workload/backend, with separate API diagnostics. WebGPU values cover only `scene.main`; GL command intervals can include CPU/driver gaps and are not pure GPU busy time. These are different scopes, not whole GPU-frame time; do not add CPU and GPU numbers or claim a GPU improvement. Last pending GL queries may be explicitly discarded at the measurement boundary; invalid/disjoint samples are excluded.

## Identity, limits and delivery

Original release WASM: `c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a`.

Retained release WASM: `292f27c12e7d7283526f5a607d85a4ace8cd0616b7623307b300af4efa4cc70b`.

No Rust, SDK, gameplay or appearance changes in this measurement task. Boundary diagnostics run outside the timed window. Live ports 8443/8878 were preserved; no commit, push or deployment. External GPU load/temperatures were not verified. These are desktop measurements, not physical Quest evidence. This report is old/new Rust, not full-game Rome parity against Three.js.

## Browse and reproduce

[Interactive results and actual image slider](index.html) · [Structured results, per-pair tails and native state](view-data.json) · [Predeclared plan and file identities](plan.json) · [Exact commands](commands.json) · [UI checks](view-verification.json) · [Archive validation](validation.json).

Each named command directory retains the full compressed report, unchanged raw samples, actual served manifests and every capture. Measurement source is in `measurement-source.tar.gz`; `run-series.py` replays command construction. Three.js reference-control evidence remains in [the earlier technique-transfer report](../2026-10-25-three-transfer/REPORT.md); those controls are not matched whole-game Rome comparisons.

Next useful experiment: profile this actual scene at representative headset resolution and isolate the limiting glTF/shader/preparation scope before another renderer change. Preserve quality and recheck the same paired workloads. Physical Quest measurement remains separate.
