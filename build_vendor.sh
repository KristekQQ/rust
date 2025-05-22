#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────  nastavení  ─────────────────────────────
VENDOR_DIR="vendor"
ARCHIVE="vendor.tar.gz"
TARGET="wasm32-unknown-unknown"
GZIP_LEVEL="-9"                    # -1 = rychlé, -9 = nejmenší

# ───────────── 1) ověř, že wasm32 target už je v toolchainu ────────────
if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "❌  Target '$TARGET' není nainstalován."
  echo "   ➜ Spusť:  rustup target add $TARGET"
  exit 1
fi

# ───────────── 2) smaž starý vendor + archiv ───────────────────────────
rm -rf "$VENDOR_DIR" "$ARCHIVE"

# ───────────── 3) dočasně povol crates-io (odlož .cargo/config) ────────
CONFIG_MOVED=0
if [ -f .cargo/config.toml ]; then
  mv .cargo/config.toml .cargo/config.off
  CONFIG_MOVED=1
fi

# ───────────── 4) stáhni a ulož crate zdrojáky ─────────────────────────
echo "📦  cargo vendor → $VENDOR_DIR"
cargo vendor "$VENDOR_DIR"

# ───────────── 5) vrať offline config zpět ─────────────────────────────
if [ "$CONFIG_MOVED" -eq 1 ]; then
  mv .cargo/config.off .cargo/config.toml
fi

# ───────────── 6) zabal vendor do .tar.gz  (macOS, Linux, WSL) ─────────
echo "📦  balím $ARCHIVE"
if command -v pigz >/dev/null 2>&1; then
  # pigz existuje → paralelní komprese
  if tar --version 2>/dev/null | grep -q 'GNU tar'; then
    # GNU tar (Linux, WSL, gtar na macOS)
    tar --use-compress-program="pigz $GZIP_LEVEL" -cvf "$ARCHIVE" "$VENDOR_DIR"
  else
    # BSD tar (výchozí macOS) – použij pipe
    tar -cf - "$VENDOR_DIR" | pigz $GZIP_LEVEL > "$ARCHIVE"
  fi
else
  # fallback: klasický gzip (BSD i GNU tar)
  GZIP="$GZIP_LEVEL" tar -czvf "$ARCHIVE" "$VENDOR_DIR"
fi

echo "✅  hotovo – velikost: $(du -h "$ARCHIVE" | cut -f1)"

cat <<EOF

┌──────────────────────────────────────────────────────────────┐
│  Obnova na stroji BEZ internetu:                            │
│    tar -xzvf $ARCHIVE                                       │
│    cargo build --target $TARGET --release --offline         │
└──────────────────────────────────────────────────────────────┘
EOF
