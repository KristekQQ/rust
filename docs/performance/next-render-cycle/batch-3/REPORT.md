# Batch 3 — fine invalidation during camera/model motion

Implementation, measurements and final project gate are complete locally, with the strict stress-pixel acceptance limit described below. Only batch 3 is authorized. No push, main merge, deployment or shared port 8443 restart. Parent checkpoint: `1afcd015787a3bff3fb58afd54126a6c7728a512`; isolated `codex/next-render-cycle-3`.

**Known acceptance limit:** the strict primary pixel gate is **40/42 exact, 2 FAIL** in WebGPU fully-moving1024. Those failures remain FAIL. Additional fixed-pose captures demonstrate temporal instability in the unchanged baseline, and every candidate image is a variant already observed in primary baseline. This does not prove universal parity, unchanged frequency, GPU causation or absence of a shared older bug. Exact repeatability of that stress case remains unresolved. No tolerance, reference image or quality setting was changed.

## Change and dependency boundaries

[Design](DESIGN.md) describes the implemented separation: bounded immutable compatibility tokens; per-model/draw data and basis revisions over existing canonical Uniforms; fresh original root/primitive visibility, depth grouping and order barriers for every changed view. A moving root no longer globally clears static plans. Matching groups retain prepared rows and patch changed16 B/112 B records. Actual content epochs and storage generations protect pass/eye ownership. Transparent objects use their original ordered path. No camera rounding, cached image, simulation stop or content reduction.

The existing stream reserves power-of-two bytes per group index to avoid downstream offset ripple. Current live binding widths remain exact. This can increase retained capacity and union upload spans. Fixed64 fine slots and16 MiB accounted element storage remain separate from the exact cache's16 MiB. Neither is a whole-process/GPU memory bound.

Measured fallback uses the original builder for views without instancing, or when at least 8 changed current contributing roots own at least 75% of at least 512 currently selected items. A single heavy visible/offscreen root cannot trigger it. Conservative pose publications may cause unnecessary fallback. This simple policy is not universal tuning. Original preparation work remains counted when fine counters are zero.

## Frozen identity and protocol

Baseline is the completed batch 2 release, not batch1 or unoptimized main: source `1452a9f7ca544a1490773130f92719d223511b5feaa6c5b819bd3a9d325a0358`, WASM `af2905cc7d71af63b731c586198bd201917d63375821e80d1a70e7a99f1ea088`.

Final candidate: source `e8b39fcf650592b36a9e5897e8c4f9e85f131ab4782307ff36f5657dedfcd2dd`, WASM `bdae44e11cd107177241ec19de3d44137399603cb42f2f5de2dc4513a1c553ea`. SDK and generated bindings are unchanged; full hashes/configuration are in [manifest](MANIFEST.json).

Apple M1, 8 CPU/7 GPU cores, 16 GB; Chrome 156.0.8078.48; bundled arm64 Node 24.22.1; pinned Rust 1.99. Foreign GPU processes were preserved. Environment snapshots show contention; aggregate AGX utilization cannot be attributed to a process or interpreted as isolated capacity.

Primary:1280×720,DPR1,seed1337, unchanged effects/content;8 s warmup/25 s sample, 3 AB/BA/AB pairs independently per backend. Baseline-only bounded calibration extended static/camera stress to2048 actual drones after 1024 reached60Hz. Mixed/all-moving stress remains 1024, small moving 16; ordinary/native/cut 64/Neon controls remain. No busy-wait or synthetic delay. Engine profiling, native preparation clocks and GL timing queries are verified OFF in every primary run. No owned build or second GPU runner overlapped clean measurement.

108 runs retain131,673 full frame records. Every RAF/callback mean, median,p95,p99,max was independently recomputed; all requested-file/build/config hashes, cleanup and common driver identity were rechecked. Callback wall time includes driver waits and measurement callbacks, excludes separate worker/timer handlers, and is not pure CPU computation. Primary GPU times are unavailable by design; separate diagnostic timestamps are not added to CPU wall time. No fresh Three.js or physical Quest comparison was performed.

## Paired primary results

Values below are medians of three per-run callback means and medians of `1000 / mean RAF interval`. FPS describes observed browser cadence, not compositor/headset presentation. Full individual runs, paired median ratios and all tails are in [results](RESULTS.json). Small or capped differences under contention are not proven speedups.

| Workload / load | Backend | Callback wall ms B→C | Mean-cadence FPS B→C |
|---|---|---:|---:|
| Static Rome / 2048 | webgpu | 15.626 → 15.751 | 36.22 → 36.30 |
| Static Rome / 2048 | webgl | 18.569 → 18.271 | 28.04 → 27.98 |
| Moving camera / 2048 | webgl | 30.153 → 23.180 | 31.68 → 32.78 |
| Moving camera / 2048 | webgpu | 26.884 → 23.167 | 36.92 → 42.73 |
| One moving root / 1024 | webgpu | 20.416 → 14.442 | 48.44 → 59.20 |
| One moving root / 1024 | webgl | 22.248 → 16.408 | 40.20 → 41.01 |
| Moving clustered roots / 1024 | webgl | 41.335 → 41.431 | 24.10 → 24.02 |
| Moving clustered roots / 1024 | webgpu | 41.366 → 41.287 | 24.04 → 24.08 |
| Moving clustered roots / 16 | webgpu | 2.356 → 2.361 | 60.00 → 60.00 |
| Moving clustered roots / 16 | webgl | 2.824 → 2.723 | 60.00 → 60.00 |
| Ordinary camera / 1 | webgl | 1.173 → 1.230 | 60.00 → 60.00 |
| Ordinary camera / 1 | webgpu | 0.789 → 0.838 | 60.00 → 60.00 |
| Native Rome gameplay / 1 | webgpu | 2.055 → 1.944 | 60.00 → 60.00 |
| Native Rome gameplay / 1 | webgl | 3.037 → 2.784 | 60.00 → 60.00 |
| Cut/spawn/despawn / 64 | webgl | 1.443 → 1.327 | 60.00 → 60.00 |
| Cut/spawn/despawn / 64 | webgpu | 0.950 → 0.959 | 60.00 → 60.00 |
| Native Neon arena / 1 | webgpu | 0.996 → 0.975 | 60.00 → 60.00 |
| Native Neon arena / 1 | webgl | 2.016 → 1.925 | 60.00 → 60.00 |

Camera and sparse movement show lower CPU wall time; the fully-moving and static controls are approximately neutral. Static exact-plan gains belong to batch 2 and are not claimed again. Ordinary camera callback medians are slightly adverse (+0.049 ms WG, +0.057 ms GL) while cadence remains 60 Hz; retain these results. This batch does not reduce glTF draw commands or establish general active-game/VR superiority.

## Work actually retained or rebuilt

Per-RAF medians. Prepared-row percentages cover fine-admitted groups only. Payload-refresh percentages cover instanced fine groups and mean **any partial/full refresh attempted**, not wholesale group copying. Fallback work is separately recorded. CPU copied records/bytes are actual differing record writes. A frame-wide writes/submitted-instance ratio in JSON is explicitly not a within-fine rebuild fraction.

| Workload | Backend | Prepared rows reused | Payload groups refreshed | CPU records / bytes copied | Actual instance GPU upload B |
|---|---|---:|---:|---:|---:|
| Moving camera | webgl | 88.27% | 13.40% | 6845.9 / 109535 | 1461688 |
| Moving camera | webgpu | 90.43% | 10.93% | 5738.0 / 91808 | 1421811 |
| One moving root | webgpu | 100.00% | 33.33% | 67.0 / 1072 | 540688 |
| One moving root | webgl | 100.00% | 33.33% | 67.0 / 1072 | 540688 |

One moving root retains240/240 prepared rows and copies67 records/1,072 B, but its one union upload still spans540,688 B. Fresh camera culling/depth/group selection still runs; row reuse is not zero grouping work. Material/representation/layout changes legitimately rebuild affected rows. No universal zero-copy/upload claim is made.

## Long-series tails and retained memory

For the four target cases, p95/p99 are medians of per-run quantiles; max is the worst observed sample across all three runs. These are full 25 s series, not 60-sample profiler windows. Every other case's tails remain in JSON.

| Workload/backend | Package | Callback p95 / p99 / max ms | RAF interval p95 / p99 / max ms |
|---|---|---:|---:|
| Moving camera/webgl | baseline | 47.40 / 60.50 / 75.50 | 50.00 / 66.60 / 83.30 |
| Moving camera/webgl | candidate | 36.30 / 40.30 / 68.90 | 33.40 / 50.00 / 66.70 |
| Moving camera/webgpu | baseline | 40.50 / 49.20 / 78.70 | 50.00 / 50.10 / 83.30 |
| Moving camera/webgpu | candidate | 37.90 / 46.00 / 61.60 | 33.40 / 50.10 / 66.70 |
| One moving root/webgpu | baseline | 31.00 / 38.60 / 53.50 | 33.40 / 49.90 / 66.70 |
| One moving root/webgpu | candidate | 23.60 / 29.60 / 63.20 | 16.80 / 33.30 / 66.70 |
| One moving root/webgl | baseline | 35.60 / 48.20 / 77.30 | 33.40 / 50.00 / 66.70 |
| One moving root/webgl | candidate | 26.50 / 40.90 / 58.10 | 33.40 / 49.90 / 50.00 |

Memory below is the maximum of observed debug snapshots across the three runs, not a continuously observed whole-process/GPU peak. Element accounting excludes allocator/Rc/class maps/model Uniforms and existing mirror/resource ownership. Both16 MiBcache budgets passed every sampled boundary/snapshot check.

| Workload/backend | Package | Fine elements MiB | Instance-buffer gauge MiB | Active / reserved layout MiB |
|---|---|---:|---:|---:|
| Moving camera/webgl | baseline | unavailable | 4.123 | unavailable / unavailable |
| Moving camera/webgl | candidate | 3.798 | 6.123 | 1.380 / 2.176 |
| Moving camera/webgpu | baseline | unavailable | 4.123 | unavailable / unavailable |
| Moving camera/webgpu | candidate | 3.798 | 6.123 | 1.380 / 2.176 |
| One moving root/webgpu | baseline | unavailable | 2.109 | unavailable / unavailable |
| One moving root/webgpu | candidate | 2.626 | 2.109 | 1.018 / 1.309 |
| One moving root/webgl | baseline | unavailable | 2.109 | unavailable / unavailable |
| One moving root/webgl | candidate | 2.626 | 2.109 | 1.018 / 1.309 |

## Correctness, diagnostics and completion

Maintained exact cache-off/culling-off motion tests cover camera, individual root/pose/material changes, multiple assets, edge crossings, clustering/spawn/removal, nonuniform/negative scale and16B↔112B, transparent/order barriers and lifecycle. Actual stereo attachments test small/large left-only changes and head motion independently. Visible/offscreen4096-node updates preserve unrelated static rows; small-fleet admission proves actual original-builder work. These deterministic acceptance scenarios are distinct from the unresolved crowded WebGPU repeatability case described above.


## Separate preparation/GPU/API diagnostics

16 captures, each 4 s warmup/10 s profiled sample, plus a different instrumented API page with initialization+1s cold counters and a 3 s warm window after 4 s. They are exploratory unpaired diagnostics, not additional primary repetitions. Six exclusive native preparation phases are accumulated per RAF; all six and sparse CPU-history freshness are retained in JSON. Unchanged history is unproven freshness, not a zero measurement. These scopes are CPU wall time and are not added to primary/GPU values.

| Workload | Backend | Six-phase total B→C ms/RAF | Grouping B→C | Basis B→C | Payload B→C |
|---|---|---:|---:|---:|---:|
| Static2048 | webgpu | 0.000 → 0.000 | 0.000 → 0.000 | 0.000 → 0.000 | 0.000 → 0.000 |
| Static2048 | webgl | 0.000 → 0.000 | 0.000 → 0.000 | 0.000 → 0.000 | 0.000 → 0.000 |
| Camera2048 | webgpu | 14.187 → 10.336 | 6.695 → 4.649 | 3.490 → 1.842 | 1.748 → 1.698 |
| Camera2048 | webgl | 13.670 → 9.714 | 6.680 → 4.347 | 3.567 → 1.938 | 1.818 → 1.766 |
| One moving1024 | webgpu | 11.244 → 6.633 | 5.599 → 3.567 | 2.809 → 0.984 | 1.285 → 0.950 |
| One moving1024 | webgl | 11.321 → 6.545 | 5.756 → 3.629 | 2.980 → 0.913 | 1.413 → 0.903 |
| All moving1024 | webgpu | 7.734 → 7.152 | 3.675 → 2.396 | 2.062 → 2.763 | 0.908 → 0.876 |
| All moving1024 | webgl | 7.521 → 6.992 | 3.771 → 2.411 | 2.073 → 2.828 | 0.900 → 0.908 |

Camera selection remains about 1.27–1.43 ms/RAF and grouping 4.35–4.65 ms; basis/payload remain real work. Other large sparse observed CPU wall scopes are render.encode.overlays(~5.3 ms) and scene.gpuUpdates(~2.3–2.8 ms). Nested render.encode.color/gltf.queue scopes overlap these phases; do not sum them. Full-moving update/driver work remains expensive despite a cheaper queue phase.

Valid GPU intervals below are separate diagnostics. WebGPU is native `scene.main` pass timestamps; FXAA timestamps are separately retained. WebGL2 is an inclusive whole-RAF asynchronous query interval, potentially including queue/idle gaps, not the same scope or a pure draw-execution measurement. No cross-backend GPU winner or causal GPU improvement follows from this table; clean primary GPU time remains unavailable.

| Workload | GPU interval scope | Samples B→C | Mean B→C ms | p95 B→C ms |
|---|---|---:|---:|---:|
| Static2048 | WebGPU scene.main | 12 → 12 | 27.032 → 26.493 | 31.047 → 27.552 |
| Static2048 | WebGL2 whole RAF | 10 → 10 | 76.597 → 76.278 | 80.400 → 79.915 |
| Camera2048 | WebGPU scene.main | 13 → 15 | 19.244 → 19.203 | 20.490 → 23.320 |
| Camera2048 | WebGL2 whole RAF | 12 → 12 | 69.652 → 68.354 | 73.753 → 73.792 |
| One moving1024 | WebGPU scene.main | 18 → 20 | 15.075 → 15.236 | 16.387 → 16.080 |
| One moving1024 | WebGL2 whole RAF | 14 → 14 | 55.794 → 54.928 | 57.481 → 58.050 |
| All moving1024 | WebGPU scene.main | 8 → 8 | 14.401 → 14.518 | 16.364 → 20.130 |
| All moving1024 | WebGL2 whole RAF | 8 → 8 | 23.477 → 20.122 | 36.823 → 35.163 |

Whole-API instrumentation is separate from exact native glTF counters. Warm buffer/resource creation is zero in the static, camera-only and one-moving-root windows; the all-moving candidate still creates about 0.870 WebGPU / 0.957 WebGL2 buffers per engine frame in the separate API window. Preserve that churn rather than claim global zero allocation; cold initialization+1s creates many per-draw resources and is not a pure startup duration. All API call counts, cold resources, uploads and warm submitted work remain in JSON. WebGL indexed primitive proof exceeds the existing 8,192-entry metadata bound under these stress loads; exact whole-API triangle totals stay unavailable. No bound inflation or invented total was used. Native glTF instances/triangles remain exact within their own scope.

| Candidate workload | Backend | Cold createBuffer calls | Warm createBuffer/frame | API draws/frame | Exact API primitive proof |
|---|---|---:|---:|---:|---|
| Static2048 | webgpu | 139377 | 0.000 | 308.00 | True |
| Static2048 | webgl | 139374 | 0.000 | 309.00 | False |
| Camera2048 | webgpu | 139439 | 0.000 | 307.00 | True |
| Camera2048 | webgl | 139436 | 0.000 | 308.00 | False |
| One moving1024 | webgpu | 70702 | 0.000 | 241.00 | True |
| One moving1024 | webgl | 70699 | 0.000 | 242.00 | False |
| All moving1024 | webgpu | 70702 | 0.870 | 215.77 | True |
| All moving1024 | webgl | 70699 | 0.957 | 216.16 | False |

## Actual worker cuts and publication

24 maintained simulated-XR replays (three/package/backend at ordinary 0 and stress 1024),144 completed real worker jobs,72 installation-stage events. Every replay verified both separated halves with nonzero pixels in both actual eye attachments, then evaporation/lifecycle. This is simulated XR, not physical Quest acceptance. Runtime/source inputs and actual backend were rechecked. These grouped same-package repetitions are not alternating performance pairs; no causal worker-speedup claim is made.

The message-handler maximum, installation-stage p95, request-to-publication mean and worst death/cut RAF below cover different scopes. Publication runs from first nonempty rimTakeCutJob through the end of the first simulated XR frame exposing complete uploaded pieces; it excludes GPU completion/compositor. Completion-from-kill includes observation delay. Worker-internal computation is unavailable. Native accept/prepare distributions and all frame events are retained separately. With only 3 replays/9 stage samples per row, tail estimates are descriptive.

| Package/backend/load | Handler max ms | Install-stage p95 ms | Request→publish mean ms | Death/cut RAF max ms |
|---|---:|---:|---:|---:|
| baseline/webgpu/0 | 0.200 | 0.300 | 161.10 | 4.00 |
| candidate/webgpu/0 | 0.300 | 0.300 | 162.37 | 3.20 |
| baseline/webgl/0 | 0.300 | 0.300 | 182.03 | 36.70 |
| candidate/webgl/0 | 0.300 | 0.300 | 188.57 | 40.20 |
| baseline/webgpu/1024 | 0.200 | 0.300 | 295.43 | 18.70 |
| candidate/webgpu/1024 | 0.400 | 0.200 | 290.03 | 21.50 |
| baseline/webgl/1024 | 0.300 | 0.300 | 340.07 | 56.50 |
| candidate/webgl/1024 | 0.300 | 0.300 | 330.07 | 49.80 |

Preserve ordinary WebGL candidate 40.2 ms worst death/cut frame versus baseline 36.7 ms, and stress WebGL 49.8 ms. These runs do not prove hitch-free VR or faster physics. Completed own Node CLIs retaining handles were cleaned only after final PASS/output and browser/server cleanup; verified PIDs, null signal exits and original coordinator are retained. This cleanup is not an extra performance test or a pixel PASS.

## Strict pixels and rejected experiments

The deterministic primary cases have 40 exact pairs and 2 strict FAIL (fully-moving 1024 WebGPU, 4 pixels/max 1 LSB). Native-gameplay final capture has no deterministic reset, so its 12 pairs deliberately have pixel acceptance unavailable rather than a false pass. Additional fixed .375 pose before/after snapshots show baseline same-page repeat 14 pixels/max 63. All candidate images are exactly one of the two image SHA variants already present in primary baseline; however one candidate independent cache-off/culling-off comparison and restoration comparisons still fail by the known 4 pixels/1 LSB. This weakens attribution to a new cache defect but does not establish unchanged frequency or eliminate a shared older bug. Exact repeatability remains unresolved; do not describe every pixel test as passing. Original failures/captures and zero tolerance remain intact.

Rejected evidence is retained: tight offsets copied ~87% of camera records despite~91% membership hits; global publication-count fallback let one offscreen heavy root discard unrelated reuse; contributing-model e57 regressed fully-moving callbacks ~5% WG/~8% GL. A reduced basis proof alone left ~2.9% GL overhead. Removing duplicate 112 B draw data recovered large-motion neutrality, but e33 still added.09–.14ms for 16 moving WG roots. Final eligible-item-weighted admission removes that observed small overhead in fresh pairs. A coordinator-path mistake loaded an older WASM; runtime-hash audit rejected/moved all six affected runs. No attempt was renamed as final, removed or mixed into final repetitions. [Journal](../EXPERIMENTS.md) and archive retain the unfavorable outcomes.

## Commands and final verification

Commands actually executed include the following; exact per-run arguments/timestamps and all failures are in the evidence archive. Scripts under output are evidence coordination around maintained tools, not a second benchmark framework.

```sh
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --release -- --locked
VERIFY_WASM_PROFILE=release node scripts/build-fingerprint.mjs --write
EXPECTED_CANDIDATE_WASM=bdae44e11cd107177241ec19de3d44137399603cb42f2f5de2dc4513a1c553ea VERIFY_WASM_PROFILE=release node output/next-render-cycle-3/run-admission-primary.mjs
python3 output/next-render-cycle-3/audit-results.py
node output/next-render-cycle-3/run-diagnostics.mjs
node output/next-render-cycle-3/run-workers.mjs
python3 output/next-render-cycle-3/summarize-extra.py
VERIFY_WASM_PROFILE=release npm run verify:push
```

A representative clean invocation uses the existing render-performance.mjs compare command, frozen baseline/candidate roots, original calibrated --config, --max-load 2048, --warmup 8000 --sample 25000 --pairs 3 --no-profile --pixel-channel 0 --pixel-fraction 0. The pixel gate stays strict even where it fails. Additional same-package/pixel-oracle diagnostics are explicitly not primary performance data.

Final project gate PASS: 423 native, 141/141 browser, SDK 32, harness 13 Node + 13 Python, XR-platform 59, Rust format/native and WASM Clippy/types. Two existing manual native timing benchmarks are ignored. 44 separate unchanged benchmark/API helper units pass. All 141 browser scenarios use the requested actual backends, without fallback, software rendering or sandbox bypass. The reused release matches current sources/generated bytes/configuration and the frozen measured candidate. The matching npm PASS receipt was rechecked without rerunning the full gate. all_sources.txt is regenerated for this owned checkpoint. [Verification](VERIFICATION.json).

This project-gate PASS does not convert the separate strict stress-pixel failures into PASS. The candidate remains a local measured checkpoint with unresolved exact repeatability; this is not universal production-quality acceptance. The verified archive and owned local checkpoint finish this batch. Stop before batch 4. Shared port 8443 was not changed or re-audited; this candidate is not a headset deployment.


## Evidence storage

[Lossless evidence archive](evidence.tar.gz):59,753,125bytes;1,391stored entries including811unique payloads and580exact-content hardlinks;407,470,137recorded uncompressed bytes. SHA256:`231273647b4b7e83e39de94dd24d4025c6c7a08632d6483249f466ca6af330ca`. Every stored member and hardlink target was read back and checked. Full manifests for all7frozen runtimes, changed source and package bytes, all primary/adverse runs, actual captures, profiler/API/worker results and final gate logs remain. Unchanged base assets and the private unfiltered process list are excluded; reconstruction instructions are inside. [Manifest](MANIFEST.json), [results](RESULTS.json), [verification](VERIFICATION.json).

Only this batch's paths and required source dump belong to the local checkpoint. The primary checkout's foreign dirty/index work, other worktrees and shared services are preserved. No next batch was started.
