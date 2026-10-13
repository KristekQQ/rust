# Development and verification

Use checks for the changed area, after dependent writers finish. Group native build/check commands into one verification stage; inspect the affected page and console in one browser session. Keep a compact PASS/FAIL record with artifact/version. After fixes repeat affected checks; required final checks still apply. Do not rebuild merely for a new screenshot or unchanged page.

| Change | Required checks |
|---|---|
| CPU state/camera/visibility/loader | `cargo test`; actual browser/input workflow when visible behavior changes |
| Renderer/WGSL/web exports | wasm32 check, WASM build, affected Rust page and console; WebGPU + WebGL for shared rendering paths |
| SDK/types/callers | Build matching `pkg/`, `npm run typecheck`, `tests/scene-sdk.html` on affected backends |
| Canvas/output/targets | `tests/render-target.html` and its verify button on WebGPU + WebGL |
| Shadow/reflection/pass cache | `tests/render-effects.html` verify button + SDK integration on WebGPU + WebGL |
| Postprocessing/PBR interaction | Affected example and its verify control on WebGPU + WebGL; inspect real effect on/off or pose change |
| glTF runtime/PBR | `tests/gltf-full.html`, `tests/gltf-sdk.html` on WebGPU + WebGL; actual textured/animated model for rendering changes |
| Mirrored transforms / winding | `tests/render-mirrored.html` pixel assertions and `tests/render-effects.html` verify on WebGPU + WebGL |
| SDK lifecycle/loading | `npm run test:sdk`; `tests/scene-sdk.html` and `tests/gltf-sdk.html` on both backends |
| Backend initialization | `index.html?backend=auto` and `index.html?backend=webgl` |
| Guidance/docs only | Validate links/config/skill as relevant; no GPU rebuild. Before commit still run native tests + pre-push |

```sh
cargo test
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web
npm run typecheck
npm run test:sdk
```

`scripts/run-local.sh [port]` builds then serves loopback HTTP with no-store headers. Reuse a healthy running server; do not build a second time just to start serving. Generated `pkg/` is ignored. Match SDK exports to freshly built WASM and use final resource URLs after redirects. Initialize with `await initEngine(...)` before commands. SDK owns no simulation/RAF loop.

Native tests omit the wasm32 renderer/web bridge. PASS text checks only its explicit assertions: it does not alone prove shading, animation pixels or hardware performance. Inspect the affected real Rust page (`cube.html`/`render_webgpu.js` are separate demos). Capture only images needed to judge the change. Avoid concurrent render tabs, and restore/close temporary test pages.

Source ownership: `renderer.rs` owns scene GPU resources and accepts views/targets; `output.rs` presentation; `state.rs` connects them. `effects.rs` pass ordering/dirty caches; `textures.rs` reusable targets. Preserve per-pass visibility and static revision invalidation. Both `data.rs`/`shader.wgsl` and `gltf_gpu.rs`/`gltf_pbr.wgsl` require matching GPU layouts.

Inspect Cargo sources before dependency/offline changes. The offline config uses `vendor/`; toolchain restoration in `offline.sh` assumes Linux x86_64 and needs review on Apple Silicon. Packaging/restoration details are in README; do not run them for ordinary development.

Before commit/push, require `cargo test` PASS then `scripts/pre-push`; commit only owned paths. Successful checks on unchanged code may be reused within the same milestone; invalidate them when relevant source/dependencies/config changes.
