# Handoff: new VR game using this Rust engine

User explicitly confirmed the target: **native Quest application — APK/OpenXR**, using our Rust engine. This is not another WebXR game. Start with a short portability/toolchain audit and the smallest runnable native-room/interaction prototype; the user can define the full game in the new chat. Preserve the existing browser engine while extracting/reusing suitable core code. Native-device/build acceptance must be evidenced, not inferred from a browser demo.

## Source and ownership

Repository KristekQQ/rust, canonical /Users/Honza/WebstormProjects/rust. Parent integration branch codex/renderer-integration-20261103 contains this chat's18renderer-history commits + current animation/XR work. Confirm final origin/main identity from the parent delivery. Canonical local main is old/dirty and must NOT be reset, cleaned, stashed or used as the new game's build root. Follow AGENTS.md, renderer-strategy, rust-webgpu-project and parallel-work. Create a separately owned worktree from verified origin/main and record it. One writer/file; don't run concurrent builds into another checkout's pkg/output/production or stop another server. Preserve all old worktrees.

## Actual technical state

Rust owns simulation, scene, collision/cutting, animation, render scheduling and GPU commands; JS SDK is thin. The project is currently a browser Rust/WASM engine. `src/lib.rs` gates `render` and `web` on wasm32; Cargo currently emits only a cdylib; wgpu uses webgpu/webgl features with default features off; no native OpenXR app host is present. A reusable Rust library/host split may require an rlib/workspace adjustment, which is new target work, not an already working native app. Native unit tests exercise reusable CPU code, not a native GPU executable. Also inspect SceneManager model GPU publication: queue/take paths use wasm32/test gating, so removing only the render-module cfg would still leave a native renderer without the normal glTF publication path. Reuse core scene/gameplay/math/assets and render principles; assess platform-specific modules/host and native GPU/runtime support honestly before claiming APK feasibility. Don't substitute Three/Unity/another renderer or rewrite all core systems by default.

Existing browser examples retain separate examples/<name>/main.js +scene.js and Rust gameplay/physics. For the confirmed native target inspect required renderer abstraction, runtime swapchain/image lifetime, input/tracking, Android toolchain/dependencies and safe first runnable milestone. Research current primary SDK/API docs in the new task. Begin with the smallest real scene/interaction, keeping the existing browser backend working; do not claim Quest acceptance from desktop/simulation.

## Performance and unresolved issues

Use wasm-render-performance and docs/performance/STATE.md; there is an existing deterministic benchmark runner and bounded whole-frame protocol. No need to invent a second harness. New work must preserve quality/gameplay and compare matched scenarios, backend and device. Raw CPU callback wall, GPU timing and presentation cadence are different metrics. Latest CPU animation allocation improvement is not proven universal FPS gain. Three common comparison has unequal culling/triangle work and no production/VR parity.

Ordinary Rome/Neon WebGL uses canvas-transfer. Shared-framebuffer scene FFR requires explicit query opt-in and is NOT physically accepted: Quest first-bind bug was corrected, but opaque completeness remains unresolved. Do not re-enable the prototype by default or hide errors. WebGPU uses existing owned-eye staging and final stereo submission; no owned-scene FFR. Historical pixel blockers remain independently open.

## Delivery expectations

Czech concise reports; English internal design/skills. State source/build/server/commit/push/CI/device acceptance separately. Respect configured model/default; no automatic Astra/High. New user-owned chat is not permission to merge future game work or replace the shared server. Full verify:push is required before any requested commit/push. Current parent server is a local Quest preview with its own recorded owner; IP must be freshly detected before linking/serving after a move.
