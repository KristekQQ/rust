# Change-driven model GPU publication — 2026-10-30

One optimization after K checkpointfcc9dda3b. Selected candidate uses per-model change/removal queues before GPU publication. It removes repeated checks of unchanged models; shaders, geometry, resolution, simulation and draw preparation are unchanged. This is a CPU-headroom improvement, not a demonstrated general FPS increase or Three/Quest victory.

## Identity and implementation

Baseline K:WASM `887e7de920fb8d4488855dea8689459381f48fe06bcdbc6931e55899e860d81c`. Final:WASM `a81d6ee104b30e8fe09a3915f8659d16c48c999f0197b68b66e8f63e00e8fcc0`, source `91c9d9bf877092e07e4ef6e57e543fb53e117f01014474f81df6dabf3b93b90f`. Branch `codex/model-gpu-publication`, parentfcc9dda3b. Release opt-level3/fatLTO/codegen-units1/wasm-optO3; final snapshot794files. The containing local checkpoint is the source handoff; no push/merge/shared deployment.

- Scene changes and direct animation/physics paths enqueue model IDs once. A queue-membership bit survives clone/commit and is reset on consumption; recycler preserves retry work. Add/remove/clear record lifecycle events, including clear→reload before publication.
- The renderer publishes changed models and removes actual GPU generations before view preparation. Existing prepared-plan/content epochs, resource leases and per-eye selection remain responsible for their original dependencies. No new GPU pool/buffer or view cache.
- `setModelPublicationReuseEnabled(false)` provides the full-scan pixel reference. New diagnostics count visits/full scans/idle calls and pending/capacity CPU bytes; no per-model clock calls are added.
- CPU-only native builds have no GPU consumer: collection is disabled outside wasm/test. This fixes a potential accumulation found in review of the first candidate. Dispose drops the Scene queues; clear recycles bounded peak capacity rather than claiming physical GPU memory release.

## Fixed protocol and final individual runs

M1/16GiB, Chrome156.0.8078.74, AC,1280x720DPR1,seed1337. Baseline-only fixed-load calibration,3AB/BA/AB pairs8s warmup/25s sample. Profiling off, minimal preallocated recorder,0periodic polls/no overflow. GPU times unavailable in clean runs. CPU callback wall time includes browser/driver interactions, not pure compute. Background mediaanalysisd/WindowServer makes this a nonisolated desktop experiment. Use per-pair results; do not pool different campaigns as one stable machine.

Common2048GL is explicitly limited base-color/noAA/lights/shadows/post. Camera2048WG/GL and moving1024GL keep production Rome graphics. Moving512 pilot was60Hz capped; the baseline-only1024 probe was fixed before main moving pairs. No busy-wait, reduced update rate or delayed visible cut.

|Final workload|K FPS pairs|Candidate FPS pairs|K callback mean ms|Candidate callback mean ms|K / candidate callback p99 ms|
|---|---|---|---|---|---|
|commonGL|40.11/40.15/40.19|39.88/39.88/39.79|4.91/4.87/4.92|3.16/3.27/3.41|5.90/5.80/5.90 / 4.00/4.00/4.00|
|movingGL|33.94/33.58/33.34|34.24/34.10/32.94|29.31/29.63/29.84|29.06/29.17/30.22|30.70/31.00/31.80 / 29.80/30.40/32.40|
|cameraWG|54.93/52.97/52.80|54.97/53.08/53.04|16.90/17.10/17.38|16.13/16.23/16.30|22.00/22.20/22.70 / 21.50/21.30/21.40|
|cameraGL|35.60/36.84/36.88|35.60/35.40/36.96|18.28/18.41/19.28|17.65/18.80/17.62|22.10/23.30/27.10 / 21.80/28.70/21.50|

Static CPU callback falls about31–36%, with p99~5.8–5.9→4.0ms. Static FPS is consistently~0.6–1.0%lower in this campaign; do not erase that adverse observation or call it an FPS win. CameraWG CPU improves in all pairs by~0.8–1.1ms; cadence changes are small. CameraGL has two improved callback runs and one adverse run (p99~23.3→28.7ms, FPS36.84→35.40). All-moving final results are mixed within~1.5%; an earlier intermediate pair showed a larger adverse result, retained below. No repeatable material general throughput gain is established. Accept the removed CPU work/headroom with those limits; do not claim the7xThree CPU gap is solved.

Pilot-only ordinary Rome and mixed-one-root controls remained60Hz capped. Separate final Neon pilots on both backends also remained60Hz; these are small-scene regression controls, not throughput proof. This task does not rerun Three: the prior bounded Three audit (archive path `context/three/REPORT.md`) has its own limited graphics/culling contract and historical runtime. Exact new native pixels do not close inherited A/B/C/D image blockers from [closeout](../render-followup/CLOSEOUT.md).

## Verification and limits

Three targeted native queue tests PASS: dedup/no-op/unrelated objects, animated/spinning/projectile paths, failed command, removal/clear/reload/inflight requeue. Initial test compile failure was ambiguous emptyVec equality; fixed assertions and retained log. Native non-test clippy--libPASS. Independent Lupa review found no remaining invalidation defect, and separately confirmed the native no-consumer guard. Review is code inspection, not headset proof.

New gltf-publication pixel fixture passes both backends: static/camera0visits; one changed root1visit; nonuniform/negative transforms; render options; pose/morph/active scene; removal, replacement, clear/reload, dispose/reinitialize. Full-scan reference receives the same current scene inputs. Existing motion/fullglTF/effects scenarios7PASS on intermediateV1; final preflight3PASS includes real asymmetric WebGPU eye attachments. The final maintained gate below runs the complete final code.

Actual worker cut replay3runs/backend PASS with simulated XR64stress; request/publication and cut callback distributions retained, not mistaken for instantaneous effects or a speedup.60s sampled pool cycles: queue capacity4096B constant on both backends, pending0/16/1008/1024 according to visibility transitions,61440model visits/window; accounted instance pool2211456B constant. Pending IDs after the JS workload tick await the next native publication; this is not unbounded growth. These sampled memory runs are not clean timing or physical GPU memory evidence.

IntermediateV1 WASM41453294 is preserved separately. It lacked the native no-consumer guard/boundary capacity gauges. Moving1024 pair2 was~8%adverse; final fresh3pairs did not reproduce that large drop. A brief formatter/source edit occurred during its frozen camera campaign and is logged in INTERMEDIATE_LIMITS.json; it is not substituted for final identity/performance. No unfavorable run was deleted or relabelled as final.

Final `VERIFY_WASM_PROFILE=release npm run verify:push`: **PASS**, 436Rust/2ignored, 173browser({'webgpu': 67, 'webgl': 106}); SDK32,XR-platform71,Node18+Python13. Format/clippy native+WASM, wasm32 check, types and source dump PASS. Current counts are freshly executed, not historical. Matching final WASM reused.

Independent raw/PNG audit: 70run-sides/55082RAF rows, 31strict native pairs, 0strict failures, 0integrity errors. Authored gameplay pairs remain nongating.

## Reproduction and delivery

Original commands/host snapshots are in archived commands.jsonl; every run has raw.json.gz, PNGs, actual quality/work counters and served identities. RUNS.csv/JSON contains every final and intermediate row including p95/p99/max. Existing object/alias archive format and restorer are reused; runtime overlays omit unchanged assets and nested historical archives. Reconstruct roots over the verified K runtime from closeout, then validate all render-build.json file hashes.

```sh
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
MODEL_PUBLICATION_CANDIDATE=output/model-publication/candidate-final python3 output/model-publication/run.py final commonGL,movingGL,cameraWG,cameraGL
python3 output/model-publication/final_checks.py worker
python3 output/model-publication/final_checks.py soak
VERIFY_WASM_PROFILE=release npm run verify:push
```

Existing output labels are refused. Use new labels/directories on replay; never erase earlier evidence. The candidate's frame/model publication change is general, not keyed to an example name. Physical Quest FPS/comfort and the default XR transfer/staging paths remain unvalidated/unchanged by this task. Existing own preview8899 still serves K;8443/shared production untouched. No automatic next optimization round.

Archive validation:1042unique objects/2086aliases reopened and hash-checked; restored into a new directory. Both runtime overlays reconstructed over K and independently matched all794manifest files. [Archive verification](ARCHIVE_VERIFICATION.json), [restore verification](RESTORE_VERIFICATION.json), [manifest](MANIFEST.json). Unchanged final runtime inputs retain the full content-bound PASS after documentation updates.
