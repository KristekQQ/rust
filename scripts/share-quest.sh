#!/usr/bin/env bash
# Temporary public HTTPS preview. Copy only application runtime assets into a separate root.
set -euo pipefail
cd "$(dirname "$0")/.."
repo_root="$PWD"
cloudflared_bin="${CLOUDFLARED_BIN:-$repo_root/output/tools/cloudflared}"
if [[ ! -x "$cloudflared_bin" ]]; then
    cloudflared_bin="$(command -v cloudflared || true)"
fi
if [[ -z "$cloudflared_bin" || ! -x "$cloudflared_bin" ]]; then
    echo 'Install cloudflared or set CLOUDFLARED_BIN to its executable path.' >&2
    exit 1
fi
for file in pkg/webgpu_wasm.js pkg/webgpu_wasm_bg.wasm js/engine.js js/webxr.js examples/webxr/index.html examples/webxr/main.js examples/webxr/scene.js examples/webxr/geometry.js docs/WEBXR.md; do
    [[ -f "$file" ]] || { echo "Missing $file; build WASM first." >&2; exit 1; }
done
share_root="$(mktemp -d "${TMPDIR:-/tmp}/rust-quest.XXXXXX")"
server_pid=''
tunnel_pid=''
cleanup() {
    [[ -z "$tunnel_pid" ]] || kill "$tunnel_pid" 2>/dev/null || true
    [[ -z "$server_pid" ]] || kill "$server_pid" 2>/dev/null || true
    rm -rf "$share_root"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$share_root"/{pkg,js,examples/webxr,docs}
cp pkg/webgpu_wasm.js pkg/webgpu_wasm_bg.wasm "$share_root/pkg/"
cp js/engine.js js/webxr.js "$share_root/js/"
cp examples/webxr/{index.html,main.js,scene.js,geometry.js} "$share_root/examples/webxr/"
cp docs/WEBXR.md "$share_root/docs/"
printf '%s\n' '<!doctype html><meta http-equiv="refresh" content="0;url=/examples/webxr/"><a href="/examples/webxr/">Otevřít VR</a>' > "$share_root/index.html"
health_name="health-$(basename "$share_root")"
printf '%s' "$health_name" > "$share_root/$health_name"
port="${QUEST_PORT:-8001}"
(cd "$share_root"; exec python3 "$repo_root/scripts/serve-local.py" "$port") &
server_pid=$!
# Fail before exposing a tunnel if this server could not bind its port.
ready=false
for attempt in {1..30}; do
    kill -0 "$server_pid" 2>/dev/null || { wait "$server_pid"; exit 1; }
    if [[ "$(curl --silent --fail "http://127.0.0.1:$port/$health_name" || true)" == "$health_name" ]]; then ready=true; break; fi
    sleep 0.1
done
[[ "$ready" == true ]] || { echo 'Preview server did not start.' >&2; exit 1; }
kill -0 "$server_pid"
echo 'Open the HTTPS trycloudflare.com address below in Meta Quest Browser.'
echo 'Only the copied WebXR runtime is shared. Keep this terminal and Mac running.'
"$cloudflared_bin" tunnel --no-autoupdate --url "http://127.0.0.1:$port" &
tunnel_pid=$!
wait "$tunnel_pid"
