#!/bin/bash
set -euo pipefail

# Skip extraction if vendor directory already exists
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
