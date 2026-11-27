#!/usr/bin/env bash
# Install Network Neighbourhood binaries and system integration.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
CRATE="$ROOT/mde/crates/mde-network-neighbourhood"
DATA="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-/usr}"

TARGET_DIR="$ROOT/mde/crates/target"
# Prefer a user-writable target when release/ is root-owned from a prior sudo build.
if [[ ! -w "$TARGET_DIR/release" ]]; then
  TARGET_DIR="$ROOT/mde/crates/target-user"
fi

echo "Building Network Neighbourhood…"
(
  cd "$ROOT/mde/crates"
  CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p mde-network-neighbourhood
)

install -D -m 0755 "$TARGET_DIR/release/network-neighbourhood" \
  "$PREFIX/bin/network-neighbourhood"
install -D -m 0755 "$TARGET_DIR/release/network-neighbourhoodd" \
  "$PREFIX/bin/network-neighbourhoodd"

install -D -m 0644 "$DATA/network-neighbourhood.desktop" \
  "$PREFIX/share/applications/network-neighbourhood.desktop"
install -D -m 0644 "$DATA/network-neighbourhoodd.service" \
  /etc/systemd/system/network-neighbourhoodd.service
install -D -m 0644 "$DATA/org.mde.NetworkNeighbourhood1.conf" \
  /etc/dbus-1/system.d/org.mde.NetworkNeighbourhood1.conf
install -D -m 0644 "$DATA/org.mde.NetworkNeighbourhood1.policy" \
  /usr/share/polkit-1/actions/org.mde.NetworkNeighbourhood1.policy

mkdir -p /etc/network-neighbourhood/creds
chmod 0755 /etc/network-neighbourhood
chmod 0700 /etc/network-neighbourhood/creds
mkdir -p /var/lib/network-neighbourhood
chmod 0755 /var/lib/network-neighbourhood
if [[ ! -f /etc/network-neighbourhood/places.json ]]; then
  install -m 0644 "$DATA/places.json" /etc/network-neighbourhood/places.json
fi
# Session apps (Music) need to list places; secrets stay in creds/.
chmod 0644 /etc/network-neighbourhood/places.json 2>/dev/null || true

mkdir -p /mnt/network/{SMB,SSH,NFS,WebDAV,Video,Audio}

# Allow non-root clients (Files) to access FUSE mounts created by the daemon.
for fuse_conf in /etc/fuse.conf /etc/fuse3.conf; do
  if [[ -f "$fuse_conf" ]] && ! grep -q '^user_allow_other' "$fuse_conf"; then
    if grep -q '^#user_allow_other' "$fuse_conf"; then
      sed -i 's/^#user_allow_other/user_allow_other/' "$fuse_conf"
    else
      echo 'user_allow_other' >> "$fuse_conf"
    fi
  fi
done
# sshpass is required for reliable SSH password mounts (sshfs password_stdin is flaky).
if ! command -v sshpass >/dev/null 2>&1; then
  dnf install -y sshpass >/dev/null 2>&1 || true
fi

# File manager “Network” bookmark (Windows 11–style sidebar entry) for existing users.
bookmark_line="file:///mnt/network Network"
for home in /home/*; do
  [[ -d "$home" ]] || continue
  gtk="$home/.config/gtk-3.0"
  mkdir -p "$gtk"
  bookmarks="$gtk/bookmarks"
  touch "$bookmarks"
  if ! grep -qxF "$bookmark_line" "$bookmarks" 2>/dev/null; then
    echo "$bookmark_line" >> "$bookmarks"
  fi
  # Thunar also honours GTK bookmarks; Nautilus/Nemo likewise.
  user="$(basename "$home")"
  chown -R "$user:$user" "$gtk" 2>/dev/null || true
done

systemctl daemon-reload
systemctl enable --now network-neighbourhoodd.service
dbus-send --system --type=method_call --dest=org.freedesktop.DBus \
  /org/freedesktop/DBus org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true

echo "Network Neighbourhood installed."
