#!/usr/bin/env bash
# MDE package sets: Rust platform (mde-comp DRM + mde-shell). greetd/seatd login.
# shellcheck disable=SC2034

# Session plumbing for the smithay compositor (seatd) + greetd login.
# cage hosts the iced mde-greeter under greetd; without it install falls back to agreety.
MDE_SESSION_PKGS=(
  seatd greetd cage
)

# Devel libs for building mde-comp --features smithay on this machine.
MDE_COMP_BUILD_PKGS=(
  libseat-devel libinput-devel mesa-libgbm-devel libxkbcommon-devel
  wayland-devel mesa-libEGL-devel mesa-libGLES-devel libdrm-devel
  gcc pkgconf-pkg-config
)

MDE_SHELL_PKGS=(
  fd-find
  xfce4-taskmanager gnome-logs hardinfo2
  sysbench iperf3 vulkan-tools
)

MDE_WAYLAND_PKGS=(
  # XWayland available but off by default in mde-config
  xorg-x11-server-Xwayland
  # Screenshots: grim/slurp/swappy + wl-clipboard (see fidelity/CAPTURE.md).
  # Optional (not installed by default): gnome-screenshot, spectacle,
  # hyprpicker, wl-color-picker.
  wl-clipboard grim slurp swappy wf-recorder
  xdg-desktop-portal xdg-desktop-portal-gtk xdg-desktop-portal-wlr
  wtype brightnessctl playerctl
  jq libnotify xdg-user-dirs
)

MDE_PLUMBING_PKGS=(
  plymouth-theme-spinner
  gnome-keyring gnome-keyring-pam
  gvfs-smb gvfs-afc gvfs-gphoto2 gvfs-nfs
  Thunar thunar-volman thunar-archive-plugin
  tumbler tumbler-extras ffmpegthumbnailer
  pipewire pipewire-pulseaudio wireplumber
  NetworkManager bluez
)

MDE_BASELINE_PKGS=(
  firefox terminator vlc mousepad ristretto xarchiver dnfdragora
  firewalld dnf-automatic zram-generator chrony fwupd
)

MDE_THEME_DEPS=(
  google-noto-sans-fonts google-noto-serif-fonts google-noto-emoji-fonts
  cascadia-mono-fonts
)

# Dropped from default install (archive / optional only).
MDE_LEGACY_PKGS=(
  miracle-wm miracle-wm-config DankMaterialShell wlrctl
  swayfx swayfx-config-upstream
  sway sway-config-upstream sway-config-fedora sway-systemd
  labwc labwc-menu-generator
  lightdm lightdm-gtk lightdm-gtk-greeter lightdm-gtk-greeter-settings
  nwg-bar wofi SwayNotificationCenter swayosd kanshi
)

mde_pkg_installed() {
  rpm -q "$1" >/dev/null 2>&1
}

mde_ensure_pkgs() {
  local label="$1"; shift
  local missing=() p
  for p in "$@"; do
    mde_pkg_installed "$p" || missing+=("$p")
  done
  ((${#missing[@]})) || { info "Already installed: $label"; return 0; }
  run_or_skip "$label" dnf_yes install "${missing[@]}"
}

mde_install_compositor() {
  info "Installing seatd/greetd for the Rust MDE compositor"
  mde_ensure_pkgs "mde-session-plumb" "${MDE_SESSION_PKGS[@]}"
  mde_ensure_pkgs "mde-comp-build" "${MDE_COMP_BUILD_PKGS[@]}"
  mde_drop_legacy
}

mde_install_shell_pkgs() {
  mde_ensure_pkgs "mde-shell" "${MDE_SHELL_PKGS[@]}"
  mde_ensure_pkgs "mde-wayland" "${MDE_WAYLAND_PKGS[@]}"
  mde_ensure_pkgs "mde-theme-deps" "${MDE_THEME_DEPS[@]}"
  mde_drop_legacy
}

mde_drop_legacy() {
  local installed=() p
  for p in "${MDE_LEGACY_PKGS[@]}"; do
    mde_pkg_installed "$p" && installed+=("$p")
  done
  ((${#installed[@]})) || return 0
  run_or_skip "drop-mde-legacy" dnf_yes remove "${installed[@]}"
}

mde_install_plumbing() {
  mde_ensure_pkgs "mde-plumbing" "${MDE_PLUMBING_PKGS[@]}"
}

mde_install_baseline() {
  mde_ensure_pkgs "mde-baseline-apps" "${MDE_BASELINE_PKGS[@]}"
}

mde_install_packages() {
  mde_install_compositor
  mde_install_shell_pkgs
  mde_install_plumbing
  mde_install_baseline
}
