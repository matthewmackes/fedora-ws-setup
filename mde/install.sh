#!/usr/bin/env bash
# MDE — Mackes Desktop Environment 2026
# Standalone git/tarball installer. Layout: /usr (bin, share/mde, etc/xdg/mde).
#
# Usage:
#   sudo ./install.sh [--dry-run] [--profile=minimal|workstation] [user...]
#   sudo ./install.sh --uninstall [--purge-config]
set -euo pipefail

MDE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=install/lib/mde-common.sh
source "$MDE_ROOT/install/lib/mde-common.sh"
# shellcheck source=install/lib/core.sh
source "$MDE_ROOT/install/lib/core.sh"
# Override core's WS_ROOT (points at install/) so logs land under mde/logs.
WS_ROOT="$MDE_ROOT"
WS_LOG_DIR="${WS_LOG_DIR:-$MDE_ROOT/logs}"
mkdir -p "$WS_LOG_DIR"
WS_LOG_FILE="${WS_LOG_FILE:-$WS_LOG_DIR/mde-install-$(date +%Y%m%d-%H%M%S).log}"

# shellcheck source=install/lib/packages.sh
source "$MDE_ROOT/install/lib/packages.sh"
# shellcheck source=install/lib/look_shared.sh
source "$MDE_ROOT/install/lib/look_shared.sh"
# shellcheck source=install/lib/look_mde.sh
source "$MDE_ROOT/install/lib/look_mde.sh"

MDE_PURGE_CONFIG=0
MDE_DO_UNINSTALL=0
USERS=()
# Optional detect vars used by look_shared; safe defaults for standalone install.
WS_SCALE_TIER="${WS_SCALE_TIER:-}"
WS_FEDORA_VER="${WS_FEDORA_VER:-}"
WS_ARCH="${WS_ARCH:-$(uname -m)}"
WS_DE="${WS_DE:-mde}"

usage() {
  cat <<EOF
$(mde_version_string) — $MDE_NAME

Usage: sudo $0 [options] [user...]

Options:
  --profile=minimal|workstation  Shell only, or shell + workstation baseline
  --dry-run                      Print actions only
  --uninstall                    Remove MDE files (keeps user config unless --purge-config)
  --purge-config                 With --uninstall, back up then remove per-user MDE config
  -h, --help                     Show this help

Default profile: workstation. Default user: the sudo caller.
Does not remove other desktop environments (GNOME/XFCE/KDE coexist).
EOF
}

require_root() {
  if [[ "$(id -u)" -ne 0 ]]; then
    die "MDE install requires root (try: sudo $0)"
  fi
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run) WS_DRY_RUN=1 ;;
      --profile=minimal|--profile=workstation) MDE_PROFILE="${1#*=}" ;;
      --profile)
        shift
        MDE_PROFILE="${1:-}"
        [[ "$MDE_PROFILE" == "minimal" || "$MDE_PROFILE" == "workstation" ]] ||
          die "profile must be minimal or workstation"
        ;;
      --uninstall) MDE_DO_UNINSTALL=1 ;;
      --purge-config) MDE_PURGE_CONFIG=1 ;;
      -h|--help) usage; exit 0 ;;
      --*) die "Unknown option: $1" ;;
      *) USERS+=("$1") ;;
    esac
    shift
  done
  ((${#USERS[@]})) || USERS=("${SUDO_USER:-$(id -un)}")
}

mde_install_data_files() {
  if is_dry_run; then
    info "DRY-RUN: install session files, schemas, xdg defaults to $MDE_PREFIX"
    return 0
  fi
  mkdir -p "$MDE_BIN" "$MDE_LIBEXEC" "$MDE_SHARE" \
    "$MDE_DATA/wayland-sessions" "$MDE_DATA/glib-2.0/schemas" \
    "$MDE_STATE_DIR" \
    /usr/share/backgrounds/mde /usr/share/fonts/mde

  # One session binary. The bash wrapper remains only if the Rust build is absent.
  local rs="$MDE_ROOT/crates/target-user/release/mde-session"
  if [[ -x "$rs" ]]; then
    install -m 0755 "$rs" "$MDE_BIN/mde-session"
    install -m 0755 "$rs" "$MDE_BIN/mde-session-rs"
  else
    install -m 0755 "$MDE_ROOT/install/bin/mde-session" "$MDE_BIN/mde-session"
  fi
  # Python labwc shell is no longer part of the default install tree.

  install -m 0644 "$MDE_ROOT/data/wayland-sessions/mde.desktop" \
    "$MDE_DATA/wayland-sessions/mde.desktop"
  # Drop the vanilla Sway session this tree used to install. Leave Fedora's
  # own sway.desktop alone when it does not carry our comment.
  if [[ -f "$MDE_DATA/wayland-sessions/sway.desktop" ]] &&
     grep -q 'without the MDE shell' "$MDE_DATA/wayland-sessions/sway.desktop"; then
    rm -f "$MDE_DATA/wayland-sessions/sway.desktop"
  fi

  install -m 0644 "$MDE_ROOT/data/glib-2.0/schemas/org.mde.gschema.xml" \
    "$MDE_DATA/glib-2.0/schemas/org.mde.gschema.xml"
  if [[ -f "$MDE_ROOT/data/glib-2.0/schemas/20_org.mde.gschema.override" ]]; then
    install -m 0644 "$MDE_ROOT/data/glib-2.0/schemas/20_org.mde.gschema.override" \
      "$MDE_DATA/glib-2.0/schemas/20_org.mde.gschema.override"
  fi
  rm -rf "$MDE_XDG/sway"
  glib-compile-schemas "$MDE_DATA/glib-2.0/schemas" >/dev/null 2>&1 || \
    warn "glib-compile-schemas failed"

  # Private look assets under /usr/share (themes/icons) and /usr/share/mde.
  if [[ -d "$MDE_ROOT/assets/themes" ]]; then
    mkdir -p /usr/share/themes
    cp -a "$MDE_ROOT/assets/themes/." /usr/share/themes/
    look_patch_padOS /usr/share/themes/padOS-master 2>/dev/null || true
    # After adw-gtk3-theme is installed (mde_install_packages), bridge GTK 4.
    look_bridge_pados_gtk4 2>/dev/null || true
  fi
  if [[ -d "$MDE_ROOT/assets/icons" ]]; then
    mkdir -p /usr/share/icons
    cp -a "$MDE_ROOT/assets/icons/." /usr/share/icons/
  fi
  if [[ -d "$MDE_ROOT/assets/fonts" ]]; then
    mkdir -p /usr/share/fonts/mde
    cp -a "$MDE_ROOT/assets/fonts/." /usr/share/fonts/mde/
    fc-cache -f /usr/share/fonts/mde >/dev/null 2>&1 || true
  fi
  if [[ -d "$MDE_ROOT/assets/brand" ]]; then
    mkdir -p "$MDE_SHARE/brand"
    cp -a "$MDE_ROOT/assets/brand/." "$MDE_SHARE/brand/"
  fi
  local wp_name="${LOOK_WALLPAPER_NAME:-ws-wallpaper.jpg}"
  if [[ -f "$MDE_ROOT/assets/wallpapers/$wp_name" ]]; then
    cp -a "$MDE_ROOT/assets/wallpapers/$wp_name" "/usr/share/backgrounds/mde/"
  elif compgen -G "$MDE_ROOT/assets/wallpapers/*" >/dev/null; then
    local first
    first="$(find "$MDE_ROOT/assets/wallpapers" -type f | head -1)"
    cp -a "$first" "/usr/share/backgrounds/mde/$wp_name"
  fi

  # Plymouth (required branding).
  if [[ -d "$MDE_ROOT/assets/plymouth/neon-tech-penguin" ]]; then
    mkdir -p /usr/share/plymouth/themes
    cp -a "$MDE_ROOT/assets/plymouth/neon-tech-penguin" /usr/share/plymouth/themes/
    if command -v plymouth-set-default-theme >/dev/null 2>&1; then
      plymouth-set-default-theme neon-tech-penguin -R >/dev/null 2>&1 || \
        plymouth-set-default-theme neon-tech-penguin >/dev/null 2>&1 || \
        warn "Could not set plymouth theme"
    fi
  fi

  WS_APPLIED=$((WS_APPLIED + 1))
}

mde_write_system_config() {
  if is_dry_run; then
    info "DRY-RUN: write /etc/xdg/mde defaults (Rust platform; no labwc tree)"
    return 0
  fi
  rm -rf /etc/xdg/mde/sway /etc/xdg/mde/labwc
  mkdir -p /etc/xdg/mde /etc/xdg/xdg-desktop-portal
  cat >/etc/xdg/mde/README <<'EOF'
MDE system defaults. Per-user state lives in ~/.config/mde/config.toml
(owned by mde-config / Config Editor). Session entry is mde-session.
EOF
  cat >/etc/xdg/xdg-desktop-portal/mde-portals.conf <<'EOF'
[preferred]
default=gtk
org.freedesktop.impl.portal.Screenshot=wlr
org.freedesktop.impl.portal.ScreenCast=wlr
EOF
}

mde_seed_user() {
  local user="$1" home
  home="$(getent passwd "$user" | cut -d: -f6)"
  [[ -n "$home" && -d "$home" ]] || return 0

  if is_dry_run; then
    info "DRY-RUN: seed ~/.config/mde for $user"
    return 0
  fi

  mkdir -p "$home/.config/mde/profiles"
  if [[ ! -f "$home/.config/mde/config.toml" ]]; then
    # First login migrates legacy ws-* JSON via mde-session / mde-config.
    cat >"$home/.config/mde/config.toml" <<'EOF'
schema = 1
active_profile = "default"

[profiles.default]
display_name = "Default"
EOF
  fi
  chown -R "$user:$user" "$home/.config/mde" 2>/dev/null || true
}

mde_configure_greetd() {
  if is_dry_run; then
    info "DRY-RUN: enable greetd/seatd; default session mde-session"
    return 0
  fi
  mkdir -p /etc/greetd
  # Fedora's greetd package creates user "greetd" (not upstream "greeter").
  # Prefer cage + mde-comp greeter (iced UI); fall back to agreety without cage.
  local greet_cmd
  if command -v cage >/dev/null 2>&1; then
    greet_cmd='cage -s -- /usr/bin/mde-comp greeter'
  else
    greet_cmd='agreety --cmd /usr/bin/mde-session'
    warn "cage not found; greetd using agreety (install cage for graphical greeter)"
  fi
  cat >/etc/greetd/config.toml <<EOF
[terminal]
vt = 1

[default_session]
command = "${greet_cmd}"
user = "greetd"
EOF
  # Greeter needs DRM/input; session users need seat for /run/seatd.sock.
  usermod -aG seat,video,input greetd 2>/dev/null || true
  local u
  for u in "${USERS[@]}"; do
    usermod -aG seat "$u" 2>/dev/null || true
  done
  # Drop LightDM MDE overrides; login is greetd + cage/mde-greeter (or agreety).
  rm -f /etc/lightdm/lightdm.conf.d/99-mde-session.conf
  rm -f /usr/share/xgreeters/ws-greeter.desktop
  if [[ "$MDE_PROFILE" == "workstation" ]]; then
    systemctl enable seatd.service >/dev/null 2>&1 || true
    systemctl enable greetd.service >/dev/null 2>&1 || true
    systemctl disable lightdm.service >/dev/null 2>&1 || true
    systemctl set-default graphical.target >/dev/null 2>&1 || true
  fi
  WS_APPLIED=$((WS_APPLIED + 1))
}

mde_save_state() {
  if is_dry_run; then
    info "DRY-RUN: write $MDE_STATE_FILE"
    return 0
  fi
  mkdir -p "$MDE_STATE_DIR"
  cat >"$MDE_STATE_FILE" <<EOF
name=mde
version=${MDE_VERSION_FULL}
profile=${MDE_PROFILE}
EOF
  # Compatibility for fedora-ws-setup detection.
  mkdir -p /var/lib/fedora-ws-setup
  echo "de=mde" >/var/lib/fedora-ws-setup/state
}

mde_install() {
  info "$(mde_version_string) install profile=$MDE_PROFILE users=${USERS[*]}"
  [[ "$WS_DRY_RUN" == "1" ]] || require_root
  SWAY_HELPER_DIR="$MDE_BIN"
  mde_install_packages
  mde_install_data_files
  look_sway_install_helpers
  mde_write_system_config
  local u
  for u in "${USERS[@]}"; do
    mde_seed_user "$u"
  done
  mde_configure_greetd
  mde_save_state
  info "Done. Use greetd session “Mackes Desktop Environment” (mde-session)."
  info "Applied=$WS_APPLIED skipped=$WS_SKIPPED failed=$WS_FAILED"
}

mde_backup_user_config() {
  local user="$1" home dest
  home="$(getent passwd "$user" | cut -d: -f6)"
  [[ -d "$home" ]] || return 0
  dest="$home/mde-backup/$(date +%Y%m%d-%H%M%S)"
  if is_dry_run; then
    info "DRY-RUN: backup $user MDE config to $dest"
    return 0
  fi
  mkdir -p "$dest"
  local p
  for p in mde sway ws-taskbar ws-start ws-settings nwg-bar swaync wofi swaylock swayosd kanshi; do
    [[ -e "$home/.config/$p" ]] || continue
    tar -C "$home/.config" -czf "$dest/${p}.tar.gz" "$p" 2>/dev/null || true
  done
  chown -R "$user:$user" "$home/mde-backup" 2>/dev/null || true
  info "Backed up $user config to $dest"
  if [[ "$MDE_PURGE_CONFIG" == "1" ]]; then
    for p in mde ws-taskbar ws-start ws-settings; do
      rm -rf "$home/.config/$p"
    done
    info "Purged MDE user config for $user"
  fi
}

mde_uninstall() {
  info "Uninstalling MDE ${MDE_VERSION_FULL}"
  [[ "$WS_DRY_RUN" == "1" ]] || require_root
  local u
  for u in "${USERS[@]}"; do
    mde_backup_user_config "$u"
  done
  if is_dry_run; then
    info "DRY-RUN: remove MDE files under /usr and /etc"
    return 0
  fi
  rm -f "$MDE_BIN/mde-session" "$MDE_BIN/mde-lightdm-session" "$MDE_BIN/mde-autostart"
  # Helpers installed by look_sway_install_helpers
  local h
  for h in ws-run-if ws-drawer ws-start ws-lock ws-settings ws-taskbar ws-watermark \
           ws-mmc ws-winx ws-sway-autostart ws-show-desktop ws-osd ws-greeter \
           ws-power ws-screenshot ws-save-displays regedit \
           mde-comp mde-shell mde-control mde-session-rs mde-privileged mde-neighbourhood; do
    rm -f "$MDE_BIN/$h"
  done
  rm -f /usr/share/applications/regedit.desktop /usr/share/mime/packages/mde-regedit.xml \
    /usr/share/applications/mde-control.desktop /usr/share/applications/mde-config-editor.desktop \
    /etc/greetd/mde-config.toml /usr/share/polkit-1/actions/org.mde.privileged.policy
  # Leave /etc/greetd/config.toml if the admin customized it; remove our known marker copy.
  if [[ -f /etc/greetd/config.toml ]] && grep -Eq 'mde-comp greeter|agreety --cmd (/usr/bin/)?mde-session' /etc/greetd/config.toml 2>/dev/null; then
    rm -f /etc/greetd/config.toml
  fi
  rm -rf /usr/share/mde/ws-regedit /usr/share/mde/ws-taskbar /usr/share/mde/ws-start \
    /usr/share/mde/ws-settings /usr/share/mde/ws-greeter /usr/share/mde/ws-mmc
  rm -rf "$MDE_SHARE" /usr/share/backgrounds/mde /usr/share/fonts/mde
  rm -f "$MDE_DATA/wayland-sessions/mde.desktop"
  rm -f "$MDE_DATA/glib-2.0/schemas/org.mde.gschema.xml"
  glib-compile-schemas "$MDE_DATA/glib-2.0/schemas" >/dev/null 2>&1 || true
  rm -rf /etc/xdg/mde
  rm -f /etc/lightdm/lightdm.conf.d/99-mde-session.conf
  rm -f /usr/share/xgreeters/ws-greeter.desktop
  rm -f /usr/libexec/ws-mmc-helper
  rm -f /etc/systemd/system/ws-mmc-devices.service
  systemctl disable ws-mmc-devices.service >/dev/null 2>&1 || true
  rm -rf "$MDE_STATE_DIR"
  info "MDE files removed. Other desktops were left installed."
}

main() {
  parse_args "$@"
  if [[ "$MDE_DO_UNINSTALL" == "1" ]]; then
    mde_uninstall
  else
    mde_install
  fi
}

main "$@"
