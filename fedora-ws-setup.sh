#!/usr/bin/env bash
# Fedora Workstation Setup — Anaconda fullscreen installer (XFCE, MDE, or PiPedal)
set -euo pipefail

WS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/core.sh
source "$WS_ROOT/lib/core.sh"
# shellcheck source=lib/ui.sh
source "$WS_ROOT/lib/ui.sh"
# shellcheck source=lib/detect.sh
source "$WS_ROOT/lib/detect.sh"
# shellcheck source=lib/de.sh
source "$WS_ROOT/lib/de.sh"
# shellcheck source=lib/base.sh
source "$WS_ROOT/lib/base.sh"
# shellcheck source=lib/look.sh
source "$WS_ROOT/lib/look.sh"
# shellcheck source=lib/look_sway.sh
source "$WS_ROOT/lib/look_sway.sh"
# shellcheck source=lib/surface.sh
source "$WS_ROOT/lib/surface.sh"
# shellcheck source=lib/catalog.sh
source "$WS_ROOT/lib/catalog.sh"
# shellcheck source=lib/hardening.sh
source "$WS_ROOT/lib/hardening.sh"
# shellcheck source=lib/pipedal.sh
source "$WS_ROOT/lib/pipedal.sh"
# shellcheck source=lib/cli.sh
source "$WS_ROOT/lib/cli.sh"

usage() {
  cat <<EOF
Usage: sudo $0 [options] [cli|tui|xfce|mde|sway|pipedal]
       $0 --dry-run [options]

Options:
  --dry-run                  Print actions without changing the system (no root required)
  --noninteractive           Skip the installer; all users, no catalog upgrades, apply now
  --cli, --tui               Text installer (whiptail, or numbered prompts). Ten questions.
  --de=xfce|mde|pipedal   Desktop to install. MDE uses labwc. --de=sway and --de=miracle are rejected.
  --mde-profile=minimal|workstation
                             MDE install profile (default: workstation)
  --force-surface=laptop|pro Force Surface baseline (Laptop or Pro profile)
  --no-surface               Disable Surface detection/baseline
  --simulate-surface=laptop|pro
                             Dry-run only: pretend host is Surface Laptop/Pro
  -h, --help                 Show this help

Text installer examples:
  sudo $0 --cli
  sudo $0 pipedal
  sudo $0 --cli --de=mde --mde-profile=minimal
  WS_CLI=1 sudo $0

With no display, the text installer opens on its own. --noninteractive never prompts.
MDE is the standalone Mackes Desktop Environment under ./mde/.
EOF
}

WS_NONINTERACTIVE="${WS_NONINTERACTIVE:-0}"
WS_CLI="${WS_CLI:-0}"
WS_DUMP_UI=0
WS_APPLY_CHOICES=""

parse_args() {
  while (($#)); do
    case "$1" in
      --dry-run) WS_DRY_RUN=1 ;;
      --noninteractive) WS_NONINTERACTIVE=1 ;;
      --cli|--tui) WS_CLI=1 ;;
      --dump-ui) WS_DUMP_UI=1 ;;
      --apply-choices=*) WS_APPLY_CHOICES="${1#*=}" ;;
      --de=xfce|--de=mde|--de=pipedal) WS_DE_CLI="${1#*=}" ;;
      --de=sway|--de=miracle) die "--de=sway and --de=miracle have been removed. Use --de=mde (labwc)." ;;
      --de=*) die "Invalid --de (use xfce, mde, or pipedal)" ;;
      --mde-profile=minimal|--mde-profile=workstation) WS_MDE_PROFILE="${1#*=}" ;;
      --mde-profile=*) die "Invalid --mde-profile (use minimal or workstation)" ;;
      --force-surface=laptop|--force-surface=pro)
        WS_FORCE_SURFACE="${1#*=}"
        ;;
      --force-surface=*)
        die "Invalid --force-surface (use laptop or pro)"
        ;;
      --no-surface) WS_FORCE_NO_SURFACE=1 ;;
      --simulate-surface=laptop|--simulate-surface=pro)
        WS_SIMULATE_SURFACE="${1#*=}"
        ;;
      --simulate-surface=*)
        die "Invalid --simulate-surface (use laptop or pro)"
        ;;
      -h|--help) usage; exit 0 ;;
      cli|tui) WS_CLI=1 ;;
      sway|miracle) die "sway and miracle have been removed. Use mde (labwc)." ;;
      xfce|mde|pipedal)
        WS_CLI=1
        WS_DE_CLI="$1"
        ;;
      *) die "Unknown option: $1" ;;
    esac
    shift
  done
  if [[ -n "$WS_DE_CLI" ]]; then
    if ! WS_DE_CLI="$(de_canonical "$WS_DE_CLI")"; then
      die "--de=sway and --de=miracle have been removed. Use --de=mde (labwc)."
    fi
  fi
  if [[ -n "$WS_SIMULATE_SURFACE" && "$WS_DRY_RUN" != "1" ]]; then
    die "--simulate-surface requires --dry-run"
  fi
}

run_apply_choices() {
  local file="$1" dir
  [[ -n "$file" && -f "$file" ]] || die "Choices file not found: $file"
  dir="$(mktemp -d)"
  python3 - "$file" "$dir" <<'PY'
import json, pathlib, re, sys
src, dest = sys.argv[1:]
data = json.loads(pathlib.Path(src).read_text(encoding="utf-8"))
de = data.get("de", "")
if de in ("sway", "miracle"):
    raise SystemExit("sway and miracle have been removed; use mde")
if de not in ("xfce", "mde", "pipedal"):
    raise SystemExit("invalid desktop")
profile = data.get("mde_profile", "workstation")
if profile not in ("minimal", "workstation"):
    profile = "workstation"
user_re = re.compile(r"^[a-z_][a-z0-9_-]*\$?$")
item_re = re.compile(r"^[A-Za-z0-9_.:-]+$")
users = data.get("users") or []
items = data.get("items") or []
remove_items = data.get("remove_items") or []
uninstall_de = data.get("uninstall_de") or ""
if uninstall_de is True:
    uninstall_de = "all"
uninstall_de = str(uninstall_de).strip()
if uninstall_de not in ("", "xfce", "mde", "pipedal", "all"):
    raise SystemExit("invalid uninstall_de")
if any(not user_re.match(str(u)) for u in users):
    raise SystemExit("invalid user")
if any(not item_re.match(str(i)) for i in items):
    raise SystemExit("invalid item")
if any(not item_re.match(str(i)) for i in remove_items):
    raise SystemExit("invalid remove_items")
out = pathlib.Path(dest)
(out / "de").write_text(de + "\n", encoding="utf-8")
(out / "mde_profile").write_text(profile + "\n", encoding="utf-8")
(out / "users").write_text("".join(f"{u}\n" for u in users), encoding="utf-8")
(out / "items").write_text("".join(f"{i}\n" for i in items), encoding="utf-8")
(out / "remove_items").write_text("".join(f"{i}\n" for i in remove_items), encoding="utf-8")
(out / "uninstall_de").write_text(uninstall_de + "\n", encoding="utf-8")
PY
  WS_DE="$(tr -d '\n' <"$dir/de")"
  WS_MDE_PROFILE="$(tr -d '\n' <"$dir/mde_profile" 2>/dev/null || echo workstation)"
  WS_TARGET_USERS=()
  WS_SELECTED_ITEMS=()
  WS_REMOVE_ITEMS=()
  WS_UNINSTALL_DE="$(tr -d '\n' <"$dir/uninstall_de" 2>/dev/null || true)"
  if [[ -s "$dir/users" ]]; then
    mapfile -t WS_TARGET_USERS <"$dir/users"
  fi
  if [[ -s "$dir/items" ]]; then
    mapfile -t WS_SELECTED_ITEMS <"$dir/items"
  fi
  if [[ -s "$dir/remove_items" ]]; then
    mapfile -t WS_REMOVE_ITEMS <"$dir/remove_items"
  fi
  rm -rf "$dir"
  local filtered=() u local_u
  for u in "${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}"; do
    for local_u in "${WS_LOCAL_USERS[@]+"${WS_LOCAL_USERS[@]}"}"; do
      if [[ "$u" == "$local_u" ]]; then
        filtered+=("$u")
        break
      fi
    done
  done
  WS_TARGET_USERS=()
  if ((${#filtered[@]})); then
    WS_TARGET_USERS=("${filtered[@]}")
  fi
  de_mark_switch
  info "Apply choices: de=$WS_DE users=${WS_TARGET_USERS[*]:-none} items=${#WS_SELECTED_ITEMS[@]} remove=${#WS_REMOVE_ITEMS[@]} uninstall_de=${WS_UNINSTALL_DE:-none} switch=$WS_DE_SWITCH"
  wizard_apply
}

wizard_apply() {
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    pipedal_wizard_apply
    return 0
  fi
  info "=== APPLY START ==="
  if is_dry_run; then
    info "Dry-run apply — no system changes"
  fi

  local steps=(
    "Snapshot before changes"
    "Remove previous desktop"
    "Uninstall desktop experience"
    "Base desktop"
    "Core hardening"
    "Surface baseline"
    "Look / themes"
    "Remove selected catalog items"
    "Selected upgrades"
    "Snapshot after changes"
    "Save desktop state"
  )
  local i=0
  for i in "${!steps[@]}"; do
    local n=$((i + 1))
    ui_phase "$n" "${#steps[@]}" "${steps[i]}"
    info "APPLY STEP $n/${#steps[@]}: ${steps[i]}"
    case "$i" in
      0) harden_install_snapper; snapshot_create "ws-setup pre" ;;
      1) de_remove_other ;;
      2) de_uninstall_current ;;
      3) base_apply ;;
      4) hardening_apply ;;
      5) surface_apply ;;
      6) look_apply_all_users ;;
      7) catalog_remove_selected ;;
      8) cli_apply_extras; catalog_apply_selected ;;
      9) snapshot_create "ws-setup post" ;;
      10) de_save_state ;;
    esac
    ui_phase_done "${steps[i]}" 1
  done

  info "=== APPLY DONE applied=$WS_APPLIED skipped=$WS_SKIPPED failed=$WS_FAILED ==="
  ui_write_result
}

run_noninteractive() {
  de_resolve_noninteractive
  info "Noninteractive mode: all local users, no catalog upgrades, desktop=$WS_DE"
  WS_TARGET_USERS=("${WS_LOCAL_USERS[@]}")
  WS_SELECTED_ITEMS=()
  WS_REMOVE_ITEMS=()
  WS_UNINSTALL_DE=""
  info "$(preflight_text)"
  info "$(base_summary_text)"
  wizard_apply
  info "Failures: ${WS_FAILURES[*]:-none}"
  surface_mok_instructions || true
}

run_installer() {
  ui_prepare_display
  ui_ensure_setup_ui
  local model bin
  model="$(mktemp)"
  ui_write_model "$model"
  bin="$(ui_setup_ui_bin)" || die "mde-setup-ui binary missing after ensure"
  info "Opening the Rust setup UI ($bin)"
  # anaconda_ui.py is no longer the live graphical path (compile-model only).
  "$bin" --model "$model" --script "$WS_ROOT/fedora-ws-setup.sh"
  rm -f "$model"
}

main() {
  parse_args "$@"
  if [[ "$WS_DUMP_UI" == "1" ]]; then
    detect_all
    de_detect
    surface_apply_overrides
    ui_write_model /dev/stdout
    exit 0
  fi
  require_root
  detect_all
  de_detect
  surface_apply_overrides
  info "Starting (dry-run=$(is_dry_run && echo yes || echo no) noninteractive=$WS_NONINTERACTIVE de_present=$WS_DE_PRESENT)"
  if [[ -n "$WS_APPLY_CHOICES" ]]; then
    run_apply_choices "$WS_APPLY_CHOICES"
    exit 0
  fi
  if [[ "$WS_NONINTERACTIVE" == "1" ]]; then
    run_noninteractive
    exit 0
  fi
  if [[ "$WS_CLI" != "1" ]]; then
    if ! ui_find_display; then
      info "No graphical session. Opening the text installer."
      WS_CLI=1
    fi
  fi
  if [[ "$WS_CLI" == "1" ]]; then
    run_cli
    exit 0
  fi
  run_installer
}

main "$@"
