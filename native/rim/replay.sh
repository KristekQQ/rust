#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cargo build --locked --manifest-path "$here/Cargo.toml" --bin rim-preview
if [[ "$(uname -s)" == Darwin ]]; then
  vulkan_dir="${VULKAN_LIB_DIR:-$repo/output/native-tools/MoltenVK/MoltenVK/dynamic/dylib/macOS}"
  [[ -f "$vulkan_dir/libvulkan.dylib" ]] || { printf 'Set VULKAN_LIB_DIR to a Vulkan/MoltenVK loader directory.\n' >&2; exit 1; }
  export DYLD_LIBRARY_PATH="$vulkan_dir${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
fi
exec "$here/target/debug/rim-preview" --output "$repo/output/native-rim" "$@"
