#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $# -ne 0 && $# -ne 2 ]]; then
    echo "Usage: $0 [certificate.pem private-key.pem]" >&2
    exit 2
fi
# Never serve whichever dev/release binary happens to be left in pkg/.
bash scripts/build-release.sh
lan_ip="${QUEST_HOST:-$(ipconfig getifaddr en0 2>/dev/null || true)}"
[[ -n "$lan_ip" ]] || { echo 'Set QUEST_HOST to the Mac LAN IP.' >&2; exit 1; }
port="${QUEST_PORT:-8443}"
if [[ $# -eq 0 ]]; then
    scripts/setup-quest-tls.sh "$lan_ip"
    cert="$HOME/.local/share/rust-webxr/tls/server.crt"
    key="$HOME/.local/share/rust-webxr/tls/server.key"
else
    cert="$1"
    key="$2"
fi
printf 'Production release: https://%s:%s/examples/neon-arena/?backend=auto\n' "$lan_ip" "$port"
echo 'No tunnel is started. Quest must be on a reachable local network.'
echo 'Confirm the local certificate on the headset, or trust the development CA there.'
echo 'Press Ctrl+C to stop.'
exec python3 scripts/serve-local.py "$port" --host "$lan_ip" --cert "$cert" --key "$key" --production --directory output/production
