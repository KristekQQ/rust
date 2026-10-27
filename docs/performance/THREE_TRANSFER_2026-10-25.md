# Three.js technique transfer — 2026-10-25

This is a new experiment after the [first optimization report](2026-10-25/REPORT.md), not a reinterpretation of its numbers. The user requested implementation based on actual Three techniques, direct comparisons and durable memory. Work remains isolated on `codex/wasm-render-performance`; no commit/push/deployment is implied.

## Observed implementation differences

The frozen local reference is Three r170 (`output/render-performance/three-reference/vendor/three.module.js`). `Matrix4.compose` at line6648 writes affine columns directly; `Object3D.updateMatrix` at7803 invokes it, and `updateMatrixWorld` at7811 reuses the local matrix, copying it for parentless objects. These are observations from the exact local source, not a claim about every current Three version.

Our `Transform::model` in `src/scene.rs` used `T * Ry * Rx * Rz * S`, constructing five matrices and multiplying four times. Changing a transform refreshes `ObjectRenderData`, but `finish_scene_updates` independently recomputes the model root for every glTF model. The GPU updater then computes `root * node_render_matrix`, inverse-transpose, determinant and primitive bounds. Multi-primitive nodes repeat some of this work. Root/options-only updates also refill the active-node set and compare skin palettes. These are separate candidates; one change is measured at a time.

Do not simplify general glTF normal matrices to rotation: nested nonuniform/negative scale can produce shear. Preserve singular fallbacks, node/pose transaction rules, material state and per-eye/pass ownership. Matrix/pose preparation belongs before both XR eyes.

## Experiment1: direct affine composition

Hypothesis: direct Euler YXZ affine columns remove four general matrix products per root computation. It preserves the existing Euler convention rather than switching the engine to quaternions. Shader, geometry, material, resolution, simulation and render ordering are unchanged.

Baseline release: `6f9a3bba80c0681fbf4908ac4dbda7fd5088ed72aa6c4f8523b3a2d5a68518b9`, immutable `output/render-performance/candidate-adaptive`. Candidate release: `6236f6b42b7dca9faf86a6abdb7aa4bd958d529546d036d167d15aca00d0e5f6`, immutable `candidate-compose`.

Actual checks so far:
- `cargo test --locked direct_transform_composition`: PASS, independent old chained-matrix oracle,4096 seeded cases, mirrored/nonuniform/singular scales, translations, transformed points.
- `cargo test --locked transform`:14 PASS, including normal perpendicularity, conservative bounds, mirrors, cache invalidation and cutting transforms.
- Release WASM build + content-bound fingerprint + immutable source/runtime freeze: PASS.
- Maintained browser command `env VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page gltf-batching,gltf-gpu-transforms,gltf-visibility,render-mirrored,shadow-cache,rim-gameplay,neon-arena-xr,webxr-webgpu --backend all`:14 PASS, including independent unbatched image reference and actual simulated-XR cut halves. No physical Quest evidence.
- Independent Astra source review: no scoped correctness blocker; oracle does not replace WASM/backend pixel checks.

Performance decision pending. Planned protocol is recorded before evaluation in `output/render-performance-three-transfer/plan.json`; actual commands, timestamps and exit status accumulate in `commands.json`. New reports/raw files remain separate from the immutable first experiment archive.


## Experiment2: reuse an existing published matrix

The direct-only pilot (one1s/5s pair) completed successfully on both backends: WebGPU CPU15.6→15.2ms; WebGL16.1→15.3ms, both captures exactly equal. This is small, short evidence and does not independently establish a reliable improvement.

The second step adds `ObjectRenderData::model()` and changes only the root argument used by `finish_scene_updates`. It reuses the matrix already computed by `refresh_render_data`, analogous to Three's reuse of `Object3D.matrix`. It introduces no new cache key, allocation, layout, shader or normal-matrix shortcut. All existing pose/world/visibility invalidations remain active. A read-only audit found no stale glTF root path; root setters, animations, projectile integration and model pose commit refresh the existing data.

`cargo test --locked cached_gltf_root_model`: PASS using the actual Fox GLB, setter→spin/projectile→playback/update→seek, comparing the cached matrix to a fresh computation after every stage. Initial test compilation correctly rejected passing a JS-style clip name to the native index API; the test now resolves the actual Walk clip index. This transient test authoring error did not modify the renderer or any benchmark data.

The final paired comparison measures the combined two-step path against the unchanged6f9 baseline. It must not assign a separate percentage to the cache reuse without an isolated accepted paired study.

## Main paired decision

- wg: baseline CPU medians [15.6, 15.4, 15.6]ms; combined candidate [15.1, 14.6, 14.9]ms; median paired ratio 0.95512821 (4.49% lower). Pixel pairs [True, True, True]. Driver inputs unchanged; no server errors.
- gl: baseline CPU medians [15.6, 15.6, 15.5]ms; combined candidate [15.7, 14.7, 14.7]ms; median paired ratio 0.94838710 (5.16% lower). Pixel pairs [True, True, True]. Driver inputs unchanged; no server errors.

Retain the combined low-risk preparation changes: all three WebGPU pairs and two of three GL pairs improve; GL pair0 worsens0.1ms (~0.64%). WebGPU first candidate RAFp99 is33.3ms versus16.8ms baseline; later pairs do not repeat that pattern. Do not claim all tails improved or independent statistical confidence from adjacent frames. Repeated median benefit is narrow (~4–5%), not a solution of the whole Three gap. Primary source/pkg freshness rechecked at292f release after tests; no subsequent Rust/WGSL changes. Direct-only pilot remains separate and not a reliable standalone percentage.

## Direct Three control results

Three r170 objects,3pairs4s/10s: retained Rust CPU14.9/14.7/14.8ms, Three6.6/6.8/6.8ms. Same6144 submitted quads/12288 triangles; Rust6 actual commands, Three6144. Instanced Three: Rust14.5/14.5/14.6ms, Three1.7/1.7/1.7ms, both6commands. This second control has pre-established groups and skips dynamic group discovery. Both references differ in shading/lighting/conventions and are not full-game equivalence. The remaining gap persists; do not replace this direct evidence with the preceding self-improvement percentages.

## Additional configuration audit — not implemented or performance-proven

Astra inspected local glam0.34.0 `src/f32.rs` backend selection and current rustc cfg/dependency features. Our recorded release flags have no explicit `+simd128`; glam selects its scalar matrix path under that compiler configuration. `scalar-math` is not forced: only libm/serde are enabled. This is a concrete new experiment candidate, not a proven explanation of the Three gap. A safe experiment uses a separate SIMD build/output with identical sources, verifies numeric and both-backend pixel behavior, and measures matched pairs. A production change would need feature detection before loading and a distinct scalar fallback artifact, since the SIMD module itself cannot fall back on an unsupported browser. General inverse/determinant, near-singular transforms, layout/ABI and physics behavior must stay correct. No SIMD flags or SDK loader were changed in this task.

## Final controls and acceptance boundary

All27 bounded CLI commands completed exit0, with unchanged driver inputs and no server/page/runtime errors. Of16 same-renderer image pairs,15 are exact; the Rome static WebGL pair passes the pre-established pixel tolerance with max channel difference1/255, mean channel difference0.0000020345 and zero pixels beyond tolerance8. All6 primary paired captures are exact. No golden images were modified. The6 Three comparison pairs deliberately have no pixel equivalence gate.

Short controls2s/8s (one pair each): dynamic512 WG4.1→4.2ms (+0.1ms), GL4.4→4.4ms; Rome static WG1.0→0.9ms,GL1.5→1.5ms; arena static WG0.8→0.7ms,GL1.2→1.2ms. These small controls do not establish universal game/FPS improvements. All final WebGL timer windows were valid and completely cleaned up. Resource/quality invariants remain intact. The combined preparation optimization is retained; the remaining renderer/Three gap and physical Quest behavior remain unresolved.

Independent Astra data review confirmed primary ratios, fixed actual work, exact main captures and interpretation limits; final controls satisfy the pixel/failure gate with the small Rome byte difference explicitly disclosed.

## 2026-10-27: spatial batches and conditional GPU attributes

[Full experiment/report](2026-10-27-spatial-batches/REPORT.md). Exact local r170 explicit InstancedMesh-per-part grouping motivated generic cross-root opaque batches; Rust keeps semantic ordering permission, camera-relative depth bounds, compatible resources and1024-instance cap. Depth4 reaches107calls but loses GL throughput; selected depth2 yields174 at512models. Shared exact-basis16B translation records and strict root-frustum proof are native adaptations, not claims of identical Three implementation.

Three r170 program parameters atthree.module.js:20896 request tangents only for normal maps/anisotropy; USE_TANGENT at20029 also excludes flat shading. Native's existing absent-normal-map specialization makes that input unconsumed: omit16B, retain full map/deformation fallbacks. Known fixture upload184vs248B; independent CPU pixels, skin/morph/material mutations and bothXR eyes pass. Selected stress pairs are pixel-exact, total rigid upload−25.69%, instance allocations−84.85%. Contended repeated FPS gains are modest and GL includes an adverse pair; direct Three remains faster. No Quest/graphics-parity claim.

Rejected/unimplemented: blind vertex-normal shadow bias changes final-fragment normal semantics; approximate Three Fresnel is not a quality-preserving transfer. Identity-white colors/minimal legal indices/coherent prepared-plan retention remain separate future hypotheses. Raw adverse pilots, exact build/request hashes and independent review scope are preserved.
