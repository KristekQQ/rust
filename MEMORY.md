# Current project map — 2026-10-19

Browser `webgpu_wasm`: Rust/WASM + wgpu/WGSL; not a native app. Check current code before relying on this dated map. Read only the relevant source/doc; historical evidence is in [the archive](docs/history/MEMORY-2026-10-13.md), not required context.

| Area | Entry points |
|---|---|
| CPU state, simulation, visibility | `src/scene.rs`, `src/visibility.rs`, `src/input/` |
| WASM exports and thin SDK | `src/web.rs`, `js/engine.js`, `js/engine.d.ts` |
| Rendering and canvas output | `src/render/renderer.rs`, `state.rs`, `output.rs`, `target.rs` |
| Shadows/reflection/targets | `src/render/effects.rs`, `textures.rs`; `docs/RENDER_PASSES.md` |
| Postprocessing | `src/render/postprocess.rs`; `docs/POSTPROCESSING.md` |
| Frame graph, transient buffers/textures, profiling | `src/frame_graph.rs`, `src/postprocess_plan.rs`, `src/render/graph_resources.rs`, `src/render/profiler.rs`; [FRAME_PIPELINE.md](docs/FRAME_PIPELINE.md) |
| glTF asset/animation/PBR | `src/gltf_model.rs`, `gltf_loader.rs`, `gltf_animation.rs`, `src/render/gltf_gpu.rs`, `gltf_pbr.wgsl`; `docs/GLTF.md` |
| Authored JS scenes, optional tools | `examples/<name>/main.js`, `scene.js`, `examples/shared/tools.js`; `examples/README.md` |
| Local build/server and verification | `scripts/run-local.sh [port]`, `scripts/serve-local.py`; `docs/DEVELOPMENT.md` |

## Decisions and useful boundaries

- Concurrent work (2026-10-22): use [parallel-work](.agents/skills/parallel-work/SKILL.md). Keep substantial tasks in separate worktrees, record file ownership in the shared Git-common-directory registry, and assign one integrator per shared remote release target. Compare relevant input identities at milestones; changed bytes do not identify their author. Runtime ownership/status belongs in that local registry, not this versioned map. Never combine another task's dirty changes or replace its build/package as part of a push.
- Stable object/light IDs are never reused. Transforms use radians; scheduled rotation deltas use degrees, timing seconds. Four lights; Rust host layouts must match WGSL.
- Model handles own Rust hierarchy/pose. Import never activates a camera; embedded cameras require explicit SDK commands. Current skin/morph deformation is CPU-cached by changed pose; shader PBR supports core maps/alpha modes. Extension/IBL limits are documented in `docs/GLTF.md`.
- Approved cathedral view `[6,2.5,2]` toward `[-12,8,0]`, first light `[-19,12,0.2]`. Keep this placement unless asked to change it. 72,513 triangles; glass faces omitted to expose window apertures. Stone metallic=0/roughness=.95; asset CC BY-NC. Light is point/perspective, not directional sun.
- God-ray example density=.055, exposure=40, strength=.8; engine defaults .035/2.5. Helpers stay outside reflection/postprocess depth. Server sends no-store to avoid mismatched cached modules.
- Engine review milestone `c1b3ef00`: 59 native tests; detailed dated evidence is in `docs/CODE_REVIEW.md`. New harness entry points and runtime dispose/reinit contracts are in `docs/DEVELOPMENT.md` and README. Use a current verification report rather than historical counts as proof for changes.

## Current Quest / neon decisions — 2026-10-19

- Stable XR baseline `b07ae09b`: user confirmed SSR artifacts fixed on Quest 3 / Browser 152.1 with owned per-eye color/depth staging and one final stereo submit. Keep that presentation boundary; Browser 149.1's zero-color-metadata normalization is an earlier, separate workaround. New frame-pipeline code has desktop GPU/WebGL and simulated XR checks; actual headset validation is pending. No headset performance or WebGPU/WebGL color-parity measurement is established. Route XR support through [quest-webgpu-xr](.agents/skills/quest-webgpu-xr/SKILL.md); details: [WEBXR.md](docs/WEBXR.md).
- Generic frame planning uses single-writer logical resources and exact-descriptor transient aliasing. Rust encodes actual passes; JS exposes graph/profile diagnostics. See [frame-pipeline](.agents/skills/frame-pipeline/SKILL.md) for extension and invalidation rules. GPU physics migration and fixed-step simulation changes are not part of this implementation.
- `examples/neon-arena` offers glass-like colored facets with opaque neon seams (default) or the original open wire style before VR entry. `FRUIT_SCALE=.105` halves gameplay fruit, independently of gallery icons. Glass cuts retain colored translucent caps; open style uses black caps + `.25` cutout. Both retain closed CPU solids. Restart is at Z=-1.45 m; glass uses stable high-contrast facet masks (base .06, variation 1, per-variant coverage .35–1). Fifteen diverse fill masks per kind are selected and ordered before VR, share topology and cycle without repeats across rounds. See the project skill for presentation rules.

- WebGPU XR color contract: all eye output uses owned sRGB render targets and compatible UNORM source views for byte-preserving final stereo presentation. Browser UNORM views cannot be reinterpreted as sRGB; direct linear output was darker. Quantitative gray tests pass with effects on/off; actual Quest color parity/performance still needs user validation. See docs/FRAME_PIPELINE.md.

- Final composition/overlay fusion (2026-10-22): the last postprocess draw shares one physical pass with alpha/rings/glow/helpers; no overlays means no depth attachment. Keep WebGPU read-only sampled depth and WebGL separate packed depth. Logical graph counts differ from physical passes; see [FRAME_PIPELINE.md](docs/FRAME_PIPELINE.md). No resolution/shader reduction and no established Quest FPS gain; use controlled same-package headset measurements.
