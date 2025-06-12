#!/usr/bin/env bash
set -euo pipefail

# Use local cache directories so we can run completely offline.
export RUSTUP_HOME="$PWD/.rustup"
export CARGO_HOME="$PWD/.cargo"

# Assemble cached toolchain and unpack vendored crates.  This relies on
# the helper script `offline.sh` so all offline setup logic lives in one
# place.
"$(dirname "$0")/offline.sh" unpack-all

# Run the test suite using the offline toolchain.
export RUSTUP_TOOLCHAIN="stable-offline"
cargo test --offline --target x86_64-unknown-linux-gnu

