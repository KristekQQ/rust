# Batch9 — actual commands and repeatable evidence

Checkout `/private/tmp/rust-next-render-cycle-9`, branch `codex/next-render-cycle-9`, parent B8 `813a5abb266609cceea644e4b1086129a89ca535`, tooling checkpoint `0fcf7b1589bcf323a5cc65eb5c41289d9db03123`. Final evidence is the commit containing this document. Bundled Node24.22.1 and pinned Rust1.99.0; actual browser executable/version/hardware are recorded per run. B9 reuses verified B8 runtime; no new compilation/minified production deployment or shared-server restart.

## Executed workload commands

Inside [EVIDENCE.tar.gz](EVIDENCE.tar.gz), `measurements/` contains exact argument arrays/status/exit codes:

- `calibration-commands.json`: initial bounded pilots, not primary results. The overwritten early1024-WebGPU pilot is excluded as unavailable; raw remaining failure evidence is preserved.
- `primary-commands.json`:16 completed compare commands,88 clean original/final runs.
- `remaining-commands.json`:30 completed commands: later calibrations,18 clean Three runs, diagnostics, four actual-worker commands.
- `three-gpu-commands.json`:8 completed supplement commands, common-profile GPU/API plus Neon diagnostics.
- `power-confirm-commands.json`:empty under the predeclared discrete-endpoint decision; no fabricated confirmation runs.

`primary.py`, `remaining.py`, `three-gpu.py`, their logs, per-case `config.json`, `report.json`, actual served file manifests, raw compressed RAF/counter/API samples, PNGs and worker replay records are included. They executed sequentially, without concurrent owned GPU/build work. Process exit0 means workload completion; **read the separate pixel verdict**, which includes a retained FAIL. Do not rerun those orchestration scripts blindly: absolute old output paths are preserved as provenance and the driver refuses existing outputs.

Representative completed primary configuration, with a NEW output label for reproduction:

```sh
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
A=/private/tmp/rust-next-render-cycle-1/output/next-render-cycle/baseline
B=/private/tmp/rust-next-render-cycle-8/output/next-render-cycle-8/candidate
node scripts/render-performance.mjs compare \
  --baseline-root "$A" --candidate-root "$B" \
  --config output/next-render-cycle-9/cal-rim-static-webgpu/config.json \
  --backend webgpu --family rim-static --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --startup-timeout 90000 --max-time 2400 --no-profile \
  --pixel-channel 0 --pixel-fraction 0 --out output/perf/NEW_LABEL
```

Use the matching separately frozen WebGL configuration with `--backend webgl`. Dynamic64 and pool256 use60s/120s samples; exact arguments are in the manifest. Never recalibrate separately on a candidate. Limited Three controls use final native baseline plus final Three root and the existing pinned67-part instancing; full production settings remain separately unmatched. See [PROTOCOL.md](PROTOCOL.md).

Cold driver executed initially with7/12 capture-tool failures, then repaired with12/12 probes; both attempts retained. Actual Rust-worker commands executed original/final on both backends, three replays each:

```sh
node scripts/benchmark-cold-variants.mjs --baseline-root "$A" --candidate-root "$B" --out output/perf/NEW_COLD_LABEL
node scripts/benchmark-rim-cuts.mjs NEW_BASELINE_WEBGPU "$A/pkg" webgpu 64
node scripts/benchmark-rim-cuts.mjs NEW_FINAL_WEBGPU "$B/pkg" webgpu 64
# Repeat worker commands with webgl and distinct labels.
```

## Actual analysis and bounded correctness follow-up

These commands executed, after the completed GPU runs; they do not launch new measurements except the explicitly named camera probes:

```sh
python3 output/next-render-cycle-9/summarize.py
python3 output/next-render-cycle-9/analyze-extra.py
node output/next-render-cycle-9/audit-pixels.mjs
python3 output/next-render-cycle-9/audit-inputs.py
python3 output/next-render-cycle-9/publish-results.py
python3 output/next-render-cycle-9/finalize-report.py
```

Results:134 runs/193076 full RAF records, recomputed summaries/hashes zero errors;2377 accepted input checks/142 paths zero errors.39/40 native exact pairs,16 native gameplay pairs unavailable,11 cross-engine diagnostics non-gating. The full report preserves correction of old24/24 prose and the candidate6pixel/max2 failure. `camera-oracle.mjs` executed three final-runtime sessions; `camera-isolation.mjs` executed original/final axis isolation through the maintained runner/browser. Cache-only disabled is exact; culling disabled differs5014pixels in both builds. Their JSONs/source reviews are included; no root-cause claim or tolerance change.

## Actual final gate, identity and skill validation

The full command below executed **once**, exit0, on final stable code; do not rerun merely to resume this work. The maintained gate generated a byte-identical source dump. Full logs/receipt are in the archive under `verification/`; [VERIFICATION.json](VERIFICATION.json) records exact counts and limitations.

```sh
VERIFY_WASM_PROFILE=release npm run verify:push
VERIFY_WASM_PROFILE=release node --input-type=module -e 'import {assertFreshBuild} from "./scripts/build-fingerprint.mjs"; console.log(JSON.stringify(await assertFreshBuild()))'
VERIFY_WASM_PROFILE=release npm exec --offline -- node scripts/verification-cache.mjs check all
node --test scripts/render-performance.test.mjs scripts/render-performance-api.test.mjs
```

428 native pass/2 existing manual ignored,149 browser pass (55WebGPU/94WebGL),32SDK,71XR,14Node+13Python harness; separate49 performance helper tests. Original/final/current exported `assertFreshBuild()` rechecked, frozen296/297 runtime files and current297 files rehashed. A direct CLI call to `build-fingerprint.mjs --check` does not execute this exported check. Fresh final report/source reviewers found no material defect within their stated scope, not extra runtime verification.

Final docs/skill checks use isolated PyYAML installed only in `/private/tmp/b9-skill-validation-site`:

```sh
PYTHONPATH=/private/tmp/b9-skill-validation-site python3 /Users/Honza/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/wasm-render-performance
```

New/changed JS modules pass `node --check`; evidence Python helpers pass `python3 -m py_compile`. New local Markdown references and archive links are checked for existing targets. The final receipt check reuses the content-bound PASS; docs/skill/evidence changes do not change code-gate inputs. No historical test count is relabelled fresh.

## Self-contained archive and restoration

The archive contains accepted frozen source/runtime A/B, current harness scripts/tests, configs, raw results, failures, captures, actual gate logs and analysis helpers. Historical B2–8 archives remain separately versioned; they are not nested again. Index aliases deduplicate exact bytes. Full foreign process command lines remain in local recovery only; role-only snapshots and original-inventory hashes are exported. Original failed cold-tool source bytes were not frozen; hashes/failures remain, repaired harness bytes are included. Those limits are explicit in the index.

Actually executed:

```sh
python3 output/next-render-cycle-9/archive.py
python3 output/next-render-cycle-9/restore-evidence.py docs/performance/next-render-cycle/batch-9/EVIDENCE.tar.gz /private/tmp/rust-b9-evidence-restore-final
```

The archive creator reopens/hashes every object and alias; the restoration was executed into a new directory, then every restored path was rehashed. Matching baseline/final bytes are independent files, not mutable shared hardlinks. [EVIDENCE_INDEX.json](EVIDENCE_INDEX.json), [archive verification](ARCHIVE_VERIFICATION.json) and [manifest](MANIFEST.json) record identity/counts.

To restore elsewhere (NEW_DIRECTORY must not exist):

```sh
tar -xOf docs/performance/next-render-cycle/batch-9/EVIDENCE.tar.gz RESTORE.py > /private/tmp/b9-restore.py
python3 /private/tmp/b9-restore.py docs/performance/next-render-cycle/batch-9/EVIDENCE.tar.gz /private/tmp/NEW_B9_RESTORE
```

Use `runtime/baseline`, `runtime/final` and `measurements/` configs from that restored root. Run from this B9 checkout, or restore its archived `harness/scripts`, `harness/tests` and `source/package*.json` into a new scratch checkout with matching dependencies. Replace old root/config/output paths in the recorded argument array; keep every workload/quality/protocol option fixed. Do not overwrite accepted evidence. Analysis helpers preserve original path provenance; remap those roots to replay audits elsewhere.

This is desktop/simulated-XR evidence. No physical Quest, CI, push, merge, deployment or replacement of the running8443 service occurred. Final image acceptance remains blocked; stop this series.
