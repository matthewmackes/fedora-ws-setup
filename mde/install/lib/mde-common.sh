#!/usr/bin/env bash
# MDE common paths, version, and logging shims.
# shellcheck disable=SC2034

MDE_NAME="Mackes Desktop Environment"
MDE_SHORT="MDE"
MDE_VERSION="2026"
MDE_VERSION_FULL="2026.0"

# Install prefix (Fedora DE-like layout).
MDE_PREFIX="${MDE_PREFIX:-/usr}"
MDE_BIN="${MDE_BIN:-$MDE_PREFIX/bin}"
MDE_LIBEXEC="${MDE_LIBEXEC:-$MDE_PREFIX/libexec}"
MDE_SHARE="${MDE_SHARE:-$MDE_PREFIX/share/mde}"
MDE_DATA="${MDE_DATA:-$MDE_PREFIX/share}"
MDE_SYSCONF="${MDE_SYSCONF:-/etc}"
MDE_XDG="${MDE_XDG:-$MDE_SYSCONF/xdg/mde}"
MDE_STATE_DIR="${MDE_STATE_DIR:-/var/lib/mde}"
MDE_STATE_FILE="${MDE_STATE_FILE:-$MDE_STATE_DIR/state}"

# Resolve tree root (…/mde) from this file: install/lib/mde-common.sh
MDE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Profiles: minimal = shell only; workstation = shell + baseline.
MDE_PROFILE="${MDE_PROFILE:-workstation}"

mde_version_string() {
  printf '%s %s\n' "$MDE_SHORT" "$MDE_VERSION_FULL"
}
