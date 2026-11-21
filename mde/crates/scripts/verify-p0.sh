#!/usr/bin/env bash
# Automate the subset of WORKLIST P0 items that do not need a human VT1 click-pass.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${CARGO_TARGET_DIR:-$ROOT/target-user}/release"
LOCAL="$HOME/.local/bin"
fail=0

ok() { echo "OK  $*"; }
bad() { echo "FAIL $*"; fail=1; }

echo "== P0 automated checks =="

# V12 — XWayland off unless enabled
CFG="${XDG_CONFIG_HOME:-$HOME/.config}/mde/config.toml"
if [[ -f "$CFG" ]] && grep -qiE 'xwayland\s*=\s*true' "$CFG"; then
  bad "V12 config has xwayland=true (expected false for default)"
else
  ok "V12 xwayland not enabled in config (or no config yet)"
fi

# V13 — launchers on PATH
for c in neighbourhood-music mde-neighbourhood regedit ws-settings mde-control mde-music; do
  if command -v "$c" >/dev/null 2>&1; then
    ok "V13 launcher $c"
  else
    bad "V13 missing launcher $c"
  fi
done

# V14 — session desktop + greetd/seatd
if [[ -f /usr/share/wayland-sessions/mde.desktop ]] && grep -q 'Exec=mde-session' /usr/share/wayland-sessions/mde.desktop; then
  ok "V14 mde.desktop → mde-session"
else
  bad "V14 mde.desktop missing or wrong Exec"
fi
if systemctl is-enabled greetd >/dev/null 2>&1 && systemctl is-enabled seatd >/dev/null 2>&1; then
  ok "V14 greetd + seatd enabled"
else
  bad "V14 greetd/seatd not both enabled"
fi

# Bins + lock PAM selftest
for b in mde-comp mde-shell mde-lock mde-greeter mde-control; do
  if [[ -x "$LOCAL/$b" ]] || [[ -x "$BIN/$b" ]]; then
    ok "bin $b present"
  else
    bad "bin $b missing"
  fi
done
LOCK="${LOCAL}/mde-lock"
[[ -x "$LOCK" ]] || LOCK="${BIN}/mde-lock"
if [[ -x "$LOCK" ]]; then
  out="$("$LOCK" --selftest)"
  echo "$out" | grep -q '"ok":true' && echo "$out" | grep -q 'pam-login' \
    && ok "mde-lock PAM selftest" \
    || bad "mde-lock selftest: $out"
fi

# Live stack (informational)
if pgrep -f 'mde-comp --session' >/dev/null; then
  ok "live mde-comp --session"
  if readlink "/proc/$(pgrep -f 'mde-comp --session' | head -1)/exe" 2>/dev/null | grep -q '(deleted)'; then
    echo "NOTE compositor is still an old (deleted) binary — re-login to load snap/workspace/lock"
  fi
else
  echo "NOTE mde-comp not running (no live seat)"
fi
pgrep -x mde-shell >/dev/null && ok "live mde-shell" || echo "NOTE mde-shell not running"

echo
echo "V1–V11 interactive chrome/Settings still need a human VT1 pass (see WORKLIST.md)."
if [[ "$fail" -ne 0 ]]; then
  echo "VERIFY-P0 FAILED"
  exit 1
fi
echo "VERIFY-P0 OK (automated subset)"
