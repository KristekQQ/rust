# Integration inspection

Initial evidence checkpoint9986cd4f9/B8 runtime → final code08c2d212a. No new renderer edits in F6. Read-only independent reviewer inspected defaults, content epochs, plan/pool lifetime and current test registration; found no actionable integration defect. Inspection is not executed verification.

Accepted production: GL bitmap info-panel live-buffer capacity reuse only. Immutable text and WG keep original path. No generic global pool added.
Retained explicit controls: original/immutable-guard overlay selection, original/reused bitmap buffers, union/coalesced/full instance upload, mode3 exact-order ordinal transpose. Original overlay selection, union uploads and mode0 remain defaults. Rejected automatic promotions are absent from default dispatch; controls remain to reproduce negative experiments, not silently accepted optimizations. Shared-GL XR remains opt-in; canvas transfer and WG staging unchanged.

At src/render/gltf_gpu.rs, policy/mode changes invalidate input epochs, actual buffer writes advance content epochs, clear_instance_pages drops prepared/fine plans, pool and queue scratch. Layout compatibility and per-pass/eye ownership remain inherited from the nine-batch series. No vertex/index layout or shader-quality edits in followup6.

A/B/C are individually unresolved. D includes historical F2 clustered4pixels/max1 and new native comparisons. E means dynamic deterministic fixtures, not cross-engine Three pixels: panel-buffers, gltf-plan-motion/cache, gltf-upload-ranges, gltf-ordinal-motion/passes, rim-ordinal-xr and existing simulated-XR lifecycle fixtures. Full authored playing Rim/Neon does not have a common deterministic capture oracle; its non-strict images cannot supply that acceptance.

Cold/packaging boundary: both A/B roots use release WASM and their frozen source SDK. F6 does not build or deploy a new minified Quest package. The production LAN listener was inspected read-only and serves a different older WASM; loopback refuses connections because the listener is bound to the LAN address.
