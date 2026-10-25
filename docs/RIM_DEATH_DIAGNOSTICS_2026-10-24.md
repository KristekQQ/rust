# Rome death effect and common runtime diagnostics

Local follow-up on `088fdb34`, branch `codex/core-render-optimization`, isolated checkout `rim-publish/rust`. Previous optimization changes remain intact and uncommitted. This is not a claim that the renderer is fully optimized or faster than Three.js.

## Fixed death presentation

The actual cause of stationary cut halves was `SceneManager::update_projectiles`: fading pieces deliberately bypass projectile motion. Registering the halves for dissolution therefore suppressed their separation velocity. Rome now owns deterministic fragment motion in Rust, using the same simulation step as the presented scene. All chunks on one side share one direction; separation grows at 0.52 m/s to 0.65 m on each side, with a small delayed downward offset. Dissolve begins at 1.26 s and removes the pieces at 3 s.

The original model freezes at the impact pose while real posed-triangle clipping runs in the existing Rust/WASM worker. Hidden staging, renderer upload acknowledgement, atomic publication, 3500 prepared triangles and 9000 staged vertices per tick remain in place. Worker latency no longer consumes the visible effect lifetime. Reset/stale/error cleanup remains native.

This does not introduce a physics worker. The worker computes geometry; the main Rust simulation presents the halves. Cut meshes retain vertex colors and base-color factors, not full source UV/PBR materials, cap surfaces or the later Three.js Explode animation. Complete material/animation parity is still open.

## Diagnostics

All 18 Rust example bootstraps in this checkout use the optional shared example wrapper; the core SDK remains opt-in and spawns no scene objects by default. The common desktop panel displays backend, canvas extent/DPR, FPS/frame/p95, named CPU scopes, asynchronous GPU pass samples and their age, actual draw calls, BVH/visibility, upload bytes and numeric cut counters when available. Missing triangle/write/GPU/XR-target counters are explicitly unavailable rather than inferred. Polling is limited to 4 Hz, without a new render or simulation loop. Stats text passes pointer events through to underlying controls; its hide/show button remains interactive. `?info=0` disables the example panel and default profiling.

In XR, the DOM panel hides and a native spatial diagnostics mesh uses a feet/reference-frame attachment: offset `[0.9,0.35,-1]`, size `0.66 x 0.40 m`. Gaze rotation does not move it around the player. Existing head equipment is preserved; session exit/disposal releases panel and binding. Native rows include renderer, FPS/frame/p95, CPU/culling, sampled GPU sum/age, uploads, visibility and draws. The SDK exposes validated head/feet offset options. Desktop Rome now passes real elapsed time separately from simulation time, so bullet time does not inflate FPS.

The running Three reference on 8878 uses the same generic desktop formatting and a bounded observational adapter. Its CPU measurement is the entire animation callback (update plus render submission), unlike individual native scopes. Its GPU timestamp and upload counters are unavailable. Its spatial panel now includes these measured fields and follows the common visibility setting. Source adapter: `examples/shared/three-runtime-info.js`; runtime copy and reference edits live outside this Git checkout under `Documents/Codex/2026-10-22/ude/outputs/anticke-mesto/dist`.

## Evidence and delivery

- Native library: 350 passed, 2 ignored. Includes actual half offsets, delayed fade/removal, feet-anchor gaze/reference/floor behavior and tracking loss.
- SDK: 29 passed, including offset dispatch/validation. Typecheck, formatting and diff checks passed. Locked wasm32 check and release WASM build passed.
- Final production-root browser checks: 13 applicable cases passed: Rome, neon-enemies and culling on both backends; Rome simulated XR and WebXR preview on WebGL; scene SDK and postprocessing on both; direct WebGPU XR simulator (64 assertions). Rome XR specifically passed 22 assertions including panel lifecycle and real cut rendering. An initial overlay interception failure was fixed and both affected backend cases rerun successfully.
- Both desktop backends captured impact/separation/evaporation before reset. Simulated XR preserves a labeled three-stage pixel strip before disposal. Inspected these images; evidence is no longer only a positive piece count.
- Three reference desktop smoke: live metrics, hide/show, `info=0` and no console errors passed. These are not matched performance benchmarks.
- One independent scoped source review found a Three VR visibility issue; fixed by gating spatial visibility on the common adapter state.

Ignored local artifacts: `output/verification/rim-{webgpu,webgl}-{impact,separated,evaporating}.png`, `rim-gameplay-report.{json,png}`, `three-runtime-info.{json,png}`, `rim-served-bytes.json`; `browser-report.json` contains the last five-case batch. The preceding eight-case final batch also passed in the maintained runner.

Final release WASM SHA256: `eda0b9ee6dea71eec71dc99334231f7dd29df397f5c91589809577ab3b48d195`. HTTPS 8443 served bytes match this WASM and current Rome/shared modules; validation used the explicit local development CA, without changing system trust. Three remains on 8878. No new commit/push, CI result or physical Quest acceptance.
