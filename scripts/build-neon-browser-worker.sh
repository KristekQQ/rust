#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export VERIFY_WASM_PROFILE="${VERIFY_WASM_PROFILE:-release}"
export RUSTFLAGS="${RUSTFLAGS:---cfg=web_sys_unstable_apis}"
worker_node="${BROWSER_NODE:-$(git config --local --get verification.node || true)}"
worker_node="${worker_node:-node}"
if ! "$worker_node" scripts/neon-worker-fingerprint.mjs --check >/dev/null 2>&1; then
    wasm-pack build games/neon-browser-worker --target web --release --out-dir ../../pkg-neon-worker -- --locked
    "$worker_node" scripts/neon-worker-fingerprint.mjs --write
fi
