#!/usr/bin/env bash
set -euo pipefail

# ----------------------------------------------------------------------
# Nastavení
# ----------------------------------------------------------------------
VENDOR_DIR="vendor"
ARCHIVE="vendor.tar.gz"
CARGO_FLAGS=(--locked)     # přidej např. --offline, pokud chceš
GZIP_LEVEL="-9"            # -1 rychlejší, -9 nejmenší archiv

TARGET="wasm32-unknown-unknown"

# ----------------------------------------------------------------------
# Kontrola targetu
# ----------------------------------------------------------------------
if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "❌  Target '$TARGET' není nainstalován."
  echo "    Spusť jednorázově:  rustup target add $TARGET"
  exit 1
fi

# ----------------------------------------------------------------------
# 1) Vyčisti staré artefakty
# ----------------------------------------------------------------------
if [ -d "$VENDOR_DIR" ]; then
  echo "🧹   Removing existing '$VENDOR_DIR' directory"
  rm -rf "$VENDOR_DIR"
fi
rm -f "$ARCHIVE"

# ----------------------------------------------------------------------
# 2) Vygeneruj nový vendor z Cargo.lock
# ----------------------------------------------------------------------
echo "📦  Running 'cargo vendor' → $VENDOR_DIR"
cargo vendor "${CARGO_FLAGS[@]}" "$VENDOR_DIR" | sed 's/^/    /'

# ----------------------------------------------------------------------
# 3) Zabal do gzipu
# ----------------------------------------------------------------------
echo "📦  Creating $ARCHIVE (gzip ${GZIP_LEVEL/-/})"
if command -v pigz >/dev/null 2>&1; then
  # paralelní komprese, pokud je pigz k dispozici
  tar -I "pigz $GZIP_LEVEL" -cvf "$ARCHIVE" "$VENDOR_DIR"
else
  # standardní gzip s nastavenou úrovní
  tar -I "gzip $GZIP_LEVEL" -cvf "$ARCHIVE" "$VENDOR_DIR"
fi

echo "✅  $ARCHIVE ready ($(du -h "$ARCHIVE" | cut -f1))"

# ----------------------------------------------------------------------
# 4) Krátký návod k obnově (offline)
# ----------------------------------------------------------------------
cat <<EOF

📝  Obnovení na stroji bez internetu:
     tar -xzvf $ARCHIVE
     cargo build --offline

EOF
