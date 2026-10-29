# Batch 9 reproduction and executed-command index

Worktree `/private/tmp/rust-next-render-cycle-9`, branch `codex/next-render-cycle-9`, parent `813a5abb266609cceea644e4b1086129a89ca535`. Use the bundled Node24.22.1 path and pinned Rust1.99.0. Browser is the actual executable recorded by each report, not an assumed Git configuration. No shared server or production package is replaced.

`A=/private/tmp/rust-next-render-cycle-1/output/next-render-cycle/baseline` is the original B1 frozen release; `B=/private/tmp/rust-next-render-cycle-8/output/next-render-cycle-8/candidate` is the hash-verified final release. B9 deliberately reuses B8 runtime bytes because it changes measurement tooling/documentation only. Maintained B9 test modules are overlaid by `serveFrozen`; actual served files and common harness hashes are recorded independently of the frozen runtime snapshot.

Current durable backup contains exact completed-attempt argument arrays in `calibration-commands.json` and `primary-commands.json`, raw reports/configurations and orchestration/analysis scripts. The held parent has not yet collected the last completed child's exit. `remaining.py` is a prepared plan only; `remaining-commands.json` and a final evidence archive do not exist yet because those measurements have not run. Argument arrays plus raw completion/cleanup records distinguish completed, failed and interrupted/held commands. Always choose a fresh output label; the runner rejects existing directories before opening a browser/server.

Representative primary invocation (same configuration on A/B; current local snapshot paths):

```sh
A=/private/tmp/rust-next-render-cycle-1/output/next-render-cycle/baseline
B=/private/tmp/rust-next-render-cycle-8/output/next-render-cycle-8/candidate
export PATH=/Users/Honza/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH
node scripts/render-performance.mjs compare \
  --baseline-root "$A" --candidate-root "$B" \
  --config output/next-render-cycle-9/cal-rim-stress-webgpu/config.json \
  --backend webgpu --family rim-stress --max-load 2048 \
  --width 1280 --height 720 --warmup 8000 --sample 25000 --pairs 3 \
  --startup-timeout 90000 --max-time 2400 --no-profile \
  --pixel-channel 0 --pixel-fraction 0 --out output/perf/NEW_LABEL
```

Use separately calibrated WebGL2 configurations for `--backend webgl`. B9 families/loads and longer dynamic/pool sample lengths are fixed in [PROTOCOL.md](PROTOCOL.md). Limited Three controls use final native `--baseline-root "$B" --three-root "$B" --family rim-common` and its own final-native calibration, preserving existing pinned instancing; production controls are explicitly unmatched in AA/shaders. Profiling/API diagnostic pages are separate from clean primary measurements.

Cold and real-worker checks:

```sh
node scripts/benchmark-cold-variants.mjs --baseline-root "$A" --candidate-root "$B" --out output/perf/NEW_COLD_LABEL
node scripts/benchmark-rim-cuts.mjs NEW_BASELINE_WEBGPU "$A/pkg" webgpu 64
node scripts/benchmark-rim-cuts.mjs NEW_FINAL_WEBGPU "$B/pkg" webgpu 64
# Repeat the last two with webgl and distinct labels.
```

The final report/verification manifest records actual completed unit/gate commands and counts after execution. Do not infer that an illustrative command above or a historical gate was freshly executed. The final local publication gate does not publish:

```sh
VERIFY_WASM_PROFILE=release npm run verify:push
VERIFY_WASM_PROFILE=release node --input-type=module -e 'import {assertFreshBuild} from "./scripts/build-fingerprint.mjs"; console.log(JSON.stringify(await assertFreshBuild()))'
VERIFY_WASM_PROFILE=release npm exec --offline -- node scripts/verification-cache.mjs check all
```

No physical Quest run or CI is implied by desktop/browser/simulated-XR checks. No push/merge/deployment is authorized by this document.


Fresh stable-code completion: `VERIFY_WASM_PROFILE=release npm run verify:push` executed once, exit0; standalone `assertFreshBuild()` and release-env receipt check executed and passed. See VERIFICATION.json for current counts. Also actually executed `node --check` on the new cold driver/page and changed performance driver/lib; `node --test scripts/render-performance.test.mjs scripts/render-performance-api.test.mjs`49PASS; isolated PyYAML-backed skill validator PASS. Performance scheduling is held for power, so prepared remaining.py commands have **not** executed and are not reported as results.
