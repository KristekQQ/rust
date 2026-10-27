# Actual-light glTF program specialization — 2026-10-26

Completed local implementation and verified Quest deployment. Solo work as explicitly requested; no helper/model delegation, commit or push.

## Hypothesis and implementation

Pinned local Three r170 `examples/rim-three/vendor/three.module.js:20920–20921` selects actual directional/point counts; `:19638–19643` replaces program defines; `:14088` contains the separate light-class loops. Our previous glTF fragment shader always iterated four slots, tested zero colors, and selected a normalized point vector versus directional sun per pixel.

`src/render_lights.rs` derives a program key from the **exact uploaded** four RGB slots and appearance's directional-slot-zero policy. It preserves original slot order and shadow indices. It emits only active per-light bodies, selecting the correct direction formula; BRDF, ambient, exposure, normal/specular-AA, shadow PCF and geometry are unchanged. Other lights, their positions/colors, and enabled appearance remain mutable. Numeric light changes reuse a program until a zero/nonzero slot or light-class transition. At most24 distinct normalized classes exist; new class/material combinations can compile cold programs. No prewarming or claim of hitch-free arbitrary first-use transitions.

Pass caches hold the class corresponding to their actual uniform upload; it refreshes even when object geometry/transform cache hits. Pipeline/shader identity includes class. Instance storage identity normalizes class away: changing lighting cannot duplicate buffers holding identical transforms/material records. Shadow shaders retain their existing source and normalized class. CPU cutting, animation state, object identities and per-eye/pass presentation are unchanged.

## Correctness before timing

Two native tests exhaust all16 masks × appearance on/off and exact nonzero/mutation identities. Strict wasm32 Clippy and release build pass. The independent frozen original generic-loop image oracle uses one persistent static scene with two real instanced normal-mapped metallic objects. It exercises all32 mask/type combinations plus light color/position mutation and exact restoration, on both backends. All70 comparisons pass the predeclared8/255,0.1% gate; max channel difference1/255,68 exact comparisons. Both35-state sequences retain exactly2 instance buffer creations and464 bytes; real instanced commands are asserted by recorded counters. No GL console errors/fallback accepted. Typecheck and11 maintained browser cases pass (PBR/skin/morph/material cache/opaque batching/shadows/actual Rim cuts and direct WebGPU simulatedXR).

## Experiment journal

- Initial oracle's foreground assertion incorrectly rejected a deliberately zero-light dark material. Initial illuminated geometry remains checked; zero-light captures compare to independent generic pixels without an inappropriate brightness floor. Pixel gate unchanged.
- v1/v2 exposed a real GLSL backend failure in multi-light unrolling: Naga suffixed the original local `gl` to reserved `gl_1` etc. It was not a visual tolerance issue. v3 renames only this local before emission; all four-light WebGL cases pass. Failures/logs retained.
- Instance stream normalization was added after source review to prevent unnecessary retained GPU allocations on class transitions. The final35-state mutation counter oracle verifies it.
- Frozen baseline WASM547da17e; final candidate v3 dcce4c0c. Short WebGL pilot2s/6s:15.463→20.224 RAF FPS,308 actual calls and10,553,756 glTF triangles/RAF both sides; max pixel delta1. Pilot is not final acceptance.

## Final repeated measurements

Desktop AppleM1,512 authored Rim stress models,2880×1920,DPR1,profiler off;3 alternating AB/BA/AB pairs,8s warmup/25s samples. FPS is1000/mean application RAF interval, then median across3 runs. It is not compositor/headset FPS.

| Self-comparison | Baseline FPS | Candidate FPS | Actual total calls before→after |
|---|---:|---:|---:|
| Native WebGL2 | 15.221 | 20.081 |308→308|
| Native WebGPU | 37.201 | 38.907 |308→308|

WebGL ratio of medians+31.93%; median paired FPS ratio1.303114; all3 pairs improve. WebGPU ratio of medians+4.59%; median paired ratio1.045856; **one pair is slightly adverse**, so a consistent WebGPU gain is not established. Per-run FPS and paired ratios are retained in summary.json, not hidden by median refresh intervals. Callback wall spans are not pure CPU busy time; no scoped GPU-time reduction was measured in these profiler-off runs.

Separate fresh diagnostic: final nativeGL15.931FPS/308calls versus instancedThree24.754FPS/108calls; Three wins all3 pairs. These absolute values are from a **different sequential campaign**, not interchangeable with the self-comparison above. AA/PBR/shadows differ, and other work migrated shared8443 at23:32:47 during this comparison. Background GPU load/thermal/concurrent work were not isolated. No universal performance or equal-full-graphics claim.

All native stress runs preserve10,553,756 submitted glTF triangles and34,335 instances/RAF,307 glTF commands plus1postprocess command,zero warm instance buffer creations/uploads. Rigid upload payload remains14,664,320B. Retained instance storage is7,568,720B in all GL runs and all but one WG candidate (7,404,352B); this single lower high-water gauge is not assigned a memory-saving claim. Class-mutation oracle independently retains464B and2 buffer creations throughout.

All3GL stress comparisons pass with max channel delta1/255,zero pixels beyond tolerance8. WebGPU is **not exact**: max deltas5/60/17;0/22/2pixels exceed8. Worst22/5,529,600=0.0003979%, below unchanged0.1% gate. Reviewed difference map localizes sparse differences to cropped drone edges/highlights at the top; no broad missing region. Numerical cause not proven. Independent70-state oracle max1/255 remains separate. Ordinary960×640 Rim controls stay60.00FPS,40calls on both versions/backends: capped controls prove no throughput headroom.

## Verified delivery

Release/minified app3064e3f3ea / WASMdcce4c0c / source535ac6bc is installed at home192.168.0.17:8443. All10 final production browser cases pass (both backends' mutable-light, actual Rim entry/cuts/PBR, directWebGPU simulatedXR and Three stress/VR-button control). Registered future full-gate light-class regression also passes both backends; its first attempt lacked the runner's backend marker, corrected in the test only, without changing renderer or timing inputs. Two native tests, strictwasmClippy/release, types,13 nonproduction browser cases and70 frozen-generic-loop comparisons passed. No full push gate, commit/push or CI requested.

A legitimate concurrent task replaced own old65385 with sharedserver76565. Its existing serve:quest transport reads our owned fallback production directory and mounts independently built primary Neon at/_neon. We did not terminate/rebind it or change its watcher/script/arena build. Installed the already release-built, browser-verified package by prepared-directory rename; prior fallback remains atoutput/production-before-light-classes-20261026. All254 manifest payload identities match the tested stage;12actualHTTPS responses match installed hashes/TLS/no-store.5currentNeon payloads unchanged. Same public URLs require only ordinary refresh, no page version token. This shader change is in the owned branch/fallback; primary Neon source/build was not integrated or optimized by this task.

A failed backup copy hit disk exhaustion. Automatic review initially rejected unverified deletion; read-only audit proved548exact duplicate files and7truncated duplicate prefixes,0unique files; complete original release manifest was revalidated before approved conditional removal. Identical files in own immutable failed-v1/v2 snapshots were deduplicated by SHA-verified hard links, preserving every snapshot/unique byte and reclaiming477,118,591duplicate payload bytes. No user/other-task files removed. Stage install then required no extra full package copy. Disk remains low; this was targeted recovery, not whole-machine cleanup.

## Previous physical Quest evidence

One actual user's **previous** app767d2bb4b3 / WASM547da17e WebGL Quest3 session supplied14windows×180tracked frames: median window cadence58.554FPS (49.380–76.576), requested90Hz,1680×1760per eye,scale1,foveation0,AAfalse,XRWebGLLayer. Median callback-submit wall scope6.966ms; no GPU attribution. Sampling excludes untracked/large-gap intervals and is not compositor FPS or a matched Three replay. Simulation/visual sequence matched in the final sample,4cut commits and0reported transport/cut errors. This is **baseline telemetry only**, no physical acceptance of candidate dcce4c0c.

## Next bounded work

Generalize from measured properties, not Rim names. Remaining preparation candidate: reuse cached complete per-model bounds and prove full frustum containment before repeating every primitive test; preserve unknown/deformed bounds and eye/pass/mutation fallbacks. For the actual Quest gap, separate per-eye PBR/FXAA work from WebGL host canvas-copy/presentation before selecting another optimization. Current callback telemetry cannot isolate GPU busy/presentation cost. Four-light correctness passed; generalized throughput beyond this single-sun Rim workload is not established. New light classes may incur cold pipeline compilation; no hitch-free arbitrary first-use claim.

## Evidence limits

An unknown-owned headless Neon Arena PID30810 remains untouched; its GPU helper was observed around29% CPU. Permission to close it is still pending. Runs are contended desktop application cadence, not isolated GPU capacity or Quest FPS. Same process presence does not ensure constant thermal/load interference. Native-vs-Three AA/PBR/shadow/power-preference differences remain; direct engine comparison is diagnostic, not equal full graphics/gameplay. Browser/simulatedXR is not physical-headset acceptance. User reported the previous batching version feels better in Quest but Three still wins; no new headset verdict applies to this experiment.
