#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg=web_sys_unstable_apis"
wasm-pack build --target web --dev
port="${1:-8000}"
echo "Open http://127.0.0.1:$port/index.html"
exec python3 -m http.server "$port" --bind 127.0.0.1
