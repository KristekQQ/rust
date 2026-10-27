# Spatial opaque batches and lossless GPU records — 2026-10-27

Selected local implementation: v5. Scope: reusable Rust preparation/instancing, not a glTF format replacement. No example/asset-name branches, geometry reduction, quality-setting changes or gameplay changes.

## Result

Desktop AppleM1, Chrome155 ANGLE/Metal,2880×1920,DPR1,profiler off;3 alternating AB/BA/AB pairs,8s warmup/25s measurement. FPS=1000/mean application RAF interval, then median across runs. Separate campaigns and different loads are not a three-way ranking.

| Self-comparison | Stress models | Baseline FPS | Selected FPS | Actual total calls before→after |
|---|---:|---:|---:|---:|
| Native WebGL2 |512|17.281|17.721|308→174|
| Native WebGPU |1024|24.233|24.898|576→241|

GL per-run baseline[18.588, 15.841, 17.281] / selected[17.218, 17.985, 17.721]; paired FPS ratios[0.9262633623594865, 1.1353598207699471, 1.0254629629629632]. WG baseline[23.961, 24.257, 24.233] / selected[24.898, 24.585, 26.121]; paired ratios[1.039095626281036, 1.0135005513567188, 1.0779011061268897]. GL has1/3 adverse pairs; WG has0/3. Background/thermal conditions remain uncontrolled, so a general reliable FPS gain is unproven. Preparation counters and resource reductions below are separate evidence.

Separate direct authored-Three diagnostic,512models/same camera/resolution: selected nativeGL18.505FPS/174calls versus Three r17026.179FPS/108calls. Three wins this diagnostic. AA(FXAA versus MSAA4),PBR/shadows and full gameplay differ; do not call this equal full graphics or combine its absolute FPS with the self-series above. Requested stress asset/poses match; native10,553,756 versusThree10,554,110 submitted triangles/RAF differ by354 in the original scene.

## Implementation and measured work

- Replace the fixed128-root opaque scheduling window with a camera-relative depth span(far/near≤2), keeping the1024-instance draw limit independent. Explicit opaque-reorderable permission, rigid assets, same shared geometry/material/options, mirrored transforms, alpha/order barriers, clipping and auxiliary-pass fallback remain. Depth4 reached107total calls but repeatedly reduced GL throughput; retained as a rejected experiment, not the selected policy.
- Cache complete per-pose root bounds and active opaque/blend ordinal lists under the existing successful update stamp. Skip child-plane tests only with strict six-plane full-root containment; partial/touching/invalid bounds keep individual tests. GL primitive tests/RAF34,343→2699, unchanged10,553,756 submitted glTF triangles. Camera, pose/options, removal and two-eye mutations remain covered.
- Exact identical affine basis/normal coefficients share the material uniform and use16B absolute translations, retaining the full112B instance record for differing bases. No approximate normal reconstruction. Mixed streams use byte offsets/capacities and separate pass/view ownership.512-model retained allocated instance buffers7,568,720→1,146,800B(−84.85%). This is instance-buffer allocation, not total process/physical VRAM. Stable timed runs assert no new instance buffers or instance uploads. New format/material combinations can compile cold pipelines; arbitrary first-use transitions are not proven hitch-free.
- Conditional absent-normal-map tangent omission removes16B per rigid vertex (56→40B without texture UVs), with full96B CPU fallback offsets unchanged; exact4-vertex fixture proves184B upload including indices, versus248B. Original normal-map layouts, skin/morph position/normal math and CPU queries remain.

Actual glTF event counters, including translations, root-proof tests and spatial barriers, are exposed through existing diagnostics. Whole stress-scene calls above are actual glTF submitted calls plus one FXAA compose; estimated root/primitive submission counters are not substituted.

## Correctness and evidence

Native4bounded-policy/frustum cases, strictwasmClippy, release build and TypeScript pass. Unchanged performance-helper unit suite29PASS reused. Independent opaque/root-order and CPU-deformed pixel oracles cover translated/nonuniform/mirrored/rotated instances, stream grow/shrink/offsets, culling, materials, skin/morph and cleanup. Seventeen v3 and seventeen v5 minified browser cases pass, plus4focused v4 cases; direct WebGPU and WebGL simulatedXR include asymmetric eyes,256stressmodels, actual cut-worker completion/visible halves and continuation. These are browser/simulation checks, not physical Quest acceptance.

All6 selected before/after stress image comparisons are pixel-exact and pass the unchanged8/255,0.1% gate; see summary.json for exactness/maxima. GL/WG v4 were all pixel-exact. Independent read-only Sol review covered cache invalidation, mixed buffer lifetime/layout and shader keys. It caught a v5 unsigned UV-filter underflow before build; guarded and reinspected, then byte/pixel/runtime tests passed. Reviewer did not independently reproduce performance.

Frozen baseline source535ac6bc/WASMdcce4c0c. Selected WASMc7091506871c74c5055d501dfd85b3a12d8e70dd7905003135dee7939f8109eb; branchcodex/spatial-opaque-batches from restored archive7b90d4d18. Full input/build/request manifests and per-run raw frames are retained in evidence.tar.gz; summary.json and EXPERIMENTS.md retain adverse pilots/repetitions. source.patch captures the selected implementation; attribute-experiment.patch preserves the v4→v5 experiment. No commit/push/CI acceptance in this task.

## Limits and next bounded investigation

Existing unknown headlessChrome30810/GPU30816 stayed running(~30%CPU on its GPU helper); permission to close was not assumed. Background/thermal state was not isolated, and absolute native GL cadence changed strongly between campaigns. Repetitions do not eliminate that uncertainty. No physical-headset FPS/latency/comfort comparison is available.

Genuine v5 profiler diagnostics are separate from throughput: WebGPU1024yielded5valid pass samples(scene.main mean31.700ms,FXAA1.984ms); GL EXT elapsed queries yielded4samples(mean127.186ms) in a contended context, not isolated GPU busy time. Do not sum these scopes or claim a before/after GPU reduction. Earlier v4 stage named profile-gl inherited profiler-off calibration and supplies no GPU-time evidence.

Next candidates remain unimplemented: omit provably identity-white vertex colors with full-color fallback/variant ownership; choose lossless16-bit indices where legal(current native glTF usesu32); or retain prepared groups under complete view/pose/pipeline/resource keys. Three r170 conditionally requests vertex colors/tangents; any transfer needs its own pixels and matched timings. Vertex shadow projection must preserve current final-fragment normal bias; blindly copying Three's vertex bias changes graphics. No checklist establishes perfect optimization.

## Delivery

Final staged package, browser entry checks, actual HTTPS payload hashes and live release identity are recorded after installation in deployment.json. Normal8443URLs remain unchanged; no page version parameter. Shared server and primary Neon remain under their existing ownership. Final device acceptance remains outstanding.

Final local delivery: app`ba920dd931`,WASM`c7091506`,260manifest payloads validated;11final minified scene/refresh/material/shadow/batching cases PASS and15actualHTTPS hashes/TLS/no-store PASS. Existing broken8443transport required restoration(PID79190→12350), same mounts/certificate/score-store, after successful ephemeral-port preflight. Stale cwd inode observed; exact failure stack unavailable. Five mountedNeon payloads remained identical. No primary source/build edit. Final comparison UI and refreshed native captures verified; screenshot`ui-results.png`. Source remains local/uncommitted/unpushed.
