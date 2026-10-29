# Reproduction and executed-command ledger

Owned checkout `/private/tmp/rust-render-followup-2`, branch `codex/render-followup-5`, base `503502250de09250db3c2a8710c8bc51b25b091d`. Node24.19.0 from `/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin`; Rust1.99.0. Release WASM uses `RUSTFLAGS=--cfg=web_sys_unstable_apis`. No shared server or deployment changes.

The archive's `measurements/commands.jsonl` retains actual argv arrays, exit codes, start/duration, AC/thermal snapshots. `measurements/run.py` is the serial coordinator for pilots, diagnostics, primary pairs, Three and single-axis GPU controls. Templates below describe reproduction; actual execution and failures are established by the ledger/logs, not by these templates.

```sh
node scripts/render-performance.mjs compare \
  --baseline-root RESTORED/measurements/baseline \
  --candidate-root RESTORED/measurements/probe2 \
  --config RESTORED/measurements/cal-rim-camera-stress-webgpu/config.json \
  --backend webgpu --family rim-camera-stress --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --candidate-experiment ordinal-merge --no-profile \
  --pixel-channel 0 --pixel-fraction 0 --startup-timeout 90000 \
  --max-time 2400 --out NEW_OUTPUT
```

Use matching frozen configuration for each backend/family. Three uses `--three-root RESTORED/measurements/baseline` instead of candidate root/experiment; default `--three-representation instanced` is retained. `rim-common` is the limited common profile, `rim-stress` production comparison is graphically unmatched. Diagnostic controls use `smoke --experiment production|cheap-fragment|no-fxaa|no-shadows`, separately changing target size only for the resolution control. Do not merge diagnostic timings into clean pairs.

Native order oracle: `cargo test ordinal_merge --lib`. Focused browser cases: `gltf-ordinal-motion,gltf-ordinal-passes,rim-ordinal-xr` on both backends. Full checkpoint gate, its actual status/counts and input receipt are recorded separately in VERIFICATION.json. Browser/simulated XR is not physical Quest.

Offline analysis runs after GPU measurement stops. `analyze.py` summarizes full recorded frame/callback distributions and counters; independent PNG verification checks exact decoded pixels and archived hashes. It does not increase tolerances or select only favorable pairs. Raw series, failures and frozen builds are preserved in the content-addressed archive.

Restore to a NEW directory using the existing verified restorer:

```sh
tar -xOf docs/performance/render-followup/batch-5/EVIDENCE.tar.gz RESTORE.py > /private/tmp/restore-followup5-evidence.py
python3 /private/tmp/restore-followup5-evidence.py \
  docs/performance/render-followup/batch-5/EVIDENCE.tar.gz /private/tmp/followup5-restored-NEW
```

Frozen historical documentation is omitted from the archive to avoid recursively duplicating already-versioned evidence. Runtime source/assets/SDK/WASM and the maintained shared harness are retained with hashes. Never modify a restored configuration's hash to bypass identity checks.

Actually executed offline checks after the serial GPU coordinator stopped:

```sh
python3 output/followup-5/analyze.py
/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 output/followup-5/validate_evidence.py
python3 output/followup-5/finalize.py
VERIFY_WASM_PROFILE=release npm run verify:push
VERIFY_WASM_PROFILE=release npm exec -- node scripts/verification-cache.mjs check all
VERIFY_WASM_PROFILE=release node --input-type=module -e 'import {assertFreshBuild} from "./scripts/build-fingerprint.mjs"; console.log(JSON.stringify(await assertFreshBuild()));'
python3 output/followup-5/seal.py
```

Final runtime was not rebuilt or edited after the measured probe2 freeze:288source/SDK/harness/package files match its hashes. Final docs and generated source dump are later. VERIFICATION.json records the fresh433native/169browser gate, not historical counts. The initial offline analyzer failure and corrected parser are both archived; cross-engine gates remain as originally recorded, native tolerance remains zero.

Archive verification actually executed:

```sh
python3 output/followup-5/archive.py
python3 output/followup-5/restore-evidence.py docs/performance/render-followup/batch-5/EVIDENCE.tar.gz /private/tmp/rust-followup5-evidence-restore-final
```

All2903restored files were then independently reread and SHA-256 checked. The temporary independent restore copy was removed afterward; the archive, frozen originals and worktree remain. ARCHIVE_VERIFICATION.json records97,007,317archive bytes,1171unique objects and the archive hash. Only this final gate's verification files are included, with preflight/failure captures separately preserved; older unrelated output artifacts were left untouched locally.

Coordinator sequence: `run.py pilot`, then `run.py diagnostic` and `run.py primary`, then `run.py three`. The supplemental unsupported productionWG/Three request stopped that last phase before browser startup; `resume.py` completed productionGL, followed by `run.py axes`. The initial coordinator is also preserved. Use the supported CLI templates/configs above for fresh replay; do not bypass the guard or overwrite old evidence.
