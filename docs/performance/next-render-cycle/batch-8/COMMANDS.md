# Commands actually executed

Working directory `/private/tmp/rust-next-render-cycle-8`; Node24.22.1 bundled runtime on PATH; pinned Rust1.99.0. Exact arguments/settings and environment/power boundaries also live in each raw report. Browser/server commands need the host's ordinary loopback/Chromium permissions; sandbox restrictions caused one recorded server-bind failure, retried via approved host execution. No Chromium sandbox disable flag.

```sh
# Initial ancestry package snapshot (matching B7 release reused, not recompiled).
VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/next-render-cycle-8/baseline
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown --locked
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo clippy --target wasm32-unknown-unknown --locked -- -D warnings
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --release -- --locked
VERIFY_WASM_PROFILE=release node scripts/build-fingerprint.mjs --write
# Release built twice: initial prototype, then fixed UV/guards. Both logs retained.
node --test tests/xr-simulator-unit.test.mjs tests/xr-presentation-profile-unit.test.mjs
npm run typecheck
VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --backend webgl --page xr-gl-presentation,rim-gameplay-gl-shared --capture
# Narrow xr-gl-presentation reruns repaired asset URL, then attachment/RGBA guards.
VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/next-render-cycle-8/candidate
```

Common runner: `node scripts/benchmark-xr-presentation.mjs --frozenRoot output/next-render-cycle-8/candidate --out output/next-render-cycle-8/NAME`, followed by:

| NAME | Arguments |
|---|---|
| pilot-rome-ordinary | `--warmup 1000 --sample 2500 --pairs 1 --workload rome --load 0 --eyeWidth 320 --eyeHeight 320 --routes baseline` |
| pilot-rome-ordinary-2 | same, after explicit abort/camera harness repair |
| pilot-rome-stress | `--warmup 1000 --sample 2500 --pairs 1 --workload rome --load 512 --eyeWidth 960 --eyeHeight 1008 --routes baseline` |
| smoke-rome-stress | `--warmup 1000 --sample 2500 --pairs 1 --workload rome --load 512 --eyeWidth 960 --eyeHeight 1008` |
| smoke-neon | `--warmup 1000 --sample 2500 --pairs 1 --workload neon --load 0 --eyeWidth 320 --eyeHeight 320` |
| primary-rome-ordinary | `--workload rome --load 0 --eyeWidth 320 --eyeHeight 320` |
| primary-rome-stress | `--workload rome --load 512 --eyeWidth 960 --eyeHeight 1008` |
| primary-neon | `--workload neon --load 0 --eyeWidth 320 --eyeHeight 320` |
| primary-rome-symmetric | `--workload rome --load 0 --eyeWidth 320 --eyeHeight 320 --asymmetric false` |
| diagnostic-rome-stress | `--pairs 1 --diagnostics true --workload rome --load 512 --eyeWidth 960 --eyeHeight 1008` |
| diagnostic-rome-symmetric | `--pairs 1 --diagnostics true --workload rome --load 0 --eyeWidth 320 --eyeHeight 320 --asymmetric false` |
| diagnostic-neon | `--pairs 1 --diagnostics true --workload neon --load 0 --eyeWidth 320 --eyeHeight 320` |

Primary defaults8s/25s/3pairs; diagnostics8s/25s/1pair. No other owned GPU/build workload ran concurrently with this series. Exact callback series/PNG/API/profile/power records retained; no benchmark was terminated to discard an adverse result.

```sh
# After stable implementation and measurements; local checkpoint gate, no push.
VERIFY_WASM_PROFILE=release npm run verify:push
# Exact final identity/receipt checks (same release profile required).
VERIFY_WASM_PROFILE=release node --input-type=module -e 'import {assertFreshBuild} from "./scripts/build-fingerprint.mjs"; console.log(JSON.stringify(await assertFreshBuild()));'
VERIFY_WASM_PROFILE=release npm exec --offline -- node scripts/verification-cache.mjs check all
```

An extra standalone fingerprint probe without VERIFY_WASM_PROFILE=release correctly rejected the default dev configuration. Correct release-profile retry passed; no rebuild/source substitution was needed.
