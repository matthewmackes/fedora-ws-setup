#!/usr/bin/env bash
# Install Neighbourhood Music binary and desktop entry.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
CRATE_DATA="$ROOT/mde/crates/mde-music/data"
DATA="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-/usr}"

TARGET_DIR="$ROOT/mde/crates/target"
if [[ ! -d "$TARGET_DIR/release" ]] || [[ ! -w "$TARGET_DIR/release" ]]; then
  TARGET_DIR="$ROOT/mde/crates/target-user"
fi
# Prefer an already-built release binary's target dir when present.
if [[ -x "$ROOT/mde/crates/target-user/release/neighbourhood-music" ]]; then
  TARGET_DIR="$ROOT/mde/crates/target-user"
elif [[ -x "$ROOT/mde/crates/target/release/neighbourhood-music" ]]; then
  TARGET_DIR="$ROOT/mde/crates/target"
fi

echo "Building Neighbourhood Music…"
(
  cd "$ROOT/mde/crates"
  CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p mde-music
)

install -D -m 0755 "$TARGET_DIR/release/neighbourhood-music" \
  "$PREFIX/bin/neighbourhood-music"

# Desktop entry must use an absolute Exec — MDE/Sway sessions often omit ~/.local/bin from PATH.
DESKTOP_SRC="$CRATE_DATA/neighbourhood-music.desktop"
DESKTOP_TMP="$(mktemp)"
sed -e "s|^Exec=.*|Exec=$PREFIX/bin/neighbourhood-music|" \
    -e "s|^TryExec=.*|TryExec=$PREFIX/bin/neighbourhood-music|" \
    "$DESKTOP_SRC" >"$DESKTOP_TMP"
install -D -m 0644 "$DESKTOP_TMP" \
  "$PREFIX/share/applications/neighbourhood-music.desktop"
rm -f "$DESKTOP_TMP"

install -D -m 0644 "$CRATE_DATA/style.css" \
  "$PREFIX/share/neighbourhood-music/style.css"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
fi

# Runtime deps hint (RPM/script consumers)
cat >"$DATA/REQUIRES.txt" <<'EOF'
gtk4
gstreamer1
gstreamer1-plugins-base
gstreamer1-plugins-good
gstreamer1-plugins-ugly-free
EOF

echo "Installed neighbourhood-music → $PREFIX/bin/neighbourhood-music"
