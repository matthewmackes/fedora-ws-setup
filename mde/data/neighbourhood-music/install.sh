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

install_prefix() {
  local prefix="$1"
  local bin_dir="$prefix/bin"
  if [[ ! -d "$bin_dir" ]] && ! mkdir -p "$bin_dir" 2>/dev/null; then
    echo "Skipping $prefix (not writable)"
    return 0
  fi
  if [[ ! -w "$bin_dir" ]]; then
    echo "Skipping $prefix (not writable)"
    return 0
  fi
  install -D -m 0755 "$TARGET_DIR/release/neighbourhood-music" \
    "$prefix/bin/neighbourhood-music"
  local desktop_tmp
  desktop_tmp="$(mktemp)"
  sed -e "s|^Exec=.*|Exec=$prefix/bin/neighbourhood-music|" \
      -e "s|^TryExec=.*|TryExec=$prefix/bin/neighbourhood-music|" \
      "$CRATE_DATA/neighbourhood-music.desktop" >"$desktop_tmp"
  install -D -m 0644 "$desktop_tmp" \
    "$prefix/share/applications/neighbourhood-music.desktop"
  rm -f "$desktop_tmp"
  install -D -m 0644 "$CRATE_DATA/style.css" \
    "$prefix/share/neighbourhood-music/style.css"
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$prefix/share/applications" 2>/dev/null || true
  fi
  echo "Installed neighbourhood-music → $prefix/bin/neighbourhood-music"
}

# The requested prefix, plus the other one when this process can write it.
# ~/.local/bin is ahead of /usr/bin on PATH, so the two copies must match.
install_prefix "$PREFIX"
if [[ "$PREFIX" == /usr && -n "${SUDO_USER:-}" ]]; then
  user_home="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
  if [[ -n "$user_home" ]]; then
    install_prefix "$user_home/.local"
  fi
elif [[ "$PREFIX" != /usr ]]; then
  install_prefix /usr
fi

# Runtime deps hint (RPM/script consumers)
cat >"$DATA/REQUIRES.txt" <<'EOF'
gtk4
gstreamer1
gstreamer1-plugins-base
gstreamer1-plugins-good
gstreamer1-plugins-ugly-free
EOF

echo "Release artifact: $TARGET_DIR/release/neighbourhood-music"
