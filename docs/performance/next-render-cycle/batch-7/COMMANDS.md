# Executed commands and evidence locations

Cwd `/private/tmp/rust-next-render-cycle-7`; bundled arm64 Node24.22.1 on PATH, pinned Rust1.99.0. Commands below are actual execution, not recommendations. No shared server or production package change.

```sh
cargo fmt --check
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --release -- --locked
VERIFY_WASM_PROFILE=release node scripts/build-fingerprint.mjs --write
VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/next-render-cycle-7/restored
python3 /tmp/b7-restored.py
python3 /tmp/b7-audit.py
python3 /tmp/b7-results.py
VERIFY_WASM_PROFILE=release npm run verify:push
```

The final command is the R gate; disposition is in VERIFICATION.json after completion. The earlier full gate on C also passed, but is superseded because C was rejected and source restored. Preserve both; no test receipt was reused across changed source.

The archive includes exact argv/start/finish/exit and matching coordinator scripts:

- `matrix-commands.json`:16single-axis diagnostic runs plus calibration/pilots, frozen D.
- `primary-commands.json`:48clean8s/25s D/C runs, two separate profiled/API followups, calibrations.
- `three-commands.json`:18clean pinned-Three comparisons and separate2run profiled pair, with actual driver/runtime metadata.
- `restored-commands.json`:12clean D/R regression runs, threeAB/BA/AB pairs per backend, existing frozen configurations, profiler off, zero pixel tolerance.
- `final-input-audit.json`: independent raw quantiles/capture/runtime/driver/frozen-file checks. Former C-current audit preserved as `rejected-candidate-input-audit.json`.
- `replay/b7-*.py`, `matrix-driver/`, `final-driver/`: exact coordinator/audit and phase-specific maintained driver sources. `REPLAY.md` describes reconstructing parent assets and adjusting the isolated path prefix.

Native/WASM strict Clippy, SDK/harness/XR/types, release identity, maintained browser pixels/cutting/lifecycle/simulated XR and generated source dump are checked by the integrated gate. Do not equate gate success with physical Quest/remote CI acceptance. No push/main merge/deploy.
