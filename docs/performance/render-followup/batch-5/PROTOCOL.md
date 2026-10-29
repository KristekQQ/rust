# Followup5: independent audit and bounded experiment

Base503502250 in exclusively owned /private/tmp/rust-render-followup-2, codex/render-followup-5. Original release is frozen output/followup-4/final (7829658f WASM/a4c47fa9 source). No push/merge/deploy/shared process changes. Only batch5; no batch6. Defaults remain original unless independent correctness and repeated whole-frame results justify change.

## Whole-frame hypotheses, before new timing

| Priority / observation | Mechanism | Disproof | Small experiment | Correctness risk / possible scope |
|---|---|---|---|---|
|1. F4 moving-camera/sparse groups scope approximately6ms at2048; source cursor merge revisits every root for each next ordinal, even empty/culled lists.|Transpose contiguous eligible model-major records by stable ordinal buckets; keep exact same ordinal/model order and existing compatibility/root barriers. Improve locality and avoid empty-root probes, no new cache.|Same group time or worse full-frame cadence/tails; added scatter memory cost dominates.|Explicit mode3 bounded stable counting transpose, original mode0 reference; fresh profile, strict order/pixels, three alternating pairs.|No visibility/math/order change allowed; test gaps, barriers,1024cap, negative/full transforms, alpha/skin, passes/XR/cuts. Mostly dynamic CPU-bound preparation; static cache hits should be unchanged.|
|2. B7 main GPU spans dominate static stress; fragment simplification was particularly effective onGL.|PBR/shadow work, throughput or overdraw, not language alone.|Fresh low-resolution/cheap shader controls insensitive; GPU-query absence limits attribution.|Separate diagnostic production/cheap/noFXAA/low-resolution anchors; do not deploy quality reduction.|Shader arithmetic changes can break strict pixels. Do not port Three Fresnel approximation or remove color varying.|
|3. Repeated normal normalization exists twice in source.|Compiler might not CSE derivative input and initial normal.|Generated shader/compiler already shares work or no full-frame gain.|Source comparison only initially, not automatic shader rewrite.|Exact math required; source duplication is not evidence of duplicate GPU instructions.|
|4. Per-pass light Vec and pre-cache model Vec allocations exist.|CPU allocation/iteration even on cache hit.|Previously tiny scene.prepare/gltf.queue scopes remain tiny.|Read-only call graph and new full-frame profiles; defer if negligible.|Low risk but small likely scope; do not optimize tiny work while GPU-bound.|
|5. wgpu draw_prepared sets per-draw material/vertex/index state; Three state cache differs.|Redundant API/state commands or validation cost.|Bindings mostly distinct, encoding scopes tiny, GPU remains limiting.|Actual command/state counters and pinned source audit.|Handle identity alone insufficient; any later suppression must respect pass/layout/content. No new interop in this batch.|

## Protocol

Reuse fixed F4 loads: camera/sparse/static stress2048, all-moving/clustered1024, ordinary gameplay and Neon controls.1280x720 DPR1 seed1337; same quality/transforms/updates. Fresh baseline-only fixed-load1s/3s confirmations bind the current shared harness. Main8s warm/25s sample, three AB/BA/AB pairs, profiler off. Pilot1s/5s explicitly preliminary. Use bounded2048 when60Hz-capped; no busy wait, quality reduction or artificial Three slowdown. Rejected experiments stay recorded.

Profile separately (2s/8s with per-frame context; asynchronous valid queries when available, otherwise null), preserve raw callbacks and p95/p99/max. Whole-API observer is separate from clean runs. Actual counts/triangle work and upload/resource changes recorded, not normalized into supposedly identical cross-engine geometry.

Three uses vendored r170 and existing per-part InstancedMesh. A: production profiles are graphically unmatched. B: common base-color profile shares GLBs/camera/Idle@0/viewport but culling/submitted triangles may differ; AA/lights/shadows/post absent only in this explicit diagnostic. C: original/candidate native exact settings and strict pixels. No fresh Quest claim; canvas-transfer default and WebGPU owned-eye/stereo remain unchanged.

One meaningful algorithm change only; no depth reorder, published-count reuse, interpolated-color removal, shader approximation or new batching rules. Counts<=65536 ordinals, temporary records<=4MiB requested storage; original cursor fallback above limits. Scratch retained per renderer and released by existing clear lifecycle; no GPU resources added.

Before the Three phase starts, include the production paired reference on BOTH native backends (initial coordinator planned WebGL only). Same already-frozen2048 configs,3pairs8s/25s, no harness/runtime change. This avoids an unpaired WebGPU production comparison; initial coordinator and exact executed ledger are preserved.
