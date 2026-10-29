# Followup2 — overlay selection diagnosis and rejected automatic optimization

2026-10-29; isolated `codex/render-followup-2`, base `0b7af5b75082a2508d6fb19d05e7fef4e51c5c77`. Only batch2. No push, merge, deployment, shared service restart or physical Quest test.

## Decision

The redundant work is proven and a minimal removal is implemented/testable. **Automatic enablement is rejected on both backends.** It removed expensive CPU presence scans, but repeated static stress cadence did not improve and some tails worsened. Original selection remains the default. The experimental immutable guard is available only through explicit `setOverlaySelectionReference(false)` for diagnosis; `true` restores the original path. No game/example enables it. This is not a claimed production FPS optimization.

Retained changes: low-overhead profiling/work counters, explicit independent reference switch, bounded immutable eligibility query, exact lifecycle/alpha/XR oracles, and reproducible positive/adverse evidence. No additional GPU cache, resource pool, pose revision machinery, reduced quality or altered rendering order. Batches3–6 are not started.

## Proven cause and measured work

`State::render_to` → `draw_postprocess_overlays` → `render.overlays.select`. Inside are glTF overlay existence, native mesh transparency, helpers, neon eligibility/preparation and cut-ring preparation. Actual transparent batching/encoding follows the scope. Shadow/reflection and HUD/presentation are outside it. Postprocess order stays composite → glTF BLEND → mesh alpha → rings → glow → helpers.

The old glTF query asks for presence using `draw_count(model, true) > 0`: it scans every primitive and probes the active-node set before immutable material alpha. Repeated roots containing only OPAQUE/MASK therefore do expensive work with no relevant output. There are no temporary collections in this glTF stage. The candidate stores one immutable BLEND-membership bit per GPU model; mixed models inspect alpha then the **live active set**, with early exit. It preserves last-pose/error semantics, view selection, fades, dissolution and downstream ordering. This differs from rejected B7 published pose-count reuse: no published count or pose invalidation state is used, and all other `draw_count` callers stay unchanged.

Separate profiler-on 8s/25s diagnostics (all invocation totals, not a rolling 60-sample estimate):

| Workload | Original / candidate selection calls | Models/call | Primitive visits/call before → after | glTF selection mean wall ms before → after |
|---|---:|---:|---:|---:|
| Rome stress2048 WebGPU | 945 / 941 | 1946 | 130287 → 0 | 7.611640 → 0.044527 |
| Rome stress1024 WebGL2 | 1065 / 1054 | 1026 | 68647 → 0 | 4.507981 → 0.039848 |

Active-set probes similarly fall to zero for these opaque-only workloads. Both still visit visible model roots and render identical submitted geometry. Neon has zero glTF model visits in both paths; native transparent mesh/glow work remains. Neon selection's glow stage measured roughly0.015–0.020ms, not Rome's multi-millisecond scan. Detailed counters/clocks are profiling-only; clean runs retain a single cumulative selection-call counter and the original query by default.

## Clean whole-frame result

Protocol: release build,1280×720,8s warmup +25s sample, three alternating AB/BA/AB pairs/configuration, profiling explicitly false. No quality, resolution, content or tolerance change; no busy wait. Main workloads preserve existing frozen load/settings. Baseline-only fixed-load calibration regenerated SDK-compatible manifests after the identity guard correctly rejected historical SDK manifests. No bypass. Full invocations: [COMMANDS.md](COMMANDS.md), per-run distributions: [RUNS.md](RUNS.md), machine data: [RESULTS.json](RESULTS.json).

Total **90 clean runs / 102137 complete RAF interval samples / 45 pairs**. Exact capture verdicts: **32 exact, 1 FAIL, 12 unavailable**. Unavailable native active-game/Neon verdicts are not PASS; fixed-state acceptance fixtures are separate evidence.

| Experiment | Paired FPS observations | Interpretation |
|---|---|---|
| Original → optimized WebGPU stress2048 | 37.497→37.014;37.632→37.292;37.736→37.452 | All three adverse (−1.29%,−0.90%,−0.75%); reject automatic WG default |
| Matched diagnostic reference → optimized WebGL2 stress1024 | 42.562→42.345;42.550→42.174;42.682→42.373 | All three adverse despite lower CPU; reject automatic GL default |
| Original → optimized WebGPU clustered1024 | 25.475→27.023;25.321→27.281;25.578→27.059 | Observed +5.79–7.74%; conditional benefit, strict image issue below blocks universal acceptance |
| Small static/active Rome and active Neon, both APIs | approximately60 FPS | Refresh capped; not proof of throughput superiority |

Original→optimized WG stress callback mean fell about14.919→7.855ms, yet whole-frame mean FPS fell37.621→37.253. RAF p95/p99 stayed33.4ms; per-run maxima changed50/50/66.7→116.6/116.7/100ms. These adverse tails are retained. Reclaiming CPU time does not guarantee faster frames when rendering/presentation limits throughput. The exact mechanism of the small cadence loss is not isolated on this contended host; no invented GPU scheduling explanation. Final restored-default controls: WG2048 37.551→37.497,37.642→37.551,37.337→37.671; GL1024 42.454→42.374,42.414→42.493,42.442→42.465. All six pairs are exact, differences small and mixed; not an optimization gain.

Work/content: stress2048 has308 actual commands (307 glTF + composition),126823 submitted glTF primitive instances and39067116 submitted triangles/frame, unchanged in the exact static pairs. Stress1024 has241 commands. No steady-state glTF instance/uniform uploads or instance-pool creation in static stress. Active native workloads remain dynamic with uploads, visibility changes and occasional pool growth; their sampled draw counts need not match at different phases. Full counts/bytes are in RESULTS.json. Cutting is tested by maintained functional/worker/XR scenarios on the delivered default. The experimental opt-in is not independently pixel-compared through a complete worker cut/fade/dissolve sequence; it remains diagnostic, not accepted for production. No new claim about long-duration cut latency in this batch.

## CPU/GPU evidence boundaries

Clean runs have **GPU timing unavailable**, not zero. Separate WebGPU diagnostics provide31 asynchronous main/fxaa interval samples for stress: main mean26.055→26.234ms, fxaa3.107→3.280ms. These instrumented, nonpaired intervals are not a proven GPU regression, full-frame GPU busy time or sums to add to callback CPU. WebGL native pass timestamps are unsupported; the harness's separate elapsed query has a different interval, retained in DIAGNOSTICS.json. No readPixels/gl.finish/synchronous waits were added to production loops. Diagnostic fixture readback is outside performance measurement.

Headless Chrome156.0.8078.48 on Apple M1/16GiB, macOS26.6.1 build25G76; Node24.22.1, Rust1.99.0, release/wasm-optO3. Sandbox remains enabled. Other browser/GPU processes stayed active; AC power/thermal observations are preserved. Desktop RAF cadence is not compositor presentation or headset comfort. No fresh Three comparison or physical Quest acceptance is asserted.

## Image correctness and new independent case D

New exact fixtures compare original → optimized → restored with fixed identical inputs, covering absent overlays, OPAQUE/MASK/BLEND, per-scene BLEND activation, native opacity, glow/bloom toggles, instances, remove/reload/clear, desktop asymmetric views, and actual simulated XR eyes. WG fixture21 exact states; GL25 including four stereo cases. Shared XR checks now prove visible BLEND contribution in **each** eye before testing exact selector equivalence, including left-only asymmetric projection. A reviewer caught the initial opaque-only XR gap; it was repaired and independently re-reviewed before execution.

The primary clustered1024WG pair0 **fails strict equality: four pixels, max1/255**, coordinates(651,114),(652,115),(653,115),(653,116), distant drone edges. Independent decoding of all six images finds exactly two complete image variants, **both present in original AND candidate runs**. This disproves candidate selection as a necessary condition, but does not prove a numerical/capture mechanism. The maintained capture applies pose(.375), then its existing five-RAF helper; clean timing does not record complete native revisions/command epochs at capture. The failure remains open, not “GPU noise”, not fixed by tolerance changes, and not attributed to prior A/B/C. Raw failure stopped the main runner; remaining predeclared configurations ran separately without weakening the gate. See [CLUSTER_PIXEL_INVESTIGATION.json](CLUSTER_PIXEL_INVESTIGATION.json).

Prior A six-pixel camera, B historical eight-pixel moving and C culling/shadow issues remain separate/open. Batch2 does not repair or merge those diagnoses. No golden/tolerance change.

## Build, verification and handoff

Original runtime WASM `f14c4beb5f3e5b59f16cae9ae90ddccba88b152ed10f07f3ce999d5a765a12de`.
Delivered restored-default WASM `d0677791488085cb26b7423a4c062fb4411044e90f55d958193558bd7c5f0798`;
source `7cd160e788ece6dfd78b6d913957ba758cb19dc8ad4abae0d52c6114d263ca4f`;
bindings `504f0f506ebe4e1839bf3dfc4b169109b35b8ba8c490fdf1c99f5665827080fe`.
The archive directory `final-candidate` is a historical intermediate, **not delivered final default**. [MANIFEST.json](MANIFEST.json) identifies all five frozen roots and configuration. Owned source/pkg/frozen files are checked independently of any live production service.

One fresh `VERIFY_WASM_PROFILE=release npm run verify:push` completed with exit0: 428 Rust tests PASS (2 ignored), 32 SDK, 14 Node harness +13 Python harness, 71 XR-platform tests, format/native and WASM Clippy/check/types, and **151 browser cases (56 WebGPU/95 WebGL2)** PASS. Matching release was reused. Source dump regenerated. PASS receipt revalidated through the same npm environment without rerunning the suite; source/pkg/tests identity remains matched. See [VERIFICATION.json](VERIFICATION.json), [INTEGRITY.json](INTEGRITY.json) and [archive restoration proof](ARCHIVE_VERIFICATION.json). Functional-gate PASS does not clear the benchmark caseD FAIL. Local checkpoint only; no publication.

## Remaining priorities (separate authorization)

1. Keep automatic guard disabled; do not repeat B7 or this experiment without a new whole-frame hypothesis.
2. Next requested batch concerns non-glTF Neon allocations, which this glTF presence guard cannot solve.
3. CaseD needs isolated capture/state investigation if pursued; prior A/B/C remain separately tracked.

Stop after this local evidence checkpoint. No next batch, push, merge, deployment or shared-server restart.

Evidence archive SHA-256 `f6373e23790933dc4ae680bc5ae3700e0067d5a61c4e6f2acfc268c7edfd82d5`,91,713,093bytes. All1185unique objects and4399restored independent files verified; restored baseline configurations pass the maintained identity guard. Historical docs archives inside snapshots are excluded to prevent recursive duplication; source/runtime/assets and all failed/adverse measurements remain. Current report/state are authoritative over archived work-in-progress notes.
