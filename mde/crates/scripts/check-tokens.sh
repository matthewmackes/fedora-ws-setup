#!/usr/bin/env bash
# Token gate.
#   check-tokens.sh            full scan of shell, control, greeter, files
#   check-tokens.sh --theme    theme crate only (T1). Passes while surfaces
#                              still use literal sizes; those literals are the
#                              surface cards, which stay blocked until specs
#                              are measured.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

mode="${1:-all}"
fail=0

if [[ "$mode" == "--theme" ]]; then
  if grep -n '#202020' mde-theme/src/lib.rs | grep -v 'assert_eq!' >/dev/null; then
    echo "theme: lib.rs still hardcodes #202020; palette must come from tokens"
    exit 1
  fi
  if [[ ! -f mde-theme/src/color.rs ]]; then
    echo "theme: color.rs missing"
    exit 1
  fi
  echo "check-tokens: theme crate ok"
  exit 0
fi

scan() {
  local dir="$1"
  [[ -d "$dir" ]] || return 0
  if command -v rg >/dev/null 2>&1; then
    if rg -n --glob '!**/target/**' 'Color::from_rgb|Color::from_rgba|\.size\([0-9]' "$dir"; then
      fail=1
    fi
  else
    if grep -R -n -E 'Color::from_rgb|Color::from_rgba|\.size\([0-9]' "$dir"; then
      fail=1
    fi
  fi
}

scan mde-shell/src
scan mde-control/src
scan mde-greeter/src
if [[ -d mde-files/src ]]; then
  scan mde-files/src
fi

if [[ "$fail" -ne 0 ]]; then
  echo "check-tokens: literal colors or text sizes remain outside mde-theme"
  exit 1
fi
echo "check-tokens: ok"
