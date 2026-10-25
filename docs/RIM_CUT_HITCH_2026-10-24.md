# Rome cut hitch follow-up — 2026-10-24

User reported a freeze when slicing. Local uncommitted follow-up in isolated `rim-publish/rust`, base `088fdb34`; previous owned changes preserved.

## Cause and correction

The worker did the actual triangle clipping, but hidden fragment insertion called the ordinary physical mesh constructor on the main thread. That eagerly baked a convex contact proxy for every death fragment (26 directional scans plus hull-plane enumeration), although these fragments are noninteractive and only separate/dissolve. Internal visual-only insertion now preserves exact CPU geometry, bounds, volume, renderer upload and atomic publication while omitting that unused proxy. The ordinary physical constructor and contact behavior are unchanged.

Cut preparation also cloned complete `ModelVertex` values (dynamic skin influences and UV arrays) for each triangle-index occurrence and calculated unused tangents. A shared deformation loop now projects into compact position/normal/RGB records for cuts. Full deformation retains its tangent/handedness behavior. Morph, skinning and inverse-transpose normal calculations remain shared and numerically identical for the cut fields. Triangle/vertex work budgets, worker clipping and visible effect lifetime are unchanged. This is not a new physics worker.

## Measurement

Maintained paced simulated-XR scenario on Apple M1, hardware WebGL, 3 runs per variant. Initial before/after order plus a reverse-order frozen comparison; the latter used identical JS/assets/test instrumentation (verified by served content hashes), differing only in WASM. Metrics below are pooled mean / maximum in milliseconds, not headset FPS or total browser scheduling time. `stageMs` excludes GPU upload; the simulated frame CPU sample includes rendering submission, not GPU completion. Raw runs, hashes and exploratory runs are retained in [benchmark data](benchmarks/rim-cut-hitch-2026-10-24.json).

| Metric | Before mean / max | After mean / max |
|---|---:|---:|
| stageMs | 3.040 / 5.300 | 0.190 / 0.400 |
| prepareMs | 1.967 / 3.300 | 1.017 / 2.000 |
| deathAndCutFrameMs | 7.814 / 11.300 | 4.457 / 6.400 |
| framesMs | 7.962 / 17.800 | 5.489 / 20.800 |
| completionMs | 213.467 / 234.500 | 182.000 / 189.600 |

The remaining 15–21 ms first measured gesture-frame spike occurred while kills were still zero and no cut job was queued. Its separate pre-death cause is not fixed or diagnosed by this change; total-frame maxima are not claimed eliminated. Post-death/cut frame CPU and staging costs decreased. Worker callback maximum remained below 1 ms. Complete geometry publication still takes roughly 0.2 seconds asynchronously; the original impact pose remains until atomic replacement. Heavy later enemy types and simultaneous deaths were not performance-benchmarked.

## Verification and delivery

- Native: 22 Rome tests, 3 deformation tests, 5 mesh-data tests and 23 ordinary cutting tests passed. New tests compare exact compact/full fields across the three Rome assets and skinned Fox with morphs, duplicate indices and mirrored/nonuniform joint transforms; visual-only vs physical render data/bounds/volume are identical, normal meshes retain contact proxies, and Rome staged pieces are noncuttable without proxies.
- wasm32 Clippy with warnings denied and optimized release WASM build passed.
- Packaged browser acceptance: all 9 cases passed: Rome actual split/separation/dissolve, glTF full, glTF SDK and normal cutting on WebGPU and WebGL2, plus Rome simulated XR on WebGL. Inspected WebGPU separated-half screenshot. Source review by user-requested Astra consultant found no blocking issue in the scoped implementation.
- HTTPS 8443 HTML, main module, worker, manifest and WASM bytes match the accepted production package; no-store headers checked. Application `acb802a2ea`, WASM `bf90a5d71a541f2c8426f5431b3e9fcfa6be58d7ef373f6a0a9a2aa6b2f1e0c3`.
- No physical Quest validation, commit, push or CI run. User's existing browser tab was not inspected or refreshed.
