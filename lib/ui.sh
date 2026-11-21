#!/usr/bin/env bash
# Anaconda fullscreen installer launch helpers.
# shellcheck disable=SC2034

WS_UI_BACK=254
WS_UI_ESC=255

ui_find_display() {
  local home uid runtime sock
  if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" && -n "${SUDO_USER:-}" ]]; then
    uid="$(id -u "$SUDO_USER")"
    home="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
    runtime="/run/user/$uid"
    if [[ -S /tmp/.X11-unix/X0 ]]; then
      export DISPLAY=:0
    fi
    if [[ -z "${XAUTHORITY:-}" && -n "$home" && -f "$home/.Xauthority" ]]; then
      export XAUTHORITY="$home/.Xauthority"
    fi
    if [[ -z "${WAYLAND_DISPLAY:-}" && -d "$runtime" ]]; then
      for sock in "$runtime"/wayland-*; do
        [[ -S "$sock" ]] || continue
        export WAYLAND_DISPLAY="${sock##*/}"
        export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$runtime}"
        break
      done
    fi
    if [[ -n "${XDG_RUNTIME_DIR:-}" && -z "${DBUS_SESSION_BUS_ADDRESS:-}" && -S "$XDG_RUNTIME_DIR/bus" ]]; then
      export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
    fi
  fi
  [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]
}

ui_prepare_display() {
  ui_find_display || die "The Anaconda installer needs a graphical session. On a text console run: $WS_ROOT/fedora-ws-setup.sh --cli"
  if [[ -n "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" && -n "${SUDO_USER:-}" ]]; then
    runuser -u "$SUDO_USER" -- xhost +si:localuser:root >/dev/null 2>&1 || true
  fi
}

ui_have_anaconda() {
  python3 -c 'import gi
gi.require_version("Gtk", "3.0")
gi.require_version("AnacondaWidgets", "3.4")
from gi.repository import Gtk, AnacondaWidgets' >/dev/null 2>&1
}

ui_ensure_anaconda() {
  ui_have_anaconda && return 0
  if [[ "$(id -u)" -eq 0 ]] && ! is_dry_run; then
    info "Installing Anaconda installer widgets..."
    dnf_yes install anaconda-widgets python3-gobject python3-dbus gtk3 || true
  fi
  if ! ui_have_anaconda; then
    die "Anaconda's graphical installer widgets are required. Install with: dnf install -y anaconda-widgets python3-gobject python3-dbus gtk3"
  fi
  python3 -c 'import dbus, dbus.service' >/dev/null 2>&1 || die "python3-dbus is required for the Anaconda installer."
}

# Build the JSON model the fullscreen installer reads. Writes to $1.
ui_write_model() {
  local dest="$1"
  local tmp saved de file title n
  tmp="$(mktemp -d)"
  printf '%s\n' "$(preflight_text)" >"$tmp/preflight.txt"
  if ((${#WS_LOCAL_USERS[@]})); then
    printf '%s\n' "${WS_LOCAL_USERS[@]}" >"$tmp/users.txt"
  else
    : >"$tmp/users.txt"
  fi
  printf '%s\n' "$(de_default_choice)" >"$tmp/default_de.txt"
  printf '%s\n' "$WS_LOG_FILE" >"$tmp/log.txt"
  printf '%s\n' "$WS_FEDORA_VER" >"$tmp/fedora.txt"
  if is_dry_run; then
    printf '1\n' >"$tmp/dry.txt"
  else
    printf '0\n' >"$tmp/dry.txt"
  fi
  printf '%s\n' "${WS_FORCE_SURFACE:-}" >"$tmp/force_surface.txt"
  printf '%s\n' "${WS_FORCE_NO_SURFACE:-0}" >"$tmp/no_surface.txt"
  printf '%s\n' "${WS_SIMULATE_SURFACE:-}" >"$tmp/simulate_surface.txt"
  saved="${WS_DE:-}"
  for de in xfce mde pipedal; do
    WS_DE="$de"
    de_mark_switch
    base_summary_text >"$tmp/base-$de.txt"
    printf '%s\n' "$WS_DE_SWITCH" >"$tmp/switch-$de.flag"
    de_switch_summary >"$tmp/switch-$de.txt"
    mkdir -p "$tmp/cats-$de"
    n=0
    while IFS= read -r file; do
      [[ -n "$file" ]] || continue
      catalog_visible_file "$file" || continue
      title="$(catalog_category_title "$file")"
      printf '%s\n' "$title" >"$tmp/cats-$de/$n.title"
      catalog_load_checklist_args "$file" >"$tmp/cats-$de/$n.args" || true
      n=$((n + 1))
    done < <(catalog_files)
  done
  WS_DE="$saved"
  python3 "$WS_ROOT/lib/anaconda_ui.py" --compile-model "$tmp" "$dest"
  rm -rf "$tmp"
}

ui_phase() {
  local n="$1" total="$2" title="$3"
  if [[ -n "${WS_PROGRESS_FILE:-}" ]]; then
    printf 'PHASE\t%s\t%s\t%s\n' "$n" "$total" "$title" >>"$WS_PROGRESS_FILE"
  else
    printf '==> [%s/%s] %s\n' "$n" "$total" "$title" >&2
  fi
}

ui_phase_done() {
  local title="$1" ok="${2:-1}"
  if [[ -n "${WS_PROGRESS_FILE:-}" ]]; then
    printf 'DONE\t%s\t%s\n' "$ok" "$title" >>"$WS_PROGRESS_FILE"
  else
    printf '==> %s %s\n' "$([[ "$ok" == "1" ]] && echo done || echo failed)" "$title" >&2
  fi
}

ui_write_result() {
  [[ -n "${WS_RESULT_FILE:-}" ]] || return 0
  local tmp
  tmp="$(mktemp)"
  if ((${#WS_FAILURES[@]})); then
    printf '%s\n' "${WS_FAILURES[@]}" >"$tmp"
  else
    : >"$tmp"
  fi
  surface_mok_instructions >"$tmp.mok" || true
  pipedal_mok_instructions >>"$tmp.mok" || true
  python3 - "$WS_RESULT_FILE" "$tmp" "$tmp.mok" "$WS_APPLIED" "$WS_SKIPPED" "$WS_FAILED" "${WS_DE:-xfce}" "$WS_LOG_FILE" <<'PY'
import json, pathlib, sys
dest, failp, mokp, applied, skipped, failed, de, log = sys.argv[1:9]
fails = [ln for ln in pathlib.Path(failp).read_text(encoding="utf-8").splitlines() if ln]
mok = pathlib.Path(mokp).read_text(encoding="utf-8")
pathlib.Path(dest).write_text(json.dumps({
    "applied": int(applied),
    "skipped": int(skipped),
    "failed": int(failed),
    "failures": fails,
    "desktop": de,
    "log": log,
    "mok": mok,
}, indent=2), encoding="utf-8")
PY
  rm -f "$tmp" "$tmp.mok"
}
