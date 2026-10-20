---
name: frame-pipeline
description: Extend or diagnose this engine's frame graph, transient texture/buffer allocation, postprocessing scheduling and CPU/GPU profiling. Use for resource lifetimes, pass dependencies and submission boundaries; use quest-webgpu-xr for XR API or attachment import support.
---

# Frame pipeline

Read [FRAME_PIPELINE.md](../../../docs/FRAME_PIPELINE.md) for the current source map,
buffer example and verification choices. Keep planning in Rust; the JS SDK queries
diagnostics and selects established effects. This is not an arbitrary WGSL editor.

- One logical handle has one writer. Use separate handles for versions, eyes and
  independently scheduled subresources; no in-place read/write pass is supported.
- Declare every read and write, including uniforms, depth and imported inputs.
  The encoder must follow `compiled.ordered_passes`; the graph does not encode it.
- Only identical transient descriptors with strictly disjoint inclusive lifetimes
  may alias. Persistent/external resources remain caller-owned. Keep allocations
  cached across stable frames; invalidate on descriptor or active-plan changes.
- Preserve the Quest owned per-eye stage and one final stereo presentation submit.
  Shared uniforms cannot be overwritten across batched eyes without separate ranges.
- Profiler is opt-in via `initEngine({profiling:true})`; neon arena uses `?profile=1`.
  Compare FPS with profiling disabled too; never treat sampled timings as free.
- Batch only main → postprocess → overlays within one view; flush before depth
  resolve, HUD, presentation or another eye. Keep shadow/reflection/environment
  submissions immediate because they overwrite shared uniforms. Postprocess
  submission counters advance only after the owning batch is submitted.
- `State::render_to` is shared by wgpu WebGPU and WebGL: its batching changes
  affect both. The 7 → 3 submit observation belongs to the tested WebGPU XR SSR
  path, including its final stereo presentation; do not claim that count for
  WebGL or that shared batching explains the backend FPS gap without measurements.
- Static scenes may cache immutable geometry, bindings, plans and eligible shadow
  or environment results using dependency invalidation. Static objects do not make
  the camera, XR eyes, SSR or final presentation static. Recompute view-dependent
  results as views change; choose render bundles or other reuse only where the
  backend supports it and measurements justify it. Keep current batch boundaries
  until shared uniform writes have been audited or given independent ranges.
- Keep scene finalization after controller mutations and before either eye. Adding
  a profiler scope must not change simulation stepping or force GPU readback waits.
- Vertex-color `setOpacity` defers translucent facets to the existing final overlay
  (before glow), while bright opaque facets remain in main/depth/SSR. Preserve
  `Instance.emission.zw` opacity/opaque-threshold, `surface_params.x` stable facet
  variation (default 0), `.y` optional sparse/dense coverage (default -1), and
  inheritance through both cut paths. Instance stride is
  176 bytes; attribute 13 at offset 160 must match all shared vertex layouts. See [POSTPROCESSING.md](../../../docs/POSTPROCESSING.md) for sorting limits;
  do not treat transparent fill as SSR input or add separate queue submissions.
- CPU triangle alpha draws use cached index subsets sharing the original vertex
  buffer. Preserve conservative common-channel filtering for gradient colors,
  threshold/source-identity invalidation and cleanup; resident/no-CPU paths fall
  back to the full index buffer. Compare translucency submitted/full index stats
  separately from GPU time or headset FPS.

Use the smallest affected native test while iterating; native success excludes
wasm renderer execution. For GPU changes follow [DEVELOPMENT.md](../../../docs/DEVELOPMENT.md)
and the affected browser scenarios. Timestamp absence is valid fallback; distinguish
sampled GPU duration, CPU encoding duration and XR callback cadence. Do not infer
Quest performance from desktop pixels or historical headset acceptance.
