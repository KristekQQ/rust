# Executed commands and reproducible handoff

Cwd `/private/tmp/rust-render-followup-2`. Local isolated `codex/render-followup-3`. Full argument arrays, power endpoints, timestamps and exit codes are preserved in evidence `measurements/commands.jsonl`; Python wrappers only sequence the maintained runner. No shared server or production root was changed.

```sh
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
export RUSTFLAGS=--cfg=web_sys_unstable_apis
export VERIFY_WASM_PROFILE=release
cargo test --locked buffer_capacity_tests
wasm-pack build --target web --release -- --locked
node scripts/build-fingerprint.mjs --write
node scripts/test-browser.mjs --backend all --page panel-buffers --capture
node scripts/freeze-render-build.mjs output/followup-3/final
python3 output/followup-3/run.py calibrate
python3 output/followup-3/run.py primary
python3 output/followup-3/run.py diagnostics
python3 output/followup-3/run.py profile
python3 output/followup-3/run-final.py primary
```

The first source check reported a private-field compile error; corrected to State's existing Deref forwarding. Invalid CLI `--backend both` was corrected to `all`; initial event fixture did not cross the target with its blade and correctly failed. Initial/refined XR probes had insufficient visible text at their chosen diagnostic distances; move the fixture head closer, retaining exact/foreground rules. Preserve these attempts. The first build without explicit Rust cfg was not measured; matched release rebuilds precede every accepted browser run. Freeze names are never overwritten.

Main final command shape:
```sh
node scripts/render-performance.mjs compare \
 --baseline-root output/followup-2/restored-default \
 --candidate-root output/followup-3/final \
 --config output/followup-3/cal3-arena-active-webgl/config.json \
 --backend webgl --family arena-active --width 1280 --height 720 \
 --warmup 8000 --sample 25000 --pairs 3 --no-profile \
 --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 \
 --max-time 2400 --out output/NEW_ARENA_COMPARISON
```

For `panel-events`, use the frozen instrumented original-path baseline `output/followup-3/candidate`, its `cal3-panel-events-<backend>/config.json`, `--panel-reuse original --candidate-panel-reuse reuse`; load16 is frozen from bounded1..16 calibration. This explicitly tests the **opt-in experiment** on WebGPU, whose delivered default remains original. Main arena comparisons use delivered defaults. Diagnostics and profiled runs are separate from clean pairs.

Never reuse an output directory; the runner refuses to erase evidence. Frozen runtimes/assets and common current harness have separate actual-served hashes. Runtime/source matching does not establish current production deployment.

Final diagnostic, verification and evidence commands actually run:
```sh
python3 output/followup-3/final-tail.py
node output/followup-3/command-wall.mjs
VERIFY_WASM_PROFILE=release npm run verify:push
VERIFY_WASM_PROFILE=release npm exec -- node scripts/verification-cache.mjs check all
node output/followup-3/aligned-lifecycle.mjs
python3 output/followup-3/summarize.py
python3 output/followup-3/audit-results.py
python3 output/followup-3/independent-pixels.py
python3 output/followup-3/archive.py
```
All wrappers/helpers use the maintained runner/server/browser options, never production services. Gate was run once, not simultaneously with GPU diagnostics. Additional lifecycle script has no source edits and checks afterfourRAFturns; it does not add a production synchronization wait. Main run argument arrays are immutable in commands.jsonl. Wrapper failures and invalid calibration attempts remain in the evidence. Source/pkg identity is in MANIFEST, gate counts/hash/receipt in VERIFICATION.

Archive aliases include `measurements/baseline`, `measurements/candidate` (V1), `measurements/final`, full raw rows/captures/scripts and `verification/`. Historical docs inside frozen roots are deliberately excluded to avoid recursively storing prior archives. Restore to a NEW directory using the included RESTORE.py or existing previous-batch restorer:
```sh
python3 output/followup-3/restore-evidence.py  docs/performance/render-followup/batch-3/EVIDENCE.tar.gz  /private/tmp/rust-followup3-evidence-restore
```
For reruns, pass restored runtime roots to the maintained driver in this checkout; keep frozen configs, viewport, seed, original/reuse flags and alternating pairs. Do not assume archived frozen sources include later standalone diagnostics. Restore is data-only; it does not install dependencies or start a server.
