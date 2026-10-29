# Executed commands and reproduction

Working directory `/private/tmp/rust-render-followup-1`, branch `codex/render-followup-1`, base9986cd4f9. Node24.22.1 from the bundled runtime, pinned Rust1.99.0. Local browser configured to `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`; actual Chrome156.0.8078.48, sandbox enabled. Owned temporary localhost servers only. Browser/bind permissions were approved through the execution tool; ordinary sandbox attempt failed before reproduction.

## Actual reproduction commands

```sh
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
node output/followup-1/reproduce.mjs host-reproduction-3
node output/followup-1/history-isolation.mjs history-isolation
node output/followup-1/analyze.mjs
```

These directories already exist; do not overwrite them. Reproduction drivers accept a fresh output-directory name. Their frozen-root constants and original reference paths are explicit at their tops; restore old evidence and remap only filesystem roots if historical temporary directories are gone. Scene/quality/protocol options remain frozen. The retained original failed diagnostic attempts have their source snapshots and FAILURE.json; corrected injection/driver are separately retained. `analyze.mjs` is exactly the executed version; it expects successful raw report.json files, which were subsequently gzip-compressed losslessly for space. Decompress in a fresh evidence restore before analysis.

Unmodified maintained runner's recorded command:

```sh
node scripts/render-performance.mjs compare \
  --baseline-root /private/tmp/rust-next-render-cycle-1/output/next-render-cycle/baseline \
  --candidate-root /private/tmp/rust-next-render-cycle-8/output/next-render-cycle-8/candidate \
  --config /private/tmp/rust-next-render-cycle-9/output/next-render-cycle-9/cal-rim-camera-stress-webgpu/config.json \
  --backend webgpu --family rim-camera-stress --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --startup-timeout 90000 --max-time 2400 --no-profile \
  --pixel-channel 0 --pixel-fraction 0 --out output/followup-1/unchanged-protocol
```

Keep the original config,8s warmup/25s sample,3pairs and zero pixel tolerances. The driver checks actual backend/profile state. This command produced three exact pairs; it did not reproduce original per-pose clock history. Diagnostic replay inputs are in replay-times.json (1230steps; reconstructed warmup plus recorded sample intervals).

## Actual verification

```sh
VERIFY_WASM_PROFILE=release npm run verify:push
# exit101: disk full at wasm-check, after native/SDK/harness/XR PASS
# removed only own rebuildable cache; later user explicitly requested more unused-target cleanup
VERIFY_WASM_PROFILE=release npm run verify:browser
# exit0: remaining WASM checks/types/149browser scenarios; release package freshness verified/reused
scripts/dump_sources.sh
# exit0: byte-identical all_sources.txt
```

Native stages were not redundantly rerun. There is no new all-mode PASS receipt; all required checks completed in the two recorded invocations. See VERIFICATION.json and archived logs. No current push/CI/device claim.

## Evidence recovery

This batch reuses the prior verified B9 object archive format/restorer rather than introducing another archive framework. New archive contains diagnostics, failure snapshots, PNGs, raw intervals, verification logs and tool scripts; runtime/harness dependency remains the verified B9 archive. `EVIDENCE_INDEX.json` lists aliases/hashes. The existing B9 `RESTORE.py` is included unchanged.

```sh
tar -xOf docs/performance/render-followup/EVIDENCE.tar.gz RESTORE.py > /private/tmp/followup1-restore.py
python3 /private/tmp/followup1-restore.py docs/performance/render-followup/EVIDENCE.tar.gz /private/tmp/NEW_FOLLOWUP1_RESTORE
```

To replay analysis, copy restored `measurements/` into a fresh checkout's `output/followup-1/`, decompress successful `report.json.gz` files there and restore/remap the original B9 reference root. The saved analysis script includes original absolute provenance; it never modifies PNGs. Never restore over accepted outputs. Archive creator reopens all objects and aliases; restoration and every restored file hash are checked. Archive verification is recorded separately.

Auxiliary post-run host inventory hung and was interrupted only after diagnostic browser and servers had closed and COMPLETE had been saved. No host-after.json was produced; absence is not evidence of isolated GPU/power state. Later read-only process inventory found other old diagnostic processes and normal browser activity; preserved.
