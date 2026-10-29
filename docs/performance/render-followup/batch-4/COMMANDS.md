# Reproduction and actual command evidence

Worktree: `/private/tmp/rust-render-followup-2`, `codex/render-followup-4`, base `237279b6a39d7843fa47258faf9d09ae6dfb34d3`. Node used by this batch is **24.19.0** from the bundled runtime; the historical B3 freeze records 24.22.1. Both sides of each present comparison use the same current driver/Node/browser. Rust 1.99.0; release WASM with `--cfg=web_sys_unstable_apis`.

The evidence archive retains `measurements/commands.jsonl`: actual argv arrays, start/duration, exit status and host power/thermal observations. It also retains failures and each runner log. Read that ledger rather than interpreting the templates below as proof of execution.

From the local checkpoint, extract only the included restorer, then restore to a NEW directory. It verifies every object hash and refuses an existing destination. No overwrite flags or calibration-hash bypasses.

```sh
tar -xOf docs/performance/render-followup/batch-4/EVIDENCE.tar.gz RESTORE.py > /private/tmp/restore-followup4-evidence.py
python3 /private/tmp/restore-followup4-evidence.py docs/performance/render-followup/batch-4/EVIDENCE.tar.gz /private/tmp/followup4-restored-NEW
```

The actual retained run roots are restored under `measurements/`; worker reports are under `workers/`. The common driver is the final checkpoint's maintained scripts, which intentionally overlays its current performance fixture onto each frozen runtime. Complete actual-served identities are retained in the run manifests.

A primary comparison template:

```sh
node scripts/render-performance.mjs compare \
  --baseline-root RESTORED/measurements/probe \
  --candidate-root RESTORED/measurements/probe \
  --config RESTORED/measurements/cal-probe-rim-mixed-webgpu/config.json \
  --backend webgpu --family rim-mixed --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --upload-policy union --candidate-upload-policy coalesced --upload-gap 0 \
  --no-profile --pixel-channel 0 --pixel-fraction 0 \
  --startup-timeout 90000 --max-time 2400 --out NEW_OUTPUT
```

Use matching frozen configs for camera2048, sparse2048, all-moving1024, clustered1024, pool256, static2048, ordinary/static or active Rome. Pool main samples are60000ms. The fixed-load calibration pilots are1s/3s and do not establish performance. Policy pilots are1s/5s. Detailed diagnostics use `--profile --upload-diagnostic 1 --frame-context 1 --diagnostics --diagnostic-sample 5000`; never combine their CPU/GPU observations with clean FPS as if measured simultaneously.

A single real-worker replay (three alternating pairs are orchestrated by archived extra.py):

```sh
node scripts/benchmark-rim-cuts.mjs UNIQUE_LABEL RESTORED/measurements/probe/pkg webgpu 64 1 0
```

Final argument0=union,1=coalesced; penultimate argument is repetitions. The maintained scenario uses960×640 and simulated XR, a separate protocol from desktop1280×720. Publication latency and handler/install timings are separate, never inferred from disappearance of one long callback.

Focused correctness used the maintained `scripts/test-browser.mjs` runner; exact commands/logs are archived. `gltf-upload-ranges` tests sparse/affine/page/lifecycle cases; registered `gltf-upload-motion`, `gltf-upload-full-motion`, `gltf-upload-passes` and `rim-upload-xr` reuse independent existing motion/pass/gameplay fixtures. No physical headset proof.

Required checkpoint gate is `VERIFY_WASM_PROFILE=release npm run verify:push` once on the integrated final inputs. Final exit/counts are recorded in VERIFICATION.json. Attempt1 failed on SDK JSDoc before browser execution; the comment-only correction required a retry of the maintained gate. Both logs are preserved. Do not run a redundant full native suite and then the gate on the same inputs.


Offline analysis commands actually executed after the GPU measurement writer stopped:

```sh
/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 output/followup-4/analyze.py
python3 output/followup-4/analyze_clean_tails.py
python3 output/followup-4/analyze_diagnostics.py
python3 output/followup-4/analyze_worker_soak.py
python3 output/followup-4/finalize_report.py
```

The bundled Python provides Pillow/numpy for independent PNG equality. Other analyzers use the standard library. The command ledger contains calibration, pilot, main, controls, diagnostics, resumed campaigns and worker/soak argv. `resume_home.py`, `quiet.py`, `extra.py` and `control_home.py` preserve exact serial orchestration. The interrupted pool campaign is incomplete; do not treat its two retained sides as a completed three-pair experiment.

Final freeze/archive verification actually executed:

```sh
VERIFY_WASM_PROFILE=release npm exec -- node scripts/verification-cache.mjs check all
RUSTFLAGS=--cfg=web_sys_unstable_apis VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/followup-4/final
python3 output/followup-4/archive.py
python3 output/followup-4/restore-evidence.py docs/performance/render-followup/batch-4/EVIDENCE.tar.gz /private/tmp/rust-followup4-evidence-restore-final
```

All3348restored files were then independently read and SHA-256 checked against the index. The disposable restore copy was removed after verification; archive and frozen originals remain. `ARCHIVE_VERIFICATION.json` records exact bytes/hash/counts. `IDENTITY_AUDIT.json` verifies the final SDK by exact whole-file equality after only the two specified JSDoc edits; no whitespace/tolerance normalization was accepted.
