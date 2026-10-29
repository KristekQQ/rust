# B9 camera-stress strict pixel mismatch

Scope: CPU-only inspection of existing B9 evidence and targeted source. No browser, build, GPU run, source edit or tolerance change. Frozen comparison is original B1 versus final B8; B9 does not change renderer source.

## Confirmed evidence

`primary-rim-camera-stress-webgpu/report.json` SHA-256 is `1c038a945a0f7c11f76c5c877c44653f7a7720a8c7ddc1b8370bed36a34bc1e2`. At load 2048 / 1280×720, pair 1 fails channel tolerance 0 and fraction tolerance 0: six changed pixels, maximum channel delta 2/255. CPU `pngjs` decode independently reproduces the report.

All three baseline PNGs and candidate pairs 0 and 2 have identical decoded RGBA pixels. Their recorded PNG SHA-256 is `a5d531295d3ef42f6de3c4383cfd102f165c8b40c12926242f2d5789599b5979`; candidate pair 1 is `dd7eb52d975d3e55c8ac468098817beac469d964c1bfcd73731c19de59a51f5c`. Thus observed A/A is exact; B/B has the same six-pixel mismatch. Baseline instability is not evidenced by these captures.

Candidate pair 1 differences against every other capture (zero-based PNG coordinates):

| x,y | Reference RGBA | Candidate pair 1 RGBA |
| --- | --- | --- |
| 382,215 | 252,120,194,255 | 252,121,194,255 |
| 383,216 | 169,162,171,255 | 169,163,171,255 |
| 394,267 | 208,101,162,255 | 208,100,162,255 |
| 397,267 | 149,68,114,255 | 149,67,114,255 |
| 45,351 | 247,111,188,255 | 247,111,187,255 |
| 203,456 | 54,32,43,255 | 56,34,45,255 |

## Source inspection and evidence limits

- `tests/render-performance.js:84` stops the workload and calls `pose(.375)`. The camera-only branch at lines 65–66 assigns the same position/target; stress models use paused Idle at time 0 (`examples/rim/stress.js:75–78`). Measured motion before capture remains elapsed-RAF driven, so prior view/cache histories are not identical between runs.
- `scripts/render-performance.mjs:120–122` captures after this reset. `tests/pixels.js:3–12` waits four RAFs and copies in a fifth RAF. This does not prove stale output, nor record the capture-time camera matrices, shadow keys, plan keys/content epochs or ordered membership needed to attribute history dependence.
- The inspected main cull key includes view-projection and scene revision (`src/render/renderer.rs:960–974`). The opaque plan key includes exact camera/view bits, input/content epochs and scene revision (`src/render/gltf_gpu.rs:2612–2621`); lookup checks ordered model identity/generation/data revision (`:276–294`). Shadow keys use light matrix and caster revision (`src/render/effects.rs:342–369`); camera-only motion does not itself require shadow invalidation. No omitted dependency explaining these pixels was established by inspection.

The strict image-equivalence claim fails for this capture. A candidate-only intermittent render difference is established; a particular renderer source defect, cache corruption, capture timing issue or GPU numerical cause is **not** established. Small magnitude alone does not discharge the gate. The historical B4 crowded-motion issue remains separate; these six camera-stress pixels do not establish the same cause.

## Smallest bounded independent follow-up

After existing serialized GPU work, use only this WebGPU workload/load/viewport and unchanged appearance. Start with the maintained motion-test oracle pattern (`tests/gltf-plan-motion.js:68–76`): at fixed `.375` camera pose compare candidate cached → prepared-plan cache off plus frustum culling off → restored, with exact RGBA and unchanged tolerances. This reference bypasses both prepared/fine reuse and culling, rather than only repeating the optimized path. Save captures and capture-time camera/matrix bits, ordered visible membership, plan input/content epochs and shadow revisions; save consecutive settled-frame captures to detect continuing variation. Run three fresh sessions because the existing anomaly occurs in only one of three. Use an explicit deterministic camera-step history before the capture to remove elapsed-RAF history as an uncontrolled axis.

If the oracle differs reproducibly, isolate cache versus culling in the next bounded check; if both paths vary together, retain the failure and next compare fixed candidate original vertex layout/index controls separately. Neither result retrospectively clears the existing strict failure. No generic scene matrix, performance rerun or relaxed tolerance is needed for this first diagnostic.
