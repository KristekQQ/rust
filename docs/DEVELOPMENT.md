# Development and verification

## Automated entry points

`npm ci` installs the committed lockfile. Use `npm run verify` for native tests/Clippy/format, SDK and harness tests, locked wasm32 check/Clippy/build, typecheck, then all ten browser scenarios on **WebGPU and WebGL**. Install the pinned Rust toolchain, Clippy (`rustup component add clippy`) and wasm-pack 0.15.0 first. The runner uses this repository's pinned Puppeteer, closes each page before the next, and starts/stops its own loopback server on a free port.

```sh
npm run verify:native
npm run verify:fast                            # rychlý lokální gate bez Rust/GPU buildu
npm run verify:browser                         # matching WASM build + both backends
npm run verify:browser -- --page lifecycle,gltf-full
npm run test:browser -- --backend webgpu       # reuse a matching build
npm run test:browser -- --backend webgl --software
node screenshot_render.js --backend webgl
git config --local core.hooksPath scripts
```

`test:browser` compares source and generated-WASM/binding hashes to the successful build marker; it fails on stale `pkg/`. `--backend webgpu` uses the app's auto request but requires the actual `BrowserWebGpu` backend, so fallback never counts as WebGPU success. `--backend webgl` requires `Gl`. `--software` explicitly selects SwiftShader; CI runs only that software WebGL state, not a hardware WebGPU benchmark. Set `BROWSER_EXECUTABLE` to use another installed Chromium/Chrome binary. On Apple Silicon use an arm64 Node installation to avoid Rosetta browser startup. `--headed` opens the automated browser; `--capture` saves canvas images from passed scenarios.

Pro běžnou práci používej `npm run verify:fast`; kontroluje typy, SDK a testovací harness bez opakovaného Rust/WASM/GPU buildu. `npm run verify:native` přidej při změně Rustu nebo hooku. `npm run verify:browser -- --page <affected>` spusť s jedním backendem při iteraci; oba back-endy a všechny scénáře patří až na integrovaný milestone nebo před push. Browser test znovu nespouštěj jen kvůli screenshotu, pokud se nezměnil zdroj ani build marker.

Results, browser version, source/WASM and authored SDK/test/example hashes, and failed-page screenshots are saved under ignored `output/verification/`. Timeouts, failed page assertions, uncaught JS errors, error-level console output and HTTP failures produce a nonzero exit. Transport cancellations (`ERR_ABORTED`) are recorded separately because SDK tests intentionally cancel loads; awaited imports and pixel assertions still reject failed loading. The native harness regression deliberately fails the hook's test and dump stages and verifies that an existing source dump survives failed generation. The lifecycle browser page checks dispose during initialization, three dispose/reinit cycles, removed listeners, canceled RAF and rejected stale SDK handles. Presentation and glTF tests additionally inspect real canvas pixels. None of these establish performance across hardware or full glTF conformance.

For machine-specific executables, set `BROWSER_NODE` / `BROWSER_EXECUTABLE` or local Git options `verification.node` / `verification.browser` to absolute executable paths. These overrides are not committed or inherited by CI. The Node override also works when npm itself uses an Intel Node on an Apple Silicon Mac.

The installed `scripts/pre-push` always runs native tests before atomically generating `all_sources.txt`; any failure blocks push. Keep unrelated staged changes intact. The hook is local configuration; each new clone installs it explicitly.

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
| SDK lifecycle/loading | `npm run test:sdk`; `tests/lifecycle.html`, `tests/scene-sdk.html` and `tests/gltf-sdk.html` on both backends |
| Character movement/collisions/chase camera | `cargo test` (BVH, sweep, gravity, jumping, invalidation); matching WASM build and `test:browser -- --page fox-control` on WebGPU + WebGL (real keyboard/mouse, solid wall, two platform landings, reset, clips and pixels) |
| Frustum BVH / Rust HUD | Native brute-force pass/invalidation/pruning tests; `test:browser -- --page culling,fox-control` on both backends (10k pruning, F3, glyph pixels, cached refresh, disabled counters, texture output) |
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
