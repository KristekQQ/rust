#!/usr/bin/env bash
set -euo pipefail

# Directory with rustup and cargo data (allow overriding via env)
RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

# Pack rustup directory
if [ -d "$RUSTUP_HOME" ]; then
  echo "Creating rustup cache archive..."
  tar -czf rustup_cache.tar.gz -C "$RUSTUP_HOME" .
  split -b 100m -d -a 2 rustup_cache.tar.gz rustup_cache.part.
  rm rustup_cache.tar.gz
else
  echo "Rustup directory '$RUSTUP_HOME' not found" >&2
  exit 1
fi

# Pack cargo directory
if [ -d "$CARGO_HOME" ]; then
  echo "Creating cargo cache archive..."
  tar -czf cargo_cache.tar.gz -C "$CARGO_HOME" .
  split -b 100m -d -a 2 cargo_cache.tar.gz cargo_cache.part.
  rm cargo_cache.tar.gz
else
  echo "Cargo directory '$CARGO_HOME' not found" >&2
  exit 1
fi

echo "✅ Cache archives created: rustup_cache.part.* and cargo_cache.part.*"
