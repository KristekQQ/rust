#!/usr/bin/env bash
set -euo pipefail

TARGET="wasm32-unknown-unknown"
KEEP=0
if [[ "${1:-}" == "--keep" ]]; then
  KEEP=1
fi

RUST_VERSION=$(rustc -V | cut -d' ' -f2)
HOST_TRIPLE=$(rustup show active-toolchain | awk '{print $1}' | cut -d'-' -f2-)

# Check target installation
if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "Target '$TARGET' not installed. Run: rustup target add $TARGET" >&2
  exit 1
fi

# Clean previous artefacts
rm -rf vendor vendor.zip vendor.z?? vendor_full.zip vendor.zip.*

# Temporarily move Cargo config to allow network access
CFG_MOVED=0
if [ -f .cargo/config.toml ]; then
  mv .cargo/config.toml .cargo/config.toml.bak
  CFG_MOVED=1
fi

# Vendor crates
cargo vendor vendor/

# Restore Cargo config
if [ "$CFG_MOVED" -eq 1 ]; then
  mv .cargo/config.toml.bak .cargo/config.toml
fi

STD_ARCHIVE="rust-std-wasm32-unknown-unknown-${RUST_VERSION}-${HOST_TRIPLE}.tar.xz"
URL="https://static.rust-lang.org/dist/${STD_ARCHIVE}"

mkdir -p vendor
echo "Downloading $URL"
if ! curl -fL --progress-bar "$URL" -o "vendor/${STD_ARCHIVE}"; then
  echo "Failed to download $URL" >&2
  exit 1
fi

# Create split zip archive
zip -r -s 50m vendor.zip vendor

# Verify sizes <=50 MiB
for f in vendor.zip vendor.z*; do
  [ -f "$f" ] || continue
  size=$(stat -c%s "$f")
  if [ "$size" -gt $((50*1024*1024)) ]; then
    echo "$f exceeds 50 MiB" >&2
    exit 1
  fi
done

# Show archive sizes and reminder
du -h vendor.zip*
echo "Commit only vendor.zip and vendor.z0*; ignore raw vendor/"

if [ "$KEEP" -ne 1 ]; then
  rm -rf vendor
fi

cat <<'EOT'
unzip:  zip -s 0 vendor.zip --out vendor_full.zip && unzip vendor_full.zip
rustup: tar -xf vendor/rust-std*.tar.xz -C /tmp/std && \
        rustup toolchain link wasm-offline /tmp/std/rust-std-* && \
        rustup override set wasm-offline
cargo:  cargo check --target wasm32-unknown-unknown --offline
EOT
