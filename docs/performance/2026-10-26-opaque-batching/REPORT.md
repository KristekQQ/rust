# Generic rigid opaque batching and vertex storage — 2026-10-26

Local implementation on `codex/wasm-render-performance`, base `64bfda680483e71f6aeb6bf902e63b29b3e90791`. No commit/push/CI or physical-headset acceptance is implied.

## Result and evidence boundary

Fixed authored Rim stress: 512 requested drones, 2880×1920 DPR1, unchanged native geometry/effects/appearance, profiler off. Three alternating AB/BA/AB pairs use 8s warmup and 25s sample per run. FPS below is 1000 / mean RAF interval, summarized across runs; it is application cadence, not compositor/headset output.

| Comparison | Baseline median FPS | Candidate median FPS | Baseline calls | Candidate calls |
|---|---:|---:|---:|---:|
| Native WebGPU before/after | 43.042 | 55.082 | [5266] | [308] |
| Native WebGL2 before/after | 16.230 | 19.053 | [5266] | [308] |
| Final native WebGL2 / Three instanced WebGL2 | 18.081 | 31.622 | [308] | [108] |

A pre-existing orphan headless Neon Arena from October22 remained active on the Mac (PID30810, GPU helper observed around30% CPU). Its ownership was unknown; permission to close it was requested and not assumed. All these pairs retained that background workload, which does **not** guarantee constant interference or isolate thermal/frequency effects. Do not present these numbers as clean GPU capacity, universal renderer superiority or predicted Quest FPS. The previously interrupted v2 GL series has five completed rows and no final raw/cleanup record; it is retained solely as incomplete diagnostic evidence.

## Exact technique transferred

Pinned local Three r170 `examples/rim-three/vendor/three.module.js:29758` routes InstancedMesh to `renderInstances`; `:17797` calls `gl.drawElementsInstanced`. The reference scene explicitly prepares per-part InstancedMesh groups (`examples/rim-three/stress.js`); this is not automatic batching of arbitrary Three roots. Rust already shared assets and instanced disjoint roots. Root AABB overlap fragmented the same512 drones into5265 main draws.

New `opaqueReorderable` is an explicit, mobility-independent author promise of no inter-root coplanar/order dependency. The renderer conservatively requires the whole immutable asset to be rigid and non-BLEND. Default false, unsupported/deformed/blended roots and intervening order-sensitive roots retain their fallback. Eligible same-asset runs merge existing ordinal streams within a128-root scheduling window, preserving input order between windows and1024-instance page caps. Mirror/material/options/culling/pass/eye/lifetime constraints remain intact. Real planar reflections use `clipped`; `PipelineKey.reflected` is winding, including authored main cameras. Shadow/reflection/clipped queues retain their ordering.

Rigid vertices now copy only the Float32 attributes already used by the material pipeline into a compact GPU buffer (56–96B). Offsets/stride and pipeline cache distinguish compact rigid storage from the original96B skin/CPU-deformation path. Source CPU geometry, collisions/cuts, normals, tangent math, precision, color and shader formulas are preserved. Material classes determine used UV slots; no per-example or model-name branch is added.

## Correctness and resource evidence

All three native WebGL2 before/after captures are exact. WebGPU captures are
**not exact**: each has1972 pixels over channel tolerance8 (0.0356626% of the
image), max channel difference103/255, mean absolute channel difference0.0082323.
This passes the unchanged, predeclared0.1% pixel-fraction gate. Visual review of
the archived difference map localizes changes to small drone edges/highlights,
without a broad missing region; the numerical cause is not proven. The v2 batch
pilot already differed at1873 pixels, so this is not attributed solely to packed
storage. Do not describe the authored WebGPU scene as pixel-identical.

The independent browser fixture uses separately loaded assets and a zero-morph CPU96B reference. OPAQUE/MASK cutouts, mirror/nonuniform scale, sparse UV4 and all-five-map layouts, shared rigid+skin primitive, main authored camera, policy-only static toggles, barriers,128/129-root windows, movement/options/removal and default coplanar/BLEND/morph fallback pass both backends. Auxiliary shadow/reflection actual draw order and pixels match. Offset views are not XR. Existing full glTF/PBR/animation/visibility/batching/mirror/shadow and actual simulated Rim cuts/stress/directWebGPU XR scenarios pass. Detailed browser reports are archived; no physical Quest acceptance is claimed.

New native policy revision test,32SDK tests,5stress authoring tests, strictwasm32Clippy and releasebuild passed. Further final production/native checks are recorded in `deployment.json` when completed.

- pairs-wg: identical submitted glTF triangles 10553756→10553756, instance records 34335→34335; warm instance uploads 0B / buffer creations 0. Rigid upload payload 24081912→14664320B; retained instance-buffer gauge 4181088→7568720B, scratch 1201088→1079392B. These gauges are not total GPU/process memory.
- pairs-gl: identical submitted glTF triangles 10553756→10553756, instance records 34335→34335; warm instance uploads 0B / buffer creations 0. Rigid upload payload 24081912→14664320B; retained instance-buffer gauge 4181088→7568720B, scratch 1201088→1079392B. These gauges are not total GPU/process memory.

## Experiment decisions and limits

- v1 rejected as wiring: `!PipelineKey.reflected` accidentally disabled authored main-camera batching; calls stayed5265. Frozen bytes and pilot remain archived.
- v2 global part-major batching improved all3WebGPU pairs with matched pixels. Partial GL rows reduced callback time but did not establish a consistent FPS gain; interrupted final pair is not counted as completed evidence.
- v3 bounded128-root windows were tested as a locality/early-depth hypothesis. One GL pilot20.13→19.71FPS did not prove a gain or the overdraw cause. The bounded scheduling policy is retained in the final combined experiment, not assigned an isolated speedup.
- v4 lossless vertex packing passed all oracles and cut rigid upload payload by about39%; isolated short GL pilot19.21→19.42FPS is within noise and is not a standalone speed claim. Final repeated comparisons measure the combined final implementation.
- Retain adverse rows and full distributions in summary/raw archives. The driver median-interval ratio may hide missed refreshes; this report explicitly uses mean-interval-derived FPS. Callback wall time can include GPU/driver backpressure and is not CPU busy time.
- Diagnostic WG v3 sampled pass medians: scene.main16.152ms, FXAA7.842ms. GL asynchronous application-RAF query scopes include submission gaps/competition; their120–137ms values are not pure GPU busy time and are not summed with CPU or RAF periods.
- Native-versus-Three uses identical requested assets/poses/load/camera/resolution. MSAA4 versusFXAA, shader/lighting/shadow implementation, power preference and counter ownership differences remain. No cross-engine pixel/full-game equivalence is asserted.
- Dynamic spawn/cut/despawn and ordinary Rim controls below establish bounded transfer/regression evidence, not standalone reliable speedups.

- control-dynamic-webgl: 60.003→60.003RAF FPS; pixel result [True]; actual per-run commands retained in summary.
- control-dynamic-webgpu: 60.003→60.003RAF FPS; pixel result [True]; actual per-run commands retained in summary.
- control-rim-static-webgl: 60.002→60.003RAF FPS; pixel result [True]; actual per-run commands retained in summary.
- control-rim-static-webgpu: 60.002→60.002RAF FPS; pixel result [True]; actual per-run commands retained in summary.

Baseline WASM `36cabbf4ac596d867181c8278a848b99fc3197910f344835b02a3aba519c7ea2`.

Final WASM `547da17e984617d00d63d681e36401b1d77da7c5cdd1aabc902eb5f61cc23356`.

Local frozen roots under `output/opaque-batching/` retain runtime/source snapshots. `evidence/implementation.patch.gz` is scoped from that baseline to the final source. Archives contain manifests, driver/build identities, all completed per-run data and capture hashes. Full PNG captures remain in the local frozen-run output directories, preventing duplicate multi-megabyte images in versioned evidence. No quality gate was widened. The next performance verdict requires an isolated matched Quest run; further GPU shader/postprocess work remains an experiment rather than a completed optimization.


## Next bounded candidates (not implemented)

The remaining Three WebGL gap is unresolved. Before another throughput verdict,
remove competing GPU work with authorization and repeat both sides; do not assign
that environmental change to renderer code. Then profile native PBR light-class
specialization against pinned Three NUM_DIR_LIGHTS/NUM_POINT_LIGHTS variants:
our four-light loop retains dynamic zero-light tests and a `select(unit(delta),
directionalSun, ...)`, while Three prepares programs from the actual light class.
Preserve arbitrary multi-light changes and per-pass invalidation. Another candidate
is exact 1:1 FXAA integer reads for its first five texel-center samples, with edge,
odd-size and XR viewport oracles and a scaled-source fallback. Neither is a proven
speedup. Packed WebGL depth is already conditional on effects reading depth;
FXAA-only uses no extra depth-color attachment, so removing such an attachment
would be a fictitious optimization. Initial per-model material buffers remain a
separate loading/memory candidate, not proof of steady-state FPS cost.


## Verified local Quest delivery

Release/minified application `767d2bb4b3`, WASM `547da17e` / source `a1efeeb0`
is served at `https://192.168.0.17:8443` through `npm run serve:quest`.
Ten final minified browser cases pass: both native backends, independent layout
oracles, refresh, actual cut halves/stress, direct WebGPU XR simulation, and the
Three instance/mobile/VR-button control. Physical headset remains untested.
All254 deployed manifest entries and their compressed variants match the verified
staging package;11 actual HTTPS payloads match SHA256 and `no-store` with the
existing CA chain verified. Owned old PID52797 was replaced by PID65385
(session69215); backup is `output/production-before-opaque-batching-20261026`.
Other checkouts, original Three8878 and the unknown old headless Neon Arena were
preserved. Current input/source/WASM identity was rechecked; final typecheck and
37SDK/stress tests pass. No commit/push/CI result is claimed.


A second preparation candidate is the still-repeated per-primitive queue work:
v3 diagnostic sampled `gltf.queue` medians were9.5msGL/8.6msWG, versus12msGL on
baseline. These are profiled, contended diagnostic spans, not final profiler-off
CPU costs. Investigate cached compatible groups and a conservative proof that an
entire model root is inside the view frustum before repeating every primitive
frustum test. Retain fallback for missing/deformed bounds, camera/pass/eye changes,
pose/topology/options mutations and removals. This is unimplemented; do not report
those potential savings as achieved.
