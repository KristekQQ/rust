# Next renderer cycle

Batches 1 and 2 have been separately authorized. Batch2 is complete locally; batches3–9 require separate requests.
Base source is the live GitHub main observed at start, not the historical commit in the task description. See MANIFEST.json and STATE.md.

## Nine batches

1. Verified baseline, measurement coverage and optimization plan — complete; report, verified evidence and local checkpoint.
2. Cache prepared batches for immutable scenes — complete locally; see [report](batch-2/REPORT.md) and [design](batch-2/DESIGN.md).
3. Fine-grained invalidation during motion and VR — not started.
4. Bounded GPU resource pools — not started.
5. Omit exactly white vertex colors losslessly — not started.
6. Lossless 16-bit indices where representable — not started.
7. Isolate GPU costs and deliver one demonstrated optimization stage — not started.
8. WebGL XR presentation between contexts — not started.
9. Integration, regressions, results and skill update — not started.

## Invariants and decisions

One significant hypothesis/change, correctness checks, matched measurements, then accept/revise/revert. Never reduce final quality, resolution, content, effects or gameplay to report speed. Use our Rust/WASM engine; another renderer is not a substitute. Preserve WebGPU, WebGL2, WebXR, cutting and independent lifecycle. Reuse existing revisions, pools, profiler and benchmark infrastructure. Consult the historical experiment journals before repeating completed/rejected work; a new attempt needs a new hypothesis. Preserve adverse runs and failures. Never rewrite reference pixels or assertions merely for a PASS. Repair or revert regressions; an unsuccessful experiment is not a speedup. Finish each batch with a report, local owned checkpoint and STATE.md update; do not begin the next batch automatically. No push, main merge, deployment, shared server restart or termination of foreign processes is authorized here.

## Baseline protocol

Reuse [the maintained protocol](../../RENDER_PERFORMANCE.md), [performance skill](../../../.agents/skills/wasm-render-performance/SKILL.md), render-performance.mjs, freeze-render-build.mjs and benchmark-rim-cuts.mjs. The checkout is isolated; runtime source/WASM/SDK are frozen before measurement. Any missing controls extend the existing harness only. Record frozen runtime and common harness identities separately. An earlier verified release may be reused only after current source/build/configuration/hash verification; copying a build is not a new compilation.

Primary desktop series: 1280x720, DPR1; 8 seconds warmup and 25 seconds sample; 3 independent repetitions. Alternate backend order between repetitions and use the same frozen inputs, seed, camera functions and load per case. Future A/B: three AB/BA/AB pairs with the same 8s/25s protocol, baseline-calibrated loads, and explicit profiling off. Adjacent frames are not independent experiment repetitions. No candidate optimization or speedup claim exists in batch 1.

Cases: A frozen Rome/fixed camera; B same Rome/deterministic camera-only motion; C native Rome gameplay plus a separately labelled deterministic actual-drone clustering/visibility case; D verified mesh spawn/cut/despawn plus separate real cut-worker replay; E authored neon arena. Keep ordinary cases and add actual Rome model-load stress. Calibrate stress on the baseline only with a bounded sequence (128, 512, 1024 additional models, maximum 1024); stop after a completed clearly over-budget probe. Freeze selected backend configurations before primary runs. Do not change resolution, use busy-wait, or synthesize CPU delay to cross 60 Hz. Pilot runs use 1s warmup/2.5s sample and are never primary evidence.

Clean primary runs assert engine profiler disabled and no GL timer queries. Separate diagnostic runs enable existing native scopes/asynchronous GPU timers and separate cold/warm API counters; their overhead is excluded from throughput. Never interpret queue.submit wall time as GPU time or add CPU/GPU scopes. RAF callback wall time excludes message/timer handlers and includes waits/driver effects. Retain full raw frame series (p95/p99/max), cumulative counter boundaries, visible work and requested-vs-submitted distinction. Missing metrics stay null with a reason, including worker-internal compute, unavailable GPU timing and uninstrumented scopes. Cut-handler, native prepare/installation and publication timing are reported separately from RAF.

Environment snapshots include hardware/browser/toolchain, power and foreign GPU processes. Do not terminate foreign work. Contended desktop measurements cannot prove small gains, isolated GPU capacity or physical Quest behavior. Capture pixels after timing. Use maintained independent browser correctness scenarios, including cutting and XR lifecycle; they are simulation evidence, not headset acceptance.

## Storage and completion

Keep PLAN/STATE/EXPERIMENTS/MANIFEST compact and versioned here. Large raw data, exact commands, request manifests and captures live in the owned ignored output directory and a compressed evidence archive linked from the final report. Do not copy historical reports into project memory. Local checkpoint includes only owned changes and the required source dump; run the project final gate once before that commit. Batch 1 ends after evidence, report and checkpoint; batch 2 remains blocked on a separate user instruction.

## Existing groundwork; do not repeat it blindly

The accepted spatial-batching and Three-transfer reports remain linked from [the prior state index](../STATE.md). Current source already has opaque reorder permission, depth-bounded grouping, pose-stamped bounds/active lists, exact shared-basis instance records, material-used rigid layouts, retained per-view instance streams, scene/view revision caches, cached static shadows and light-class shader selection. The depth4/107-call policy was rejected for worse WebGL throughput. These are baseline features, not improvements from this cycle.

Source audit found `prepare_draws` still reconstructs prepared groups (`src/render/gltf_gpu.rs:1574`), while existing streams/pages grow/reuse storage (`:1793`, `:1878`). Future batches2–4 must distinguish persistent prepared-group retention from those existing caches and pools. Native `gltf.queue` combines group preparation and packing; `scene.prepare` is broader. Separate durations for grouping versus instance packing are currently unavailable, so do not attribute all queue time to either substage. Warm glTF event counters and whole-API diagnostic windows have different coverage. Exactly-white attribute omission and legal u16 indices remain future work; neither is implemented in batch1. WebGL XR context-copy has simulator correctness coverage but no new physical Quest timing in this cycle.
