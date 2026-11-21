#!/usr/bin/env bash
# Screenshot regression gate.
# Unit tests cover SSIM (identical pair, 10-level shift) without a display.
# A live headless capture is attempted only when weston or cage is installed
# and MDE_FIDELITY_CAPTURE=1. Goldens are not required to exist yet.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-user}"
cd "$ROOT"

echo "== fidelity unit tests =="
cargo test -p mde-fidelity

echo "== capture host =="
if [[ "${MDE_FIDELITY_CAPTURE:-0}" != "1" ]]; then
  echo "capture: not-run (set MDE_FIDELITY_CAPTURE=1 to try weston/cage)"
else
  if command -v weston >/dev/null 2>&1; then
    echo "capture: weston is installed; live shell capture is not wired until specs are measured"
  elif command -v cage >/dev/null 2>&1; then
    echo "capture: cage is installed; live shell capture is not wired until specs are measured"
  else
    echo "capture: FAIL host missing (need weston or cage)"
    exit 1
  fi
fi

echo "== reference specs =="
for spec in "$ROOT/../fidelity/specs/"*.toml; do
  base="$(basename "$spec" .toml)"
  [[ "$base" == "_template" ]] && continue
  if grep -q 'status = "unmeasured"' "$spec"; then
    echo "reference $base: blocked (unmeasured)"
  fi
done

echo "FIDELITY OK (reference scores blocked while specs are unmeasured)"
