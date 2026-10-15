# Current project map — 2026-10-13

Browser `webgpu_wasm`: Rust/WASM + wgpu/WGSL; not a native app. Check current code before relying on this dated map. Read only the relevant source/doc; historical evidence is in [the archive](docs/history/MEMORY-2026-10-13.md), not required context.

| Area | Entry points |
|---|---|
| CPU state, simulation, visibility | `src/scene.rs`, `src/visibility.rs`, `src/input/` |
| WASM exports and thin SDK | `src/web.rs`, `js/engine.js`, `js/engine.d.ts` |
| Rendering and canvas output | `src/render/renderer.rs`, `state.rs`, `output.rs`, `target.rs` |
| Shadows/reflection/targets | `src/render/effects.rs`, `textures.rs`; `docs/RENDER_PASSES.md` |
| Postprocessing | `src/render/postprocess.rs`; `docs/POSTPROCESSING.md` |
| glTF asset/animation/PBR | `src/gltf_model.rs`, `gltf_loader.rs`, `gltf_animation.rs`, `src/render/gltf_gpu.rs`, `gltf_pbr.wgsl`; `docs/GLTF.md` |
| Authored JS scenes, optional tools | `examples/<name>/main.js`, `scene.js`, `examples/shared/tools.js`; `examples/README.md` |
| Local build/server and verification | `scripts/run-local.sh [port]`, `scripts/serve-local.py`; `docs/DEVELOPMENT.md` |

## Decisions and useful boundaries
- Stable object/light IDs are never reused. Transforms use radians; scheduled rotation deltas use degrees, timing seconds. Four lights; Rust host layouts must match WGSL.
- Model handles own Rust hierarchy/pose. Import never activates a camera; embedded cameras require explicit SDK commands. Current skin/morph deformation is CPU-cached by changed pose; shader PBR supports core maps/alpha modes. Extension/IBL limits are documented in `docs/GLTF.md`.
- Approved cathedral view `[6,2.5,2]` toward `[-12,8,0]`, first light `[-19,12,0.2]`. Keep this placement unless asked to change it. 72,513 triangles; glass faces omitted to expose window apertures. Stone metallic=0/roughness=.95; asset CC BY-NC. Light is point/perspective, not directional sun.
- God-ray example density=.055, exposure=40, strength=.8; engine defaults .035/2.5. Helpers stay outside reflection/postprocess depth. Server sends no-store to avoid mismatched cached modules.
- Engine review milestone `c1b3ef00`: 59 native tests; detailed dated evidence is in `docs/CODE_REVIEW.md`. New harness entry points and runtime dispose/reinit contracts are in `docs/DEVELOPMENT.md` and README. Use a current verification report rather than historical counts as proof for changes.
