# Animation workspace — measured local candidate, 2026-10-31

The accepted change removes repeated pose/channel/topology-workspace allocations from active animation updates. It does not change shaders, resolution, content, pose arithmetic or the live Quest package. CPU component work improves; the tested desktop game remains capped at60Hz, so an FPS gain is not established.

## Identity and scope

- Checkout: `/Users/Honza/WebstormProjects/rust/output/worktrees/animation-workspace`; branch `codex/animation-workspace`; base `c20f03fed6737e87654067b279ebc60aad21a30a`.
- Baseline release source: `9c9735606e3509141cc5e81260172c8c40bc14a7056d6ead2353110a58ac015b`; WASM `cb3106b675c738b7e05c8481548d9df543d55eb85e91d16c4f189b8a116afc42`.
- Candidate release source: `c8b6ee4490c8cc9fee089ec125f6ff94e2a3874aae3f13bd4e4ba214d7850ec8`; WASM `59615b78ac729b361a611047ecc5bf6d56ef1e99571a0ddc14b3d35f296fc6eb`.
- Identical generated SDK binding hash: `141ad06fdff5afe66b89a9984349a12dc8e29a4155f0d176330636a0a2e76f58`. Both roots contain the same inherited, uncommitted FFR prototype, so the base commit alone does not identify either build. The original render-closeout checkout is unchanged.
- Frozen roots: `/Users/Honza/WebstormProjects/rust/output/worktrees/render-closeout/output/model-publication/animation-baseline-runtime` and `/Users/Honza/WebstormProjects/rust/output/worktrees/animation-workspace/output/animation-workspace/candidate-runtime`. All frozen-file hashes and current candidate runtime/build inputs were rechecked after measurements. Both use release Rust1.99.0; Chrome156.0.8078.74 on Apple M1/ANGLE Metal. Timing uses frozen raw runtime modules and matching release WASM, not a comparison between dev and release builds. Packaged/minified browser regression acceptance is separate.
- Baseline-only authored-Rome calibration:2s warmup/5s sample, load1,960×640,DPR1; pilot only. Authored gameplay does not replicate via the load axis. No throughput stress result is claimed. Main runs:8s warmup/25s sample, three alternating pairs/backend. Native microbenchmarks use120warmup/900sample updates at90Hz; these much shorter component windows are not equivalent to browser25s runs.
- Per-run host power endpoints report AC; no thermal warning was recorded. This is not continuous thermal/power monitoring. Ordinary desktop/Codex/Chrome/WindowServer GPU/CPU activity remained; no foreign processes were stopped. Small frame differences are not an isolated universal speedup. See raw process-context.json.

## Change and correctness

`AnimationWorkspace` is lazy and per-instance. Reusable candidate pose, nested weight/skin vectors, channel output, traversal stack and parent array replace repeated hot-path clones. Graph validation remains per-update because public asset topology can mutate. Preparation validates pose, bounds and root before swapping published state/time/revision; failures preserve the old state. Manual morph overrides and animated-node flags remain exact. Public cold command helpers preserve their allocating transactional API. The active-ID collection remains allocated; this is not a zero-allocation scene update.

Never-animated instances retain only an optional pointer. Previously animated paused/stopped instances intentionally retain scratch for resume: the source comment about paused instances must be read with that distinction. Clones start cold. Retained scratch is limited to2MiB per instance; larger valid assets can run with transient allocation after trimming. There is no global scratch pool. Accounted bytes include workspace capacities/bookkeeping but exclude the published pose, shared assets and allocator headers. They are not RSS or WASM heap residency. Successful oversized-publication timing was not separately measured; the explicit trim/clone path has a unit regression.

The reusable result-buffer principle also exists in the vendored Three r170 `Interpolant` (`examples/rim-three/vendor/three.module.js:43064`). We adapted workspace ownership and transaction safety to this engine. There is no fresh Three timing or superiority claim in this task. The retained `gltf.animation` scope is narrower after refactoring; historical nested scope timings are not compared. All clean numbers below enclose the whole Scene::update or whole application callback.

## Native allocation diagnostic

Actual Rome GLBs (wraith/tank/orbiter/madmechanic), Idle;15roots use3/3/8/1 distribution. Counts are native allocator observations over900updates after warmup; cumulative requested bytes are not live/retained bytes. Allocation-observer elapsed times are excluded from performance acceptance.

| Roots | Allocations before → after | Cumulative requested bytes before → after | Candidate retained scratch B |
|---:|---:|---:|---:|
| 1 | 52,200 → 900 | 10,303,200 → 14,400 | 6,100 |
| 15 | 988,200 → 2,700 | 228,740,400 → 100,800 | 133,930 |
| 64 | 3,546,900 → 4,500 | 884,952,000 → 446,400 | 520,960 |

All64post-sample state hashes at each load match the original implementation. Hashes include TRS/world/weights/skin/active nodes, revision and playback time. This complements, rather than replaces, independent bitwise-reference and rendered-pixel tests.

## Native clean timing

Custom allocation observer compiled **out** (feature disabled). Whole Scene::update, ms/update; preserve every pair, including the adverse one-root sample.

| Pair | Side | 1root | 15roots | 64roots |
|---:|---|---:|---:|---:|
| 1 | baseline | 0.004645 | 0.083474 | 0.308424 |
| 1 | candidate | 0.007194 | 0.068838 | 0.205263 |
| 2 | candidate | 0.002970 | 0.054372 | 0.204631 |
| 2 | baseline | 0.004399 | 0.080465 | 0.307867 |
| 3 | baseline | 0.004375 | 0.080591 | 0.309225 |
| 3 | candidate | 0.003004 | 0.054466 | 0.208170 |

The64root native component is approximately one third shorter across these pairs; this does not mean a one-third reduction of the entire rendered frame. One-root candidate pair1 is slower. Clean candidate binary predates only the final additional test-only morph-override regression; runtime core is unchanged. Clean/allocation binary hashes are separate in MANIFEST/RESULTS and must not be presented as byte-identical whole test builds. Morph-heavy/large-asset timing and Quest allocator behavior were not measured.

## Authored active Rome — clean whole callback

Movement/animations remain active: six native enemies at both boundaries, no kills/cuts/spawns during timed windows. This is ordinary stationary-player enemy motion, **not full combat**, camera stress or cut-latency benchmarking. Real dt makes exact timed-frame snapshots nondeterministic; paired gameplay captures correctly remain unavailable, not PASS. Resolution/effects and original simulation remain enabled.

| Backend | Pair | Side | Mean CPU wall ms | p95 | p99 | Max | FPS | Frame p95/p99/max ms |
|---|---:|---|---:|---:|---:|---:|---:|---|
| wg | 1 | baseline | 2.1418 | 2.60 | 2.80 | 4.20 | 60.00 | 16.80/16.80/16.80 |
| wg | 1 | candidate | 2.0217 | 2.50 | 2.60 | 4.30 | 60.00 | 16.70/16.80/16.80 |
| wg | 2 | candidate | 1.9786 | 2.50 | 2.70 | 3.50 | 60.00 | 16.80/16.80/16.80 |
| wg | 2 | baseline | 2.2440 | 2.70 | 2.90 | 4.50 | 60.00 | 16.80/16.80/16.80 |
| wg | 3 | baseline | 2.1073 | 2.60 | 2.80 | 3.30 | 60.00 | 16.80/16.80/16.80 |
| wg | 3 | candidate | 2.1219 | 2.50 | 2.70 | 4.10 | 60.00 | 16.80/16.80/16.80 |
| gl | 1 | baseline | 2.7961 | 3.50 | 3.80 | 4.20 | 60.00 | 16.70/16.80/16.80 |
| gl | 1 | candidate | 2.6131 | 3.40 | 3.80 | 4.40 | 60.00 | 16.70/16.80/16.80 |
| gl | 2 | candidate | 2.7433 | 3.40 | 3.70 | 4.40 | 60.00 | 16.70/16.80/16.80 |
| gl | 2 | baseline | 2.8557 | 3.60 | 4.00 | 5.00 | 60.00 | 16.70/16.80/16.80 |
| gl | 3 | baseline | 2.9182 | 3.60 | 3.90 | 4.20 | 60.00 | 16.80/16.80/16.80 |
| gl | 3 | candidate | 2.6997 | 3.50 | 3.80 | 4.40 | 60.00 | 16.70/16.80/16.80 |

WebGL2 mean callback wall time improves in all three pairs. WebGPU improves in two pairs; pair3 regresses by about0.7% in its mean and has equal median. Existing driver median-callback ratio summaries are0.9643(GL) and0.9545(WG); do not confuse them with ratios of run means. Both versions stay approximately60FPS and16.67ms mean intervals: no throughput/FPS gain proved. CPU callback wall time includes scheduling/API effects; it is not pure CPU compute. GPU times are **unavailable** (zero valid samples with profiling disabled), not0ms. No CPU/GPU sums.

The driver records `profiling:false`, minimal measurement, zero sampler polls, no recorder overflow, unchanged driver input hashes and no frozen-server errors in all12runs. Driver `profilerOffControl:false` means the optional separate sampled on/off control campaign was not requested; actual per-run profiling is false. No GPU/API observer runs concurrently with clean timing.

Draw/instance/triangle/upload boundary counters are retained per run in RESULTS.json and raw reports. They are glTF command-scope counters, not whole-scene backend API counts. The legacy final gauge is463draws/79,360uploadB on both sides; it must not be confused with cumulative actual glTF counters. Uniform upload already includes instance uploads; never add them. Different live dt/visibility and one boundary frame can change submitted per-RAF work slightly; no triangle-normalized equal-work percentage is invented. GPU allocation and long memory plateau were not separately measured for this CPU scratch change.

## Actual verification

- `cargo test --offline --lib`: **441PASS,3ignored**; full current native log retained. Five new focused regression tests cover independent original math, cubic/endpoints, mutated graph rollback/recovery, scratch clone/trim and manual morph override clip/seek transitions. Existing bounds/deformation atomicity also passes.
- `bash scripts/build-release.sh`: release WASM PASS in isolated checkout; final assertFreshBuild matches frozen candidate.
- `VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --production-root output/production --backend all --page animation-workspace,gltf-full,gltf-publication,rim-gameplay,neon-arena`: **10PASS**, both backends. Includes actual cutting/gameplay and lifecycle regressions. This is targeted packaged acceptance, not the complete publication gate.
- `node scripts/compare-animation-workspace.mjs BASELINE CANDIDATE NEW_OUT`: **24/24strict exact PNG pairs**,0changedpixels, RGBA tolerance0. Fixed90Hz authored tank/Fox skin/morph at frames6/24, two eyes and two backends; visible content, pose progress, validation scopes and reentry checked. Capture is diagnostic outside timing; simulated XR does not emulate physical hardware foveation.
- Independent read-only Sova review — GPT-6.1 Sol Medium: no correctness blocker in scoped design/code review. Removed redundant pre-budget scan and repeated channel/override scans before final build. Reviewer did not substitute for test execution.
- Historical A/B/C/D image blockers remain independently open. These24new exact states do not close them or prove equality of all live gameplay frames. Physical Quest, headset FPS/comfort, long-session memory and hardware FFR remain unverified.

## Reproduce / raw evidence

Use a new output path for every probe/compare. The isolated baseline source includes the identical diagnostic feature/helper but no workspace optimization. Native helpers require real assets relative to cwd. Commands were run with the configured toolchain and Node path.

```sh
cargo test --offline --lib
cargo test --offline --release --lib --no-run --message-format=json
cargo test --offline --release --lib --features animation-allocation-probe --no-run --message-format=json
# Frozen clean or allocation test binary, as appropriate (serial):
ANIMATION_PROBE_OUT=NEW.json ./FROZEN_TEST_BINARY animation_workspace_probe::animation_workspace_probe --exact --ignored --nocapture --test-threads=1
bash scripts/build-release.sh
VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --production-root output/production --backend all --page animation-workspace,gltf-full,gltf-publication,rim-gameplay,neon-arena
VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs --runtime-only output/NEW-runtime
node scripts/compare-animation-workspace.mjs BASELINE_ROOT CANDIDATE_ROOT output/NEW-pixels
# Both backends independently: use the saved calibration/config.json unchanged.
node scripts/render-performance.mjs compare --baseline-root BASELINE_ROOT --candidate-root CANDIDATE_ROOT --config SAVED_CONFIG --out output/NEW-pairs --warmup 8000 --sample 25000 --pairs 3 --no-profile --measurement minimal --max-time 900
```

Raw local evidence: `/Users/Honza/WebstormProjects/rust/output/worktrees/animation-workspace/output/animation-workspace` (native-pairs.json, separate allocator JSONs, frozen binary/build logs, pixels/report.json and PNGs, rome-{wg,gl}-pairs/report.json/full series, browser/native logs, final-build-identity.json). Frozen manifests hold all per-file hashes. Reproduction of the exact original commands, including calibration, is in their driver report `command` field. [Manifest](MANIFEST.json), [derived rows](RESULTS.json).

## Delivery

Accepted as a scoped local CPU/allocation improvement, with adverse small samples and capped-frame limits preserved. No source reduction, quality reduction, tolerance increase or effect delay. No commit, push, merge, production package replacement or shared-server restart was performed for this task. Home8443 remains on the prior FFR runtimecb3106b6; the animation candidate59615b78 is **not deployed**. Prepared animation-only patch/checkpoint is local output, distinct from the inherited FFR diff.
