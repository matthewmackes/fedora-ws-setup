#!/usr/bin/env bash
# Compatibility shim: MDE look lives in mde/install/lib/look_mde.sh.
# fedora-ws-setup still sources this file; MDE installs go through mde/install.sh.
# shellcheck disable=SC1091
MDE_ROOT="${MDE_ROOT:-$WS_ROOT/mde}"
if [[ -f "$MDE_ROOT/install/lib/look_mde.sh" ]]; then
  # look_mde expects MDE paths for shell modules and /usr/share/mde installs.
  MDE_BIN="${MDE_BIN:-/usr/bin}"
  # shellcheck source=../mde/install/lib/look_mde.sh
  source "$MDE_ROOT/install/lib/look_mde.sh"
else
  warn "MDE look_mde.sh missing under $MDE_ROOT; Sway/MDE look helpers unavailable"
fi
