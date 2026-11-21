#!/usr/bin/env bash
# Uninstall MDE. Forwards to install.sh --uninstall.
# Usage: sudo ./uninstall.sh [--dry-run] [--purge-config] [user...]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$ROOT/install.sh" --uninstall "$@"
