#!/usr/bin/env bash
# Collects Rust source files, shader files, and the Cargo manifest into a
# single text file for easy copy&paste into other tools like GPT.

set -euo pipefail

OUTPUT_FILE="all_sources.txt"

# Start fresh
: > "$OUTPUT_FILE"

echo "# Combined Cargo.toml" >> "$OUTPUT_FILE"
cat Cargo.toml >> "$OUTPUT_FILE"

# Find all source and shader files under src/
find src -type f \( -name '*.rs' -o -name '*.wgsl' -o -name '*.vert' -o -name '*.frag' \) | sort | while read -r file; do
    echo "\n# File: $file" >> "$OUTPUT_FILE"
    cat "$file" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
done

echo "Sources written to $OUTPUT_FILE"
