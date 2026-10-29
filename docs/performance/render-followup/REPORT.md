# Followup 1 — image correctness diagnosis

2026-10-29. **A remains unresolved / not reproduced. No renderer repair is claimed.** Runtime, SDK, production capture, tolerances and golden images are unchanged. Previous nine-batch improvements are preserved. Only this batch was investigated; the remaining five require separate authorization.

## Verified starting point

Isolated branch `codex/render-followup-1`, base evidence checkpoint `9986cd4f9e37b1687af339eb741b22ec77f5c720`. Its historical checkout/report still exists. The primary checkout is separately on `ffa2e6bb64aa9df12541f0c897ab5564fb16e4a6` with unrelated changes, preserved. This evidence checkpoint is not the deployed runtime.

Final runtime is B8 `813a5abb266609cceea644e4b1086129a89ca535`, source fingerprint `54c6cde834bd3ad2e5363aabece753cd77cf4aa4537d378147405f6a814797f1`, WASM `f14c4beb5f3e5b59f16cae9ae90ddccba88b152ed10f07f3ce999d5a765a12de`, bindings `d359800638f899ae7df46d50ab453a75b477044dec2d538d8ed39d91d24d44ba`. Freshness check and served request manifests verify actual runtime bytes. Original B1 WASM is `b5cc8eba529f7c3990b1a4eacbc16c13786ac774b8a366ad15eba6943340987a`.

B9 archive SHA256 `9748a3d8962c01f8c32f532fe45916b6b381c8c1dee479fb74f20f15b4922aa9`: all 1285 unique objects rehashed, 2142 aliases; ten original case files checked against archive identities. [BASELINE.json](BASELINE.json) records exact paths/hashes. Neither shared HTTPS server nor production package was replaced.

## Three independent issues

| Issue | Prior evidence | This batch | Status |
|---|---|---|---|
| A: camera2048 WebGPU | Final candidate pair1: six pixels, max2/255; other five original/final captures exact | Independent PNG decoding reconfirms original six pixels. No occurrence among new normal-path captures | **Unresolved; not repaired** |
| B: B4 moving1024 WebGPU | Eight pixels, max2/255 | Historical report/archive preserved; no new B4 runtime replay or causal diagnosis | **Separate/open** |
| C: culling-off | 5014 pixels, max79 in B1 and final | Six new final culling-off captures reproduce count/max; other settings restored exactly | **Separate/open**; global switch also changes shadow selection |

A's archived coordinates (zero-based) are `(382,215)`, `(383,216)`, `(394,267)`, `(397,267)`, `(45,351)`, `(203,456)`; final pixel has maximum2, others maximum1. Small magnitude is not grounds to dismiss failure. No numerical mechanism or GPU-noise explanation has been proven.

## Executed reproduction

Hardware desktop Chrome156.0.8078.48, WebGPU required (fallback rejected),1280×720/DPR1,2048 models, spatial order, seed1337, production experiment, profiling disabled. Same frozen model/shader/effect inputs and maintained benchmark. This is not physical Quest evidence. Host GPU use was not isolated: other browser/process activity remained, including old B9 diagnostic processes observed during cleanup. No foreign process was stopped. No timing/performance gain is claimed here.

| Experiment | Captures | Exact against original anchor | Known C difference | A reproduced |
|---|---:|---:|---:|---:|
| Instrumented fresh + elapsed33s, four fresh pages, final/original/final/original; final control pilots |40|36|4|0|
| Unchanged original runner,8s warmup/25s sample, AB/BA/AB, no injection |6|6|0|0|
| Recorded camera-path reconstruction, final/original; restore before each isolated axis |21|19|2|0|
| Total |67|61|6|0|

The three unchanged-runner pairs each pass exact comparison. Their six PNGs match the original anchor, not merely one another. Separate byte-wise PNG analysis reconfirms counts and hashes. All successful diagnostic pages report no console errors. Failed attempts are preserved, not replaced by the successful runs.

Replay uses 750 recorded sample intervals anchored to the old observed phase start, reconstructed60Hz warmup and final phase end:1230 steps. Original individual pose clocks and renderer/workload RAF interleaving were not recorded. Thus it repeats the scenario and an approximation of its history, **not the exact failing session**. Present frame cadence also differs from the historical run. Fresh pages share one browser process per experiment, not a fresh OS/GPU state.

Single-axis final controls restore cache, pool budget or culling before proceeding. Cache-off has unchanged output; zero pool byte/resource budgets evict269 inactive resources (2161472 bytes), while4258624 active bytes remain. This is an inactive-retention control, **not complete pool bypass**. The earlier pilot changes multiple settings between stages and is not used as proof of strict single-axis isolation.

## Capture audit and recorded state

Maintained `tests/pixels.js` waits four RAF callbacks and copies in a fifth. It has no explicit acknowledgment of which renderer frame consumed the final pose. This is a diagnostic limitation; it does **not** establish stale capture as the cause. Requested fixed pose alone cannot prove complete render-state equality.

Diagnostic-only browser injection records submitted render commands and a renderer callback identity, then copies the canvas immediately after that callback returns. The legacy capture is also retained. Neither method reproduced A; therefore changing production capture is unsupported. No readPixels/gl.finish/synchronous GPU wait was added to production.

Captured data includes actual canvas dimensions, requested fixed pose, camera position, workload phase, frame/copy times, queue-submit counters, available native plan input/content epochs, hit/miss counters, pool gauges, bound buffer IDs/API-write epochs/hashes, draw order and available pipeline/shader descriptors. All61 instrumented captures have identical actual view-projection uniform bits; camera position is `[0.23557229340076447,1.7307186126708984,42.06543731689453]`. Normal captures contain307 draw commands. One checked final fresh→elapsed33 pair shares519 buffer IDs with unchanged content hashes; this is a bounded observation, not proof of equality for the archived failed session.

Evidence limits:

- Callback acknowledgment means CPU returned after submission, not independently verified GPU completion/presentation.
- Buffer hashes describe CPU shadows of observed API writes/maps/copies, not GPU readback. Texture content changes and arbitrary GPU writes are not comprehensively traced.
- Frozen runtime does not expose every object/material/pose/visibility revision or native resource generation. Those are unavailable, not inferred from stable handles. Separate projection matrix is not independently exported; actual combined view-projection is captured.
- Pipeline definitions are exported at session end. Earlier culling-off pipelines are missing in six capture summaries (36 descriptor occurrences). No complete shader-identity claim is made for them.
- `commandOrderSha256` summarizes pass labels, pipeline IDs and draw arguments only; raw command streams separately retain bind-group/vertex/index bindings. That digest alone cannot prove complete draw-input identity or visible membership. IDs are session-local.
- Instrumentation adds overhead and retains CPU shadows. It is never used as a clean performance result. Independent unchanged-runner captures address tool perturbation only partially.

Initial sandbox serving failed before reproduction. Two later attempts failed serializing oversized traces (all live resources and dynamic-offset backing arrays); retained failures are tool defects. Effective dynamic-offset slices and reachable-resource-only snapshots make completed traces serializable. Around139433 buffer records existed versus520 reachable buffers in normal capture; these numbers describe the diagnostic tracker, not a measured live-GPU allocation defect. No renderer change follows from them.

Read-only source review and independent evidence review agree that A remains unresolved. The latter identified the command-digest and descriptor limits above; both reviews are retained with raw evidence.

## Verification, delivery and next action

See [VERIFICATION.json](VERIFICATION.json), [COMMANDS.md](COMMANDS.md), [EVIDENCE_INDEX.json](EVIDENCE_INDEX.json) and [ARCHIVE_VERIFICATION.json](ARCHIVE_VERIFICATION.json). Full raw traces are compressed/deduplicated outside this concise state; scripts and failed attempts are included. Frozen runtime/harness remains recoverable from the already verified B9 archive, referenced rather than duplicated.

One requested final gate was started. Disk exhaustion interrupted WASM checking after native/SDK/harness/XR success. Only the missing browser-mode gate was resumed; native checks were not repeated. Subsequent user-authorized cleanup removed verified unused compilation caches, preserving source/pkg/output and active work. Exact fresh outcomes are recorded in VERIFICATION.json; a successful historical gate is not relabeled current.

No failing-before/passing-after renderer repair exists. Stop this batch with A unresolved. If authorized to continue this defect later, capture a future failing session with complete per-capture resource definitions and actual native revision/view identity, and retain every camera-step time. Investigate B and C independently; do not assume fixing A closes either. No next batch, publication, deployment or Quest acceptance occurred.
