# Batch 5 — exact-white rigid vertex attributes

Completed locally on the containing codex/next-render-cycle-5 checkpoint, parent6e80f327. No push/main merge/deployment/shared8443 restart. Only batch5 implemented; stop before batch6.

## Accepted work and exactness boundary

Scan every effective CPU RGBA color in each immutable primitive (including unindexed vertices), exact finite [1,1,1,1], no epsilon. Eligible rigid buffers/layouts omit16B per vertex. Geometry/Draw and both shader/pipeline keys carry the fact. Vertex shader writes constant white. Original CPU colors/topology, material base/alpha, textures, normal mapping, skin/morph fallbacks, instance payloads, auxiliary passes and cutting are retained. Shared asset/layout policy cannot change in place while assets are alive. Compact counters stay in debugStats; detailed actual Buffer.size/stride use explicit gltfVertexLayouts outside timing.

**Rejected sub-experiment:** removing Output color varying and fragment multiplication. WebGPU XR original and optimized self-repeats were exact; cross-variant right eye differed1pixel,max2/255. Retain the original interpolator/fragment arithmetic. The exact split between interpolation and backend codegen is unproven. No tolerance/reference weakening. Rejected structured report/build identity/source patch retained; rejected binary and raw eye buffers were not snapshotted. Final retained-interpolator checks are exact. Also preserve/exclude six exploratory ordinary-WG runs with detailed per-primitive JSON in polled stats; that diagnostic serialization was removed before final clean pairs.

## Initial geometry upload and live requested GPU capacity

All four independent ordinary/stress/backend original-layout pairs exact (1280x720). Rome has258 eligible rigid primitives in this workload:

| Metric (bytes) | Original | Candidate | Saved |
| --- | ---: | ---: | ---: |
| Initial rigid vertex upload | 9,417,848 | 5,650,760 | 3,767,088 (40.00%) |
| Live requested shared rigid vertex buffers | 9,417,848 | 5,650,760 | 3,767,088 (40.00%) |
| Initial rigid vertex+index upload counter | 10,897,232 | 7,130,144 | 3,767,088 (34.57%) |

This is not whole-engine VRAM or total network/loading time. CPU geometry, index width, textures and driver allocation overhead are outside the vertex-capacity saving. Immutable geometry sharing makes ordinary and stress asset capacity equal. New asset proof/variant compilation adds cold work; page cold readiness includes network/setup and cannot isolate it.

Native fixture asserts actual stride/Buffer.size: untextured40→24B; base+normal mapped72→56B; base textured48→32B; near-white/alpha/non-white/unindexed non-white remain colored. Missing COLOR_0 and RGB white are eligible. Native boundaries reject each nonfinite/nonidentity RGBA component. Actual WebGPU module/pipeline descriptors confirm absent Input color, constant white assignment, retained Output color and actual vertex strides; GL does not expose GPUDevice descriptors, so use actual buffer sizes/layout assertions, backend validation and independent rendered pixels. GEOMETRY-PROOF/SUMMARY retain all observations.

## Final paired desktop timings

Baseline-only pilots1s/2.5s. WG1024 reached60Hz so bounded WG stress2048 selected; GL1024 already over budget. Same frozen selections/content/effects/seed on both versions. Primary24runs:3AB/BA/AB pairs per configuration,8s warmup/25s sample,1280x720DPR1. Engine profiling/GL timer queries explicitly off; no wrapped API pages, builds or owned GPU runners during timing. Detailed geometry probes were separate. Their Node browser.close cleanup hung after the browser exited; only own idle Nodes were terminated after all eight records/images/manifests were saved and no children remained. Maintained primary cleanup succeeds.

CPU callback wall time includes waits/driver effects, not pure CPU cycles. Named GPU times are unavailable by this clean protocol and are not inferred from submit or FPS. Foreign GPU process30816 remained active; AC power and process/utilization snapshots are archived. Do not claim small gains, isolated throughput or Quest results. Values below are medians of per-run means/quantiles; maxima span all runs. Full per-run frame-interval p95/p99/max, paired ratios, commands/instances/triangles/uploads and raw RAF rows are in RESULTS/evidence. Adjacent frames are not independent repetitions.

| Scene / additional load / backend | FPS | Callback mean ms | p95 ms | p99 ms | max ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| rim-static / 1 / webgpu | 60.003 → 60.002 | 0.716 → 0.666 | 1.200 → 1.100 | 1.400 → 1.400 | 3.400 → 2.700 |
| rim-stress / 2048 / webgpu | 37.441 → 37.616 | 14.955 → 14.835 | 30.700 → 30.400 | 31.800 → 31.200 | 37.800 → 43.200 |
| rim-static / 1 / webgl | 60.002 → 60.002 | 1.369 → 1.350 | 2.100 → 2.100 | 2.600 → 2.500 | 4.000 → 3.800 |
| rim-stress / 1024 / webgl | 42.362 → 42.442 | 8.290 → 8.237 | 15.500 → 15.600 | 17.300 → 17.100 | 25.900 → 20.800 |

**No steady-state FPS gain is proven.** The capped ordinary controls cannot prove throughput, stress gains are small/mixed relative to contention. Geometry/upload/retained-capacity reduction is the demonstrated benefit. Do not generalize static results to active gameplay or VR. All12 final baseline/candidate image pairs exact;4 additional same-release original-layout geometry pairs exact. The prior B4 moving1024-WebGPU strict-pixel issue remains open.

## Verification and reproducibility

[Verification](VERIFICATION.json), [manifest](MANIFEST.json), [results](RESULTS.json), [design](DESIGN.md), [protocol](PROTOCOL.md). Final gate includes strict native/WASM/type/SDK/harness checks and all registered browser scenarios, including real cutting publication, Neon and asymmetric/reentrant XR. Dedicated zero-tolerance mixed-primitive fixture covers implicit/exact/near-white/RGB/alpha, texturing/normal maps, translation/full negative/nonuniform instancing, actual shadow/reflection updates, postprocessing, CPU morph fallback and clear/dispose/reinit. Stronger XR checks require original-vs-initial, reference-vs-candidate and both self-repeats exact in actual per-eye targets. One independent read-only source reviewer; root owns runtime evidence.

Executed command logs are in evidence; core commands: release wasm-pack build with locked dependencies and web_sys_unstable_apis; targeted cargo test exact_white_requires; strict wasm clippy and npm typecheck; maintained test-browser --backend all --page gltf-white-colors,webxr,webxr-webgpu --capture; freeze-render-build baseline/candidate; maintained render-performance calibrate/compare (full args and environment archived); original-layout geometry probe; helper units; VERIFY_WASM_PROFILE=release npm run verify:push. The first sandbox wasm-opt EPERM build is retained as a failed build and was not final timing evidence. Final gate attempt1 found the historical40B assertion in gltf-batching; it was intentionally stopped through runner cleanup (the interrupted cutting case is not a cutting-regression result). The test now asserts intended24B plus an independent original40B exact-pixel reference. Its first reference attempt correctly rejected policy change before deferred clear synchronized; two actual frames complete clear before switching, preserving the lifetime guard. Both-backend targeted rerun passes. Final full gate reruns the stabilized inputs; partial/failed logs are retained.

Archive evidence.tar.gz excludes unchanged historical archives/assets and uses exact-content hardlinks, with every payload/link read back against SHA256SUMS. Reconstruct from parent checkpoint plus frozen changed inputs/pkg; full frozen hash manifests and actual served bytes retained. Final-source contains post-freeze strengthened tests; measured Rust/SDK/pkg are unchanged. INPUT-AUDIT checks both frozen and served identities and final source dump. No physical Quest, fresh Three comparison, CI or publication evidence.
