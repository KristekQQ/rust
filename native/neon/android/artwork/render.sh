#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
render_tmp="$(mktemp -d)"
trap 'rm -rf "$render_tmp"' EXIT
qlmanage -t -s 512 -o "$render_tmp" "$here/app-icon.svg" >/dev/null
qlmanage -t -s 1024 -o "$render_tmp" "$here/app-banner.svg" >/dev/null
cp "$render_tmp/app-icon.svg.png" "$here/../res/drawable/app_icon.png"
sips --cropToHeightWidth 512 1024 "$render_tmp/app-banner.svg.png" --out "$here/../res/drawable/app_banner.png" >/dev/null
