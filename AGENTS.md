## Project context

Respond in Czech, as requested in `CESKY.md`.

This repository is a browser 3D scene prototype built with Rust, WebAssembly,
`wgpu`, and WGSL. The crate is `webgpu_wasm`; this is not a native desktop app.
Read `MEMORY.md` for the source map and dated findings. Check the current code
before relying on those findings.

## Working on the renderer

The user wants scene management, logic, calculations and simulation in Rust.
`src/scene.rs` is the CPU SceneManager; the renderer consumes its state. JS/TS
is a thin command SDK holding IDs, not a second scene manager. Even the fixed
example is defined in Rust; `js/scene.js` only requests it. Keep automatic
spawning out of the example unless requested.

- `index.html` is the main Rust/WASM scene. `cube.html` and `render_webgpu.js`
  are independent JavaScript graphics demos, not tests of the Rust renderer.
- Changes to exported functions in `src/web.rs` must stay consistent with the
  API in `js/engine.js` and callers in `js/scene.js` / `js/app.js`. Rebuild `pkg/` to test them;
  generated JavaScript and WASM are ignored by Git.
- `src/render/renderer.rs` owns scene GPU resources and accepts RenderView/RenderTarget;
  `output.rs` owns canvas/surface presentation, and `state.rs` connects them. Verify
  `tests/render-target.html` in WebGPU and WebGL when changing output paths.
- `render/effects.rs` owns shadow/reflection pass ordering and dirty caches;
  `render/textures.rs` owns reusable color/depth targets. Preserve independent
  per-pass visibility and the static scene revision. Test `tests/render-effects.html`
  and SDK integration in both WebGPU and WebGL. See `docs/RENDER_PASSES.md`.
- Keep Rust GPU data layouts in `src/render/data.rs` consistent with
  `src/shader.wgsl`, including padding and the light count.
- Native `cargo test` covers the SceneManager and camera modules but not the renderer and
  web bridge, which are gated behind `target_arch = "wasm32"`. For renderer changes also run
  `RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown`.
- For visible rendering or input changes, build with
  `RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web`, serve the
  repository over HTTP, and test `index.html` in a browser. Check console
  errors, the rendered scene, and the affected controls. A screenshot of
  `cube.html` does not verify this workflow.
- `rust-toolchain.toml` pins this project's toolchain. `scripts/run-local.sh`
  builds the WASM demo and serves it on loopback; an optional argument sets the port.
  Test both automatic backend selection and `index.html?backend=webgl` when
  changing backend initialization.
- Inspect Cargo configuration before changing dependency sources. The current
  `.cargo/config.toml` has no source replacement; the offline config uses
  `vendor/`. Offline toolchain restoration in `offline.sh` assumes Linux
  x86_64 paths and needs review before use on Apple Silicon.

SDK changes should also pass `npm run typecheck` after building `pkg/`, and
the real WASM integration page `tests/scene-sdk.html`.

## Before committing or pushing

Before committing or pushing changes to this repository:

1. Run `cargo test` and ensure all tests pass.
2. Then run `scripts/pre-push` to update `all_sources.txt`.

If needed you can run `./offline.sh pack-all` to build offline archives. Use `./offline.sh unpack-all` to restore them.
See the README for full instructions.
