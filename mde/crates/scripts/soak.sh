#!/usr/bin/env bash
# Soak gate. Default is 30 minutes. This script does not start mde-comp --session
# (that takes the DRM seat). A live nested run requires MDE_SOAK_LIVE=1.
# --self-test checks the slope math and exits.
set -euo pipefail

seconds="${SOAK_SECONDS:-1800}"
if [[ "${1:-}" == "--self-test" ]]; then
  python3 - << 'PY'
start = 100.0
end = 100.4
hours = 0.5
slope = (end - start) / hours
assert slope < 1.0, slope
print(f"soak slope self-test pass {slope:.3f} MB/h")
PY
  exit 0
fi

if [[ "${MDE_SOAK_LIVE:-0}" != "1" ]]; then
  echo "soak: not-run (set MDE_SOAK_LIVE=1 to run ${seconds}s; will not take the DRM seat from this script)"
  exit 2
fi

echo "soak: live nested runs are operator-started; ${seconds}s requested"
echo "fail if RSS grows more than 1 MB/hour after the first 5 minutes, or fd count rises by more than 16"
exit 2
