# Renderer experiment journal — 2026-10-25

## Frozen inputs and evidence boundaries

Owned branch `codex/wasm-render-performance`, base `64bfda680483e71f6aeb6bf902e63b29b3e90791`, isolated checkout `/Users/Honza/.codex/worktrees/rim-publish/rust`. The dirty primary checkout, other worktrees, HTTPS8443 and Three8878 were preserved. No publication is requested by this task.

Original release WASM: `c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a`. Counter-only baseline: `63d42031818030e43ce8eda32814782c976051f750c1b5279bb58e03b217496b`. Candidate: `fac16c7cd6ef698fcb196e980348eb965f88c7e13d1aa5a4bc318a6f43a2175b`. Original baseline is the primary comparison; instrumented baseline is explanatory evidence only. Manifest files distinguish source hashes, SDK/assets, WASM, common test harness and actual served bytes. Full local snapshots live in ignored `output/render-performance/`; compact evidence and compressed raw samples are archived beside this journal when the series completes.

Desktop: Apple M1, Chrome155.0.8059.85, hardware ANGLE/Metal. No physical Quest was available. Simulated XR verifies contracts/content, not headset performance, thermal stability or input-to-display latency. The user was asked to close other 3D pages; absence of external GPU activity is not confirmed. Our builds and GPU runs are serialized.

## Accepted hypothesis under test

Rigid same-asset groups previously compared each new world-space root AABB against every prior disjoint root: quadratic comparisons even when four or six instanced draws render thousands of objects. GPU transforms, skinning, instancing, primitive culling, shadow-caster revisions and packed-depth elision already exist; they were not reimplemented.

The isolated algorithm replaces that scan with an insertion-ordered binary forest of AABB unions. It preserves exact greedy groups and touching/overlapping order. Small groups≤16 stay linear. A query visits at most64 tree nodes, then falls back to a complete exact scan; this bounds additional query tests, not all construction/memory overhead. Stored tree nodes stop at8192 leaves; larger groups return to linear scans. Leaf storage remains O(N). Invalid/missing/inverted/nonfinite primitive bounds conservatively disable grouping; each primitive is validated before a union can hide NaN. No shader, vertex precision, resolution, effect, shadow quality or gameplay change is included.

Native independent pairwise grouping oracle covers random/spatial/shuffled/alternating/dense/touching/invalid bounds,4096 objects,8193+ and reuse. Maintained browser tests additionally compare actual independent unbatched pixels, overlap/order, cap boundaries, mirrors/nonuniform transforms, skin/culling, cut gameplay and XR paths. Scratch byte accounting includes tree storage. `instancePagesActive` is the sum of last recorded active counts per pass/view key, not a current-frame census.

## Candidate audit and next experiments

| Candidate | Current evidence and decision |
|---|---|
| A — WebGL XR presentation | Source confirms second GL context, canvas upload and per-eye blit. Not the desktop batching bottleneck. Whole presentation GPU cost/shared-context prototype remains unmeasured; retain current fallback and wgpu state ownership. |
| B — XR parity | Local Three r170 uses AA, DPR cap1.5, ACES/exposure1.25, cached PCF soft shadows and default foveation1. Rust GL AAfalse and direct/owned-eye WebGPU differ. No matched physical XR comparison, eye-size/frequency acceptance or20% claim. |
| C — normal/light shader | No-normal constant128/255 decodes slightly nonneutral. A true absent-normal variant changes material interpretation and needs its own glTF/pixel reference. N·L lighting branch also unmeasured. Neither is mixed with this grouping experiment. |
| D — grouping/pools | Exact pairwise scan selected after CPU profile. Tree optimization measured separately. `instance_pages.truncate(page_index)` still exists; retained bounded GPU-page pooling remains a candidate, not a delivered optimization. |
| E — CPU transforms | Changed primitives still compute matrix inverse/determinant/bounds, and CPU morph deformation/upload remains. Per-node caching requires dedicated animated/multi-primitive evidence; no blanket clone removal. |
| F — vertex/passes | Rigid vertices remain96B and morph fallback uploads96B/vertex. Compact float32 layouts and further pass specialization require geometry/fragment evidence; no layout rewrite in this experiment. |
| Uniform arena | Prior experiment rejected. Not repeated without a new hypothesis. |

## Retained failures and corrections

- The first WebGPU family matrix failed dynamic cutting because a builtin primitive was incorrectly made cuttable. Replaced with a genuine closed triangle mesh and asserted one removed object/two pieces on each cut. Kept old failure reports.
- The initial8192 probe timed out during `networkidle0`. Later6144 telemetry showed a fully initialized, rendering scene while navigation still waited. The final runner uses DOMContentLoaded followed by explicit engine readiness within a bounded deadline. The6144 timeout is a demonstrated runner failure. The earlier8192 timeout remains inconclusive and was not repeated with the corrected wait; neither proves a GPU or engine startup limit. New calibration re-runs after the fix; old probes are exploratory only.
- One normal-sandbox Node HTTP unit-test process crashed in its environment; the identical elevated test passed. No product workaround or browser sandbox disabling was added.
- WebGL native frame profiler exposes no valid GPU pass times. The separate actual EXT timer is asynchronous and measures the application RAF GPU command interval, excluding compositor/worker/between-RAF work; it is not pure GPU busy time. Six actual queries produced five valid samples, with one pending sample discarded at teardown in the acceptance smoke. Disjoint/stale/nonfinite/incomplete results are rejected. Profile-off runs issue no timer queries and assert engine profiling is disabled.

## Protocol

Primary throughput/cadence comparisons use three alternating AB/BA/AB pairs,8s warmup and25s samples, profiler off, fixed baseline-calibrated overload. Profiled diagnostics use explicitly shorter4s/10s three-pair samples; real static scenes2s/8s three-pair controls; these are not substituted for the primary protocol. Captures occur only after timing. Repetitions, not adjacent frames, are independent comparisons. Calibration is baseline-only,1s warmup/2.5s sampling, geometric sweep plus bounded refinement/overload confirmation. Capacity criterion is declared by the runner before measurement, with0.5ms cadence tolerance. RAM limit covers JS heap only, not total GPU/process memory. Timers never force completion or synchronize readback during measurement.

The Three control matches actual quad geometry, visibility, camera, size and elapsed poses. Final controls distinguish1024-instance prebatched groups from independent root/mesh pairs. Material/PBR/lighting differ; it is not the original Rome gameplay or full visual parity. Its pixel difference is retained diagnostically rather than passed as a same-renderer regression gate. Active gameplay capture is not reset to a deterministic state; active pixel gates are explicitly unavailable and maintained functional tests supply correctness evidence.

Final numeric results and exact executed commands are stored in the result archive/report; unmeasured candidates above remain open.


## Adversarial acceptance trap and revisions

The initial spatial-only result is not sufficient acceptance. Originalc430 versus optimizedv1fac16, three8s/25s profiler-off pairs,6144 visible instances: median CPU ratio0.5136 on WebGPU and0.5084 on WebGL, all spatial captures exact. Profiled WebGPU glTF queue approximately17.5→3.5ms; scene.main GPU remained approximately0.25ms. Actual six glTF commands,6144 submitted instances and688128 instance-upload bytes per profiler frame were preserved. Scratch storage increased626752→1093824bytes; candidate warm instance-page creations were0. Typical authored static controls were near the cadence cap and must not be reported as a universal game speedup.

A later diagnostic buildf69de1cf (same grouping algorithm plus reason counters) failed the predeclared adversarial gate. Alternating low/high spatial insertion at6144 instances produced approximately28ms original versus55ms candidate, median paired ratio1.9679 across three1s/5s secondary pairs, with exact pixels. Shuffled order was approximately neutral (ratio0.9947). The queue automatically stopped between runs. All unfavorable results are retained. The profile exposed about41.3ms in glTF queue and18998468 leaf tests/19245165 total bounds tests per frame, six draws and6144 submitted instances. The64-node budget bounds extra comparisons, not necessarily generated-code cost.

Revision0cecd49a replaced mutable counter writes inside the fallback predicate with position()+one counter update. A diagnostic alternating pilot improved55→32.9ms, but a current-condition original control was28.3ms; this remained unsuitable for general acceptance. A second expression uses any() and remaining iterator length, keeping counter/index updates out of the hot loop. Six native tests independently verify first/last/miss counts, prior tree work, budget fallback and original greedy grouping. Compiler-codegen effects remain a hypothesis until measured; no assembly-level mechanism is claimed.

Reason diagnostics were also corrected before these controls: empty alpha queues must not count as rejected visible roots. Morph fallback is rejected at unavailable bounds before the later nonrigid draw condition; the test originally assumed the later barrier and was corrected after observing actual counters and the independent bounds-cache implementation. Root reasons count runs containing eligible draws, not unique scene objects. No material/shader behavior changed.

The driver was updated only after all initial primary/static runs completed and its exact old source was archived as driver-v1.tar.gz. New dynamic tests include bounded frustum-edge motion; their new harness identity requires a fresh calibration. Long evaluation protocol timeout now covers sample+startup+30s, avoiding a guaranteed timeout at exactly120s. There were no concurrent GPU tests or builds across these boundaries.


## Final adaptive revision and acceptance protocol

The pure-iterator revision c3a1a02 still measured about30.9ms in the adverse pilot versus the contemporary28.3ms original. It was not accepted alone. Final6f9a3bba adds a run-local adaptive fallback: after eight cumulative query-budget exhaustions, stop rebuilding/traversing the index and retain an exact linear scan until the greedy group is cleared. It never drops leaves or changes draw order. An independent native test verifies the late4097th leaf remains searchable and clear restores tree pruning. Seven native tests and14 maintained browser/backend cases pass, including independent pixels and real visible cut halves. Strict wasm Clippy and SDK typecheck pass. The JS tooling suite passes22 tests after adding the explicit Three representation option.

Final adversarial secondary controls use three AB/BA/AB pairs,1s warmup/5s measurement, fixed6144 objects (dense512), identical captures. CPU median paired ratios: WebGPU shuffled0.8775, alternating1.0287; WebGL shuffled0.8847;97% spatial overlap WebGPU1.0263 and WebGL0.9804. Small adverse deltas remain reported, not recast as proof of zero overhead. Fully coincident tiny quads produced only four visible pixels and failed the actual foreground gate on both versions; those runs remain recorded failures, and partial97% overlap is the accepted visible stress replacement.

Final main WebGPU regression mode completed three8s/25s profiler-off pairs: CPU29.3/29.5/29.9ms original versus15.3/15.4/15.6ms final, paired median ratio0.5220; all three captures exactly equal. One candidate repetition still has RAFp99=33.3ms despite median16.7ms; average/capped cadence must not hide this tail. Remaining final backend/family results are recorded in the dated report, rather than inferred from the earlier candidate.

Driver archives v1/v2/v3/v4 preserve completed measurement phases. The final Three-only fixture change adds an independent-root reference alongside prebatched geometry. The final primary WebGPU run did not load/hash the Three fixture; CLI integration happened after that run completed. Drivers hash their exact inputs per run and reject mutations during the measurement. Separate API-counter enrichment counts actual passes/state calls but never runs inside uninstrumented timing pages.


## Final tool audit and animation-start boundary

Read-only review found two tool defects before the long soaks: soak skipped common-harness hash/load-membership validation, and ratio summaries accidentally carried millisecond budget fields from a timing-stat helper. Both were corrected at a completed-run boundary, with regression tests (24 tools tests PASS). Soak now checks the exact common harness and rejects missing/uncalibrated loads while deliberately permitting a different candidate runtime. Dimensionless ratio summaries omit duration/budget fields; historical numeric ratios remain valid but their inherited timing metadata must be ignored. Source archives v5/v6 preserve these phase boundaries.

The first longer skin-family pair failed in seekAnimation during baseline setup/warmup and was retained, not accepted as a performance sample. Native seek rejects negative times. A RAF timestamp can precede the performance.now epoch captured by a start command in the same frame, so the harness now clamps elapsed time to zero before its first pose and records elapsedSeconds on future failures. This is a timing-boundary safeguard; the old failure did not record the failing value, so negative elapsed is a supported hypothesis rather than a captured value. A fresh skin calibration/pair and any calibration consumed after the common-harness change use the corrected harness. Completed earlier before/after runs retain their original stable common harness and hashes. No engine behavior or measured positive-time motion was changed by this tool fix.

Three final rotations were corrected from default XYZ to Rust's Ry*Rx*Rz (YXZ) before final Three measurement. Sixteen CPU-only cases compare the actual Three matrices with a direct trigonometric Ry*Rx formula (maximum absolute error4.44e-16). The earlier Three result is kept with an unmatched-rotation-order limitation. The independent objects mode creates one root and mesh child per logical object, rather than assuming prebatched input.


## Dynamic resource evidence and GPU-heavy limits

Final WebGPU dynamic512 soak completed120s:61 successful cuts,61 spawns,183 explicit removes; root culled gauge ranged0–153, so the frustum phase exercised actual culling. RAFp95=16.7ms/p99=16.8ms/max=16.8ms, zero tolerance-aware overruns. RAF CPU median4.2ms/p99=5.5ms/max=6.3ms. It created2648instance pages/5296buffers/2648bind groups during the warm measured interval, with zero scratch growth and final one active/retained page. Pool churn is confirmed, not fixed. The short dynamic pair was4.0→4.3ms (~+7.5%, one profiled pair), so an additional three-pair profiler-off investigation is queued rather than declaring universal improvement.

Fragment64 actual scene.main GPU medians6.96→6.79ms with CPU0.6→0.6ms show a different bottleneck. Effects32 has postprocess scope final.postprocess.overlays7.75→7.37ms and scene.main~0.24ms, CPU0.7→0.7ms; one short pair does not establish the small GPU difference. Effects calibration at64 aborted on baseline native profiler error Timestamp readback unavailable, with no reported GPU runtime loss/page error. Completed loads1–32 remain valid, over-capacity level unavailable; failure is not accepted as a renderer capacity limit. An explicit profiler-off effects calibration/control isolates the instrumentation failure. The final WebGL long soak uses profiler off; the already completed WebGPU long soak remains profiled and is labelled accordingly.


## Independent final source/evidence review

Astra Medium performed a bounded read-only audit of the final exact-overlap implementation/integration and completed WebGPU/adversarial/soak evidence. No new correctness blocker was found: all leaves survive small/tree/cap/adaptive transitions, query exhaustion uses an exact scan, touching bounds remain conflicting, primitive validation is conservative and draw ordering/compatibility remain intact. The reviewer explicitly retained O(N) leaf memory, small adverse deltas, pool churn and the need for repeated dynamic evidence. It did not run tests or provide headset acceptance. A separate tools review previously identified the corrected soak/ratio/Three-rotation issues recorded above.


A concrete next cross-engine hypothesis is now source-grounded: frozen Three r170 Object3D.updateMatrix composes position/quaternion/scale directly (vendor module line7805), while src/scene.rs ObjectTransform::model constructs T*Ry*Rx*Rz*S on each call. Investigate the number of repeated per-root/per-node compositions/inverses/bounds in scene.gpuUpdates first, then cache by the existing revisions or test an equivalent direct composition. Preserve YXZ order, negative/nonuniform scale, hierarchy/shear and normal transforms. This is an unimplemented candidate, not a claimed speedup; the measured remaining scene.gpuUpdates~9ms is the reason to prioritize it alongside proven page churn.


## Bounded teardown correction

The final WebGL skin pair finished both samples, exact pixel comparison and report/raw writes in19.178s, but its owned Node CLI remained idle more than160s after that report. Process inspection found no browser children and no TCP sockets. The exact stuck await was not stack-inspected. Only this completed Node process was terminated; the scheduler records exit-15, not a fabricated success. Numerical data remain usable with an explicit cleanup limitation. A boundary-events record hashes the unchanged original report. The CLI now bounds each owned cleanup operation, records forced/error cleanup as nonzero and exits explicitly after awaited data writes and owned resource closure; it cannot hang indefinitely on orphan library handles.25 unit tests pass, including timeout/force/error cases. Driver-v7 preserves the pre-fix source. No runtime or common timed harness changed, so earlier completed samples are not invalidated.


## Completed final decision

Final originalc430 versus adaptive6f9 WebGL primary: three8s/25s pairs, profiler off,6144 objects; CPU32.5/32.7/31.9→16.8/16.7/17.0ms, median paired ratio0.516923. All three images exactly equal and no driver/server mutation errors. WebGPU primary ratio0.522034 is independently recorded above. Short paired capacity: largest tested passing3840→4096 WG and3584→4096 GL; this does not establish a global maximum. Candidate6144 CPU tails still fail the strict criterion despite median60Hz cadence.

The predeclared response to short adverse dynamics completed three2s/10s profiler-off pairs per backend. WG pairs4.4→4.1,4.4→4.3,4.2→3.5ms give median ratio0.931818; GL4.5→4.5,4.3→4.4,4.2→3.9ms give1.0000. All six pixel pairs are exact. The initial short profiled regression is not confirmed; preserve the individual variation rather than promising improvement on every run. Astra read these final reports and the GL primary: no scoped acceptance blocker, explicit remaining pool churn/unknownQuest/Three limitations.

Final Three r170 prebatched6144 reference: CPU1.5/1.5/1.7ms, six calls/12288triangles, versus final Rust15.8/15.8/15.8ms and six calls. Independent6144root/mesh pairs: Three7.1/7.0/7.0ms and6144calls versus Rust15.6/15.5/15.5ms and six calls. Matching camera vectors/FOV/load/root motions do not establish pixel parity: native camera uses LH projection while Three uses RH conventions, and material/lighting paths differ. Submitted geometry/command work is checked. These controls show a remaining CPU preparation gap, not a universal20% or Rust-versus-JS language verdict.

All12 actual cut replays passed (three per build/backend, simulatedXR72Hz). Median request-to-complete-publication: original/final160.8/159.9ms WG and158.9/160.4ms GL; maxima159.9–163.6ms across sets. Main-thread onmessage maxima0.2–0.3ms are not worker compute time; internal worker compute duration is unavailable. No meaningful cut-latency speedup is claimed. Both opposing halves are independently visible in both eyes and dissolve/remove correctly. WebGL120s profiler-off soak completed61cuts with zero tolerance-aware RAF overruns, CPUmedian4.4/p99=6.6/max=8.3ms,2646new pages. WG soak is separately labelled profiler-on.

Effects64 profiler-off native WG rendering completed after fresh original calibration despite the retained profiled readback failure. One short pair CPU0.5→0.6ms is at coarse timer granularity, not evidence of a reliable20% regression; exact pixels and raw tails are retained. It does not solve the GPU effect cost.

Final instrumentation audit also corrected an unobserved edge: a GL query spanning a missing sampler used to be labelled invalid but could still be queued. It is now ended/discarded, with a26th unit test. Offline audit found zero invalid-window entries and zero in-run driver-mutation reports among actual completed data. A new real GL smoke returns five valid asynchronous samples, no invalid entries and clean owned teardown. Fresh Node/browser smoke through benchmark-core, final skill schema validation and current source/pkg6f9 freshness all pass. Driver-v8 preserves pre-edge-fix source. The final manifest/commands/raw archive makes the full evidence durable. No publication or production-package regeneration was performed.
