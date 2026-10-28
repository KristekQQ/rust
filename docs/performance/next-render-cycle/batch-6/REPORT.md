# Batch6 — lossless portable16-bit glTF indices

Completed on the containing isolated `codex/next-render-cycle-6` checkpoint, parent `4969d9f984f152101dc4bf2d930442be535f9c7e`. Only batch6 is implemented. No push/main merge/deployment/shared8443 restart; stop before7. [Manifest](MANIFEST.json), [results](RESULTS.json), [verification](VERIFICATION.json), [design](DESIGN.md), [protocol](PROTOCOL.md), [review](REVIEW.md).

## Implementation and correctness

Select Uint16 only when the actual normalized index sequence has maximum≤65534. Higher values retain Uint32 regardless of item count. Normalize existing line-loop/fan conversions first, preserving exact order; all other modes retain their sequence. Missing indices generate the original sequential indices. Conversion is checked; no reindexing, overflow or lossy cast. Original CPU ModelAsset/collision/cutting data remain Uint32 and unchanged. This batch changes glTF GPU storage, not every unrelated native-mesh path or network/source asset size.

[WebGL2 fixed restart](https://registry.khronos.org/webgl/specs/latest/2.0/#5.17) applies to all indexed topologies:65535 is unsafe as an ordinary Uint16 index even in triangle lists. Our conservative portable fallback handles that. Original source Uint32 can legally contain65535. [wgpu30.0.1 strip state](https://docs.rs/wgpu/30.0.1/wgpu/struct.PrimitiveState.html#structfield.strip_index_format) is explicit in the pipeline Variant key; non-strip pipelines need no extra index-format variant. Geometry→Draw→PreparedDraw preserves format and draw_prepared binds it for every pass/eye, including skin/morph paths. Immutable shared-asset identity and existing plan invalidation remain; no new camera cache or instance grouping policy.

DeviceExt pads actual allocation to4B. The logical count remains independent:3 Uint16 indices=6 payload bytes/8 buffer bytes, but only3 indices are drawn. Compact live/cumulative index stats and explicit on-demand gltfIndexLayouts expose actual Buffer.size, logical bytes, format, maximum and count. Original-U32 control rejects policy changes while assets are live; empty clear/reinit permits a new layout and invalidates old plans. Shared source removal retains buffers used by instances.

A new all-topology shadow test exposed an existing invalid pipeline: nonzero depth bias on point/line shadows. A bounded repair restricts existing bias to triangle list/strip, as required by pinned wgpu validation. Rome triangle shadow values are unchanged. Original-U32 and Uint16 auxiliary-pass oracles both run through the repaired valid path; these tests do not assert pixel identity to an invalid pre-repair point/line shadow pipeline.

## Demonstrated index capacity/upload benefit

Independent original-U32/candidate diagnostics in the same corrected release, ordinary and stress Rome on both backends, prove258 shared primitive buffers across all instances. Actual WebGPU createBuffer and setIndexBuffer calls agree with Rust Buffer.size/layout diagnostics; WebGL drawElementsInstanced type changes5125→5123. Full per-primitive details/descriptors/screenshots remain in archived INDEX-PROOF.json.

| Rome metric | Original Uint32 | Candidate Uint16 | Saving |
| --- | ---: | ---: | ---: |
| Shared index payload bytes | 1,479,384 | 739,692 | 739,692 (50%) |
| Requested live index-buffer bytes | 1,479,384 | 739,692 | 739,692 (50%) |
| Initial copied index payload bytes | 1,479,384 | 739,692 | 739,692 (50%) |
| Initial rigid vertex+index payload | 7,130,144 | 6,390,452 | 739,692 (10.37%) |

These Rome primitives happen to require no padding. Dedicated odd-count fixtures assert the actual padding separately. Shared asset capacity is equal in ordinary/stress configurations; repeating instances does not duplicate the immutable index buffer. Vertex bytes, CPU geometry, imported/network data, textures and driver overhead are unchanged. This is requested buffer capacity and copied CPU payload, not physical VRAM or measured bus traffic/loading latency.

## Paired desktop results

24clean runs/30,049RAF records:3AB/BA/AB pairs per configuration,8s warmup/25s sample,1280×720DPR1. Baseline B5 release and candidate content identities frozen, actual served bytes verified. Baseline-only1s/2.5s pilots selected WG2048 and GL1024 stress; ordinary controls are60Hz capped. No owned builds, other owned GPU runners or heavy hash audits during primary timing. Profiler/GL timers explicitly disabled. Native GPU times unavailable by clean protocol; do not infer them from queue.submit or sum CPU/GPU times. Callback wall time includes waits/driver effects and is not pure CPU-cycle time.

| Scene/load/backend | FPS | Callback mean ms | p95 ms | p99 ms | max ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| static/1/WebGPU | 60.002→60.002 | .784→.746 | 1.300→1.400 | 1.700→1.700 | 3.100→3.100 |
| stress/2048/WebGPU | 37.616→37.736 | 14.796→14.977 | 30.300→30.700 | 31.700→33.100 | 39.300→44.600 |
| static/1/WebGL2 | 60.002→60.002 | 1.253→1.262 | 2.000→2.000 | 2.400→2.400 | 2.900→3.500 |
| stress/1024/WebGL2 | 42.425→42.562 | 8.222→8.215 | 15.500→15.600 | 17.100→17.500 | 23.300→26.800 |

Values are medians of per-run means/quantiles, maxima across runs. Raw frame-interval and callback p95/p99/max, individual paired ratios and adverse spikes are preserved. Frame-interval WG stress max50.0→66.6ms is retained. Foreign GPU process30816 remained active; power/process/utilization snapshots are archived. **No steady-state speed gain is demonstrated.** Small FPS differences, mixed CPU tails, contention and capped controls do not prove improvement or isolated GPU throughput. Memory/upload savings are accepted; speed hypothesis remains unproven. No active-game/VR/Quest FPS extrapolation.

Submitted glTF commands/instances/triangles are equal in every pair. Static39 glTF commands/67,292triangles; WG stress307/39,067,116; GL stress240/20,714,924 per application RAF. All warm rigid payload deltas and new instance-buffer creation deltas are0. These are glTF counters, not all scene draw commands. All12baseline/candidate pairs and4independent original-U32 Rome pairs are exact at zero tolerance.

## Verification and retained limits

Four native focused tests cover65534/65535/65536 for all modes, many small-valued indices, few high indices, independent loop/fan/strip expected sequences, nonindexed and odd payload count. Dedicated actual-browser fixture covers every supported topology, actual format/count/padding/size/upload, original-U32 exact pixels, mirrored/nonuniform instances, skin palette and CPU morph branch assertions, auxiliary passes, shared source removal, clear/dispose/reinit. Original-U32 XR reload compares actual left/right targets exactly; existing asymmetric/reentry tests and cutting/Neon/lifecycle remain in the final maintained gate. Source review is independent; runtime proof is root-owned.

Executed commands/logs: cargo test gltf_indices --locked; strict target wasm32 clippy; locked release wasm-pack build with web_sys_unstable_apis and wasm-opt; build-fingerprint --write; npm typecheck; maintained test-browser --backend all --page gltf-indices,gltf-batching,webxr,webxr-webgpu --capture and corrected both-backend index runs; freeze-render-build baseline/candidate; maintained render-performance calibrate/compare with exact archived argument lists; original-U32 probe; final VERIFY_WASM_PROFILE=release npm run verify:push; content-bound receipt and source dump. Early compile counter type errors and pre-bindgen SDK type errors are retained. First partial browser run found invalid point/line shadows and a WebGL blank result; source editing invalidated that partial run's final build check, so it is not accepted final evidence. The original unlabeled WebGL blank case was not independently isolated. Final corrected/stable both-backend fixture passes. No test references or tolerances were weakened.

First separate diagnostic saved all8records/exact pairs/manifests but left its own idle Node after its browser exited. Verified no children, retained PID evidence, terminated only that Node; no timing overlapped. Repeated diagnostic persisted bounded browser/server cleanup (both complete/unforced), then explicit process exit after results, because native Node handles could linger. Primary maintained runners all cleaned up successfully. Pre-repair binary was not separately snapshotted; failure/build identities, source reconstruction by the bias condition, and captured failure artifacts are retained. No claim of historical failed screenshot identity beyond archived copies.

The inherited B4 moving1024-WebGPU strict image issue remains open; static exactness does not close it. No physicalQuest, CI, fresh Three.js comparison or shared-server delivery evidence. Evidence archive excludes unchanged parent assets/dependencies/history and reads every archived payload/hardlink back against SHA256SUMS; full frozen roots remain ignored locally. Final measured Rust/SDK/pkg equality and served identity audits are authoritative in VERIFICATION/MANIFEST.

Final gate result:428native PASS/2existing manual timing tests ignored,147browser PASS/0FAIL; strict WASM/SDK/harness/types PASS. Content-bound receipt reused after stable inputs. Identity audit:5,016frozen files,176served payload records,36primary driver-file checks, final Rust/SDK/pkg equality. Archive 18631245B/364entries, SHA256`7842f9ca84a5250837a6b768c1a7f74904e8f4f6c391b4b2594cd4c6318b04b0`; every payload/hardlink read back.
