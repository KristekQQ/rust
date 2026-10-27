# Rim: native WebGPU, native WebGL2 and instanced Three.js — 2026-10-26

The earlier Three stress test used ordinary shared-geometry clones. Its generic CLI label said `instanced`, but that authored-Rim adapter did not implement GPU instancing. That result was explicitly limited and must not be used as the optimized Three reference. This follow-up implements, observes and validates actual `THREE.InstancedMesh` before making a fresh three-way comparison.

## Fresh measurements

Apple M1, Chrome155, **512 additional static Drone Orbiter GLBs**, gate camera, **2880×1920**, DPR1, same asset/pose/position function, no added AI or shadow casters. Every sample has 8s warmup and25s measurement, profiling disabled. Three alternating native-GL/Three pairs (AB/BA/AB), then three separate native-WebGPU repetitions; all serialized, without builds or concurrent benchmark browsers.

| Renderer | Median observed RAF FPS | Individual FPS | Actual draw calls |
|---|---:|---|---:|
| Native Rust/WASM WebGPU | **45.962** | 49.489,45.962,44.732 | 5,266 |
| Three.js r170 WebGL2, GPU instanced | **34.865** | 34.865,35.081,34.601 | 108 |
| Native Rust/WASM WebGL2 | **20.681** | 20.682,20.641,20.681 | 5,266 |

CPU callback medians respectively19.2/21.0/21.5ms,3.4/3.4/3.5ms and47.9/48.3/48.0ms. These are submission/update CPU scopes, not GPU duration. No GPU timestamps were enabled in these primary runs. Native WebGPU's range trends downward across its three samples; no thermal/background cause was measured, and no adverse sample was discarded.

This workload favors native WebGPU and favors Three over native WebGL2. It is not a universal renderer ranking. FPS is reciprocal mean application RAF interval, not compositor presentation. Native FXAA versus Three MSAA4, PBR/shadow implementations and culling policies remain unmatched; equal graphics and active combat are not established. These are desktop results, not Quest FPS.

## Same models and real instancing

Both native backends already use shared model assets and automatic GPU instancing. `Model.createInstance` is not a private geometry copy. This scene still issues5,266 actual commands (5,265 main plus FXAA), far more than the explicitly grouped Three fixture. Do not substitute legacy `renderer_stats().drawCalls=34343`, a primitive estimate, for actual submitted calls.

The new Three path preserves the source model's67 rigid visible primitives, geometry, material arrays/groups, layer masks, static transforms and normalized scale. It creates one `InstancedMesh` per source part, with all512 poses in each batch. The authored base contributes41 calls:67+41=108. Original ordinary-root mode remains selectable as the image oracle. A private frozen tree prevents later live Idle animation from changing one representation's pose. Instance bounds are recomputed; only owned instance buffers are disposed. Skin/morph, transparent materials and unsupported mirrored/sheared/singular transforms are rejected instead of simplified.

Actual native glTF triangle events are10,553,756 per measured RAF; Three's whole-render triangle counter is10,554,110. These counters have different ownership scopes and are not a proof of exact total geometry equality. Native reports eight glTF primitive culls; the354 difference is not fully attributed. Same requested assets/count/poses are independently established. In the Three-only gate/eye-offset oracle, ordinary and instanced paths submit identical triangle counts at512.

A concrete native restriction is visible in `src/render/gltf_gpu.rs`: overlap of model-root bounds ends a merge run (around1534); draw-ordinal merging then operates only inside each run (1547–1572). Measured counters show77 root-overlap breaks and1,943 ordinal breaks per RAF. This is a supported candidate for excessive fragmentation, not an isolated causal/optimization experiment. No native renderer kernel was changed or new native speedup claimed here.

## Correctness, review and delivery

The independent ordinary-clone oracle checks32 and512 models from gate, left/right offset cameras and an oblique view. All eight pass the existing8/255,0.001 changed-fraction policy: maximum fraction above8 is0.000245768, maximum individual channel error64/255. Output is within tolerance, not bitwise exact. Aggregate instance bounds submit more offscreen geometry in the oblique view; visible pixels remain within the gate. Offset cameras are not a simulated XR session. The source mixer is deliberately advanced by0.75s after factory creation to verify the frozen-template fix found during independent review.

TypeScript SDK check and29 tooling tests passed. The independent untyped Three reference is explicitly excluded from the SDK typecheck and covered by browser acceptance. **Nine final minified-production cases passed:** native entry/refresh/mobile controls, cuts/spatial panel in simulated XR and shadow caching on both backends, plus the Three image oracle, count/mode refresh, removal and mobile hit targets. No physical Quest acceptance is claimed.

A runnable copy is now at `/examples/rim-three/`, sharing the native stress-position function. Its selectors switch0–2048 models and `instanced`/`objects`; refresh preserves meaningful query parameters. The original application is retained as `app.js`, with source identity and upstream license recorded. The Three VR panel adds count/mode beside existing FPS/call metrics. Original8878 sources/server and the primary checkout were preserved.

Release WASM remains **36cabbf4ac596d867181c8278a848b99fc3197910f344835b02a3aba519c7ea2**, source **eae9f0a3d0b60d131a4031465d86d7006cdc5ab816fa83912967a023cfab48e0**. New production app **613216e02e**,254files, is served on **192.168.11.134:8443** through `npm run serve:quest`. The full manifest matches the browser-tested staging package;16 actual HTTPS decoded payloads (native, Three, drone GLB, vendor module and WASM) match hashes/no-store headers with explicit local-CA chain/IP verification. That does not establish headset certificate trust. Previous package: `output/production-before-rim-three-instancing-20261026`.

The branch remains local `codex/wasm-render-performance`, HEAD64bfda68, uncommitted/unpushed; no CI claim. [Summary and all nine samples](summary.json), [compressed raw archives/checksums](archive-sha256.json), [deployment evidence](evidence/deployment/deployment-check.json). The collector validates exact measured driver hashes, frozen input manifests, no server errors, complete cleanup and every raw gzip. `evidence/three-example-source.json` identifies the shipped reference sources. The earlier ordinary-clone numbers remain historical and are not combined into a formal before/after speedup.

Next bounded experiment: test a safe opaque-part batching path against independent overlap/coplanar/order and stereo oracles, before relaxing the current root-overlap barrier. Preserve transparency and existing scene quality; measure both native backends rather than assuming the Three technique transfers unchanged.
