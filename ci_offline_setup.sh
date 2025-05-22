#!/usr/bin/env bash
set -euo pipefail
export RUSTUP_HOME="$PWD/.rustup"
export CARGO_HOME="$PWD/.cargo"

# Rozbal toolchain
[ -d "$RUSTUP_HOME" ] || {
  mkdir -p "$RUSTUP_HOME"
  tar -xzf rustup_cache.tar.gz -C "$RUSTUP_HOME"
}
[ -d "$CARGO_HOME" ]  || {
  mkdir -p "$CARGO_HOME"
  tar -xzf cargo_cache.tar.gz -C "$CARGO_HOME"
}

# Rozbal crate vendor
tar -xzvf vendor.tar.gz

# Pro jistotu zaregistruj toolchain pod jménem "offline"
rustup toolchain link offline "$RUSTUP_HOME/toolchains/stable-x86_64-unknown-linux-gnu"

# Sestav projekt čistě offline
cargo +offline build --target wasm32-unknown-unknown --release --offline
