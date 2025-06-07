#!/usr/bin/env bash
set -e
RUSTFLAGS=--cfg=web_sys_unstable_apis \
    cargo build --release --target wasm32-unknown-unknown --features webgl
wasm-bindgen --no-typescript --target web \
    --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/*.wasm
cd web
python - <<'PY'
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
class H(SimpleHTTPRequestHandler):
    def end_headers(self):
        if self.path.endswith('.wasm'):
            self.send_header('Content-Type', 'application/wasm')
        super().end_headers()
ThreadingHTTPServer(('',8000), H).serve_forever()
PY

