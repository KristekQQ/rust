# Pinned Three control and transfer audit

Local authoritative source: `examples/rim-three/vendor/three.module.js`, revision170, provenance `examples/rim-three/reference-origin.json`. Original bundled app is `app.js`; `main.js` is the later stress/UI wrapper. Fixed stale benchmark import paths (previous adapter pointed at the now-native `examples/rim/main.js`). No edits to original app/vendor or per-part instancing implementation.

Existing best representation: `examples/rim-three/stress.js:createThreeStressLoad` clones the source **once** as an immutable template, freezes Idle@0 and hierarchy matrices, then builds one `InstancedMesh` per mesh part (67 for drone), immutable instance transforms, computed aggregate bounds. Source geometry/materials remain shared; disposal releases only owned instance buffers. It is not one clone per enemy. Production and common controls use this existing factory.

Pinned `painterSortStable` (`three.module.js`, line21318) sorts by groupOrder, renderOrder, material.id, then depth and id. Therefore “Three globally sorts every opaque triangle front-to-back” is false. Frozen D’s depth-order prototype only reordered explicitly permitted contiguous opaque group runs, kept barriers/transparent order and did not change batching limits. Real Rome pixels nevertheless differed, so that mode was rejected and removed from final C. Lower draw count alone was already rejected as a goal in prior depth4 experiment.

## Limited shared profile

Same exact `assets/models/rim/rim-city.glb` and `drone-orbiter.glb`, original layout/root scale, Idle@0, gate camera [0,1.7,42] looking toward [0,1.7,-20],65°/.08/350, DPR1. Shared 1280×720 black/sRGB target. Native bounded cheap-fragment diagnostic and Three MeshBasicMaterial preserve base color, textures, vertex color/alpha, alpha test, sidedness, depth and blend settings. No lights/shadows/AA/fog/tone mapping/postprocess, no gameplay or extra characters. This is a deliberately reduced diagnostic profile, not production PBR parity or an accepted final quality reduction.

Preflight64: both report86 actual commands and1,372,924 submitted triangles; paired captures visually checked. Metadata records actual GL context antialias=false/SAMPLES0. Timers captured both native and Three contexts; earlier missing capture was repaired and rerun. Native per-primitive vs Three aggregate frustum policies remain different; report real submitted work at every larger load, never assume it remains equal. Cross-engine exact pixel equality is not the acceptance gate; same-renderer D/C pixels remain strict.

## Production diagnostic (separate)

Original app: renderer antialias=true at context creation, sRGB, ACESFilmic, exposure1.25; Hemisphere2.5/directional3.1, fog90–245, cached PCFSoft shadow2048 with autoUpdate=false, normalBias.12/bias-.00015. Native authored Rome uses FXAA and its own shader/material/light/shadow implementations. Actual context samples/runtime metadata and captures accompany measured rows. Same camera/load/viewport/pose are controlled; complete shading/gameplay/AA equivalence is not established. Do not interpret production FPS difference as a matched-quality renderer ranking.

Independent research used current official Three docs only for API meaning (antialias fixed at context creation, outputColorSpace, toneMapping); the local r170 code, not documentation for dev, governs the actual reference. No arbitrary vendor upgrade or artificial delay.

## Investigated technique comparison (rejected)

Pinned r170 `WebGLRenderer.renderScene` reads `currentRenderList.opaque/transparent` and checks their lengths before dispatch (`three.module.js:30300–30312`), rather than scanning scene primitives again to decide whether the transparent list is empty. Our B3 model classification already provides the corresponding information; C experimentally reused its lengths with explicit invalid-update fallback. This tested adapting the list-reuse principle to Rust lifecycle dependencies, rather than copying Three’s scene architecture or adding a second cache.

Despite CPU savings and exact images, C regressed stress FPS in all three pairs on both backends and was rejected. Final R restores B6 counting; the table reports C as an experimental runtime, not R. No claim of an accepted Three-derived production speedup.
