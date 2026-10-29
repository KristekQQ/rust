# Current render followup — 2026-10-30

Six-batch series complete locally; [final report](docs/performance/render-followup/FINAL_REPORT.md), [current state](docs/performance/render-followup/STATE.md). No broad default FPS gain proved; GL Neon allocation benefit confirmed. Fresh functional gate passes; old independent image blockers remain, Quest unverified. No push/merge/deploy; production differs from local final. Stop series and read current state before further work.

# Historical nine-batch renderer cycle — 2026-10-29

Nine-batch evidence complete; **exact-image signoff blocked**, no publication. [Final report](docs/performance/next-render-cycle/FINAL_REPORT.md), [current state](docs/performance/next-render-cycle/STATE.md), [verification](docs/performance/next-render-cycle/batch-9/VERIFICATION.json). B8 release reused after identity checks;134 runs/193076 RAF rows,428 native/149 browser PASS. Static2048 stress improves, active/tail results mixed;39/40 native image pairs exact, candidate camera6pixels/max2 and historical B4 concern unresolved. Larger culling-off difference occurs identically in original/final; root cause unproven. Shared-GL XR remains opt-in. No physical Quest/CI/push/merge/deployment. Stop series; prioritize image correctness before unconditional release.

# Historical project map — 2026-10-19

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
- Model handles own Rust hierarchy/pose. Import never activates a camera; embedded cameras require explicit SDK commands. Eligible rigid and supported skin draws use GPU transforms/palettes; morph and unsupported paths retain CPU deformation, with exact CPU query/cutting behavior. Shader PBR supports core maps/alpha modes. Extension/IBL limits are documented in `docs/GLTF.md`.
- Approved cathedral view `[6,2.5,2]` toward `[-12,8,0]`, first light `[-19,12,0.2]`. Keep this placement unless asked to change it. 72,513 triangles; glass faces omitted to expose window apertures. Stone metallic=0/roughness=.95; asset CC BY-NC. Light is point/perspective, not directional sun.
- God-ray example density=.055, exposure=40, strength=.8; engine defaults .035/2.5. Helpers stay outside reflection/postprocess depth. Server sends no-store to avoid mismatched cached modules.
- Engine review milestone `c1b3ef00`: 59 native tests; detailed dated evidence is in `docs/CODE_REVIEW.md`. New harness entry points and runtime dispose/reinit contracts are in `docs/DEVELOPMENT.md` and README. Use a current verification report rather than historical counts as proof for changes.

## Historical Quest / neon decisions — 2026-10-19

- Stable XR baseline `b07ae09b`: user confirmed SSR artifacts fixed on Quest 3 / Browser 152.1 with owned per-eye color/depth staging and one final stereo submit. Keep that presentation boundary; Browser 149.1's zero-color-metadata normalization is an earlier, separate workaround. New frame-pipeline code has desktop GPU/WebGL and simulated XR checks; actual headset validation is pending. No headset performance or WebGPU/WebGL color-parity measurement is established. Route XR support through [quest-webgpu-xr](.agents/skills/quest-webgpu-xr/SKILL.md); details: [WEBXR.md](docs/WEBXR.md).
- Generic frame planning uses single-writer logical resources and exact-descriptor transient aliasing. Rust encodes actual passes; JS exposes graph/profile diagnostics. See [frame-pipeline](.agents/skills/frame-pipeline/SKILL.md) for extension and invalidation rules. GPU physics migration and fixed-step simulation changes are not part of this implementation.
- `examples/neon-arena` offers glass-like colored facets with opaque neon seams (default) or the original open wire style before VR entry. `FRUIT_SCALE=.105` halves gameplay fruit, independently of gallery icons. Glass cuts retain colored translucent caps; open style uses black caps + `.25` cutout. Both retain closed CPU solids. Restart is at Z=-1.45 m; glass uses stable high-contrast facet masks (base .06, variation 1, per-variant coverage .35–1). Fifteen diverse fill masks per kind are selected and ordered before VR, share topology and cycle without repeats across rounds. See the project skill for presentation rules.

- WebGPU XR color contract: all eye output uses owned sRGB render targets and compatible UNORM source views for byte-preserving final stereo presentation. Browser UNORM views cannot be reinterpreted as sRGB; direct linear output was darker. Quantitative gray tests pass with effects on/off; actual Quest color parity/performance still needs user validation. See docs/FRAME_PIPELINE.md.

- Final composition/overlay fusion (2026-10-22): the last postprocess draw shares one physical pass with alpha/rings/glow/helpers; no overlays means no depth attachment. Keep WebGPU read-only sampled depth and WebGL separate packed depth. Logical graph counts differ from physical passes; see [FRAME_PIPELINE.md](docs/FRAME_PIPELINE.md). No resolution/shader reduction and no established Quest FPS gain; use controlled same-package headset measurements.

- Renderer performance workflow (2026-10-25): use [wasm-render-performance](.agents/skills/wasm-render-performance/SKILL.md), [protocol](docs/RENDER_PERFORMANCE.md) and [current state](docs/performance/STATE.md). Refresh-capped FPS is a control, not a throughput result. Measured adaptive candidate6f9a3bba replaces quadratic rigid-root overlap scans; original c4301d81 remains frozen. Paired6144-object desktop CPU improves about48% on both backends; see state/report for strict capacity, remaining Three CPU gap, rejected regressions and unresolved GPU-page churn. No Quest performance claim.

- Actual Rome progress (2026-10-27): [STATE](docs/performance/STATE.md), [spatial batch report](docs/performance/2026-10-27-spatial-batches/REPORT.md). Local appba920dd931/WASMc7091506 on home8443, sharedtransport12350 restored after old process failed; primaryNeon bytes unchanged. Generic depth2 grouping/active lists/root proof/exact16B instances/unused tangent omission:512calls308→174, instance buffers−85%, all6stresspixels exact. ContendedM1 selfGL17.281→17.721(one adverse pair),WG1024 24.233→24.898(all3improve); separate nativeGL18.505vsThree26.179/108calls remains unmatched AA/PBR diagnostic.17+11minified cases inclcuts/simXR pass. Updated /examples/rim-compare/; physicalQuest/push/CI unverified. Next candidates remain unimplemented; recheck runtime/branch facts.

- Three implementation transfer (2026-10-25): inspect the pinned local reference implementation, port one compatible technique, retain independent correctness and paired evidence; do not substitute our old-vs-new speedup for a direct Three comparison. Direct YXZ affine composition plus reuse of published glTF root matrices is retained in292f27c1 release: three6144-object desktop pairs give CPU median ratios0.9551 WebGPU/0.9484 WebGL vs6f9, all6 captures exact; GL has one0.1ms adverse pair. See [transfer journal](docs/performance/THREE_TRANSFER_2026-10-25.md) and [current state](docs/performance/STATE.md); this is not a whole-game/Quest or language verdict.

- Rim shadow policy (2026-10-26): generic independent static/dynamic maps already cache by caster revision/light matrix. Actual Rim now uses Rust-owned pooled NPC ground circles; no new FPS gain is claimed. Both-backend/stereo browser and release delivery evidence: [follow-up report](docs/performance/2026-10-26-rim-shadows/REPORT.md); physical Quest remains unverified.
