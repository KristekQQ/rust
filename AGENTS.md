# Rust/WASM browser engine

Reply in Czech. Rust owns scene state, calculations, simulation and rendering; JS/TS is a thin command SDK holding IDs. Runtime examples live in separate `examples/<name>/main.js` and `scene.js` modules with explicit imports. Optional DOM tools stay outside the SDK. No hardcoded runtime scenes or automatic spawning; `src/test_scenes.rs` is native fixtures only.

Project-owned skills live in versioned `.agents/skills/`, not personal `~/.codex/skills`. Use [rust-webgpu-project](.agents/skills/rust-webgpu-project/SKILL.md) for engine/SDK/example work; keep its links portable. Generic personal/tool skills stay outside the project.

At Honza's request, [honza-intent](.agents/skills/honza-intent/SKILL.md) is also project-owned so other instances share his intent-clarification workflow. Use it for unresolved ambiguity or corrections after misunderstanding, not typos alone.

For scene placement, rotations, scale and attached world-space labels, use [scene-object-layout](.agents/skills/scene-object-layout/SKILL.md); verify requested sides in XR coordinates, not only the desktop preview.

For direct Quest WebGPU XR, runtime support, projection textures and headset diagnostics use [quest-webgpu-xr](.agents/skills/quest-webgpu-xr/SKILL.md).

For frame scheduling, transient textures/buffers, pass dependencies and CPU/GPU profiling use [frame-pipeline](.agents/skills/frame-pipeline/SKILL.md); the extension guide is [FRAME_PIPELINE.md](docs/FRAME_PIPELINE.md).

For shared XR player/inventory work use [xr-player-equipment](.agents/skills/xr-player-equipment/SKILL.md). For cut-impact visuals and speed tuning use [cut-feedback-tuning](.agents/skills/cut-feedback-tuning/SKILL.md); prefer existing configuration knobs for numeric iteration.

Keep Rust/WGSL layouts and `src/web.rs` exports consistent with `js/engine.js`, types and callers. Rebuild ignored `pkg/` when WASM/export changes need browser verification. Test the affected Rust page; `cube.html` and JS graphics demos do not verify this renderer.

Use `MEMORY.md` only for orientation/current decisions; consult linked docs only for the affected area. Choose required checks from [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Native tests omit wasm32 renderer/web code. Preserve independent pass visibility, static revisions and resource cleanup. Respect `rust-toolchain.toml`; inspect Cargo configuration before source/offline changes.

During iteration run only the smallest affected check; do not automatically run the full suite after each edit or milestone. Docs-only changes need no GPU tests. Reuse existing matching evidence; repeat only failed/invalidated checks. Keep output to concise summaries, expand failures only.

Before a requested commit/push run `npm run verify:push` once: full native/WASM/SDK/browser checks and source dump. Commit the dump with the change. The installed pre-push hook reuses the content-bound PASS receipt, not the tests; changed inputs/options/toolchain or WASM invalidate it. Never run `cargo test`, full verify and the hook separately on the same inputs. `VERIFY_FORCE=1 npm run verify` explicitly refreshes evidence. Preserve unrelated staging.

Use targeted `cargo test <filter>`, `test:sdk`, `test:harness` or `verify:browser -- --backend <affected> --page <affected>` during development only when useful. Full both-backend coverage belongs to the final push gate. Browser verification rejects stale `pkg/`, backend fallback and console errors. Install hook: `git config --local core.hooksPath scripts`. CI software WebGL remains independent of local WebGPU; disabling Chromium sandbox is opt-in only on disposable Linux GitHub runners, never automatic on ordinary user sessions.
