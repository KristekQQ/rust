# Independent review and root follow-up

One read-only Sol reviewer, no edits/builds/GPU execution. Bounded reviews covered diagnostic keys/pass barriers, actual Three context capture and final published-count reuse. Explorer audited the pinned actual reference separately; neither report is runtime acceptance.

Findings addressed:
- Common workload initially omitted from scalable load families: repaired, actual count asserted, rerun at64. Original failed attempt retained.
- Three common canvas `world` was not captured by GL timer injection keyed to `gpu-canvas`: renamed before context creation, explicit capture guard, fresh profiled rerun with valid async results. Profiler-off earlier pilots do not prove GPU timer support.
- `draw_count()` cannot blindly return B3 list length after failed update: renderer logs errors and continues after `active` may change. C bool invalidation precedes mutation, publication follows successful rebuild; original last_pose guard and old-scan fallback retained. Same-stamp early return after a prior failure preserves invalid state.
- Documentation must assign rejected mode2 exclusively to frozen D; final source/SDK accept0–1 only. Corrected docs after review.

Scoped source review of rejected C: no material correctness defect found. Materials/draw arrays are immutable; active mutates only at audited update site. Counts3→1→3/instance6→3 and cheap OPAQUE/MASK silhouette assertions are appropriate. Stored depth buffer is not independently read; injected internal malformed-pose errors are source-audited, not executed through a public fault hook.

Root executed strict WASM/type/release and both-backend maintained cache/motion tests.48clean paired runs completed with24 exact image pairs before Three/final gate. Real-scene depth-order pixels failed despite small synthetic tests passing; removed prototype instead of weakening references. Full final acceptance is recorded separately in VERIFICATION.json, not inferred from reviewer approval.

Final acceptance review caught an evidence blocker: FPS is lower for C in all3stress pairs on both backends; run ranges do not overlap. Ambient contention does not establish causality, but cannot justify retaining the regression. Root rejected C and restored B6 draw_count; no production speedup accepted. Also corrected triangle wording: native submits7.02%fewer triangles in common2048, without an independent culling-rate denominator. Final R checks are recorded separately.

Post-timing environment follow-up: pre-run AC70% became battery67% after timing; transition timestamp unobserved. Root corrected report/manifest and did not infer a causal renderer gain or blame the C regression on power. No accepted production optimization.

Final R gate completed after the review:428native/147browser PASS,38helper units; receipt and exact frozen-R runtime/SDK/pkg identity verified by root. Archive entries/hardlinks verified independently of review. No physical Quest evidence or accepted production speedup.
