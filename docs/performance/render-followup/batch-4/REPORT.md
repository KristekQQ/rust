# Followup 4 — dynamic frames and instance uploads

Status: experiments completed; union remains the production default. Final verification/checkpoint evidence is recorded separately. Batch 5 is not authorized.

## Scope and hypothesis

Original fine payload reuse packs only changed records, but uploads their union address span. With sparse changes in several primitive groups, unchanged holes dominate transmitted bytes. This batch compares union, adjacent changed-record ranges and full active address spans. It changes neither record mapping nor scene/camera/simulation quality.

Runtime source and byte identities are in MANIFEST.json when finalized. Original B3 remains separately frozen. Probe release is `7829658f7705608ce2e4b445b36c375c010bec516f000f778414e8b61a9c0d63`; default policy remains union on both backends.

## Implementation and ownership

- `src/instance_upload.rs`: exact 4-byte-aligned address ranges, bounded to 4096 retained spans; overflow safely uploads the union. No matrix rounding or unbounded per-camera cache.
- `src/render/gltf_gpu.rs`: use original stream/page CPU mirrors, ownership and content epochs. Collect required record bytes, submitted bytes/calls and gaps. Detailed CPU-mirror byte differences and range bookkeeping time are opt-in.
- `src/render/renderer.rs`, `src/web.rs`, JS SDK: explicit diagnostic policy only; retain original default.
- Maintained benchmark helpers: deterministic all-moving roots, policy selection, strict identity checks, explicit profiler enable/disable, separate API upload timing and per-frame diagnostic context.
- Focused tests cover exact independent output, sparse/full layouts, stream/page fallback, mirrored objects, lifecycle, shadow/reflection paths and asymmetric simulated XR/real cuts.

Instance upload bytes are a subset of uniform upload bytes and are never added twice. Exact changed bytes refer to differences against the CPU mirror, not physical GPU-memory readback. CPU range timing includes diagnostic counting/marking and excludes parts of original packing/comparison; it is not a pure search-cost benchmark. Record mapping remains unchanged, so this experiment adds no remapping algorithm.

## Diagnosis (pilots and separate diagnostics)

Sparse 2048-root workload changes one root: union sends 1,081,616 bytes in one write per RAF; coalesced ranges send 1,072 bytes in 67 writes. About 282 bytes differ in the CPU mirror in the diagnostic window. Sending less data therefore increases API-command work.

The 1s warmup/5s pilot WebGL2 cadence fell from about 31.0 to 19.5 FPS with coalesced writes. The analogous WebGPU 36.0 to 36.7 FPS observation is too short and small to establish a gain. Full-active mode sent more bytes without a convincing pilot benefit, and was not advanced. Three-pair confirmation and controls are separate.

Pinned GLES source and actual API observations agree on emulated host staging → `bufferSubData`. Warm sparse windows have zero `copyBufferSubData`, `bufferData` and `createBuffer` calls. There is no evidence for per-write GPU flushes. See [source audit](SOURCE_AUDIT.md). Timing the synchronous API wrapper alone does not explain all of the observed frame difference.

Independent diagnostic tail analysis joins each slow callback to the exact recorded `frameStamp`. All-moving scenes still typically send one contiguous payload; coalescing cannot remove that traffic. Worst observed pool callbacks had no corresponding instance-buffer creates/evictions. Clustered transition frames include substantial `scene.gpuUpdates`/`gltf.update` work. These are instrumented observations and correlations, not isolated causal proofs.

## Evidence boundaries

Main runs use frozen workloads, 8s warmup, 25s samples (pool 60s), three alternating pairs, profiling off. Detailed profiling, whole-API observers and per-frame snapshots run separately. No busy-wait, `gl.finish`, synchronous GPU wait or timed readback is introduced. PNG capture is outside clean timing.

Host is a nonisolated M1 Mac. AC and thermal-warning observations are retained, but they do not measure junction temperature or establish equal GPU contention. During first long controls, two macOS storage processes together used approximately 170% CPU. Foreign processes were preserved. A short parent PNG-analysis operation also overlapped a control and is disclosed; heavy analysis is subsequently deferred. Small control differences cannot establish instrumentation neutrality or a speedup under these conditions.

Original versus probe-union controls test added instrumentation separately from policy A/B. Current pool-cycle observations do not independently reproduce the historical B9 pool-on/off experiment; they cannot retroactively prove or disprove that old p99 difference. Old image cases A/B/C/D remain separate. Desktop and simulated XR are not physical Quest acceptance.

## Worker timing boundary

The maintained cut fixture measures request-to-publication from the first nonempty `rimTakeCutJob` call through the end of the first simulated XR frame exposing fully uploaded pieces. It excludes GPU completion and the compositor. Kill-to-completion includes observation delay and is a separate field. Three deterministic replays per policy/backend are few publication events; do not present their p99 as a well-estimated population tail. No scheduling or publication deferral was added by this experiment.

## Decision

**Do not enable coalesced or full-active uploads automatically on either backend.** Keep the original union default. Retain the bounded explicit diagnostic switch and regression fixtures to make the rejected experiment reproducible. This batch establishes no accepted production FPS improvement. It does not introduce a new pool, stable record mapping, deferred effect or magic merge threshold.

The sparse WebGL2 long comparison sends1,081,616→1,072B/RAF but changes1→67 writes. All three FPS pairs regress:13.35→10.54,13.55→10.64,13.55→10.80. Median per-run callback p99 worsens90.3→179.9ms, with individual tails retained. Camera WebGL2 also regresses cadence, while clustered/all-moving results are mixed or favorable; that does not justify a global policy.

Quiet sparse WebGPU median per-run FPS is36.18→36.82, but all-moving32.40→32.34 and camera callback p99 36.6→37.2ms do not demonstrate a uniform benefit. Static stress also has no benefit. Ordinary/active Rome and pool are60Hz-capped. See [all selected three-pair results](RESULTS.md). Lower byte traffic is proven; a reusable throughput/tail gain is not.

Original B3→probe-union home WebGPU camera controls: FPS51.29→49.48,51.80→50.12,47.65→51.37; callback means19.16→19.89,18.98→19.70,20.69→19.23ms. This bounds the new diagnostics in a realistic control but does **not** prove zero overhead or a small speedup: the sign reverses, and the host is not isolated. Existing same-campaign WebGL controls remain around37FPS; ordinary controls cap at60. Do not choose only the favorable third pair or rerun until a preferred sign appears.

## Complete evidence and tails

218 completed runs:198 profiler-off runs (including40 short pilot runs) and20 separate diagnostic runs;208768 complete RAF rows, of which203956 belong to profiler-off runs. Two completed run sides/6333RAF from the subsequently interrupted WebGL pool campaign remain in the archive but are excluded from completed-campaign totals and acceptance. There are102 independently rechecked exact deterministic PNG pairs, zero new deterministic failures and six authored-active comparisons whose image equality is unavailable. Prior A/B/C/D image problems remain unresolved independently.

Clean GPU times and detailed exact-byte/range-time counters are unavailable, not zero. Detailed diagnostics use actual timed prepare-call counts: sparse WebGPU eligible/groups/basis/payload/records/resources means approximately1.00/6.24/1.95/2.55/0.029/0.620ms with union and0.92/6.12/1.87/2.43/0.020/0.360ms with coalescing. They are instrumented stage wall times, not pure compute or values to add to GPU time. Upload resource preparation is only one component of the frame.

`DIAGNOSTIC_TAILS.json` correlates the three slowest callbacks in each of20 instrumented run sides with the exact frameStamp and records phase, visibility/dirty/layout/cache/pool deltas and scopes. Worst pool callbacks do not coincide with instance creates/evictions; clustered transition tails include scene GPU updates and glTF updates. Signed active/layout capacity deltas are gauges, not resource-create counters. These correlations do not establish causality.

The clean recorder deliberately disables profiler-frame counters. Its sparse samples therefore cannot be safely joined to individual RAF stamps using the existing data. `CLEAN_TAILS.json` marks all158 such alignments unavailable, retains callback stamps and boundary totals, and does not invent same-frame events. The independent instrumented per-frame experiment supplies event context; its timings are not substituted for clean distributions. Historical B9 maximum/p99 claims are not reproduced as controlled code regressions by this batch.

## Worker publication and memory

Twelve real-worker simulated-XR replays pass (three per policy/backend). Request-to-observed-publication medians: WebGPU175.5→170.6ms; WebGL2 204.2→206.4ms. Only three publication events per cell: these are observations, not statistically established latency gains. Max worker-handler wall time spans0.2–0.4ms; actual accept/prepare/install/frame/queue context is retained in `WORKER_SUMMARY.json`. The unchanged publication schedule is not an optimization by delaying an effect.

Both120s coalesced pool soaks complete7202RAF and60 many→few→none→many cycles (241 recorded phase transitions including the boundary), with exactly622512B accounted live instance resource capacity in every30s window, no new warm buffers/bind groups, no evictions and no over-budget events. Range scratch stays32B on WebGPU and512B on WebGL2. This is engine-accounted capacity, not measured driver residency. Three60s union runs provide the paired reference. Clear/reload and the32768B wasm range scratch cap are separately checked by the focused fixture, including explicit zero scratch after clear.

## Delivery boundary

See `VERIFICATION.json` for actually executed final checks and `MANIFEST.json` for identities. The first gate stopped on a misplaced SDK JSDoc parameter; comment placement/types were corrected and the maintained gate retried. The correction changes SDK documentation bytes only, not executable behavior or measured WASM. Both probe and final SDK hashes are retained. No tolerance/reference changes. No push, merge, deployment, shared-server restart, fresh Three comparison, physical Quest or CI claim.

Only a future hypothesis is retained: prove whether exact ordered translation payload bytes can be shared across compatible primitive groups, to avoid both union gaps and many tiny writes. This is not implemented, not a reason to start batch5, and requires its own correctness and A/B evidence.
