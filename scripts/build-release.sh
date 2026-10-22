#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export VERIFY_WASM_PROFILE=release
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg=web_sys_unstable_apis"
build_node="${BROWSER_NODE:-$(git config --local --get verification.node || true)}"
build_node="${build_node:-node}"
if ! "$build_node" --input-type=module -e 'import {assertFreshBuild} from "./scripts/build-fingerprint.mjs"; await assertFreshBuild();' >/dev/null 2>&1; then
    wasm-pack build --target web --release -- --locked
    "$build_node" scripts/build-fingerprint.mjs --write
fi
"$build_node" scripts/build-production.mjs
