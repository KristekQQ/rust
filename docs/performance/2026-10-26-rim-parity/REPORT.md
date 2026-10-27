# Rome renderer: measured GPU progress, parity still unresolved

2026-10-26, Apple M1 / Chrome, release WASM. Final retained candidate `a54c09c9`; base branch `codex/wasm-render-performance`, HEAD `64bfda680483e71f6aeb6bf902e63b29b3e90791`. This milestone is local work; commit, push, CI, served-package and physical-device evidence are separate in [delivery](DELIVERY.md).

## Result

The fixed 4096×2730 authored Rome view improved from 24.56 to 38.58 FPS (1.57× observed cadence). The original Three.js comparison measured 60.00 FPS against 36.08 FPS for the native candidate. **The target of equal or better performance at equal graphics is not achieved.** Native-vs-native improvements retain geometry, render dimensions, shadow resolution, PCF kernel and effects. The missing-normal-texture change is an explicit glTF correctness correction, independently checked against a full TBN neutral-map path, not bitwise equivalence to the old biased fallback.

[Interactive results and before/after images](index.html) · [Accepted and rejected experiments](EXPERIMENTS.md) · [Archive identity](evidence/archive-manifest.json).

| Workload | Before FPS, each run | After FPS, each run | Ratio of medians | CPU callback median ms, each run |
|---|---|---|---|---|
| Řím při vyšším rozlišení · 4096 × 2730 | 24.44 / 24.56 / 24.59 | 38.58 / 38.62 / 38.46 | 1.57× | 1.20 / 1.20 / 1.30 → 1.20 / 1.20 / 1.20 |
| Původní Three.js · diagnostické srovnání · 4096 × 2730 | 36.08 / 36.44 / 35.04 | 60.00 / 60.00 / 60.00 | 1.66× | 1.30 / 1.30 / 1.30 → 3.20 / 3.20 / 3.20 |
| Aktivní scéna · WebGPU · 960 × 640 | 60.00 / 60.00 / 60.00 | 60.00 / 60.00 / 60.00 | 1.00× | 2.30 / 2.30 / 2.30 → 2.10 / 2.20 / 2.20 |
| Aktivní scéna · WebGL · 960 × 640 | 60.00 / 60.00 / 60.00 | 60.00 / 60.00 / 60.00 | 1.00× | 3.40 / 3.30 / 3.40 → 2.80 / 2.70 / 2.80 |
| Klidová scéna · WebGPU kontrola · 2880 × 1920 | 60.00 | 60.00 | 1.00× | 0.70 → 0.70 |
| Klidová scéna · WebGL2 kontrola · 2880 × 1920 | 47.92 | 60.00 | 1.25× | 1.10 → 1.00 |

FPS is calculated separately for each run as 1000 / **mean** RAF interval; the summary takes the median of runs. Do not use median frame intervals as throughput at these loads. Main comparisons use three alternating pairs, 8 s warmup and 25 s sampling, profiler OFF. 2880 controls use one pair and are labelled accordingly. The 60 Hz cap prevents headroom claims. Raw distributions and P95/P99 remain available per run. Active 960×640 runs have six moving enemies and changing simulation; they do not replay sword attacks, projectile fire or cuts.

## Retained implementation

1. Reuse equivalent material bindings with canonical resource ownership and combine eligible rigid-instance uploads into a bounded per-pass/view stream. Keep material draw order, singleton/skin/blend fallbacks and independent XR view slots.
2. Specialize shadow shader resource selection outside the unchanged ordered nine-tap PCF kernel. Specialize only the exact uploaded single-static-map state; reflection/depth/overlay and other occupancy states remain generic. Skip lookup only when the direct contribution is exactly zero. This transfers Three-style program parameter specialization; it does not copy Three's shadow filter or approximate its lighting.
3. For glTF without normalTexture, keep the authored shading normal and omit only normal-map/TBN/UV2 work. Keep flat-normal and specular-AA world derivatives and all real normal maps. All 19 city primitives have NORMAL and no UV0/TANGENT/normalTexture, so this removes work from the actual city.

See [source/experiment journal](EXPERIMENTS.md) and archived `plan.json` for declarations, exact versions and acceptance gates. The actual pinned Three r170 normal fragments condition normal-map work on map presence; [glTF normalTexture](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#material-normaltexture) defines the optional map. The independent reference uses an explicit normal texture with scale 0, preserving the full TBN path.

## Quality and limitations

- Final native static image gates use the unchanged channel tolerance 8/255 and changed fraction 0.001. Images are from fixed post-load Idle pose time 0; earlier load-dependent pose captures remain archived and are superseded. These gates do not mean every changed pixel is within eight bytes. The UI gives maximum channel error and count above tolerance.
- Neutral normal-map oracle: 24 base + 8 no-UV states per actual backend, noMap versus full-path scale0 reference maximum 1/255. Existing mapped/neutral paths were exact across the normal correction. Supplied/absent NORMAL/TANGENT, smooth/flat shading, back faces, negative scale and metal/dielectric cases are covered.
- Integer powers were tested and rejected for no pilot gain (38.671→38.707 FPS), despite exact actual-Rome pixels and independent numeric/GPU checks. Weighted PCF, uniform earlyout, loop unrolling and exact-sun failures/adverse results remain in the journal; they are not retained.
- Original Three files are unchanged and archived with hashes. The camera, pixel dimensions, city and authored lighting are aligned, but Three MSAA versus native FXAA, shader/shadow details and gameplay differ. This is a direct diagnostic comparison, **not proven equal graphics or whole-game equivalence**, and its pixel pass is null. Native submits 39 draws / 67,292 triangles versus Three's 41 / 67,326 for this static adapter.
- The first eligible occupancy-variant transition can create a pipeline; the stable Rome timing does not prove absence of cold shader-compilation hitches. No physical Quest performance, comfort, or headset graphics acceptance is established.

## Next bounded investigation

Profile the remaining native high-resolution fragment/postprocess work against a matched AA/shadow configuration and inspect translated shaders before changing it. Earlier WebGPU pass samples showed substantial scene-main and FXAA costs, but these are separate scopes, not a summed frame duration or proven cause of the WebGL gap. Keep GPU quality, geometry and headset per-eye contracts fixed. Do not replace Rust/WASM based on these results: the evidence identifies renderer work, not a language-level limit.

## Reproduction

Use [the maintained performance skill](../../../.agents/skills/wasm-render-performance/SKILL.md), [protocol](../../RENDER_PERFORMANCE.md), archived exact commands/configurations and build manifests. Final timing archive includes full raw RAF/CPU samples and driver input identities. `archive-evidence.py` packages earlier oracles/rejected experiments/source snapshots and unchanged Three; `collect.mjs` collects final timing reports; `build-view.mjs` and `verify-view.mjs` build and test the result UI. No live app is served by the results-only HTTP page.
