#!/usr/bin/env bash
# Desktop environment selection, detection, removal, and persisted state.
# MDE (Mackes Desktop Environment) replaced the inline Sway Experience.
# shellcheck disable=SC2034

WS_DE="${WS_DE:-}"
WS_DE_CLI="${WS_DE_CLI:-}"
WS_DE_SWITCH="${WS_DE_SWITCH:-0}"
WS_DE_PRESENT="${WS_DE_PRESENT:-none}"   # none|xfce|mde|sway|both|pipedal
WS_STATE_DIR="${WS_STATE_DIR:-/var/lib/fedora-ws-setup}"
WS_STATE_FILE="${WS_STATE_FILE:-$WS_STATE_DIR/state}"
WS_MDE_PROFILE="${WS_MDE_PROFILE:-workstation}"  # minimal|workstation
WS_MDE_ROOT="${WS_MDE_ROOT:-$WS_ROOT/mde}"

# Packages that belong to a desktop and are safe to remove on switch.
# Shared keep-apps (Firefox, Terminator, …) are intentionally absent.
DE_XFCE_PKGS=(
  xfce4-session xfce4-panel xfwm4 xfdesktop xfce4-settings
  xfce4-power-manager xfce4-screensaver xfce4-notifyd
  xfce4-screenshooter xfce4-screenshooter-plugin xfce4-taskmanager
  xfce4-appfinder xfce4-pulseaudio-plugin xfce4-whiskermenu-plugin
  xfce4-meowmenu-plugin xfce4-terminal xfce4-dict-plugin
  xfce4-places-plugin xfce4-weather-plugin xfce4-systemload-plugin
  xfce4-cpugraph-plugin xfce4-netload-plugin xfce4-sensors-plugin
  xfce4-battery-plugin xfce4-xkb-plugin xfce4-genmon-plugin
  xfce4-timer-plugin xfce4-notes-plugin xfce4-clipman-plugin
  xfce4-docklike-plugin
)

# Legacy Sway Experience / Miracle packages (MDE uninstall handles its own files).
DE_SWAY_PKGS=(
  labwc
  swayfx swayfx-config-upstream sway sway-config-upstream sway-config-fedora sway-systemd
  miracle-wm miracle-wm-config DankMaterialShell
  nwg-bar nwg-dock nwg-drawer nwg-panel
  waybar wofi swaylock swaybg swayidle imv swayosd
  SwayNotificationCenter cliphist grim slurp swappy wf-recorder
  xdg-desktop-portal-wlr kanshi wdisplays wlrctl lxpolkit
)

DE_DISPLAY_PKGS=(
  lightdm lightdm-gtk lightdm-gtk-greeter lightdm-gtk-greeter-settings
  xorg-x11-server-Xorg xorg-x11-server-Xwayland
)

de_valid() {
  [[ "$1" == "xfce" || "$1" == "mde" || "$1" == "pipedal" ]]
}

# sway and miracle are not desktops anymore. Callers must reject them.
de_canonical() {
  case "$1" in
    miracle|sway) return 1 ;;
    *) printf '%s\n' "$1" ;;
  esac
}

de_mde_installed() {
  [[ -x /usr/bin/mde-session ]] || [[ -f /usr/share/wayland-sessions/mde.desktop ]] ||
    pkg_installed labwc || pkg_installed sway || pkg_installed swayfx || pkg_installed miracle-wm
}

# Back-compat name used by older callers.
de_sway_installed() {
  de_mde_installed
}

de_pipedal_installed() {
  [[ -x /opt/pipedal/sbin/pipedald || -x /opt/pipedal/bin/pipedalconfig ]]
}

de_detect() {
  local x=0 m=0
  pkg_installed xfce4-session && x=1
  de_mde_installed && m=1
  if (( x == 1 && m == 1 )); then
    WS_DE_PRESENT="both"
  elif (( x == 1 )); then
    WS_DE_PRESENT="xfce"
  elif (( m == 1 )); then
    WS_DE_PRESENT="mde"
  elif de_pipedal_installed; then
    WS_DE_PRESENT="pipedal"
  else
    WS_DE_PRESENT="none"
  fi
}

de_load_state() {
  [[ -f "$WS_STATE_FILE" ]] || return 1
  local line val
  val=""
  while IFS= read -r line; do
    [[ "$line" == de=* ]] && val="$(de_canonical "${line#de=}" || true)"
  done <"$WS_STATE_FILE"
  de_valid "$val" || return 1
  # Normalize stored sway → mde
  val="$(de_canonical "$val")"
  printf '%s\n' "$val"
}

de_default_choice() {
  local from_state=""
  if [[ -n "$WS_DE_CLI" ]]; then
    printf '%s\n' "$(de_canonical "$WS_DE_CLI")"
    return 0
  fi
  from_state="$(de_load_state 2>/dev/null || true)"
  if [[ -n "$from_state" ]]; then
    printf '%s\n' "$from_state"
    return 0
  fi
  case "$WS_DE_PRESENT" in
    xfce|mde|pipedal) printf '%s\n' "$WS_DE_PRESENT" ;;
    sway) printf '%s\n' mde ;;
    *) printf '%s\n' xfce ;;
  esac
}

de_other() {
  case "$1" in
    xfce) printf '%s\n' mde ;;
    mde|sway) printf '%s\n' xfce ;;
    *) printf '%s\n' xfce ;;
  esac
}

de_other_installed() {
  local other
  other="$(de_other "$WS_DE")"
  case "$other" in
    xfce) pkg_installed xfce4-session ;;
    mde|sway) de_mde_installed ;;
    *) return 1 ;;
  esac
}

de_mark_switch() {
  WS_DE_SWITCH=0
  WS_DE="$(de_canonical "${WS_DE:-xfce}")"
  case "${WS_DE}" in
    pipedal)
      if pkg_installed xfce4-session || de_mde_installed || pkg_installed lightdm \
          || pkg_installed xorg-x11-server-Xorg || pkg_installed xorg-x11-server-Xwayland; then
        WS_DE_SWITCH=1
      fi
      ;;
    mde)
      # MDE coexist: do not remove XFCE/GNOME automatically.
      WS_DE_SWITCH=0
      if de_pipedal_installed; then
        WS_DE_SWITCH=1
      fi
      ;;
    xfce)
      if de_pipedal_installed; then
        WS_DE_SWITCH=1
      fi
      # Optional: removing MDE packages when switching to XFCE via setup.
      if de_mde_installed && [[ -x "$WS_MDE_ROOT/uninstall.sh" ]]; then
        WS_DE_SWITCH=1
      elif de_other_installed; then
        WS_DE_SWITCH=1
      fi
      ;;
  esac
}

de_resolve_noninteractive() {
  WS_DE="$(de_canonical "$(de_default_choice)")"
  if [[ -z "$WS_DE_CLI" ]]; then
    case "$WS_DE_PRESENT" in
      xfce|mde|pipedal) WS_DE="$WS_DE_PRESENT" ;;
      both) WS_DE="xfce" ;;
      none) WS_DE="xfce" ;;
    esac
  fi
  de_mark_switch
  info "Noninteractive desktop: $WS_DE (present=$WS_DE_PRESENT switch=$WS_DE_SWITCH profile=${WS_MDE_PROFILE})"
}

de_session_name() {
  case "$WS_DE" in
    mde|sway) printf '%s\n' mde ;;
    pipedal) printf '%s\n' multi-user ;;
    *) printf '%s\n' xfce ;;
  esac
}

de_pkg_list() {
  case "$1" in
    xfce) printf '%s\n' "${DE_XFCE_PKGS[@]}" ;;
    mde|sway) printf '%s\n' "${DE_SWAY_PKGS[@]}" ;;
    display) printf '%s\n' "${DE_DISPLAY_PKGS[@]}" ;;
  esac
}

de_installed_remove_list() {
  local de="$1" p
  while IFS= read -r p; do
    [[ -n "$p" ]] || continue
    if pkg_installed "$p"; then
      printf '%s\n' "$p"
    fi
  done < <(de_pkg_list "$de")
  return 0
}

de_switch_summary() {
  local other pkgs n key
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    mapfile -t pkgs < <(
      de_installed_remove_list xfce
      de_installed_remove_list mde
      de_installed_remove_list display
    )
    n="${#pkgs[@]}"
    key="$(printf '%s ' "${pkgs[@]:0:8}")"
    cat <<EOF
Switch to PiPedal

This removes XFCE, MDE/Sway, the display manager, and the X11 and
Wayland servers. The next boot is a text console. PiPedal is used from
a phone or browser at http://$(hostname -s).local/.

  Packages installed now that will be removed: ${n}
  Including: ${key:-none}

If the PiPedal source build fails, this change is rolled back: the
previous desktop and display servers are reinstalled and the RT kernel
is removed.
EOF
    return 0
  fi
  if [[ "${WS_DE}" == "mde" ]]; then
    cat <<EOF
Install MDE (Mackes Desktop Environment ${WS_MDE_PROFILE})

MDE is installed from the standalone tree (mde/). Other desktop
environments stay installed and remain selectable at login.

  Profile: ${WS_MDE_PROFILE}
  Sessions: Mackes Desktop Environment + vanilla Sway
EOF
    if de_pipedal_installed; then
      cat <<EOF

This also removes the PiPedal service and boots the Fedora kernel again.
The RT kernel stays installed and can be chosen from the boot menu.
EOF
    fi
    return 0
  fi
  if de_pipedal_installed; then
    cat <<EOF
Switch to ${WS_DE}

This removes the PiPedal service and boots the Fedora kernel again.
The RT kernel stays installed and can be chosen from the boot menu.
The ${WS_DE} desktop and its display server are installed.
EOF
    return 0
  fi
  other="$(de_other "$WS_DE")"
  mapfile -t pkgs < <(de_installed_remove_list "$other")
  n="${#pkgs[@]}"
  key="$(printf '%s ' "${pkgs[@]:0:8}")"
  cat <<EOF
Switch to ${WS_DE}

This removes the ${other} desktop packages (MDE files via uninstall when present).

  Packages installed now that will be removed: ${n}
  Including: ${key:-none}

Shared apps stay installed (Firefox, Terminator, VLC, Mousepad,
Ristretto, Xarchiver, and the audio/network helpers).

User config for ${other} is backed up under ~/ws-setup-backup/
and then removed. dnf autoremove runs afterwards.
EOF
}

de_protect_keep() {
  local installed=() p
  for p in "${BASE_KEEP_APPS[@]}"; do
    pkg_installed "$p" && installed+=("$p")
  done
  ((${#installed[@]} == 0)) && return 0
  if is_dry_run; then
    info "DRY-RUN: mark keep-apps as user-installed: ${installed[*]}"
    return 0
  fi
  if [[ "$(dnf_major)" == "5" ]]; then
    run_or_skip "protect-keep-apps" dnf_yes mark user "${installed[@]}"
  else
    run_or_skip "protect-keep-apps" dnf_yes mark install "${installed[@]}"
  fi
}

de_remove_other() {
  if [[ "$WS_DE_SWITCH" != "1" ]]; then
    info "No desktop switch; skipping removal"
    return 0
  fi
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    pipedal_remove_graphical
    return 0
  fi
  if de_pipedal_installed; then
    pipedal_remove_service
  fi
  # Installing MDE does not remove other DEs.
  if [[ "${WS_DE}" == "mde" ]]; then
    info "MDE coexist: not removing XFCE or other desktops"
    return 0
  fi
  # Switching to XFCE: prefer MDE uninstall when available.
  if [[ "${WS_DE}" == "xfce" && -x "$WS_MDE_ROOT/uninstall.sh" ]]; then
    info "Removing MDE via $WS_MDE_ROOT/uninstall.sh"
    if is_dry_run; then
      run_or_skip "mde-uninstall" "$WS_MDE_ROOT/uninstall.sh" --dry-run "${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}"
    else
      run_or_skip "mde-uninstall" "$WS_MDE_ROOT/uninstall.sh" "${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}"
    fi
  fi
  local other pkgs=()
  other="$(de_other "$WS_DE")"
  mapfile -t pkgs < <(de_installed_remove_list "$other")
  de_protect_keep
  if ((${#pkgs[@]} == 0)); then
    info "No ${other} packages installed to remove"
  else
    info "Removing ${other} packages: ${pkgs[*]}"
    run_or_skip "remove-${other}" dnf_yes remove "${pkgs[@]}"
  fi
  run_or_skip "autoremove" dnf_yes autoremove
}

de_install_mde() {
  local args=(--profile="$WS_MDE_PROFILE")
  [[ "$WS_DRY_RUN" == "1" ]] && args+=(--dry-run)
  args+=("${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}")
  if [[ ! -x "$WS_MDE_ROOT/install.sh" ]]; then
    error "MDE tree missing: $WS_MDE_ROOT/install.sh"
    WS_FAILURES+=("mde-install")
    WS_FAILED=$((WS_FAILED + 1))
    return 0
  fi
  info "Delegating to MDE installer (profile=$WS_MDE_PROFILE)"
  # Always invoke the MDE installer so its own --dry-run path runs; do not
  # short-circuit via run_or_skip's dry-run print-only behaviour.
  if "$WS_MDE_ROOT/install.sh" "${args[@]}"; then
    WS_APPLIED=$((WS_APPLIED + 1))
  else
    error "MDE install failed"
    WS_FAILURES+=("mde-install")
    WS_FAILED=$((WS_FAILED + 1))
  fi
}

de_save_state() {
  WS_DE="$(de_canonical "${WS_DE:-xfce}")"
  if is_dry_run; then
    info "DRY-RUN: write $WS_STATE_FILE de=$WS_DE"
    return 0
  fi
  mkdir -p "$WS_STATE_DIR"
  cat >"$WS_STATE_FILE" <<EOF
de=${WS_DE}
mde_profile=${WS_MDE_PROFILE}
EOF
  WS_APPLIED=$((WS_APPLIED + 1))
  info "Saved desktop state: de=$WS_DE"
}

# Uninstall a desktop/experience without requiring a switch-to target.
# WS_UNINSTALL_DE: xfce | mde | pipedal | all
de_uninstall_current() {
  local target="${WS_UNINSTALL_DE:-}"
  [[ -n "$target" ]] || return 0
  case "$target" in
    xfce|mde|pipedal|all) ;;
    *)
      error "Unknown uninstall_de target: $target"
      WS_FAILURES+=("uninstall_de:$target")
      WS_FAILED=$((WS_FAILED + 1))
      return 0
      ;;
  esac
  info "Uninstall desktop experience: $target"
  if declare -F ui_progress_info >/dev/null 2>&1; then
    ui_progress_info "Uninstalling desktop experience: $target"
  fi
  if [[ "$target" == "mde" || "$target" == "all" ]]; then
    if [[ -x "$WS_MDE_ROOT/uninstall.sh" ]]; then
      if is_dry_run; then
        run_or_skip "mde-uninstall" "$WS_MDE_ROOT/uninstall.sh" --dry-run "${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}"
      else
        run_or_skip "mde-uninstall" "$WS_MDE_ROOT/uninstall.sh" "${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}"
      fi
    else
      local pkgs=()
      mapfile -t pkgs < <(de_installed_remove_list mde)
      de_protect_keep
      if ((${#pkgs[@]} > 0)); then
        run_or_skip "remove-mde" dnf_yes remove "${pkgs[@]}"
      fi
    fi
  fi
  if [[ "$target" == "xfce" || "$target" == "all" ]]; then
    local pkgs=()
    mapfile -t pkgs < <(de_installed_remove_list xfce)
    de_protect_keep
    if ((${#pkgs[@]} > 0)); then
      info "Removing XFCE packages: ${pkgs[*]}"
      run_or_skip "remove-xfce" dnf_yes remove "${pkgs[@]}"
    else
      info "No XFCE packages installed to remove"
    fi
  fi
  if [[ "$target" == "pipedal" || "$target" == "all" ]]; then
    if declare -F pipedal_remove_service >/dev/null 2>&1; then
      if de_pipedal_installed || [[ "$target" == "pipedal" ]]; then
        pipedal_remove_service
      fi
    fi
  fi
  run_or_skip "autoremove-after-de-uninstall" dnf_yes autoremove
}
