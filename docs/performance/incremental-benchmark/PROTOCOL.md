# Incremental renderer benchmark

Goal: accumulate measured, quality-preserving improvements, including small ones, without turning normal machine drift into reported progress. This protocol uses existing scripts/render-performance.mjs and the existing cut/pixel runners; it is not another rendering harness.

## Fixed baseline and evidence boundaries

Start with the locally verified animation workspace candidate: source c8b6ee4490c8cc9fee089ec125f6ff94e2a3874aae3f13bd4e4ba214d7850ec8; WASM59615b78ac729b361a611047ecc5bf6d56ef1e99571a0ddc14b3d35f296fc6eb. Frozen runtime: output/animation-workspace/candidate-runtime in codex/animation-workspace. This includes inherited FFR work; it is not raw HEADc20f03fed. This candidate is not the home8443 runtime. Source/pkg/SDK manifests are mandatory for later candidates.

The first control is A/A: exactly this same frozen runtime on both sides. This estimates observed repeat variability for this session. It is not an optimization, an A/B effect, a confidence interval or a guaranteed lower bound for future errors. Three pairs provide an initial control, not enough evidence to resolve0.1% changes.

## Profiles

| Profile | Existing family/tool | Purpose |
|---|---|---|
| Normal Rome | rim-active | Whole callback, tails and actual enemy state at ordinary load;60Hz cap is explicitly allowed as a CPU control, not throughput proof. |
| Moving camera stress | rim-camera-stress, seed1337 | Actual repeated Rome geometry, moving camera, current per-view culling; choose real work near/over budget and freeze it. |
| Active mutations | rim-moving, rim-clustered, rim-pool-cycle | All-moving roots, visibility/batch fragmentation, spawn/removal/resource lifecycle. |
| Neon | arena-active | Independent non-glTF regression and ordinary game. |
| Cutting | benchmark-rim-cuts.mjs | Actual worker handlers/install/publication and effect latency, separately from RAF; exact fixtures for pixel acceptance. |
| Three milestone reference | Existing --three-root path, rim-common and separately production rim-stress/WebGL2 | Pinned vendored r170 and existing per-part instancing; explicit graphical/culling/submitted-work differences. |

Current initial A/A control covers only camera stress, WebGPU and WebGL2. The broader matrix is the acceptance plan for future candidate changes, not claimed completed work. It can be scoped to changed dependencies; a final accepted candidate still needs ordinary game/second-scene regression. Physical Quest is a separate device campaign; simulated XR is not an FPS/comfort substitute. Three is rerun at useful milestones, not mechanically for every tiny self-change. Use the driver's limited common profile and separately labelled production settings; never present unequal AA/shading/triangles as matched work, slow its instancing or normalize away submitted-triangle differences. The initial A/A control does not contain a new Three run.

## Calibration and clean execution

- Headless existing configured browser, plugged-in power; same dimensions/DPR/AA/lights/shadows/effects/content/seed and browser version on both sides. Keep other GPU runners/builds stopped by task sequencing. Do not close foreign apps/processes without permission. Preserve host power/thermal endpoints and contention limitations.
- Initial stress probe:1280×720,DPR1,2048roots, seed1337, production rendering.2s warmup/5s sample is **calibration only**. If still capped, no throughput precision verdict; increase actual work only within the existing safety bounds, without reducing final quality. Record inability to escape cap rather than a fake throughput result.
- Reuse the resulting backend-specific configuration unchanged for all candidates. Keep this fixed-load control separate from a capacity sweep. Do not recalibrate separately for the candidate.
- Main samples:8s warmup/25s sample, minimum3alternating pairs AB/BA/AB; --no-profile --measurement minimal. Actual profiling=false, zero sampler polls, no raw-buffer overflow, unchanged driver inputs, actual backend and dimensions must pass. No API observer, detailed scopes or GPU query instrumentation during these clean runs.
- Set --pixel-channel0 --pixel-fraction0 for control captures; inspect exact raw verdicts independently of exit code. Historical camera/culling failures remain distinct. Timed authored gameplay snapshots are unavailable exact references; use maintained deterministic fixtures separately. Never change goldens/tolerance to obtain passage.
- Raw frame series and per-run distributions stay immutable in new output paths. Keep all adverse runs. One unexpected max never proves a regression by itself.

## Metrics / decision

Primary throughput: whole frame-interval distribution and elapsed cadence under over-budget real work. Report CPU callback wall time separately; it includes API/scheduling costs, not pure CPU compute. Report per-run mean/median/p95/p99/max and paired ratios, not a giant pooled frame sample that pretends consecutive correlated frames are independent repetitions. GPU unavailable stays null; profile separate diagnostic runs only when needed.

Also retain actual submitted draws/instances/triangles, upload bytes/calls and allocation/retention evidence as appropriate. Legacy declared draw counts are not backend API counts. Do not sum parent/child timing scopes or double-count instance bytes included in uniform totals. For cut scheduling measure request-to-publication and queue length; delayed feedback is not a free optimization.

For A/A report every pair's apparent FPS and CPU mean/median change plus observed range/max absolute drift. Do not call that range a95% interval or a universal noise floor. For A/B use multiple independent run pairs and a refreshed neighboring A/A control when conditions change. Extend repetitions under a predeclared budget if the suspected gain is smaller than observed repeat variability; otherwise mark **unresolved**, not faster. Repeated testing until one favorable result is not acceptance. Do not discard an adverse pair or select a new metric after seeing results.

A reported0.1% gain requires matching work/quality and measurement precision that actually supports it. Increasing displayed decimals does not improve precision. Track tiny CPU component gains separately if whole-frame gain is unresolved; retain useful allocation/correctness improvements without fabricating FPS gains. Final project-level progress is remeasured against the fixed starting baseline; do not add individual percentages mechanically.

One significant change → independent correctness/pixels → matched A/B → regression controls → accept/revise/reject → ledger. Start with node-shared matrix/inverse/determinant work as a bounded candidate after this measurement control; XR presentation is a separate, device-sensitive experiment. Do not combine them into one patch.

## Reproduction

From the isolated checkout (configured Node on PATH), use new output directories:

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/animation-workspace/candidate-runtime --family rim-camera-stress --backend webgpu --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 2000 --sample 5000 --no-profile --measurement minimal --max-time 180 --out output/NEW-wg-calibration
node scripts/render-performance.mjs compare --baseline-root output/animation-workspace/candidate-runtime --candidate-root output/animation-workspace/candidate-runtime --config output/NEW-wg-calibration/config.json --max-load 2048 --warmup 8000 --sample 25000 --pairs 3 --no-profile --measurement minimal --pixel-channel 0 --pixel-fraction 0 --max-time 900 --out output/NEW-wg-aa
# Repeat serially with --backend webgl calibration and its own config/output.
# Candidate A/B uses a separately verified frozen candidate root, same saved config.
```

No live-server replacement, commit/push/merge or further optimization is implied by this protocol.
