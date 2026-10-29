# Followup2 — overlay presence, bounded hypothesis

Base0b7af5b75; original runtime B8/f14c4beb. Only this batch authorized. New isolated branch codex/render-followup-2. Prior A six-pixel camera, B historical moving and C culling/shadow issues remain open separately.

## Exact call boundary

State::render_to calls SceneRenderer::draw_postprocess_overlays only for a main view with a deferred final postprocess draw; desktop once, XR per eye. Shadow/reflection rendering and HUD/presentation are outside this function. Ring lookup (bounded two slots) precedes render.overlays.select. Inside: visible glTF presence via draw_count(true); non-glTF has_translucent; grid/light helper flags; two possible glow existence scans with optional prepare_neon_glow (may build geometry/pipeline on cold use); optional ring preparation. Transparent prepare_draws, mesh transparency preparation, helper pipeline creation and actual composite/overlay encoding follow outside select. Composite→glTF BLEND→mesh alpha→rings→glow→helpers order remains unchanged. No blade-trail-specific calculation exists in this scope; any submitted mesh participates through its existing category. HUD is a later pass.

Original draw_count scans every draw, performs an active-node HashSet lookup before inspecting immutable material alpha, counts all matches even though caller only needs existence. Each postprocessed eye repeats this on visible roots, including wholly OPAQUE/MASK assets. No temporary collection is allocated in this glTF selection. Existing last_pose/active/error semantics must remain. Non-glTF instance checks and optional glow preparation are separate work; this batch does not optimize their resource allocations.

## Minimal candidate

Compute one bool has_blend_draws from actual constructed draw/material membership at model creation. Both draw membership and Rc<Vec<Material>> alpha classifications are immutable during model lifetime; cutting/reload creates new models. Reject wholly non-BLEND models directly. For mixed models, inspect material then live active-node membership, stopping at first matching BLEND. Preserve last_pose=None rejection. No published pose count or active_draws cache is read; failed pose updates still consult the actual active set. Fade/dissolve uniforms and view-specific selection/order remain untouched. This is an immutable eligibility guard plus an existence query, not another view/pose cache.

B7 rejected reuse of published active_draws length required a validity flag in pose update/error paths. This candidate adds no pose update/invalidation work and leaves draw_count itself unchanged, including its other callers. Nevertheless the original B7 WG2048/GL1024 throughput workloads must pass repeated comparison; lower select time alone is insufficient. Small/large dynamic Rome and Neon controls remain required.

## Diagnostics and independent path

One cumulative calls increment per selection. Detailed visited-model/primitive/active-set/absence counts and four exclusive stage totals are opt-in under existing profiling. Clean runs perform no per-primitive counting or extra clock reads. Detailed mesh counters duplicate the original short-circuit traversal only while profiling. Clock stages are gltf, mesh, neon, rings; aggregate diagnosticCalls supplies actual invocation count, not a 60-sample-window estimate. Timings remain callback wall time.

setOverlaySelectionReference(true) explicitly executes unchanged original draw_count in clean mode; false uses the immutable guard/live query. Frozen diagnostic baseline defaults to reference; first candidate used the guard on both backends. Three clean WebGL1024 pairs regressed throughput despite lower callback wall time. The intermediate frozen policy retained the reference on WebGL (and unknown backends), using the guard only on BrowserWebGpu. Direct original-runtime WebGPU stress then regressed in all3pairs as well. The delivered source now defaults to the original query on BOTH backends; the experimental guard is explicit diagnostic opt-in only. The original backend-specific policy was rejected too; the delivered default is conservative on both APIs, not a claim about all hardware. Exact browser fixture compares both paths and restoration at fixed state; new captures may not be attributed to prior image failures without evidence. Existing XR/cut/shadow/postprocessing tests supply wider regressions. Frozen original B8 also retained independently.

No quality/resolution/content change, new GPU resources, command order change, pass merge or production server modification. No push/merge. Automatic enablement is rejected; the explicit diagnostic experiment and instrumented source diagnosis are retained. Restored-default release and five focused browser/XR cases pass; full acceptance and restored-default throughput controls are recorded separately.

## Baseline diagnostic observation

Separate profiler-on8s/25s runs: WG2048 observed945select calls; each visited1946models/130287primitives and the same number of active-set tests, zero relevant overlays. glTF stage total/call7.611640ms, mesh .044127ms, neon .000741ms, rings .000212ms. GL1024 observed1065calls/68647primitives per call; glTF4.507981ms, mesh .018216ms, neon .000657ms, rings .000751ms. These are measured diagnostic wall times including counters, not promised clean-frame savings. No geometry creation in either stress selection (glow/ring preparation count0). Full boundaries retained in raw.json.gz.

Baseline B9/B7 calibration identities include the old SDK. The maintained guard rejected reuse after adding the explicit reference API; repeated baseline-only fixed-load calibration creates valid manifests for the same predeclared workloads. No tolerance/hash bypass or candidate-dependent load selection.

## Accounting limits

`debugStats.drawCalls` is actual submitted draw-command count for the latest frame; `renderer_stats.drawCalls` is logical primitive work and must not be labelled commands. Static stress WG2048 has308actual commands (307glTF plus final composition) while the old overlay-presence query visits130287primitives. Batching therefore does not remove this CPU scan automatically. Cumulative glTF boundaries provide exact submitted-instance/triangle/upload deltas; native draw snapshots are sampled, not a complete per-frame command histogram. Native animated Rome has moving enemies and visibility changes but no fired shots/kills in this frozen workload; maintained gameplay/worker/XR checks cover cutting separately.

Detailed GPU diagnostic timestamps are available on the desktop WebGPU device; the baseline observed31fresh samples, scene.main mean26.055ms and fxaa interval3.107ms. These instrumented asynchronous command intervals are not clean-run frame times, GPU busy percentage or proof of Quest cost. The separate GL query spans a different documented interval; it must not be summed with callback wall time or compared as if it were the same scope. Clean runs deliberately have no GPU timing queries.

## New strict-image case D (not A/B/C)

Clustered1024WebGPU primary pair0 fails at4pixels, maximum1/255. Coordinates `(651,114),(652,115),(653,115),(653,116)` lie in distant repeated drone geometry. Independent decoded-PNG comparison of all six captures finds exactly two full-image variants: hash `cfbfcd6e…` in original0/original1/candidate1, hash `eada5af5…` in original2/candidate0/candidate2. Both variants therefore occur without the candidate selector. This is direct evidence of within-original-runtime variability, not an inference from the old image issues. It does not identify the numerical/capture mechanism or make strict equality pass.

The maintained capture calls `pose(.375)` then the existing five-RAF pixel helper. Inputs are intended fixed, but full native draw/resource state at capture is not traced by clean timing; previous dynamic history differs. No capture tool, golden, tolerance, matrix or shader was modified to erase the difference. The new case remains separately open. Automatic selection is already rejected by whole-frame regressions; no unrelated cache/shader refactor is justified in this batch.

## Candidate diagnostic result (separate from clean runs)

WebGPU stress2048: 941 actual selection calls, 1946 models/call, zero primitive or active-set visits, 1946 immutable rejections/call. glTF selection mean wall time 0.044527ms (baseline 7.611640ms). WebGL stress1024: 1054 calls, 1026 models/call, zero primitive/active visits, 1026 immutable rejections/call, 0.039848ms (baseline 4.507981ms). This proves removed work, not whole-frame benefit. Same 308/241 submitted commands and fixed stress content remain.

Neon diagnostic runs: 1501/1502 calls, no glTF model visits on either path; native mesh and neon content present each call. Glow stage averages roughly 0.015–0.020ms, not the source of Rome's historical multi-millisecond selection scan. There is no evidence for caching or changing Neon's glTF selection in this batch. Non-glTF allocations are separately scoped to the next authorized batch.
