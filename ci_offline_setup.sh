#!/usr/bin/env bash
set -euo pipefail
export RUSTUP_HOME="$PWD/.rustup"
export CARGO_HOME="$PWD/.cargo"

# Rozbal toolchain
[ -d "$RUSTUP_HOME" ] || tar -xzf rustup_cache.tar.gz
[ -d "$CARGO_HOME" ]  || tar -xzf cargo_cache.tar.gz

# Rozbal crate vendor
tar -xzvf vendor.tar.gz

# Pro jistotu zaregistruj toolchain pod jménem "offline"
rustup toolchain link offline "$RUSTUP_HOME/toolchains/stable-x86_64-unknown-linux-gnu"

# Testuj čistě offline
cargo +offline test --target wasm32-unknown-unknown --offline
