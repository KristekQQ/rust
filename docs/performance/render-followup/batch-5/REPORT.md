# Followup5 — independent audit and bounded ordinal-merge experiment

## Decision

**Retain the new exact-order algorithm as an explicit, validated mode3 option. Keep production mode0 as the default.** Camera stress shows modest paired gains; clustered/static/ordinary scenes do not establish a general FPS gain. This batch does not automatically enable the option in games or claim a Quest improvement. No push, merge, deployment, shared service restart or batch6.

The change is a stable counting transpose of already-selected model-major draw records into the original ordinal/model order. It removes repeated cursor probes through roots, including empty/culled lists. Geometry, compatibility/root/depth barriers, instance cap, draw order, shader math, uploads and GPU resources remain governed by the same code. At most65536ordinal counters and4MiB requested record storage; otherwise the original cursor path runs. Existing clear releases scratch. No new cache or widened batching.

At camera2048, retained queue scratch increases2,142,816→2,518,692B (+375,876B). New array capacity is included in bytes; the older queueScratchGrowths counter does not count those CPU vector growth events. Do not call it zero CPU allocations. GPU buffer/pool accounting is separate.

## Clean paired results

Release,1280×720 DPR1, seed1337,8s warmup/25s sample,3 alternating AB/BA/AB pairs. Detailed profiling and GL GPU queries are verified off. All full frame/callback series are retained; offline mean/median/p95/p99/max recomputation matches recorded statistics. Callback time is wall time, not pure CPU compute.

| Workload | API | FPS change in each paired run | Interpretation |
|---|---|---|---|
|arena-active|WebGL2|-0.00%, +0.00%, +0.00%|60Hz capped control|
|arena-active|WebGPU|-0.00%, -0.00%, +0.00%|60Hz capped control|
|rim-active|WebGL2|+0.00%, +0.00%, +0.00%|60Hz capped control|
|rim-active|WebGPU|+0.00%, +0.00%, +0.00%|60Hz capped control|
|rim-camera-stress|WebGL2|+0.22%, +6.34%, +3.86%|Modest/variable workload-specific result|
|rim-camera-stress|WebGPU|+3.08%, +3.69%, +6.65%|Modest/variable workload-specific result|
|rim-clustered|WebGL2|-0.00%, +0.47%, +0.87%|No general gain established|
|rim-clustered|WebGPU|+0.30%, +0.39%, +0.74%|No general gain established|
|rim-mixed|WebGL2|-0.07%, +0.00%, +0.13%|Modest/variable workload-specific result|
|rim-mixed|WebGPU|+0.29%, +4.26%, +4.59%|Modest/variable workload-specific result|
|rim-stress|WebGL2|-0.00%, -0.13%, -0.06%|No general gain established|
|rim-stress|WebGPU|-0.44%, -0.10%, +0.37%|No general gain established|

[Every pair, callback means and frame/callback tails](RESULTS.md); [all individual metrics/counts/cold readiness](RUNS.json); [paired deltas including maxima](SUMMARY.json). Small differences in the nonisolated host are not a universal causal gain. For example cameraWG callback p99 is34.7→34.8ms in pair0 despite higher FPS; sparseWG pair0 p99 is55.4→56.8ms. ClusterGL p99 also worsens in two pairs. No maximum-only verdict or removal of unfavorable samples.

The ordinary Rim fixture runs native input/enemy motion but is not full player combat; no cut latency gain is claimed. Actual worker cut and asymmetric simulated XR correctness are checked separately. Static warm group rebuilds are zero; this new algorithm cannot speed a cache hit.

## What actually remains expensive

Fresh, separate2s/8s diagnostics show camera groups4.066→3.438ms/prepare onWG (261/266actual timed calls),4.113→3.303ms onGL (247/247calls). Cluster groups2.168→1.815ms WG and2.241→1.804ms GL. This is measured removal of preparation work, not an equal reduction of total frame time. [Diagnostic calls/scopes/query validity](DIAGNOSTICS.json).

`render.encode.color` contains glTF preparation, and `render.encode.overlays` contains overlay selection. Do not add parent/child scopes. Static anchor histories show glTF queue about0.06ms and color scope about0.29ms WG/0.52ms GL; there is no evidence here that per-draw encoding is the dominant remaining static CPU cost. Overlay selection still costs about7.72ms WG/7.53ms GL under detailed instrumentation. Followup2 intentionally retained the original selector after adverse whole-frame results; this audit does not reactivate it or published-count reuse.

The sampler also invokes the legacy `render_stats` primitive walk (every100ms clean, every frame with detailed context). Three reads cached `renderer.info`. This concrete observer-work asymmetry limits absolute cross-engine callback/cadence attribution; its isolated time is unavailable. Actual native draw counts below use debug/work counters, never the inflated legacy unbatched estimate.

## GPU diagnostic axes — NOT final quality reductions

Single short instrumented runs, same2048 geometry/camera; change one requested axis. These are directional observations, not three-pair optimization evidence. GPU query scopes/counts/invalidity remain in each raw report.

| API / diagnostic | FPS | Callback mean ms | WG main / FXAA GPU mean ms | GL application query mean ms |
|---|---:|---:|---|---:|
|axis-anchor-webgl|31.00|29.45|unavailable / unavailable|70.87|
|axis-anchor-webgpu|32.75|30.33|26.08 / 3.48|unavailable|
|axis-cheap-webgl|34.80|28.51|unavailable / unavailable|29.95|
|axis-cheap-webgpu|36.18|27.41|24.50 / 2.65|unavailable|
|axis-low-resolution-webgl|35.13|28.28|unavailable / unavailable|36.53|
|axis-low-resolution-webgpu|32.93|30.05|24.80 / 1.81|unavailable|
|axis-no-fxaa-webgl|31.37|23.65|unavailable / unavailable|50.04|
|axis-no-fxaa-webgpu|39.88|21.14|24.95 / unavailable|unavailable|
|axis-no-shadows-webgl|32.93|30.10|unavailable / unavailable|41.47|
|axis-no-shadows-webgpu|34.68|28.57|24.97 / 2.83|unavailable|

Quarter-pixel target barely changes WG cadence/main pass, while GL is more sensitive to resolution and cheap shading. This points toward substantial non-pixel geometry/vertex cost in this WG stress scene and stronger fragment/target sensitivity onGL; it does not isolate a particular GPU instruction or prove a universal bottleneck. No hardware utilization/vertex-stage timer is available.

FXAA-off changes dependent CPU routing as well as the GPU pass: overlay-selection history drops from~7–8ms to~0.02ms. It is wrong to call the whole callback reduction the FXAA shader cost. Shadows are cached; disabling them also removes sampling, not just map rendering. GL elapsed queries cover application command spans (including potential stalls/idle/presentation interactions), not GPU busy time or the same interval asWG pass timestamps. Never add CPU/GPU times or interpret~70ms GL query as70ms mandatory serial frame work.

## Pinned Three control

Vendored r170, existing per-part InstancedMesh factory; no per-enemy clone slowdown. Same city/drone assets, stress positions, Idle@0 pose, gate camera65°/.08/350,1280×720. Native F4 baseline is the comparator; new native mode3 is compared separately above under exact same native configuration.

| Comparison | Native FPS (three runs) | Three WebGL2 FPS | Actual commands native / Three | glTF triangles native / Three render triangles |
|---|---|---|---|---|
|three-common-webgpu|44.67, 44.74, 44.62|38.55, 38.56, 38.55|287 / 86|39,061,900 / 42,009,212|
|three-common-webgl|40.31, 40.39, 39.74|38.56, 38.50, 37.28|287 / 86|39,061,900 / 42,009,212|
|three-production-webgl|30.00, 30.00, 30.00|29.96, 30.02, 29.92|308 / 108|39,067,116 / 42,014,462|

Common profile removes AA/lights/shadows/fog/tone/post on both sides, but native per-primitive culling/depth grouping and Three aggregate InstancedMesh bounds differ. Native287commands/39,061,900triangles versusThree86/42,009,212. Native command caps/depth windows remain; fewer calls alone do not establish better throughput. No triangle-normalized FPS is invented. Common captures differ in50pixels(max255), consistently in all six cross-engine pairs: this is a limited profile, not bit-exact cross-engine equivalence or an explanation of historical image cases.

Production profiles differ: native FXAA/custom shading versus Three hardware MSAA(4samples),ACES/exposure1.25,hemisphere/directional lights,fog andcached PCFSoft2048shadows. Actual state/metadata is retained per run. Native308commands include the FXAA command; native triangle metric is glTF-only and excludes the full-screen post triangle. Three per-render counts include its rendered scene. Production images differ705,262pixels(max219); no equal-quality renderer ranking follows from similar~30FPS.

The supplemental production nativeWG-versusThree attempt failed the existing CLI guard before browser startup; it is unavailable, not a completed test. Supported commonWG/GL and productionGL controls are complete. No unsafe guard bypass.

## Correctness and evidence

134completed runs,116profiler-off runs,102,380full RAF records. 72primary native run-sides;18paired Three run-sides. Independent decoded images:34/34strict native pairs exact;12authored-gameplay pairs lack deterministic capture and9cross-engine pairs have intentionally disabled exact gates. Their observed differences are retained. No tolerance/golden/reference alteration. Historical A/B/C/D remain separately open.

Preflight:2native order/bounds tests,54performance helper tests and6focused browser cases pass. Browser motion/pass oracle switches to original mode0 with cache off, then restoresmode3; actual worker cuts and asymmetric simulatedXR are included. Final full gate status/counts are recorded in VERIFICATION.json, not borrowed from prior batches.

Failures retained: sandbox wasm-opt permission/Node callback assertion (host runs pass); first six browser tests rejected mode3 at the separate SDK whitelist (fixed before measurements); unsupported productionWG/Three guard; offline parser v1 mistook an object of empty server-error arrays for errors and cross-engine `gateDisabled` comparisons for strictnative gates. Schema inspection corrected the analyzer, not the observations or tolerance. All raw percentiles and image hashes were recomputed.

## Remaining priorities (no automatic continuation)

1. Isolate observer/legacy-stats cost before making a stronger absolute Three CPU claim; do not silently change historical protocols.
2. In actual game-sized workloads, distinguish WG geometry/vertex limits from GL pixel/shadow/post cost with quality-preserving candidates; the reduced-quality controls above are not fixes.
3. Validate mode3 on a physical Quest before automatic XR adoption. Existing WebGL shared-context experiment stays opt-in; WebGPU stereo staging is untouched.

[Identities/environment/limits](MANIFEST.json), [audit and five original hypotheses](AUDIT.md), [protocol](PROTOCOL.md), [reproduction/actual command ledger](COMMANDS.md), [independent pixels](PIXEL_AUDIT.json).
