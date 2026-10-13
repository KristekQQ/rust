# Rust/WASM browser engine

Reply in Czech. Rust owns scene state, calculations, simulation and rendering; JS/TS is a thin command SDK holding IDs. Runtime examples live in separate `examples/<name>/main.js` and `scene.js` modules with explicit imports. Optional DOM tools stay outside the SDK. No hardcoded runtime scenes or automatic spawning; `src/test_scenes.rs` is native fixtures only.

Keep Rust/WGSL layouts and `src/web.rs` exports consistent with `js/engine.js`, types and callers. Rebuild ignored `pkg/` when WASM/export changes need browser verification. Test the affected Rust page; `cube.html` and JS graphics demos do not verify this renderer.

Use `MEMORY.md` only for orientation/current decisions; consult linked docs only for the affected area. Choose required checks from [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Native tests omit wasm32 renderer/web code. Preserve independent pass visibility, static revisions and resource cleanup. Respect `rust-toolchain.toml`; inspect Cargo configuration before source/offline changes.

Before committing/pushing: `cargo test` must pass, then run `scripts/pre-push` to update `all_sources.txt`. Preserve unrelated staging.
