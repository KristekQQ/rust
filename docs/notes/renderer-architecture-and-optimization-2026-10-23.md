# Renderer architecture and scene-specific optimization

Date: 2026-10-23. Scope: research, source inspection and isolated desktop diagnostics. No engine, gameplay, shader, WASM package or production output changes. Companion: [Rome quality audit](rim-render-quality-2026-10-23.md).

## Decision

Keep Rust/WASM for now. We have identified a concrete glTF visibility defect and measured avoidable animation work. A language migration would carry those design problems into a new implementation unless they were addressed separately. There is no isolated measurement establishing WASM as our dominant cost.

This is not a promise that our renderer will outperform Three.js. Three already supplies useful rendering quality and scene-management behavior that our port does not match. Shipping content with a mature renderer and maintaining an independent engine are different product investments.

## Confirmed glTF visibility defect: highest priority

`src/visibility.rs`, `RenderQueue::extract_pass`, retains imported mesh keys for existing meshes and clears their instance vectors. Only visible candidates refill those vectors. Retaining empty allocations is legitimate cache reuse.

However, `src/render/renderer.rs:1796` iterates `queue.imported.keys()` and calls `gltf_gpu.draw` without checking the associated vector. The translucent `draw_many` path also consumes keys. A model previously admitted to that view can therefore keep submitting its primitives after its root is culled. The generic imported-mesh path checks empty instances; the glTF path does not. Other key consumers require audit: pipeline preparation, shadow draw, overlay selection/draw and statistics. A retained cache key is not a current visibility decision.

An isolated browser experiment froze scene animation, removed the game driver, held a fixed camera and cumulatively moved each model root to a hidden position. The visible-object statistic decreased to one, but actual browser draw API calls did not decrease for any model:

| Backend | Browser draw calls per frame, throughout hiding |
|---|---:|
| Native WebGL2 | 384 |
| Native WebGPU | 383 |

Instrumentation wrapped WebGL2 draw methods and `GPURenderPassEncoder` draw methods; this is not merely our own draw counter. The one additional WebGL draw is consistent with the backend presentation path. Hidden geometry can be clipped away and produce no visible image while still consuming CPU command preparation, submissions and vertex processing. This finding does not quantify the eventual FPS improvement from a fix.

Evidence: `output/verification/rim-webgpu-investigation/draw-audit-api.json`, corresponding log and `draw-audit.mjs`. Hiding the city, three visible orbiters, Mad Mechanic and both swords reduced the visible count from eight to one without reducing API draws. Other tested roots were already outside the visible set. Source explains the visibility-history dependence of draw counts between earlier runs.

Proposed fix contract, not implemented: consistently derive active glTF models from nonempty pass-local imported entries. Preserve allocation reuse, per-view/per-eye visibility and transparent ordering. Audit every glTF consumer rather than patching only its diagnostic count. Acceptance: visible → hidden → visible, actual API draw reduction, expected pixels, auxiliary passes, asymmetric XR eye visibility and cleanup on both backends.

Independent read-only source review confirmed this diagnosis with no material objection. It also confirmed the same unchecked-key pattern in depth/shadow drawing (`renderer.rs:1536`), transparent color drawing and final transparent overlays (`2296`), plus redundant pipeline preparation (`1630`, `2231`). Runtime draw evidence here covers the main Rome configuration, not separate execution of every auxiliary pass.

## Measured inactive-animation cost

Eighteen desktop runs: three repetitions of each of three scenarios on each backend. Each fresh page had 1.8 seconds of warmup and five seconds of gameplay, 1280×800, DPR 1, profiling enabled, AA/shadows/fog/glow disabled. Frozen runtime snapshot files were checked against their manifest. These settings isolate costs; they do not reproduce the reference's default visual quality.

Mean CPU time inside the RAF callback, averaged over three runs:

| Scenario | WebGL2 | WebGPU |
|---|---:|---:|
| Current game | 3.513 ms | 1.761 ms |
| Pause only inactive tank and wraith animation | 2.985 ms | 1.357 ms |
| Freeze native SceneManager simulation | 2.518 ms | 0.769 ms |

Pausing two unused pool actors reduced callback CPU by approximately 15% and 23% respectively. Active actors and game simulation continued. The frozen-scene row is a diagnostic lower-work case that changes animation, not a proposed quality-preserving optimization.

Mean reported last-frame upload bytes were 84,976 / 49,456 / 16,336 across the three scenarios. These are engine counters sampled at the end, not average transfer bandwidth or a complete browser-driver accounting. Accepted runs had no page errors. An initial harness attempt to pause a clone without playback failed and was excluded; the corrected harness checks playback first.

Source explanation: `src/rim_game.rs` hides inactive actors without pausing their animation. `src/scene.rs` updates playing model instances independently of visibility. The reference Three `characters.js` advances a mixer only when its holder is visible. Pause actual inactive pool entries; do not indiscriminately pause offscreen gameplay, death animation or pending cut processing.

Evidence: `output/verification/rim-webgpu-investigation/ablation.json`, `ablation.mjs`, `probe-ablation.js` and `ablation-resume.log`.

## Additional source-based opportunities

These are candidates rather than measured independent speedups:

- **Animation bounds:** `SceneManager::commit_model` calls `model_bounds`; `src/gltf_animation.rs:497` scans even rigid primitive vertices to recompute bounds. Cache conservative local render bounds and transform their eight corners, or equivalent center/extents. Preserve exact gameplay/cutting bounds where required; current API tests must not silently change semantics. Existing GPU rigid-geometry reuse does not remove this CPU scan.
- **Animation ownership and dirty data:** playing model instances are cloned during scene update. Inspect ownership and actual changed ranges before redesigning. Repeated emission commands invalidate render revision even for unchanged values, but Rome updates active bolts, not all 32 pool entries. This is not established as the main cost.
- **Shared glTF assets:** repeated loads of the same URL parse/import independent assets and upload geometry again. Share immutable geometry/material resources across clones while keeping independent poses and lifetime ownership. Memory/startup benefits are plausible; frame-time benefit has not been isolated.
- **Command preparation:** group compatible opaque primitives and batch changed GPU data only after measuring below wgpu. Existing shader/pipeline caches and backend state filtering must be accounted for. Preserve alpha ordering and per-eye uniform ranges.

Our engine already has BVH visibility indexes, static/dynamic separation, shared mesh variants, no-op transform handling, material/pipeline caches and view caches. Recommending those as entirely missing would be inaccurate. Older passages in `docs/RENDER_PERFORMANCE.md` describing a flat scan and older instance stride disagree with current source; they are not evidence of current behavior.

## What the earlier backend profile establishes

One eight-second desktop gameplay run measured RAF callback means of 1.896 ms for Three/WebGL2, 4.224 ms for native WebGL2 and 1.942 ms for native WebGPU. Native WebGL2 scope means included simulation 0.835 ms, GPU updates 0.290 ms and color encoding 0.777 ms; native WebGPU included 1.010 / 0.315 / 0.357 ms respectively. WebGPU supplied one latest sampled main-pass GPU timestamp of 1.421 ms, 21 frames old; this is not a GPU average. WebGL GPU timestamps were unavailable.

Crucially, Three reported 252 draws, native GL 384 and native WebGPU 317. The visibility defect means native command counts depend on prior view history. Consequently this is not a matched-work language benchmark or a reliable speed ranking. It establishes a reason to investigate backend CPU work after fixing scene work, not a numerical WASM tax. RAF callback CPU duration also excludes asynchronous GPU completion and is not FPS.

Evidence: `output/verification/rim-webgpu-investigation/profile.json` and `profile.log`.

## Quality and optimization must share a contract

The inspected Mac comparison rendered approximately 2.24 times as many Three pixels as native pixels. Three requested AA and used authored sun/hemisphere lighting, cached shadows, ACES exposure and fog; the native port used lower pixel density, single-sample glTF rendering and approximate point lights. See the companion audit for exact dimensions and source pointers. Lower quality is not a property of Rust or WASM.

Restore intended scene quality before making an equal-quality throughput claim. Useful Three techniques include hemisphere diffuse fill with the authored sun, derivative-based specular antialiasing and shared immutable model resources. Do not just add more expensive point lights or bloom. All six inspected Rome GLBs are texture-free, so texture filtering is not a remedy for this scene's present appearance.

## Scene-specific sequence

| Scene | First investigation or change | Evidence still needed |
|---|---|---|
| Rome | Correct glTF visibility, pause inactive pool animation, cached rigid render bounds; then quality parity and shared assets | Before/after pixels and actual draws, CPU p50/p95/p99, GPU samples; real Quest stereo |
| Neon arena | Profile SSR, glow, transparent overdraw and fragment lifetime; retain existing shared variants and cut behavior | Current-scene GPU profile on headset; fixed resolution/effect comparison |
| Forest arena | Profile shadow lights/bloom; evaluate spatial chunks for a large environment root versus additional draws | Current-scene CPU/GPU breakdown and camera sweep; measure existing shadow-cache hits |

Only Rome received fresh runtime measurement in this audit. The other rows are source-based investigation plans. Culling a single giant environment root cannot reject spatial regions inside that root, but subdividing it can increase draw overhead; measure that tradeoff.

For future acceptance, record exact source/WASM hashes, browser/device, actual backend, eye buffer size, AA, effects, camera/poses and active wave. Separate simulation/animation CPU, bounds/culling, upload preparation, encoding/submission and sampled GPU pass time. Record spikes during cuts and first appearances, not only averages. Compare profiling-off cadence too. A 90 Hz headset has about 11.1 ms for the entire frame; desktop callback duration cannot establish that budget is met.

## Rust, WASM, JavaScript and browser GPU APIs

The current browser Rust build uses wgpu 30 with WebGPU and WebGL support. Its WebGPU backend issues browser WebGPU API calls; the WebGL path additionally uses wgpu-core processing before browser GL calls. Direct JavaScript rendering could remove some binding/dispatch work, but would not remove equivalent draw commands, shader work or GPU bandwidth. We have not isolated those overheads in our scene.

[wgpu browser platform documentation](https://wgpu.rs/doc/wgpu/documentation/platforms/web/index.html), [wgpu backend descriptions](https://wgpu.rs/doc/wgpu/struct.Backends.html), [WebGPU specification](https://www.w3.org/TR/webgpu/).

| Option | Consequence | Current recommendation |
|---|---|---|
| Rust/WASM simulation and renderer | Keep existing physics/cuts/XR architecture; repair demonstrated defects and missing quality features | First choice while evaluating the engine |
| Rust/WASM simulation + Three renderer | Retain simulation/cutting, gain mature visual features; requires a measured bulk scene-data bridge | Credible fallback if content delivery outweighs maintaining a renderer |
| Full JavaScript/TypeScript + Three | Broader gameplay/physics/cutting integration rewrite; mature rendering, no guaranteed GPU speedup | No evidence justifying this rewrite yet |
| Different native language compiled to WASM | Still WASM and browser GPU APIs | Does not address the demonstrated issues |
| Native Rust graphics app | Can leave browser/WASM, but changes distribution and deployment | A separate product decision |

For a hybrid, transfer a compact typed frame snapshot and finalize controller/simulation changes before either XR eye. Do not introduce per-object JSON traffic or an asynchronous visual/physics mismatch. Heavy cuts may remain worker jobs with an explicit completion handoff, consistent with the requested behavior; switching renderer does not settle their synchronization contract.

A migration decision needs an optimized, quality-matched scene, equivalent draw work and headset data. If renderer command overhead remains dominant after the concrete fixes, compare a narrowly scoped direct-browser renderer prototype with exactly the same geometry, materials, poses and eye outputs. Keep renderer ownership separable enough that such a comparison does not require rewriting simulation first.

## Work delivered and limits

Delivered diagnosis, reproducible diagnostic artifacts and this strategy note. No renderer fixes, production rebuild, commit or push. No physical headset performance measurement. Observed optimization percentages apply only to these desktop diagnostic conditions and cannot be added together or extrapolated to Quest FPS.


## Implementation follow-up (same day, explicitly authorized)

The later request authorized implementation and subagents. The Rome profile now
uses the authored hemisphere/sun, ACES exposure, fog, cached 2048-square shadows,
desktop DPR cap 1.5, specular AA and opaque FXAA. Three remains the original
reference. Native glTF submissions filter empty visibility entries; inactive Rome
pooled animations pause, preserving visible death animation and atomic cut swaps.

Two integration defects were caught and fixed: WebGL2 must not build an unused
transparent MRT pipeline, and specialized glTF shadow shaders must remain valid
on both Naga and Dawn. The browser runner now captures uncaught GPU validation
events, which Dawn may otherwise only print as warnings. The orthographic shadow
regression checks both a local cast shadow and preserved direct ground lighting.

These changes retain Rust/WASM. See `examples/rim/README.md` and
`output/verification/rim-quality/` for current verification/artifacts. FXAA differs
from Three MSAA; transparent overlays, HUD and fragment materials retain stated
differences. Browser/simulated XR checks do not establish physical Quest quality
or performance. No commit or push was requested. Earlier audit numbers above
remain historical, not timings of this new profile.
