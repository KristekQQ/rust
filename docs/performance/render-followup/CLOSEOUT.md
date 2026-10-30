# Renderer closeout — one selected local candidate

**Recommend K (production mode0), retaining the bounded WebGL bitmap-panel reuse and the original nine-batch gains. Functional, image and performance acceptance are separate. Overall exact-image integration remains BLOCKED; this is a review/test candidate, not an unconditional release.** No push, merge, shared-server restart or deployment is authorized or performed.

## Identity and decision

Durable checkout: `/Users/Honza/WebstormProjects/rust/output/worktrees/render-closeout`; branch `codex/render-closeout-resume`; base/evidence `185c5f71ad778d6d3f89394885ee54fb087d9e9b`. The local checkpoint containing this report is the handoff. Runtime/SDK/test/source identity was compared with frozen K:770 relevant files match exactly before the gate. The frozen K manifest contains792 files including guidance/build metadata.

| Variant | Runtime/WASM | Decision |
|---|---|---|
| A: followup initial B8/B9 | `813a5abb266609cceea644e4b1086129a89ca535`, `f14c4beb5f3e5b59f16cae9ae90ddccba88b152ed10f07f3ce999d5a765a12de` | Reference, not original pre-nine-batch B1 |
| B: A + six isolated panel-reuse hunks | `eee58d93dc923e92991444a65e5e15acffd787b47127ab9c88f2abeec6c548c9` | Isolates panel benefit; no blind wholesale rollback to B |
| C: F5 production mode0 | `08c2d212a12228874e2f998e8f96c9b840facc68`, `34acb880ddcf429a1cd1b62fe43ab07d2a69e3ab4827958d98cb7d02154e3a5d` | Retain supported defaults/API oracles |
| D: same C with asserted mode3 | Same WASM as C | Camera callback improves, no clear total-frame gain; stays opt-in |
| K: C + main-view diagnostic controls and capture/measurement repairs | `887e7de920fb8d4488855dea8689459381f48fe06bcdbc6931e55899e860d81c` | Selected local candidate |
| N: K + union bookkeeping fast path | `c78e2695b19a446bbd189374996f423279dfea0e1b12aa6ddf183a27ca35dd59` | Rejected; source/pkg restored exactly K |

B construction copies only bounded GL panel changes, not entire F2/F4/F5 refactors. C does not reproduce the old repeated clustered slowdown; a broad rollback would also remove established diagnostic APIs/XR oracles without attributing a regression. Mode0, original overlay selection, union uploads and original color interpolation remain default. Mode3, shared-GL XR and experimental upload/overlay paths remain explicit controls, not newly promoted production behavior. WebGPU owned-eye staging, shaders, resolution, geometry and gameplay were not changed.

## Actual repairs and bounded experiments

- Minimal recorder: bounded preallocated20000-row storage; no periodic renderer/profile/heap polling, overflow fails closed; multiple callbacks sharing one RAF stamp accumulate. Boundary counters are outside timing. Production scene diagnostics are preserved. This repairs the comparison contract, not the renderer.
- Capture tooling: records actual camera state and capture timing; bounded CPU write/copy shadows track content epochs and actual command/pipeline data. Unknown copy sources invalidate the trace instead of inventing known bytes. These shadows are not GPU readback and do not cover arbitrary GPU writes or all texture/object revisions. New recorder/copy tests exercise the contracts; no golden/tolerance change.
- Main-view-only culling control: root and primitive flags enter exact view/eye cache keys; auxiliary shadow/reflection selection retains its own rules. New fixture uses real glTF shadow casters and occupied shadow maps. Asymmetric WebGPU eye tests exercise all four flag combinations and restore the defaults.
- Panel reuse: retain existing bounded WebGL capacity/index-prefix reuse; WebGPU keeps its prior mapped initialization because an earlier reuse experiment regressed its event workload. This closeout reconfirms allocation benefit rather than claiming a newly invented optimization.
- One late N experiment aggregated union bookkeeping per group, preserving diagnostic/coalesced/full paths.3native/7browser checks and3strict image pairs passed, but cameraGL callback K17.914/19.897/20.135→N18.180/19.870/20.123ms; FPS37.18/36.92/36.88→37.16/36.88/36.87. No useful gain; reverted. N source, diff, WASM and unfavorable evidence remain archived.

## Measurement contract and limits

Apple M1/16GiB, macOS, Chrome156.0.8078.74, AC power;1280×720,DPR1,seed1337,spatial order, matching production quality. Main pairs use8s warmup/25s sample, pool60s, alternating AB/BA/AB; pilots1s/5s explicitly separate. Real workload calibration is frozen per scenario. Profiler is verified off, minimal sampler polls0, no overflow; GPU time in clean runs is **unavailable**, not zero. CPU callback wall time is not pure CPU computation, and is not added to GPU time.

Environment is not isolated. Recorded WindowServer/media/Codex load and later mediaanalysisd CPU activity limit small causal claims. Within-pair browser/config identity is fixed; absolute values across campaigns are not pooled as one stable machine. Diagnostic API observers and120s sampled memory runs are separate from clean timing. No physical Quest validation or new Three comparison was performed in this bounded closeout.

Same-runtime C sampled→minimal control (three pairs): clusteredGL callback improves4.4–5.3%, cameraWG9.7–13.5%, cameraGL13.1–14.1%. CameraWG cadence changes about48–49→52–54FPS while GL camera remains~35.5FPS. This is measurement observer cost, **not renderer speedup**. Old sampled and new minimal histories cannot be silently mixed.

Fresh A/C clustered1024GL: A27.08/26.72/26.94 vs C27.00/26.94/26.82FPS; mixed<1% differences. CE also does not establish an adverse F5-default direction. FE/BF broad ablations were therefore not started. C/D camera callback improves under mode3 but total FPS remains~53WG/~35.6GL; cluster/pool/Neon do not justify promotion.

## Final A versus K — individual clean runs

FPS below is1000/mean frame interval, not inverse median. Each cell lists pairs0/1/2 in pair order. Full distributions and raw rows are in the evidence archive.

| Workload | A FPS | K FPS | A callback mean ms | K callback mean ms | A/K callback p99 ms |
|---|---|---|---|---|---|
| cluster | 27.06/27.08/27.00 | 27.02/26.96/26.68 | 36.79/36.76/36.88 | 36.83/36.92/37.29 | 41.80/40.30/40.30 / 40.30/41.40/41.70 |
| cameraWG | 53.08/53.05/52.76 | 53.01/52.97/52.97 | 16.38/16.37/17.44 | 17.02/17.07/17.09 | 21.70/21.60/22.70 / 22.10/22.00/22.10 |
| cameraGL | 35.62/35.58/35.54 | 35.60/35.56/35.60 | 17.80/17.77/17.85 | 18.38/18.36/18.41 | 21.60/21.60/21.80 / 22.30/22.10/22.20 |
| pool | 59.07/59.05/59.00 | 59.00/59.00/59.00 | 4.12/4.15/4.23 | 4.24/4.20/4.16 | 19.60/19.60/19.50 / 20.10/20.00/20.00 |
| neon | 60.00/60.00/60.00 | 60.00/60.00/60.00 | 1.87/2.00/1.96 | 1.62/1.86/1.68 | 2.50/2.70/2.60 / 2.50/2.50/2.50 |

No general final FPS improvement is proved. Final cluster K is slightly slower, largest difference~1.2% in pair2; prior A/C direction was mixed. CameraGL callback consistently costs about0.56–0.59ms more than A while cadence is unchanged; N did not repair/attribute that complete difference. Preserve this adverse result. Pool tails must be read per run, not reduced to the largest maximum. Neon and ordinary-game60Hz controls are capped. Static/ordinary controls are short pilots, not a proof of long-run equivalence.

## Render work and upload boundaries

The clean boundary counters cover glTF only, not the whole renderer. Across final pairs: camera has306 glTF draw commands/RAF in both builds; cluster about202, pool about155. Neon has0 glTF work, **not zero total draws**. CameraWG instance upload is A1.396–1.397 versus K1.392–1.396MB/RAF; cameraGL A1.450–1.451 versus K1.450–1.456MB/RAF. Cluster~0.836MB/RAF, pool~0.009MB/RAF. Each camera run creates one instance-pool buffer during the window; cluster/pool create none. Exact calls/instances/triangles/bytes and per-run boundaries are retained in `FINAL_WORK.json` and original reports. Uniform bytes include instance bytes; do not add these columns. These are native counters; whole-API allocations are separately observed for panels.

## Independent image acceptance

| Case | Verified evidence and status |
|---|---|
| A: B9 camera2048WG | Original candidate pair1:6pixels/max2; other two pairs exact. Archive PNGs freshly decoded/hashed. Bounded old replays did not reproduce; raw replay captures were lost on reboot, so command records are historical observations only. **Unresolved, not repaired.** |
| B: B4 | Actual archived identity is `rim-clustered1024-WebGPU`, `RESULTS.json results[4]`, pair0:8pixels/max2; other pairs exact. Older moving/full-motion prose was inaccurate. **Unresolved independently.** |
| C: culling/shading | Fresh main-primitive-off control reproduces5022pixels/max79 twice; restore exact. Historical global/both-off5014/max79 is a different control. Main-root-only had no difference. **Reproduced integration blocker.** |
| D: F2cluster | Original pair0:4pixels/max1; other pairs exact. Archive independently decoded. **Unresolved independently.** |

C diagnosis:89520 common instance records/material/geometry/order matched,4855 extras. Rendering extras alone exactly matches suppress-all-glTF twice: no visible extra geometry contribution under this diagnostic. Keeping only common records but partitioning306draws into1898ranges still differs4021pixels/max79. Invariant-position5038/max79 and chain-rule derivative4995/max70 did not repair it. Quantized normal visualization is exact while normal-variation visualization differs14549pixels/max212, restoring exactly. This locates observable divergence at the derivative-based roughness input, not a proven underlying floating-point/driver mechanism. RGBA8/FXAA and shader dead-code differences limit that diagnostic. No quality-preserving shader fix is established; AA disabling, rounding and tolerance changes are rejected.

WGSL derivative/invariant rules were checked in the [2026-10-26 specification](https://www.w3.org/TR/2026/CRD-WGSL-20261026/#derivative-builtin-functions). They motivate controlled tests; they do not establish that the error is harmless. New exact pairs/gate PASS never close A/B/C/D. Authored gameplay frames are explicitly nongating for exact pixels; deterministic fixtures have their own narrower acceptance.

## Acceptance records

See the generated acceptance summary below and archived raw reports. Functional success does not override the independent image blockers. Physical Quest (comfort/FPS, opaque XR framebuffer/context-loss, certificate/session behavior) remains unverified; simulated stereo tests are separate.

## Reproduction and delivery

The committed `closeout/EVIDENCE_RUNTIME.tar.gz`, `closeout/EVIDENCE_RUNS.tar.gz` and `closeout/EVIDENCE_PRODUCTION.tar.gz` use the existing content-addressed format with verified per-object hashes and aliases. No historical archives are recursively nested. `INDEX_RUNTIME.json`, `INDEX_RUNS.json`, `INDEX_PRODUCTION.json` and `ARCHIVE_VERIFICATION.json` describe contents. Extract only `RESTORE.py`, then restore each archive into a new directory; never overwrite previous evidence. Recovered original historical archives remain unchanged.

Maintained commands actually used (exact labels/arguments and timing recorded in archived `measurements/commands.jsonl`):

```sh
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
python3 output/closeout/run_decision.py final A K cluster,cameraWG,cameraGL,pool,neon
python3 output/closeout/run_acceptance.py diagnostic
python3 output/closeout/run_acceptance.py soak
python3 output/closeout/run_acceptance.py worker
VERIFY_WASM_PROFILE=release npm run verify:push
npm run build:production
VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --production-root output/production --backend all --page main-view-culling,webxr-webgpu,rim-refresh,rim-gameplay,neon-arena,panel-buffers
```

Scripts refuse existing output. For a new reproduction choose new output labels/paths; do not erase old runs. After restoring archives, the equivalent maintained cameraWG comparison is:

```sh
node scripts/render-performance.mjs compare --baseline-root RESTORED_RUNTIME/runtime/A --candidate-root RESTORED_RUNTIME/runtime/final-candidate --config RESTORED_RUNS/measurements/cal-minimal-v2-A-cameraWG/config.json --backend webgpu --family rim-camera-stress --max-load 2048 --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 --no-profile --measurement minimal --pixel-channel 0 --pixel-fraction 0 --out NEW_OUTPUT
```

The current source/test harness must match the archived driver identity; changing it requires a new protocol identity. Existing preserved evidence must never be overwritten. The first panel acceptance attempt omitted required `--config` and failed before browser activity; the runner was corrected to reuse frozen calibration, retaining its failure log. Failed and rejected experiments remain evidence, not omitted samples.

Isolated package path: `/Users/Honza/WebstormProjects/rust/output/worktrees/render-closeout/output/production`. Preview command (choose a free port; does not replace8443):

```sh
cd /Users/Honza/WebstormProjects/rust/output/worktrees/render-closeout
python3 scripts/serve-local.py 8899 --host 127.0.0.1 --production --directory output/production
```

Desktop preview: `http://127.0.0.1:8899/examples/rim/?backend=webgpu` or `?backend=webgl`; Three control `/examples/rim-three/`. Loopback HTTP is not a Quest LAN/TLS deployment. Minified production SDK differs by transformation; verify manifest mapping and served hashes, not byte equality with authored JS.

Recovery limitation: reboot removed the former `/private/tmp` checkout and uncommitted raw closeout evidence. That storage choice was an agent error. Decisive runs and C diagnostics were freshly repeated in this durable checkout; missing old raw data are not represented as recovered evidence. Original historical failures were recovered from committed archives and independently verified.

Remaining priorities are only causal closure of A/B/C/D, attribution of the small cameraGL callback cost under isolated conditions, and physical Quest acceptance. This task does not automatically begin another optimization series.

## Actual final verification

Independent raw/PNG audit: 142 run-sides, 143884 RAF rows, 71 image pairs; 56 strict pairs exact, 15 nongating authored-gameplay pairs, 0 integrity errors. This includes pilots and separate diagnostics; it is not a claim of142 main clean runs. Raw frame means/p95/p99/max recomputed independently.

Panel observer, separate5s warm windows: WebGL createBuffer38→0, WebGPU38→34. WebGPU does not use GL reuse: its changed panel text/update event counts differ, so38→34 is not evidence of removed redundant WebGPU allocation. Final GL panel capacity739480B with54live buffers and30960B CPU index shadow; no inactive panel pool. Necessary creations during initialization and new content remain.

|120s sampled pool cycle|RAF rows|Accounted live bytes throughout|Creates throughout|Frame p99/max ms|Callback p99/max ms|
|---|---:|---:|---|---|---|
|webgpu|7048|[2211456]|[410, 410]|16.80/66.80|18.70/71.80|
|webgl|6367|[2145392]|[202, 202]|33.40/66.80|19.90/68.90|

No pool growth, retired bytes or budget breaches observed in these windows. Accounted resource capacity excludes driver memory; this is pool reuse, not a claim of process/GPU-memory leak freedom. Clear/dispose/reinitialize and XR lifecycle belong to the functional gate. Sampled instrumentation and concurrent system work mean these timings are not clean A/B results.

|Actual worker cuts,3fresh runs/backend|Request-to-publication ms|Maximum death/cut callback ms per run|
|---|---|---|
|webgl PASS|207.60/189.80/198.70|45.50/33.50/39.50|
|webgpu PASS|179.40/175.70/173.50|6.10/4.80/4.00|

Simulated XR64stress, actual worker publication and staged uploads. These are correctness/latency observations, not A/B speedup. WebGL cut-frame maxima33.5–45.5ms remain visible-risk evidence; publication~174–208ms is not mistaken for instantaneous response. Physical headset comfort is unverified.

Full maintained `VERIFY_WASM_PROFILE=release npm run verify:push`: PASS. Native433 PASS/2ignored; browser171 PASS(66WebGPU/105WebGL2); SDK 32; XR platform 71; Node harness 18 plus13Python. Format, clippy native/WASM, WASM check and types PASS. Matching release WASM reused. Full report/logs are archived separately before production checks overwrite current reports. No historical counts substituted.

Packaged browser verification: 11 PASS, release/minified production-root. The package manifest maps source→minified files; loaded application identity and WASM are verified by the maintained runner.


Archive validation: all three archives were reopened, every object/alias hash verified, then restored into three new directories (5513runtime,1054evidence,578production files). Restored production WASM matches K. See [restore verification](closeout/RESTORE_VERIFICATION.json) and [manifest](closeout/MANIFEST.json). The content-bound full PASS was reused after documentation changes; no second full test run.
