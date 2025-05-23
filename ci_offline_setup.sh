#!/usr/bin/env bash
set -euo pipefail
export RUSTUP_HOME="$PWD/.rustup"
export CARGO_HOME="$PWD/.cargo"

# ── složit cache z chunků ─────────────────────────────────────────────
cat rustup_cache.part.* | tar -xz
cat cargo_cache.part.* | tar -xz

# ── link offline toolchain pod jménem stable-offline ───────────────────
OFFLINE_NAME="stable-offline"
OFFLINE_PATH="$RUSTUP_HOME/toolchains/stable-x86_64-unknown-linux-gnu"
if ! rustup toolchain list | grep -q "$OFFLINE_NAME"; then
  rustup toolchain link "$OFFLINE_NAME" "$OFFLINE_PATH"
fi

# ── rozbal vendored crates ─────────────────────────────────────────────
tar -xzvf vendor.tar.gz

# ── spustit testy s offline toolchainem ────────────────────────────────
export RUSTUP_TOOLCHAIN="$OFFLINE_NAME"
cargo test --offline --target x86_64-unknown-linux-gnu

# (volitelné) formátování – funguje jen pokud máš rustfmt v cache
# cargo fmt --all -- --check

