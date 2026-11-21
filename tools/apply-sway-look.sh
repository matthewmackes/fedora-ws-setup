#!/usr/bin/env bash
# Apply / re-apply MDE (formerly the Sway look) without the full installer.
# Usage: sudo tools/apply-sway-look.sh [--dry-run] [--profile=minimal|workstation] [user...]
set -euo pipefail

WS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MDE_ROOT="${WS_MDE_ROOT:-$WS_ROOT/mde}"
PROFILE=workstation
ARGS=()
USERS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) ARGS+=(--dry-run) ;;
    --profile=minimal|--profile=workstation) PROFILE="${1#*=}" ;;
    --profile)
      shift
      PROFILE="${1:-workstation}"
      ;;
    -h|--help)
      cat <<EOF
Usage: sudo $0 [--dry-run] [--profile=minimal|workstation] [user...]
Installs or re-applies Mackes Desktop Environment from $MDE_ROOT.
EOF
      exit 0
      ;;
    *) USERS+=("$1") ;;
  esac
  shift
done

[[ -x "$MDE_ROOT/install.sh" ]] || {
  echo "MDE installer not found: $MDE_ROOT/install.sh" >&2
  exit 1
}

ARGS+=(--profile="$PROFILE")
if ((${#USERS[@]})); then
  ARGS+=("${USERS[@]}")
fi
exec "$MDE_ROOT/install.sh" "${ARGS[@]}"
