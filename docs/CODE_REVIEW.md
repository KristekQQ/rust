# Code review · 2026-10-13

Reviewed CPU scene state, visibility/culling, glTF decoding and animation, renderer/pass caches, GPU layouts and WGSL, browser initialization/output, JavaScript SDK and integration tests. One independent reviewer checked the CPU paths and then the integrated repairs; a separate implementer repaired and tested the bounded SDK changes. Existing unrelated staged files were preserved.

## Repaired findings

| Priority | Trigger and failure | Repair and regression evidence |
|---|---|---|
| P1 | Many nodes reference one large skin; source limits do not bound copied per-node joint matrices. Import could allocate tens of GiB. Shared morph weights have the same expansion risk. | Validate a 64 MiB expanded pose budget before allocation, including per-node skin matrices and morph weights. Shared-skin and shared-morph hostile assets return errors before expansion. |
| P1 | Many channels/clips reference one sampler. The accessor budget counts source once, while owned animation arrays copy it repeatedly. | Shared 64 MiB budget across expanded channels, with checked arithmetic and output-count validation before collection. A small sampler reused by 1,100 channels is rejected. |
| P1 | `TEXCOORD_4294967295` overflows `usize + 1` on wasm32 before the limit check. | Check the unsigned set index before conversion/addition. Regression covers 31, 32 and u32::MAX. |
| P1 | Concurrent/repeated SDK initialization, including a second cache-busted module import, starts additional WASM runtimes, devices and frame loops. | Share a page-local initialization promise. Reuse matching canvas/backend, reject conflicting ownership, allow retry after failure. Node tests and real scene-SDK tests cover concurrency and a second module import. Rust also rejects concurrent raw `start()` calls. |
| P2 | Clearing a scene invalidates a pending model result but leaves fetch/stream reads running. | Abort every active import and cancel its readers; release tracking on completion/failure. Tests cover root and external fetches, hanging custom streams, completion cleanup, retry and transport errors. |
| P2 | Negative scale reverses triangle winding, while legacy instances share one fixed culling pipeline. Back surfaces are rendered instead of outward surfaces. | Split winding groups while keeping instancing and independent pass caches. Select the opposite pipeline for mirrored built-ins and legacy imports; render both groups in depth passes and count additional draws. Nonmirrored batches avoid temporary partition arrays. Pixel tests check cube face colors, sphere symmetry and mixed-group draw counts on both backends. Effects tests exercise a mirrored shadow caster. |
| P2 | Oversized canvas dimensions reach GPU allocation; negative/fractional JS arguments silently coerce to u32. The camera can retain the requested aspect instead of the actual output aspect. | Reject invalid SDK dimensions, clamp initial/resize dimensions to device limits, propagate the actual aspect. Both output tests exercise u32::MAX × 1 and negative dimensions. |
| P2 | Generic GPU byte conversion accepts any `Copy` type, including structures with uninitialized padding. | Replace unsafe reinterpretation with `bytemuck::cast_slice<T: Pod>` and derive checked Pod layouts for all uploaded GPU structures. Native/wasm compilation verifies padding-free layouts. |
| P2 | Detached/missing canvas or malformed native pose arrays trigger panic/indexing errors. | Validate attached SDK canvas, return Rust startup errors, validate pose array lengths and skin inverse-bind counts. Native and SDK regressions cover these contracts. |

Updated Rust idioms for the pinned toolchain. Strict Clippy passes without global suppression; narrow argument-count annotations cover explicit GPU/FFI signatures. Test assertions were retained and expanded.

## Verification

| Check | Result |
|---|---|
| `cargo test` | 59 passed, zero ignored or failed |
| `npm run test:sdk` | 9 passed, zero skipped or failed |
| `npm run typecheck` | PASS |
| `cargo fmt --check`, `git diff --check` | PASS |
| Native `cargo clippy --all-targets -- -D warnings` | PASS |
| wasm32 `cargo clippy --target wasm32-unknown-unknown -- -D warnings` with required web-sys cfg | PASS |
| Release WASM build | PASS; browser uses freshly generated ignored `pkg/` |
| `scene-sdk.html` | PASS on WebGPU and WebGL |
| `gltf-sdk.html` | PASS on WebGPU and WebGL |
| `gltf-full.html` | PASS on WebGPU and WebGL |
| `render-mirrored.html` | PASS on WebGPU and WebGL; actual canvas pixels, not just metadata |
| `render-target.html` verify control | PASS on WebGPU and WebGL |
| `render-effects.html` verify control | PASS on WebGPU and WebGL |
| `examples/postprocessing/` verify control | PASS on WebGPU and WebGL |

Manual visual checks additionally inspected front/back sphere lighting, nonuniform transforms, morph pose changes, the Fox in two distinct animation poses, the textured PBR helmet, and the spatial 10,000-cube scene. The latter retained one instanced draw for positive scales; culling changes visible counts from 820 to 10,000 at the initial camera. These are scoped observations, not a hardware benchmark.

## Boundaries

- Browser tests cover this Mac/browser's WebGPU and WebGL implementations; they do not establish VR/device compatibility, sustained performance or absence of every leak.
- Current glTF support remains the documented core subset. Optional extensions such as KHR_materials_unlit produce a fallback warning; passing tests do not mean support for every extension, PBR environment lighting or animation blending.
- The SDK retains one canvas/backend per page. Switching ownership requires a page reload; explicit engine disposal is not part of the current API.
- Memory budgets bound individual expanded assets/poses, not total application memory across arbitrary concurrent models.
