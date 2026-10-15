#!/usr/bin/env bash
# One entry point for local and CI verification. Browser mode builds pkg once.
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-all}"
if (($#)); then shift; fi
case "$mode" in native|browser|all) ;; *) echo 'Usage: scripts/verify.sh [native|browser|all] [browser options]' >&2; exit 2;; esac
if [[ "$mode" != browser ]]; then
    cargo fmt --check
    cargo test --locked
    cargo clippy --locked --all-targets -- -D warnings
    npm run test:sdk
    npm run test:harness
fi
if [[ "$mode" != native ]]; then
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg=web_sys_unstable_apis"
    cargo check --locked --target wasm32-unknown-unknown
    cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings
    wasm-pack build --target web --dev -- --locked
    node scripts/build-fingerprint.mjs --write
fi
npm run typecheck
if [[ "$mode" != native ]]; then
    node scripts/test-browser.mjs "$@"
fi
