# Batch7 fixed protocol

Only batch7 authorized. Isolated `codex/next-render-cycle-7`, parent `c5cd402c23171cf205e72f34f4213f7b869f5dc8`. No shared server/release changes. Read batches1–6 before starting; retain B4 moving-WebGPU image limitation.

## Builds and controls

B6 frozen first (`output/next-render-cycle-7/baseline`). Instrumented unoptimized diagnostic release D frozen separately (`diagnostic`). Production mode0 preserves authored shader/rendering; verify exact same-renderer images against B6. Diagnostic modes are explicit, bounded, default off:1 base-color fragment preserving position, alpha cut, clip/depth/blend;2 only permission-qualified opaque group depth order with barriers. Shadow and FXAA switches are independent harness controls. Reduced shader/resolution/effects are diagnostic, never accepted quality-preserving optimization.

Primary diagnostic matrix D: Rome stress2048/WebGPU and1024/WebGL, unchanged seed1337/camera/assets/pose. Each case8s warmup/25s sample, one run per axis (exploratory, not three independent A/B repetitions): production1280×720;640×360;1920×1080;cheap fragment1280×720;shadows off;FXAA off;safe depth order;production1280×720 repeat. Backends alternate by axis. A single main variable per row; repeated anchors bound drift. Preserve raw series, errors and all unsuccessful attempts. No own builds/hash sweeps or second GPU runner during timing. Existing foreign GPU work remains untouched and limits attribution.

CPU preparation, glTF updates, encode/color, overlay selection, submit and output-present wall scopes are separate and may nest. CPU histories are sparse observed updates, not full-frame invocation distributions. WebGPU native asynchronous pass queries and WebGL EXT application-command interval have different boundaries. The latter may include CPU submission gaps/driver waits; neither includes compositor time, neither is pure device utilization. Unavailable/invalid stays null. Never add CPU and GPU times. Collect whole-frame p95/p99/max from the full raw25s RAF series.

## Three control

Pinned local Three r170, original bundled app, existing per-part InstancedMesh stress implementation. First verify exact shared city/drone GLBs, Idle@0, scale, gate camera, count, viewport and actual render settings. Common profile is intentionally limited: base-color, no lights/shadows/AA/fog/tone mapping/PP; explicitly not production PBR equivalence. Capture actual submitted commands/triangles and GPU queries, review paired pixels. Production original app/settings reported separately; no artificial slowing or clone substitution. Profile pilots1s/3s and initial2s/8s runs are marked pilots. Baseline-only calibrate actual count before selecting stress. Common comparison:8s/25s,3AB/BA/AB pairs, profiler off for throughput; separate profiled diagnostics. Cross-engine pixels diagnostic only, not same-renderer correctness.

## Candidate decision and acceptance

Select exactly one bounded production change from observations; record rejected alternatives. Freeze candidate C. Compare D vs C at identical quality/inputs with8s/25s,3AB/BA/AB pairs, profiler explicitly off, including ordinary Rome, backend stress and moving control plus Neon regression. Exact pixel gates and maintained cutting/lifecycle/asymmetric-XR acceptance remain independent. Separate profiled followups quantify affected CPU scopes; no static gain generalized to gameplay/Quest. If an important workload regresses without safe bounded selection, repair or revert.

Final stable content: required release `npm run verify:push` once, identity/served-driver audit, report/manifest/experiment/state update, owned local checkpoint. No batch8, push, merge or deployment.

## Predeclared restoration extension (after primary results, before restoration runs)

C reduces callback wall time, but stress FPS decreases in all three pairs on both backends, with non-overlapping per-run ranges. Foreign GPU contention prevents clean attribution; it does not justify accepting the regression. Reject C, restore the exact B6 draw_count scan and remove its validity flag. Keep default-off diagnostics and profiling scopes. Retain C and all adverse results unchanged. Three timing rows measured with C must be labeled rejected experimental-runtime results, not final shipping-runtime performance.

Freeze restored release R. Validate D/R at the already-calibrated stress loads2048/WG and1024/GL:8s warmup/25s sample,3AB/BA/AB pairs per backend, profiling explicitly off, unchanged quality, strict zero-difference images. No own GPU work/build/hash sweep during samples. This is a restoration check, not a new claimed optimization. Source equality of restored draw_count to B6 is checked independently. The previous candidate gate is superseded; changed source requires a fresh final gate on R. No additional optimization or batch8 starts.
