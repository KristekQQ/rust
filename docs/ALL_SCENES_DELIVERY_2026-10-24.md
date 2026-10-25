# All scenes delivery — 2026-10-24

## Current handoff

- Working checkout: `/Users/Honza/.codex/worktrees/rim-publish/rust`, branch `codex/core-render-optimization`, committed base `088fdb34` plus preserved local follow-ups. No new commit or push.
- Live HTTPS server: existing PID 99090 on `192.168.0.17:8443`, serving `output/production/`.
- Application `bdd7929e03`, release WASM `43c94a078d074a1a359f3c75a208d5febfb6f11f798f6001a22345a3c8c5cdc5`.
- All 22 examples now share this engine: basic, crystal-cave, culling, cutting, cynatar, forest-arena, fox-control, gltf, lights, mirror, monkey, monster-room, neon-arena, neon-enemies, neon-font-preview, neon-robot, postprocessing, projectiles, rim, shadows, table-tennis, webxr. Gallery: `/examples/`.
- User must reload existing pages; this task did not inspect or refresh the user browser tab. No physical Quest acceptance.

## Why this refresh required integration

The previous Rome publication snapshot contained 18 examples while the primary checkout already contained four newer scenes. The isolated publication checkout now includes forest-arena, crystal-cave, monster-room (Neon District), and table-tennis, their scene-owned assets and native support. The primary checkout was read-only; copied source inputs were hashed and remained unchanged. Existing dirty renderer optimizations and Rome cut-hitch repair were retained.

Added native locomotion/surface queries and compound colliders, table-tennis simulation/contact kernel, and the scene-authored melee encounter. Shared Rust/WASM/SDK hooks were merged additively. Inventory feet anchors, renderer caches, cut upload acknowledgements and the existing Rome worker path remain supported. The gallery now links all 22 examples, including previously unlisted Neon Arena/font preview.

Independent source review found melee stepping after both eyes and stale locomotion after clearing a scene. Melee now steps with current tracking before eye rendering. A new active-XR clear regression caught a second clear entrypoint: SDK `Scene.clear()` invokes `clear_all`, whereas the first repair only covered `clear_scene`. Both entrypoints now share gameplay/locomotion/tracking reset before deleting geometry, preserving their full-clear versus geometry-clear behavior. Final scoped review found no additional actionable issue in those repairs.

## Verification

- Focused native checks: 86 passed (locomotion 9, collision 21, table tennis/contact 19, melee 8, Rome 22, XR player 7).
- SDK: 30 passed, including the imported locomotion ownership regression. Typecheck, strict wasm32 Clippy and optimized release build passed.
- Final packaged acceptance: **55/55 PASS**, covering real rendering of all 22 examples on supported WebGPU/WebGL2 paths (the WebXR demo intentionally uses WebGL), new scenes' simulated XR, melee/Rome gameplay, direct WebGPU XR, SDK scene clear and runtime lifecycle. The new cave active-XR clear regression passes. Inspected forest and table-tennis render screenshots.
- First candidate: 50/51 PASS; its failed clear regression is retained separately, not treated as final acceptance.
- The accepted candidate was validated before promotion. Previous production retained at `output/all-scenes-refresh/previous-production-acb802a2ea`.
- Live trusted-CA HTTPS: all **218** package files including manifest matched accepted bytes with no-store headers; all **22** scene directory routes returned the matching HTML. No certificate/trust changes, server restart or Three.js source changes were needed.
- Local reports: `output/all-scenes-refresh/{primary-copy-inputs,initial-browser-report,final-browser-report,delivery}.json`. Compact acceptance metadata: [all-scenes-refresh-2026-10-24.json](benchmarks/all-scenes-refresh-2026-10-24.json).

This is a local combined delivery and correctness check, not a new matched performance comparison, a completed universal optimization audit, a push/CI result or physical headset validation. Before any requested push, run the repository's required content-bound final gate on the final integrated sources.

## Latest scene follow-up

Arena VR-entry/spatial-info correction now serves application `d51271cfc9` with the same release WASM above and all22scenes retained. Five affected packaged cases pass; the55-case result above belongs to the preceding combined delivery. See [Neon Arena VR information](NEON_ARENA_VR_INFO_2026-10-24.md).
