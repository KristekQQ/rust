#!/bin/bash
set -euo pipefail

# Extract vendored crates from the archived vendor.tar.zst file
# Requires `zstd` to be installed.

# Skip extraction if vendor directory already exists
if [ -d "vendor" ]; then
  echo "vendor directory already exists"
  exit 0
fi

# Decompress the archive and extract it
zstd -d vendor.tar.zst -c | tar -xv

# Synchronize Cargo's metadata with the vendor directory
cargo vendor --sync ./vendor >/dev/null

echo "Vendor directory prepared"
