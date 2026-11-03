# Renderer integration — 2026-11-03

User-authorized main integration of this chat's renderer work, followed by a separate new native-VR game task. Integrates18existing commits after mainb7de3be56 throughc20f03fed, plus owned animation workspace and XR/diagnostics/safe-default changes. Primary dirty main, previous worktrees and currently served Quest package are preserved; no server replacement is part of this merge.

## Accepted scope

- Existing nine-batch/followup/model-publication history remains intact, including unfavorable results and independent image blockers.
- Lazy transactional animation workspace with retained sampling/pose/traversal buffers, bound/root validation before publication, original test-only reference, bounded retention/clone behavior and regression fixtures. Corrected comment: previously animated paused/stopped instances retain bounded scratch; only never-animated instances cost an optional pointer.
- Optional XR layer preference and explicit scene-texture foveation prototype with bounded resource tracking/rollback. **Canvas-transfer remains default** for ordinary Rome/Neon WebGL buttons. Shared-scene FFR requires explicit xrPresentation=shared-framebuffer. Physical opaque-framebuffer completeness remains unresolved; no hardware FFR success claimed.
- Cold first-bind ownership validation preserves distinct GL state and negative inputs; known first-bind and subsequent physical completeness failures stay separately documented. Added numeric status diagnosis, not a speculative removal of safety guards.
- Actual example-button tests ensure a missing desktop FFR capability cannot mask unsafe default selection. Cutting/gameplay, stereo and other scene verification remain required by the final gate.
- Existing benchmark runner is reused. Animation, A/A repeatability and pinned Three r170 reports retain exact build/workload scopes. Static limited-profile Three results are not a production/Quest ranking. Historical exact-image blockers are not erased by functional gate passes.

## Evidence / validation

[Durable source evidence archives](evidence/MANIFEST.json) contain their original raw records/manifests and source patches without recursively including earlier archives. They do not imply that current final source is byte-identical to every historical candidate. Earlier report absolute local roots are historical provenance; archives are now committed here for portability.

Independent Lupa — GPT-6.1 Sol Medium — reviewed merged source scope and found no integration blocker. This was a source review, not test/device verification. Parent final verification uses the existing **one** `VERIFY_WASM_PROFILE=release npm run verify:push` gate. Final results and hashes are recorded below when the gate completes. No historical test count is relabelled fresh.

## Open boundaries

Native Culling/shadow/camera tiny-pixel cases from historical reports remain open. Shared-GL FFR is explicitly experimental and failed physical Quest completeness; ordinary recovered entry also awaits user confirmation after its latest package. No new on-headset FPS/comfort or native-APK acceptance.

The render module is currently wasm32-gated and Cargo's wgpu configuration targets browser backends. The user confirmed native Quest APK/OpenXR. It needs a real platform/runtime host and renderer portability work; native Rust unit tests do not mean an existing native VR executable. See [new-task handoff](NATIVE_VR_HANDOFF.md).

## Verification transport metadata correction

Two full test attempts passed441Rust/177browser scenarios. The first correctly lost its receipt when system Chrome changed. The second used a stable owned Chrome.80 and recorded PASS, but fresh Codex calls could not reuse it because the key included three execution-broker identifiers not consumed by the runner. Independent keys matched across fresh host/sandbox calls when those exact three names were absent. The cache now excludes only BROWSER_USE_ACCESS_EXECUTOR_ID, BROWSER_USE_ACCESS_PIPE_PATH and BROWSER_USE_ACCESS_VERIFYING_IDENTITY; every other browser flag, unknown future key, source/type/tool/browser identity protection remains. Regression tests cover addition/rotation/removal and retained controls. Old receipts are not rewritten: a fresh full gate is required after this script/test change. Final outcome is in VERIFICATION.json.

## Final verification PASS

After the narrow cache correction, a completely fresh final gate passed: **441Rust tests/3ignored,177browser scenarios (68WebGPU/109WebGL2),32SDK tests,72XR platform tests,19Node harness tests and13Python tests**, native/WASM checks/Clippy/types and source dump. Actual browser Chrome156.0.8078.80 was held in an owned snapshot. A subsequent fresh cache check also passed; receipts were not rewritten. [Fresh identities/counts](VERIFICATION.json).

The verification script/test fix is included in this integration and independently reviewed. Previous attempts and failure reasons remain in local output/integration/attempt-{1,2}. Neither repeated passing functional tests nor a portable receipt resolves historical image blockers or physical Quest FFR.
