#!/usr/bin/env bash
# Core helpers: logging, dry-run wrappers, queue, dnf shim, package checks.
# shellcheck disable=SC2034

set -o pipefail

WS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WS_LOG_DIR="${WS_LOG_DIR:-$WS_ROOT/logs}"
WS_LOG_FILE="${WS_LOG_FILE:-$WS_LOG_DIR/ws-setup-$(date +%Y%m%d-%H%M%S).log}"
WS_DRY_RUN="${WS_DRY_RUN:-0}"
WS_QUEUE=()
WS_FAILURES=()
WS_APPLIED=0
WS_SKIPPED=0
WS_FAILED=0

mkdir -p "$WS_LOG_DIR"

# When WS_LOG_QUIET=1, write only to the log file (the Anaconda window owns the display).
WS_LOG_QUIET="${WS_LOG_QUIET:-0}"

log() {
  local level="$1"; shift
  local msg="$*"
  local ts line
  ts="$(date '+%Y-%m-%d %H:%M:%S')"
  line="$(printf '[%s] %-5s %s' "$ts" "$level" "$msg")"
  printf '%s\n' "$line" >> "$WS_LOG_FILE"
  if [[ "$WS_LOG_QUIET" != "1" ]]; then
    printf '%s\n' "$line" >&2
  fi
}

info()  { log INFO "$*"; }
warn()  { log WARN "$*"; }
error() { log ERROR "$*"; }
die()   { error "$*"; exit 1; }

is_dry_run() { [[ "$WS_DRY_RUN" == "1" ]]; }

# Run a command, or print it in dry-run mode. Returns command status.
run() {
  if is_dry_run; then
    info "DRY-RUN: $*"
    return 0
  fi
  info "RUN: $*"
  "$@"
  local rc=$?
  if (( rc != 0 )); then
    error "Command failed ($rc): $*"
  fi
  return $rc
}

# Like run, but records failure and continues (never aborts the wizard apply loop).
run_or_skip() {
  local label="$1"; shift
  if is_dry_run; then
    info "DRY-RUN: [$label] $*"
    WS_SKIPPED=$((WS_SKIPPED + 1))
    return 0
  fi
  info "APPLY: [$label] $*"
  if "$@"; then
    WS_APPLIED=$((WS_APPLIED + 1))
    return 0
  fi
  local rc=$?
  error "SKIP after failure ($rc): [$label] $*"
  WS_FAILURES+=("$label")
  WS_FAILED=$((WS_FAILED + 1))
  return 0
}

queue_add() {
  # queue_add "label" "kind" "payload..."
  local entry
  entry="$(printf '%s\0' "$@" | base64 -w0 2>/dev/null || printf '%s\0' "$@" | base64)"
  WS_QUEUE+=("$entry")
}

queue_count() { echo "${#WS_QUEUE[@]}"; }

queue_decode() {
  local entry="$1"
  printf '%s' "$entry" | base64 -d 2>/dev/null | tr '\0' '\n'
}

# --- dnf helpers -----------------------------------------------------------

dnf_major() {
  if command -v dnf5 >/dev/null 2>&1; then
    echo 5
  else
    echo 4
  fi
}

dnf_cmd() {
  if [[ "$(dnf_major)" == "5" ]]; then
    echo dnf5
  else
    echo dnf
  fi
}

dnf_yes() {
  local cmd
  cmd="$(dnf_cmd)"
  if is_dry_run; then
    info "DRY-RUN: $cmd -y $*"
    return 0
  fi
  info "DNF: $cmd -y $*"
  # stdin from /dev/null: dnf must not block on a hidden confirmation prompt.
  if [[ "${WS_LOG_QUIET:-0}" == "1" ]]; then
    "$cmd" -y "$@" </dev/null >>"$WS_LOG_FILE" 2>&1
  else
    "$cmd" -y "$@" </dev/null 2>&1 | tee -a "$WS_LOG_FILE" >&2
  fi
}

dnf_add_repo_file() {
  local url="$1"
  if [[ "$(dnf_major)" == "5" ]]; then
    dnf_yes config-manager addrepo --from-repofile="$url"
  else
    dnf_yes config-manager --add-repo="$url"
  fi
}

pkg_installed() {
  rpm -q "$1" >/dev/null 2>&1
}

pkgs_missing() {
  local p
  for p in "$@"; do
    pkg_installed "$p" || printf '%s\n' "$p"
  done
}

ensure_pkgs() {
  local label="$1"; shift
  local missing=()
  local p
  for p in "$@"; do
    if ! pkg_installed "$p"; then
      missing+=("$p")
    fi
  done
  if ((${#missing[@]} == 0)); then
    info "Already installed: $label"
    WS_SKIPPED=$((WS_SKIPPED + 1))
    return 0
  fi
  run_or_skip "$label" dnf_yes install "${missing[@]}"
}

# Flatpak helpers
flatpak_installed() {
  flatpak info "$1" >/dev/null 2>&1
}

ensure_flathub() {
  if flatpak remotes --system 2>/dev/null | grep -qw flathub; then
    return 0
  fi
  run_or_skip "flathub-remote" flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
}

ensure_flatpak() {
  local app_id="$1"
  if flatpak_installed "$app_id"; then
    info "Flatpak already installed: $app_id"
    WS_SKIPPED=$((WS_SKIPPED + 1))
    return 0
  fi
  ensure_flathub
  run_or_skip "flatpak:$app_id" flatpak install -y --noninteractive flathub "$app_id"
}

ensure_copr() {
  local project="$1"
  run_or_skip "copr:$project" dnf_yes copr enable "$project"
}

# Generic config runner: eval a function name safely from catalog
apply_config() {
  local fn="$1"
  if ! declare -F "$fn" >/dev/null 2>&1; then
    error "Config function missing: $fn"
    WS_FAILURES+=("config:$fn")
    WS_FAILED=$((WS_FAILED + 1))
    return 0
  fi
  if is_dry_run; then
    info "DRY-RUN: config $fn"
    return 0
  fi
  if "$fn"; then
    WS_APPLIED=$((WS_APPLIED + 1))
  else
    error "Config failed: $fn"
    WS_FAILURES+=("config:$fn")
    WS_FAILED=$((WS_FAILED + 1))
  fi
  return 0
}

require_root() {
  if is_dry_run; then
    return 0
  fi
  if [[ "$(id -u)" -ne 0 ]]; then
    die "This setup must be run as root (or with --dry-run)."
  fi
}
