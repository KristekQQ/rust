#!/usr/bin/env bash
set -euo pipefail

RUSTUP_HOME="$PWD/.rustup"
CARGO_HOME="$PWD/.cargo"
export RUSTUP_HOME CARGO_HOME

# ── Slož a rozbal rustup cache ─────────────────────────────────────────
cat rustup_cache.part.* > rustup_cache.tar.gz
mkdir -p "$RUSTUP_HOME"
tar -xzf rustup_cache.tar.gz -C "$RUSTUP_HOME"

# ── Slož a rozbal cargo cache ──────────────────────────────────────────
cat cargo_cache.part.* > cargo_cache.tar.gz
mkdir -p "$CARGO_HOME"
tar -xzf cargo_cache.tar.gz -C "$CARGO_HOME"

# ── (volitelné) zaregistruj toolchain pod jménem stable-offline ────────
rustup toolchain link stable-offline "$RUSTUP_HOME/toolchains/stable-x86_64-unknown-linux-gnu"

echo "✅ Rust toolchain a cache složeny (offline ready)"
