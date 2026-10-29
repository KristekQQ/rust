# Followup series final integration — 2026-10-30

Verdict: no broad production-default FPS improvement is demonstrated. WebGL2 panel reuse removes avoidable allocations; full image acceptance remains blocked by separately unresolved historical cases. This is a local review candidate, not an approved release.

## Identity and scope

Initial runtime of this six-batch followup: B8 commit813a5abb266609cceea644e4b1086129a89ca535, carried by B9 evidence checkpoint9986cd4f9e37b1687af339eb741b22ec77f5c720 and F1 checkpoint0b7af5b75082a2508d6fb19d05e7fef4e51c5c77. Frozen root `output/followup-2/original`; WASM `f14c4beb5f3e5b59f16cae9ae90ddccba88b152ed10f07f3ce999d5a765a12de`, source `54c6cde834bd3ad2e5363aabece753cd77cf4aa4537d378147405f6a814797f1`. This is NOT the earlier nine-batch B1 baseline.

Final runtime: code checkpoint08c2d212a12228874e2f998e8f96c9b840facc68, frozen `output/followup-5/probe2`; WASM `34acb880ddcf429a1cd1b62fe43ab07d2a69e3ab4827958d98cb7d02154e3a5d`, source `79b7fcd9f8a4bd6742effc40aaf8741ba652c19fdf0f88b21b697c77b3470f20`. Both measurements use frozen source SDK files with release WASM; this is not a newly minified/deployed Quest package. F6 makes no renderer/SDK/scene changes and does not promote optional modes. Current source/SDK/scenes/pkg:373 files matched the measured final root before runs. Final repeated identity/gate evidence is recorded separately below. Frozen manifests' earlier HEAD fields identify the freeze point before the later local evidence commit, not a different WASM.

[Manifest](batch-6/MANIFEST.json), [protocol](batch-6/PROTOCOL.md), [integration inspection](batch-6/INTEGRATION.md), [all individual runs](batch-6/RESULTS.md), [raw tables](batch-6/RUNS.json), [commands](batch-6/COMMANDS.md). Read-only LAN check found production `https://192.168.0.17:8443` serving WASMc7091506871c74c5055d501dfd85b3a12d8e70dd7905003135dee7939f8109eb. It binds the LAN address, not loopback. No server replacement/restart, push, merge or CI occurred in this batch. A browser refresh there does not load this local candidate.

## Accepted changes and experimental residue

|Batch|Result retained in this local branch|Production/default status|
|---|---|---|
|1|Capture/identity diagnosis; no speculative renderer repair|A remains unresolved; B/C separate|
|2|Overlay-work counters and immutable classification experiment|Original selector remains default; automatic promotion rejected|
|3|Bounded mutable bitmap-panel vertex/index capacity and unchanged index-prefix reuse|Accepted for WebGL2 only; WG and immutable Text retain original behavior|
|4|Measured union/coalesced/full upload controls and exact byte/range diagnostics|Union remains default; no automatic coalescing promotion|
|5|Bounded exact-order ordinal transpose, tests and counters|Mode3 explicit opt-in; mode0 default; no general production/Quest gain claimed|
|6|Integration audit, fresh measurements/gate, evidence archive and skill/memory update|Bounded acceptance below; no new runtime optimization|

Previously rejected opaque depth reordering/published-count reuse are not restored. Diagnostic F2/F4 selectors remain compiled for controlled references; this is **not** a claim that every rejected experimental branch was physically deleted. Their default-path overhead is included in F6 comparisons. Shared-GL XR remains explicit opt-in, canvas fallback and WebGPU owned-eye staging remain intact. Production tests assert mode0. The limited Three common fixture intentionally selects native mode1 (cheap base-color shader) to match its flat diagnostic contract; it is explicitly quality-reduced, not a production optimization. No tolerance, golden, geometry count, quality, shader interpolator, vertex/index layout or production example changed in F6. Content-epoch, invalidation and pool ownership inspection found no actionable integration defect; that inspection alone is not runtime proof.

## Separate image acceptance

|Case|Evidence and scope|Status|
|---|---|---|
|A: B9 camera2048WG6pixels/max2|F1 independently reconfirmed archived failure; fresh normal-path captures did not reproduce it. F6 current-build paired results are recorded in the new pixel audit.|Unresolved; no causal repair or failing-before/passing-after proof|
|B: historical B4moving1024WG8pixels/max2|Historical runtime/report preserved; no new isolated replay of that historical runtime in F6.|Separate unresolved issue|
|C: culling/shadow5014pixels/max79|F1 reproduced in final culling-off and B9 observed original/final. Global switch changes shadow selection too.|Separate unresolved issue; not a pure main-camera oracle|
|D: new followup images|Historical F2cluster4pixels/max1 occurred in both original and candidate variants; later PASS does not identify its mechanism. Fresh F6 pairs and all exceptions are independently decoded below.|Historical issue open; fresh 40/40 deterministic pairs exact|
|E: dynamic scenes|Maintained deterministic panel, motion/cache/pool/layout, asymmetric-eye and actual-worker fixtures cover their defined inputs. Whole authored playing Rim/Neon has no common deterministic screenshot state.|Fixture acceptance distinct from unavailable whole-game exactness|

No successful browser count closes A/B/C/D. No new source repair is claimed for them. Three common/production images are not same-renderer regression oracles. The authored Rim control has six moving enemies, active simulation and a stationary player; observed fired/projectiles/kills/cut commits are zero. It is not a full combat benchmark. The separate dynamic64 fixture performs native cut/spawn/removal, and real-worker replays cover asynchronous publication. Dynamic simulation and real effects stay enabled in performance workloads; no temporal rounding or delayed effects are introduced.

## Measurement boundaries

Apple M1 MacBookAir10,1,16GiB; Chrome156.0.8078.48/.74 over ANGLE Metal on macOS26.6.1, release Rust1.99.0/Node24.19.0.1280x720,DPR1,seed1337,spatial ordering. AC and thermal-warning status recorded every10s in host runner context. No recorded warning does not prove equal temperatures/frequencies. Chrome/Codex/WindowServer coexist;242 observed Chrome crash handlers were idle in that snapshot and must not be mistaken for242 active GPU renderers. Chrome updated between campaigns:19 run-sides on.48,130 on.74. Each paired campaign shares one browser launch/version. Main WG static/camera pairs used.48; other primary campaigns and final gate used.74. [Per-campaign environment](batch-6/ENVIRONMENT.json) records this extra cross-workload limitation. No foreign process was killed. GPU utilization/isolation is not established; small changes are not a causal speedup proof.

Three alternating pairs AB/BA/AB:8s warmup25s sampling; pool and dynamic fixtures60s. Initial1s/5s pilots are separate. F6pool1024 is an explicitly added stress case; historical F4pool256 is replayed separately, so their different capacities are not called a leak. Separate120s soaks and actual-worker replay retain full time series. Profiler-off controls and independent PNG/raw percentile validation are required below. There is no busy-wait/readback/gl.finish in clean timing. Captures/readbacks occur outside timing.

RAF callback wall time includes engine/browser/API stalls and the documented100ms statistics sampler; it is not pure CPU compute. Legacy renderer statistics can rescan primitive visibility, while Three statistics are cached. Actual debug/work-delta draw counters are used instead of the legacy estimated primitive count. CPU parent/child scopes overlap; diagnostic lastMs/history snapshots are not all invocations. GPU pass timestamps and WebGL application-command elapsed queries have different boundaries; clean runs have GPU time unavailable. No mechanical CPU+GPU sum or queue.submit-as-GPU-time inference is made.

## Fresh performance results

149 completed performance run-sides, 243,705 recorded RAF callbacks:141 profiler-off (including short calibration/pilots),8 diagnostic. These are not149 long A/B runs. Three alternating pairs per main workload/API; every individual result and p95/p99 is in [RESULTS](batch-6/RESULTS.md) and [RUNS](batch-6/RUNS.json). Independent [pixel/raw audit](batch-6/PIXEL_AUDIT.json) recalculated all raw callback/frame percentiles and decoded PNGs:40/40 strict native pairs exact;23 non-strict pairs (14 authored gameplay,9 cross-engine) retained separately. No tolerance/golden was changed.

The following entries are medians of three run-level FPS values, not a pooled population or a confidence interval. They summarize the pairs without concealing their signs.

| Workload | Backend | Initial FPS | Final FPS | Interpretation |
|---|---|---:|---:|---|
|rim-stress|webgpu|39.42|39.50|No meaningful throughput gain established|
|rim-stress|webgl|31.04|31.08|No meaningful throughput gain established|
|rim-camera-stress|webgpu|49.69|49.65|Mixed paired changes; no reproducible broad gain|
|rim-camera-stress|webgl|37.11|37.12|Mixed paired changes; no reproducible broad gain|
|rim-clustered|webgpu|25.12|25.18|WebGL2 all three pairs slower; WebGPU mixed|
|rim-clustered|webgl|24.92|24.60|WebGL2 all three pairs slower; WebGPU mixed|
|rim-pool-cycle|webgpu|58.67|58.47|Mixed tails; see long-run evidence below|
|rim-pool-cycle|webgl|52.76|53.02|Mixed tails; see long-run evidence below|
|rim-active|webgpu|60.00|60.00|60Hz cap; moving enemies, no observed attacks/cuts|
|rim-active|webgl|60.00|60.00|60Hz cap; moving enemies, no observed attacks/cuts|
|arena-active|webgpu|60.00|60.00|60Hz cap; allocation benefit is separate|
|arena-active|webgl|60.00|60.00|60Hz cap; allocation benefit is separate|
|dynamic|webgpu|60.00|60.00|60Hz cap; event-boundary mismatch retained|
|dynamic|webgl|60.00|60.00|60Hz cap; event-boundary mismatch retained|

WebGL2 clustered1024 FPS falls25.58→25.29,24.92→24.60,23.55→22.84 (about1.1%,1.3%,3.0%). Callback means rise38.89→39.34,39.93→40.38,42.21→43.51ms. This is adverse repeated evidence, not dismissed because another scope passed. The shared environment and drift across the sequence prevent causal attribution to a particular default code change. No universal performance signoff is granted; a future isolated reproduction/bisect is needed, not an untested speculative rollback here.

WebGPU pool1024 has frame p99 16.8→33.3,16.8→33.3,16.8→16.8ms. Missed intervals cross roughly1% in the first two candidates, explaining the discrete p99 jump; it does not mean the renderer is twice as slow. Callback p99 is20.5→23.8,18.9→22.8,19.5→19.1ms. All warm logical pool creation counts are0. Historical frozen pool256 replay remains around60FPS with16.8ms frame p99 on both APIs; it is separately reported, not replaced by the new1024 stress case.

### Allocation, uploads, memory, and event response

Fresh separate5s API observer: Neon WebGL2 createBuffer38→0 and bufferData allocation4,087,180→0bytes; same14,700 submitted draws/5,182,500 triangles in each observer interval. WebGPU createBuffer38→38. These counters are actual backend calls, not CPU-vector growth. This corroborates F3's bounded WebGL panel reuse, not a new FPS gain or complete elimination of uploads. Do not sum logical counters and API observer counts. [Diagnostics](batch-6/DIAGNOSTICS.json) retain bytes, labels, uploads, panel capacities and unknown size fields.

Camera2048 default union uploads remain about1.11–1.12MB/application RAF in the diagnostic interval, one buffer write/RAF. Instance bytes are already included in uniform-upload bytes, so they must not be added. Initial runtime has no separate instance-call field; bufferUploadCalls is available. No significant default upload reduction is demonstrated. Normal preparation still pays for moving-view grouping; F5 mode3 is not activated in these production-default tests.

Four separate120s [pool soaks](batch-6/SOAK_SUMMARY.json) hold accounted live instance capacity at2,211,456bytes in each of four30s windows, initial/final and both APIs. Warm buffer/bind-group creation and eviction deltas are0. Active/inactive/retired/owned values are retained individually. This is evidence of a bounded accounted pool in this fixture, not proof of whole-engine/driver physical-memory plateau. Initial runtime lacks the new range-scratch metric; unavailable values remain null. Clear/reload and layout/lifecycle transitions are checked by maintained fixtures.

The dynamic64 event fixture contains367 native cuts across12 runs, with actual spawn/remove commands.3/6 pairs have equal event counts; the other3 differ by one cut because warmup/sample starts just across the2s event boundary. [Event audit](batch-6/EVENTS.json) checks count against phase cut-index deltas and preserves all sides. No identical-event-work speedup is inferred from unequal pairs. This workload is distinct from real asynchronous workers.

[Actual-worker replays](batch-6/WORKER_SUMMARY.json):12 PASS, one actual cut per run,3 runs per runtime/API. Request→publication samples (ms): WG initial178.1/171.0/178.4, final175.7/169.5/174.8; GL initial190.0/201.6/198.2, final186.3/179.2/184.7. End point is first observed fully uploaded pieces; not GPU completion, headset compositor latency or subjective response. Three staging steps are not three independent cuts. n=3 per cell is insufficient for a population latency/tail claim.

[Cold variants](batch-6/COLD.json):12 fresh sessions PASS,24 variant observations (white-helmet and colored-RGBA), real foreground and vertex/index layout records. Helmet load is about1.63–1.76s; colored primitive about2.8–11.1ms. No cold-load improvement is established. The loader itself renders callbacks; the separately recorded first post-load callback is not first-ever rendering. OS/driver caches were not cleared, timer quantization can report0ms. Warm results are retained separately; no first-use expense is hidden in a warm FPS figure.

### Cost breakdown, with diagnostic limits

Separate camera2048 diagnostic means, initial→final:

| Backend | glTF queue snapshot ms | Overlay selection snapshot ms | GPU observations |
|---|---:|---:|---|
|WebGPU|9.179→9.801|4.478→4.412|Main pass18.418→18.355ms; FXAA3.075→3.027ms,9 valid sparse observations each|
|WebGL2|9.412→9.866|5.010→4.886|Application GL elapsed42.681→42.327ms,9 sparse observations each|

These are instrumented scope snapshots, not all per-call latency samples or clean A/B speedup estimates. Parent color encoding includes glTF preparation. In Neon WebGPU, final composition/postprocess is about5.7–5.8ms, SSR trace1.82–1.84ms and main1.14–1.15ms in16 sparse observations. Neon WebGL elapsed about13.36→13.40ms. The principal remaining observed costs depend on workload: moving-view CPU preparation/selection, camera main GPU rendering, and Neon postprocessing. No GPU cost was inferred from queue.submit or CPU API duration. Clean-run GPU times are explicitly unavailable.

## Three comparison: limited, not a universal victory

Pinned Three r170 and existing per-part InstancedMesh representation; identical model inputs, view, resolution and frozen sequence within each declared comparison. Renderer culling and production graphics differ. All nine pairs have profiling disabled. No deliberately uninstanced clone reference or throughput normalization by triangle count.

| Profile / renderer | Median FPS of3 runs | Actual draw commands at capture | Submitted glTF / Three triangles |
|---|---:|---:|---:|
|Common native WebGPU|44.73|287|39,061,900|
|Common Three paired with WG|38.50|86|42,009,212|
|Common native WebGL2|40.20|287|39,061,900|
|Common Three paired with GL|38.48|86|42,009,212|
|Production native WebGL2|31.06|308|39,067,116|
|Production Three|30.88|108|42,014,462|

Native production total308 includes the final fullscreen pass; glTF main307. Native triangle counter covers glTF submitted geometry, not the fullscreen postprocess triangle; Three counters cover its renderer submissions. Source-matched common profile is flat city/drone, no AA/lights/shadows/fog/tone mapping/postprocessing, explicitly native mode1. Still50 changed pixels/max255, and native culling submits fewer triangles. Production native uses FXAA and its own light/material path; Three's existing production AA/light/shadow/post path differs, with705,262 changed pixels/max219. These are preserved quality-contract differences, not evidence of exact native pixel regression. Actual quality state/config/source metadata is in each report. Common native is faster here, but this is not a matched production-quality victory and not evidence that the followup changes caused that lead. ProductionGL/Three is roughly comparable and mixed across pairs. ProductionWG/Three is unsupported by this maintained fixture; its guard was not bypassed. Physical Quest cannot be inferred.

## Intermediate checkpoints and rejected experiments

These are historical stage evidence, not newly run intermediate binaries in F6. F6 directly reruns the true followup initial versus final runtime; adding historical deltas together would be invalid.

- [F2](batch-2/REPORT.md): immutable overlay classification reduces selector work but does not establish acceptable whole-frame behavior; original selector retained by default. Historical four-pixel clustered discrepancy remains open.
- [F3](batch-3/REPORT.md): WebGL-only mutable panel capacity/index-prefix reuse accepted. Fresh F6 API observations reproduce the allocation removal. WebGPU original allocation path remains; no FPS benefit proved.
- [F4](batch-4/REPORT.md): coalesced sparse uploads save bytes but increase writes; WebGL throughput regressions rejected automatic adoption. Union stays default. Optional reference controls remain compiled, not silently enabled.
- [F5](batch-5/REPORT.md): explicit mode3 bounded ordinal transpose had camera2048 paired improvements under its recorded conditions; mode0 stays default. F6 therefore does not claim to revalidate that optional gain in production. Historical mode3 regressions/tests remain available; no new promotion.

## Final maintained verification

Actually executed once with forced tests: `VERIFY_FORCE=1 VERIFY_WASM_PROFILE=release npm run verify:push`, exit0. Fresh results:433 Rust PASS,2 ignored,0 failed;169 browser PASS (65 WebGPU,104 WebGL2);32 SDK,71 XR/platform,14 Node harness and13 Python harness PASS. Rust format/Clippy, wasm32 check/Clippy and SDK types pass. Source dump regenerated. The gate reused the verified release WASM bytes, not an old test receipt. Browser gate Chrome156.0.8078.74, sandbox enabled,23:23:31–23:34:35 UTC on2026-10-29 (00:23–00:34 local30th).

[Verification](batch-6/VERIFICATION.json) records observed counts and receipt. [Final identity](batch-6/FINAL_IDENTITY.json):763 frozen files match current source/SDK/tests/scripts/assets/pkg; missing files would fail.98 asset files,185 example files and190 third-party files match initial/final exactly. Post-gate release fingerprint matches measured runtime. Receipt-only check through the same npm launch passes without another full suite. Direct-node receipt invocation has an environment-order false miss; original tool is unchanged, and that attempt is recorded. The first standalone fingerprint omitted release profile and rejected default-dev configuration; corrected release invocation passes without rebuilding.

Independent review corrected one protocol-description error (dynamic samples are60s); otherwise no actionable contradiction in the reviewed result tables and acceptance boundaries. Existing skill validator passes after installing its missing PyYAML only into an ignored local output directory. No runtime/test tolerance/reference change was needed.

## Delivery and acceptance

Functional acceptance, exact-image acceptance, and performance acceptance are independent. This branch is prepared for review, but unconditional integration/release remains blocked by image uncertainty and adverse performance cases. No production refresh claim, no physical Quest acceptance, no remote push/merge/CI, no shared-service restart. F6 changes only evidence/documentation/the existing performance skill; renderer runtime remains F5. Remaining priorities: causal reproduction of A/B/C/D; isolated clusteredGL/pool-tail attribution; physical Quest validation of defaults and explicit XR prototypes. No additional optimization batch starts automatically.


| Acceptance axis | Final status |
|---|---|
|Functional / maintained regressions|PASS in the specified native/browser/simulated-XR fixtures; not proof of every gameplay state|
|Fresh deterministic native image comparisons|40/40 exact; no new deviation found in these samples|
|Overall image signoff|BLOCKED: A,B,C and historical F2clusterD causal issues remain unresolved; E whole authored gameplay oracle unavailable|
|Production-default performance|PARTIAL: GL panel allocations improved; broad FPS gain not established; clusteredGL and WG pool-tail observations adverse|
|Optional F5 mode3|Retained opt-in with historical bounded evidence; no fresh F6 promotion or universal claim|
|Physical Quest|UNVERIFIED: both-eye visual correctness, comfort/FPS, session lifecycle and shared-GL prototype require device evidence|
|Integration/release|Local review/evidence candidate only; unconditional release remains blocked, no push/merge/deployment|

[Archive verification](batch-6/ARCHIVE_VERIFICATION.json) and [index](batch-6/EVIDENCE_INDEX.json) describe the content-addressed archive, complete object/alias validation and actual fresh-directory restoration. Historical archives are not recursively embedded. Raw outcomes, manifests, figures, scripts and current verification logs remain available. The checkpoint containing this report is the F6 local evidence commit on `codex/render-followup-6`; runtime code checkpoint is08c2d212a. Exact evidence commit can be obtained by `git log -1 -- docs/performance/render-followup/FINAL_REPORT.md`. The LAN server's WASM/bindings/SDK hashes were rechecked unchanged after the gate. This completes the six-batch series; further investigation needs a separate user assignment.
