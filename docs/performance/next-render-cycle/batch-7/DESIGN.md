# Batch7 diagnostic controls and candidate boundary

## Controls (not production optimizations)

Rust owns shader and permitted group-order variants. Final SDK accepts only modes0–1; rejected prototype2 and invalid modes reject. Frozen D separately retains mode2 for exact reproducibility. Changing a mode explicitly invalidates prepared/fine plans; pipeline/shader keys distinguish cheap fragments.0 is unchanged production shader/order.1 replaces only the material surface lighting result, retains base alpha test, clip plane and original fragment/depth/blend entrypoints; original shadow variants stay intact. The rejected mode2 prototype ordered only contiguous runs of existing groups whose every root qualified for explicit opaque reordering. It preserved transparent/shadow/clipped barriers and did not merge groups, but real crowded Rome pixels changed despite stable exact original controls (WebGL304 pixels/max102; WebGPU4533/max101). Small synthetic fixtures passed, demonstrating why real-scene pixel oracles matter. The prototype is removed from C; frozen D and the rejected patch preserve the experiment. No quality tolerance was weakened.

Existing profiler gains bounded scopes for overlay selection, submission and canvas presentation; optional timestamps on the already-existing offscreen present pass. No synchronous query reads, new render pass or forced GPU waits. Nested scopes are not additive. Matrix/harness errors remain in raw output.

## Investigated and rejected production candidate

Observed production overlay selection rescans every visible glTF model's immutable primitive array and active-node HashSet to answer whether any BLEND draw exists. B3 already publishes exact per-pose `active_draws[opaque,blend]`, yet `draw_count()` ignores those lists. Initial8s/25s anchors: select wall mean8.48ms/2048 roots WebGPU and4.58ms/1024 roots WebGL. GPU main work remains a separate limiter; eliminating CPU traversal need not increase FPS.

Small safe design: reuse the published list length after a successful pose classification, preserving `last_pose=None =>0`. Add explicit validity flag because failed skin/deform updates mutate `active` before returning an error, while the renderer logs and continues. Mark invalid immediately before active changes, mark valid only after list rebuild. Invalid falls back to the exact old scan. A later old-stamp early return must not reset invalid=true. Materials/draw membership are immutable after construction; clear/removal/dispose releases each model. No global dependency cache, new GPU resource, altered draw command, image, alpha, ordering or layout.

Read-only independent review confirmed all mutation/error/old-stamp paths; no material defect found. Correctness checks passed, but paired FPS failed acceptance. Matrix completed16 cases. Depth ordering rejected on image correctness, reduced shaders/effects/resolution rejected as final optimizations because they reduce quality. This published-count reuse is the selected single production candidate; paired acceptance failed. No command reduction is intended.

## Final disposition

C passes correctness, but stress FPS drops consistently across all3pairs on both backends. Reject the production change, remove active_draws_valid and restore B6 draw_count byte-for-byte. Diagnostic controls and scopes remain, default production shader/order untouched. Frozen C preserves the experiment; R is the restored final runtime. No accepted production FPS improvement.
