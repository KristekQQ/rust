#!/usr/bin/env bash
set -euo pipefail

cmd="${1-}"
shift || true

case "$cmd" in
  join-toolchain)
    # Combine join_toolchain.sh contents
    RUSTUP_HOME="$PWD/.rustup"
    CARGO_HOME="$PWD/.cargo"
    export RUSTUP_HOME CARGO_HOME
    cat rustup_cache.part.* > rustup_cache.tar.gz
    mkdir -p "$RUSTUP_HOME"
    tar -xzf rustup_cache.tar.gz -C "$RUSTUP_HOME"
    cat cargo_cache.part.* > cargo_cache.tar.gz
    mkdir -p "$CARGO_HOME"
    tar -xzf cargo_cache.tar.gz -C "$CARGO_HOME"
    rustup toolchain link stable-offline "$RUSTUP_HOME/toolchains/stable-x86_64-unknown-linux-gnu"
    echo "✅ Rust toolchain unpacked for offline use"
    ;;
  build-vendor)
    # Combine build_vendor.sh contents
    VENDOR_DIR="vendor"
    ARCHIVE="vendor.tar.gz"
    TARGET="wasm32-unknown-unknown"
    GZIP_LEVEL="-9"
    if ! rustup target list --installed | grep -qx "$TARGET"; then
      echo "❌  Target '$TARGET' není nainstalován."
      echo "   ➜ Spusť:  rustup target add $TARGET"
      exit 1
    fi
    rm -rf "$VENDOR_DIR" "$ARCHIVE"
    CONFIG_MOVED=0
    if [ -f .cargo/config.toml ]; then
      mv .cargo/config.toml .cargo/config.off
      CONFIG_MOVED=1
    fi
    echo "📦  cargo vendor → $VENDOR_DIR"
    cargo vendor "$VENDOR_DIR"
    if [ "$CONFIG_MOVED" -eq 1 ]; then
      mv .cargo/config.off .cargo/config.toml
    fi
    echo "📦  balím $ARCHIVE"
    if command -v pigz >/dev/null 2>&1; then
      if tar --version 2>/dev/null | grep -q 'GNU tar'; then
        tar --use-compress-program="pigz $GZIP_LEVEL" -cvf "$ARCHIVE" "$VENDOR_DIR"
      else
        tar -cf - "$VENDOR_DIR" | pigz $GZIP_LEVEL > "$ARCHIVE"
      fi
    else
      GZIP="$GZIP_LEVEL" tar -czvf "$ARCHIVE" "$VENDOR_DIR"
    fi
    echo "✅  hotovo – velikost: $(du -h "$ARCHIVE" | cut -f1)"
    cat <<EOF2
┌──────────────────────────────────────────────────────────────┐
│  Obnova na stroji BEZ internetu:                            │
│    tar -xzvf $ARCHIVE                                       │
│    cargo build --target $TARGET --release --offline         │
└──────────────────────────────────────────────────────────────┘
EOF2
    ;;
  evendor)
    if [ -d "vendor" ]; then
      echo "vendor directory already exists"
      exit 0
    fi
    if [ -f vendor.tar.gz ]; then
      tar -xzf vendor.tar.gz
    elif [ -f vendor.tar.zst ]; then
      zstd -d vendor.tar.zst -c | tar -xv
    else
      echo "No vendor archive found"
      exit 1
    fi
    cargo vendor --sync ./vendor >/dev/null
    echo "Vendor directory prepared"
    ;;
  ci-setup)
    export RUSTUP_HOME="$PWD/.rustup"
    export CARGO_HOME="$PWD/.cargo"
    cat rustup_cache.part.* | tar -xz
    cat cargo_cache.part.* | tar -xz
    OFFLINE_NAME="stable-offline"
    OFFLINE_PATH="$RUSTUP_HOME/toolchains/stable-x86_64-unknown-linux-gnu"
    if ! rustup toolchain list | grep -q "$OFFLINE_NAME"; then
      rustup toolchain link "$OFFLINE_NAME" "$OFFLINE_PATH"
    fi
    ./offline.sh evendor
    export RUSTUP_TOOLCHAIN="$OFFLINE_NAME"
    cargo test --offline --target x86_64-unknown-linux-gnu
    ;;
  *)
    echo "Usage: $0 {join-toolchain|build-vendor|evendor|ci-setup}"
    exit 1
    ;;
esac
