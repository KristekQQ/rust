# Followup2 — commands and evidence boundaries

Working directory `/private/tmp/rust-render-followup-2`, isolated branch `codex/render-followup-2`, base `0b7af5b75082a2508d6fb19d05e7fef4e51c5c77`. No push, merge, live deployment or shared-server restart. Node24.22.1, pinned Rust1.99.0, Chrome156.0.8078.48 with sandbox. Headless desktop; no physical Quest. Other browser processes remained active; small timing differences are not isolated hardware measurements.

## Executed preparation

```sh
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
export VERIFY_WASM_PROFILE=release
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo clippy --locked --target wasm32-unknown-unknown --lib -- -D warnings
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --release -- --locked
RUSTFLAGS=--cfg=web_sys_unstable_apis node scripts/build-fingerprint.mjs --write
RUSTFLAGS=--cfg=web_sys_unstable_apis node scripts/freeze-render-build.mjs output/followup-2/final-candidate
npm run typecheck
```

Frozen roots `original`, `diagnostic-baseline`, `candidate`, `final-candidate` are distinct, never overwritten. Original preserves B8/f14c4beb WASM. Diagnostic baseline runs original selection plus counters; first candidate enables the new query on both APIs. The directory named final-candidate is an INTERMEDIATE candidate: it retains original selection on WebGL/unknown backends. It is not the delivered default. Restored-default now preserves the original selector on both APIs after the WebGPU regression. See MANIFEST.json for all hashes. The first final-build attempt compiled Rust but sandbox blocked bindgen/opt; approved host execution completed without altering configuration.

## Executed comparison protocol

```sh
python3 output/followup-2/calibrate.py stress
python3 output/followup-2/run-primary.py stress
python3 output/followup-2/calibrate-original.py all
python3 output/followup-2/run-final.py all
```

These wrappers invoke the maintained `scripts/render-performance.mjs` serially. Their complete per-run argument arrays and fresh calibration manifests are archived. Calibration here is a short **fixed-load identity probe**, not a throughput result or candidate-dependent tuning. Historical B7/B9 SDK identity checks rejected attempted config reuse; no hashes/guards were bypassed. Loads, cameras, scene seeds, resolution and quality stay at the predeclared workloads.

Main command shape (new output directory required):

```sh
node scripts/render-performance.mjs compare \
  --baseline-root output/followup-2/original \
  --candidate-root output/followup-2/final-candidate \
  --config output/followup-2/original-cal-rim-stress-2048-webgpu/config.json \
  --backend webgpu --family rim-stress --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --startup-timeout 90000 --max-time 2400 --no-profile \
  --pixel-channel 0 --pixel-fraction 0 --out output/NEW_COMPARISON
```

The sample order is baseline/candidate, candidate/baseline, baseline/candidate. All samples and adverse tails are retained. FPS is 1000/mean RAF interval; median intervals alone can hide throughput changes. RAF cadence is not compositor presentation. Callback CPU scope is wall time, not pure compute. GPU timing is unavailable in clean profiling-disabled runs; no zero substitute or sum with CPU time.

Separate profiler-on diagnostics use the same `smoke` command with8s/25s and explicit load, without `--no-profile`; these include detailed counters and are not used for clean performance claims. Exact oracle fixture compares original/optimized/restored at fixed inputs; original transparent ordering and active-set traversal remain independent. Asymmetric desktop cameras do not constitute XR acceptance; maintained XR suites supply separately labelled simulated-XR coverage.

## Restored default and completed focused checks

```sh
python3 output/followup-2/run-finish.py
python3 output/followup-2/run-diagnostics.py
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --release -- --locked
RUSTFLAGS=--cfg=web_sys_unstable_apis node scripts/build-fingerprint.mjs --write
RUSTFLAGS=--cfg=web_sys_unstable_apis node scripts/freeze-render-build.mjs output/followup-2/restored-default
node scripts/test-browser.mjs --backend all --page overlay-selection,webxr,webxr-webgpu
python3 output/followup-2/audit-diagnostics.py
node output/followup-2/inspect-cluster-pixels.mjs
```

The main runner retained its strict clustered-image failure; run-finish only executes the remaining predeclared configurations. Five focused cases pass: overlay-selection WebGPU (21 exact oracles), WebGL2 (25), WebXR WebGPU/GL, and direct WebGPU XR. Simulated XR, not physical headset evidence.

## Final restored-default controls (completed)

```sh
python3 output/followup-2/run-restored.py stress
python3 output/followup-2/audit-results.py > output/followup-2/results-summary.txt
node output/followup-2/png-audit.mjs
python3 output/followup-2/write-report.py
```

Both configurations completed three AB/BA/AB pairs with 8s warmup,25s sample, profiling off. All six exact image pairs PASS. Original→restored-default FPS: WG2048 37.551→37.497,37.642→37.551,37.337→37.671; GL1024 42.454→42.374,42.414→42.493,42.442→42.465. Mixed small differences on a nonisolated host; no claimed gain. Full gate `VERIFY_WASM_PROFILE=release npm run verify:push` completed once after these controls, serially, exit0; logs retained.

## Reproduce from the committed evidence archive

After archive validation, restore to a new owned directory (the restorer refuses existing output and verifies every SHA-256 before writing). Historical report archives inside the frozen roots' docs directories are deliberately omitted; they are already versioned. Runtime source, SDK, WASM, assets, configurations and maintained harness are retained.

```sh
tar -xOf docs/performance/render-followup/batch-2/EVIDENCE.tar.gz RESTORE.py > /tmp/followup2-restore.py
python3 /tmp/followup2-restore.py docs/performance/render-followup/batch-2/EVIDENCE.tar.gz /tmp/followup2-restored
node scripts/render-performance.mjs compare \
  --baseline-root /tmp/followup2-restored/measurements/original \
  --candidate-root /tmp/followup2-restored/measurements/restored-default \
  --config /tmp/followup2-restored/measurements/original-cal-rim-stress-2048-webgpu/config.json \
  --backend webgpu --family rim-stress --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --startup-timeout 90000 --max-time 2400 --no-profile \
  --pixel-channel 0 --pixel-fraction 0 --out output/NEW_RESTORED_COMPARISON
```

The last command is a reproduction recipe, not an additional executed benchmark. For the rejected WG candidate choose `measurements/final-candidate`; for the all-backend experimental control choose baseline `measurements/diagnostic-baseline`, candidate `measurements/candidate`, and the corresponding `cal-rim-stress-*` control configuration. Never label the historical directory name final-candidate as the delivered default. For all datasets use the exact per-run arguments archived in runner JSON; do not reuse an SDK-mismatched calibration.

## Final gate identity check (executed)

```sh
VERIFY_WASM_PROFILE=release npm run verify:push
VERIFY_WASM_PROFILE=release npm exec -- node scripts/verification-cache.mjs check all
python3 output/followup-2/check-integrity.py
```

The first command was run once, not again after report edits. Receipt revalidation performs no tests. Existing receipt key serialization includes environment insertion order; direct `node` outside npm yielded a mismatched key, whereas the same npm environment verifies the original receipt. No synthetic receipt or key edit. Frozen runtime files and current source/SDK/pkg/tests/harness were independently hashed.

## Archive verification (executed)

```sh
python3 output/followup-2/archive.py
python3 output/followup-2/restore-evidence.py docs/performance/render-followup/batch-2/EVIDENCE.tar.gz /private/tmp/rust-followup2-evidence-restore
```

The archive reader checked all1185object hashes and4399alias entries, then a separate filesystem pass checked every restored file's bytes/hash and absence of symlinks. Both restored WG2048/GL1024 calibration manifests pass `validateCalibrationInputs` against restored original runtime and current maintained harness. Twelve local documentation links checked before adding archive-proof links; those added links point to the verified existing JSON.
