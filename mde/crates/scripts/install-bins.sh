#!/usr/bin/env bash
# Copy release MDE bins into ~/.local/bin and, with sudo, /usr/bin.
# Interactive sudo (Cursor askpass or a TTY). Do not pass the password on the command line.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${CARGO_TARGET_DIR:-$ROOT/target-user}/release"
install -d "$HOME/.local/bin"
for b in mde-comp mde-shell mde-control mde-mmc mde-mmc-agent mde-session mde-privileged mde-neighbourhood mde-greeter mde-lock; do
  if [[ -x "$BIN/$b" ]]; then
    install -m 755 "$BIN/$b" "$HOME/.local/bin/$b"
  fi
done
if [[ -x "$BIN/mde-session" ]]; then
  install -m 755 "$BIN/mde-session" "$HOME/.local/bin/mde-session-rs"
fi
echo "Installed to $HOME/.local/bin"
if [[ "${MDE_INSTALL_SYSTEM:-}" == "1" ]]; then
  SUDO=(sudo)
  if [[ -n "${SUDO_ASKPASS:-}" ]]; then
    SUDO=(sudo -A)
  fi
  "${SUDO[@]}" install -m 755 "$BIN/mde-comp" /usr/bin/mde-comp
  "${SUDO[@]}" install -m 755 "$BIN/mde-shell" /usr/bin/mde-shell
  "${SUDO[@]}" install -m 755 "$BIN/mde-control" /usr/bin/mde-control
  if [[ -x "$BIN/mde-mmc" ]]; then
    "${SUDO[@]}" install -m 755 "$BIN/mde-mmc" /usr/bin/mde-mmc
  fi
  # greetd Exec=/usr/bin/mde-session — keep both names on the same Rust binary.
  "${SUDO[@]}" install -m 755 "$BIN/mde-session" /usr/bin/mde-session
  "${SUDO[@]}" install -m 755 "$BIN/mde-session" /usr/bin/mde-session-rs
  "${SUDO[@]}" install -m 755 "$BIN/mde-privileged" /usr/bin/mde-privileged
  if [[ -x "$BIN/mde-neighbourhood" ]]; then
    "${SUDO[@]}" install -m 755 "$BIN/mde-neighbourhood" /usr/bin/mde-neighbourhood
  fi
  if [[ -x "$BIN/mde-greeter" ]]; then
    "${SUDO[@]}" install -m 755 "$BIN/mde-greeter" /usr/bin/mde-greeter
  fi
  if [[ -x "$BIN/mde-lock" ]]; then
    "${SUDO[@]}" install -m 755 "$BIN/mde-lock" /usr/bin/mde-lock
  fi
  echo "Installed to /usr/bin"
fi
echo "Hot-restart chrome without dropping the seat: kill the mde-shell process only."
echo "mde-session-rs respawns it. Do not kill mde-session-rs or mde-comp."
