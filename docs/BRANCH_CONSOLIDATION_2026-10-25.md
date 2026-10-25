# Branch consolidation — 2026-10-25

Audit baseline: origin/main fb986b482; 29 local branches (main plus 28 work branches), five remote branches including main. Twenty-two work-branch tips are already ancestors of origin/main. Source worktrees and their uncommitted snapshots are preserved.

## Current completed work

- codex/core-render-optimization, 0a854977d: includes 088fdb344 and the renderer, cutting, diagnostics, refresh and 22-example delivery follow-ups.
- codex/parallel-work-guidance, 80241d683: four documentation files. Merged with the current core branch without conflicts.

## Superseded experiment histories

The following four divergent 2025 branches are retained through an explicit `ours` history merge. Their source trees are superseded by the current engine; this merge retains current implementation and records their ancestry rather than replaying the historical architecture.

- codex/add-scene-manager-and-light-handling: 47c2bea96 (June 2025)
- codex/add-scene-manager-in-webgpu-wasm-bindgen: 3cdde7948 (July 2025)
- codex/create-scene-manager-with-default-values: 813e3dc2b (June 2025)
- codex/refactor-rust-+-wasm-bindgen-+-wgpu-project: cef54db42 (June 2025)

Independent read-only review found no concrete missing public capability or applicable fix among these experiments. Their small Mesh/Light/Grid and Node/light buffer demos are superseded by stable-ID object/model/light scene commands and current instance batches. SceneUniforms uses aligned POD layout with explicit layout tests; upload and buffer allocation paths are implemented by the current renderer. The audit was targeted rather than an exhaustive replay of every old hunk.

## Evidence boundaries

The current runtime passed the release verify:push gate: 400 native tests passed, two ignored; strict native/WASM Clippy, SDK, harness, XR platform and types passed; 128/128 registered browser cases passed on their supported WebGPU/WebGL backends. Consolidation changes documentation and commit ancestry only; matching content-bound verification receipts are reused. WASM SHA-256 remains c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a.

Primary checkout, staging, historical worktrees and the accepted HTTPS8443 production package remain preserved. Physical Quest acceptance and remote CI have separate evidence. Branch deletion is outside this consolidation.
