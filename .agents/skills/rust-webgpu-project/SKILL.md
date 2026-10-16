---
name: rust-webgpu-project
description: Develop or diagnose this repository's Rust/WASM browser 3D engine, including WGSL rendering, its thin JS SDK, standalone examples and renderer verification.
---

# Rust browser engine workflow

This skill belongs to this repository, not a personal skill installation. Keep project-specific skills under `.agents/skills/` with portable links. The repository root is three directories above this file's directory. Follow its [AGENTS.md](../../../AGENTS.md); use the compact [project map](../../../MEMORY.md) for orientation, not its historical archive.

Locate the affected path before editing: JS SDK → WASM export → CPU scene/runtime → renderer/pass/output. Runtime examples own their `main.js` bootstrap and `scene.js`. Keep one owner per file; agree shared interfaces before parallel work.

Use the [verification matrix](../../../docs/DEVELOPMENT.md). During iteration run targeted checks and reuse matching evidence. Native tests do not compile the wasm32 renderer; browser acceptance must use fresh matching `pkg/` and the actual Rust page. A build, DOM load, `cube.html` or separate JS graphics demo is not renderer acceptance.

Before a requested commit/push run `npm run verify:push` once: full native/WASM/SDK/harness/types checks, every registered browser scenario on both WebGPU and WebGL, then source dump. Include `all_sources.txt` in the commit. Do not substitute cutting-only checks for the full gate. Browser coverage includes direct render-target, shadow/reflection/cache, postprocessing, mirrored-transform, glTF animation/PBR and culling/pixel checks as well as scene/SDK/lifecycle/examples. A matching content-bound PASS receipt allows the pre-push hook to reuse tests; changed inputs/build options invalidate it. Preserve unrelated staging. A successful gate is not permission to commit/push, and desktop browser checks do not verify Quest/WebXR or software-WebGL CI.

Read [glTF](../../../docs/GLTF.md), [render passes](../../../docs/RENDER_PASSES.md) or [postprocessing](../../../docs/POSTPROCESSING.md) only for the affected area. Report concrete changes, actual verification and material limits concisely in Czech.

## Cutting compute experiment: prefer CPU

Decision recorded 2026-10-16: for the current apple/triangle-mesh cutting workload, local end-to-end tests found optimized Rust/WASM CPU cutting faster than both hybrid and GPU-resident WebGPU compute. Keep `auto` on CPU; treat `gpu` and `gpu-resident` as explicit experimental/comparison paths, not recommended acceleration. Do not repeat this migration as an assumed optimization; revisit when the user requests it, the workload changes, or new controlled measurements justify it. This is not a verdict against WebGPU rendering or compute for other workloads. Timings include allocation/queue/readback costs, varied substantially, and Quest 3 has not been measured; control power-saving/frame-rate limits in future benchmarks. Before cutting work read [the example decision](../../../examples/cutting/README.md); detailed limits/evidence are in [CUTTING.md](../../../docs/CUTTING.md).
