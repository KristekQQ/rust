#!/usr/bin/env bash
# Collects Rust source files, shader files, and the Cargo manifest into a
# single text file for easy copy&paste into other tools like GPT.

set -euo pipefail
cd "$(dirname "$0")/.."
OUTPUT_FILE="all_sources.txt"
temporary="$(mktemp "${OUTPUT_FILE}.XXXXXX")"
trap 'rm -f "$temporary"' EXIT

# Start fresh
: > "$temporary"

printf '# Combined Cargo.toml\n' >> "$temporary"
cat Cargo.toml >> "$temporary"

# Find all source and shader files under src/
find src -type f \( -name '*.rs' -o -name '*.wgsl' -o -name '*.vert' -o -name '*.frag' \) | sort | while IFS= read -r file; do
    printf '\n# File: %s\n' "$file" >> "$temporary"
    cat "$file" >> "$temporary"
    printf '\n' >> "$temporary"
done
mv "$temporary" "$OUTPUT_FILE"
echo "Sources written to $OUTPUT_FILE"
