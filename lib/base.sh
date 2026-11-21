#!/usr/bin/env bash
# Base XFCE desktop: repos, spin-like group, strip extras, codecs, LightDM.
# shellcheck disable=SC2034

BASE_KEEP_APPS=(
  firefox terminator vlc mousepad ristretto xarchiver
  pavucontrol blueman network-manager-applet
  dnfdragora dnfdragora-updater
)

BASE_PLUMBING=(
  xfce4-notifyd xfce4-screenshooter xfce-polkit gnome-keyring-pam
  xdg-desktop-portal-gtk xdg-desktop-portal-xapp
  tumbler tumbler-extras ffmpegthumbnailer
  gvfs-smb gvfs-afc gvfs-gphoto2 gvfs-nfs
  lightdm-gtk lightdm-gtk-greeter-settings plymouth-theme-spinner
  xfce4-pulseaudio-plugin xfce4-power-manager xfce4-screensaver
  xfce4-screenshooter-plugin xfce4-taskmanager xfce4-appfinder
  Thunar thunar-archive-plugin thunar-volman
)

BASE_THEME_DEPS=(
  gtk-murrine-engine gtk2-engines
  adwaita-sans-fonts adwaita-mono-fonts google-noto-color-emoji-fonts
  cascadia-mono-fonts
)

BASE_CODECS=(
  ffmpeg vlc-plugins-freeworld libavcodec-freeworld
  gstreamer1-plugins-bad-freeworld gstreamer1-plugins-ugly
  gstreamer1-plugin-libav mozilla-openh264
  pipewire-codec-aptx libheif-freeworld
)

BASE_EXCLUDE_PKGS=(
  xfce4-terminal initial-setup-gui
  claws-mail claws-mail-plugins-archive claws-mail-plugins-att-remover
  claws-mail-plugins-attachwarner claws-mail-plugins-fetchinfo
  claws-mail-plugins-mailmbox claws-mail-plugins-newmail
  claws-mail-plugins-notification claws-mail-plugins-pgp
  claws-mail-plugins-rssyl claws-mail-plugins-smime
  claws-mail-plugins-spam-report claws-mail-plugins-tnef
  claws-mail-plugins-vcalendar
  pidgin transmission geany galculator atril catfish gparted seahorse
  xfce4-dict-plugin asunder parole pragha xfburn
)

base_enable_repos() {
  info "Enabling RPM Fusion Free + Nonfree and OpenH264"
  local ver="$WS_FEDORA_VER"
  if ! pkg_installed rpmfusion-free-release; then
    run_or_skip "rpmfusion-free" dnf_yes install \
      "https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-${ver}.noarch.rpm"
  fi
  if ! pkg_installed rpmfusion-nonfree-release; then
    run_or_skip "rpmfusion-nonfree" dnf_yes install \
      "https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-${ver}.noarch.rpm"
  fi
  # Cisco OpenH264 is usually already present as a repo; ensure enabled
  if [[ -f /etc/yum.repos.d/fedora-cisco-openh264.repo ]]; then
    run_or_skip "openh264-repo" sed -i 's/^enabled=0/enabled=1/' /etc/yum.repos.d/fedora-cisco-openh264.repo
  fi
}

base_xfce_already() {
  pkg_installed xfce4-session
}

base_install_xfce() {
  if base_xfce_already; then
    info "XFCE session already present; skipping group install + exclusion purge"
    ensure_pkgs "base-keep-apps" "${BASE_KEEP_APPS[@]}"
    return 0
  fi

  info "Installing XFCE desktop environment group"
  if is_dry_run; then
    info "DRY-RUN: dnf group install xfce-desktop-environment"
  else
    run_or_skip "xfce-desktop-environment" dnf_yes group install xfce-desktop-environment
  fi

  # Install keep apps (may already be pulled by group)
  ensure_pkgs "base-keep-apps" "${BASE_KEEP_APPS[@]}"

  # Strip extras that come from default groups / spin-like set
  local remove=()
  local p
  for p in "${BASE_EXCLUDE_PKGS[@]}"; do
    if pkg_installed "$p"; then
      remove+=("$p")
    fi
  done
  if ((${#remove[@]} > 0)); then
    info "Removing spin extras: ${remove[*]}"
    run_or_skip "strip-extras" dnf_yes remove "${remove[@]}"
  fi
}

base_install_plumbing() {
  ensure_pkgs "base-plumbing" "${BASE_PLUMBING[@]}"
  ensure_pkgs "base-theme-deps" "${BASE_THEME_DEPS[@]}"
}

base_install_codecs() {
  # Prefer ffmpeg over ffmpeg-free when possible
  if pkg_installed ffmpeg-free && ! pkg_installed ffmpeg; then
    run_or_skip "swap-ffmpeg" dnf_yes swap ffmpeg-free ffmpeg --allowerasing
  fi
  ensure_pkgs "base-codecs" "${BASE_CODECS[@]}"
}

base_install_gpu_vaapi() {
  case "$WS_GPU" in
    intel)
      ensure_pkgs "vaapi-intel" intel-media-driver libva-utils
      ;;
    amd)
      ensure_pkgs "vaapi-amd" mesa-va-drivers-freeworld libva-utils
      ;;
    nvidia)
      info "NVIDIA VA-API deferred to Hardware Drivers catalog item"
      ;;
    *)
      info "Unknown GPU; skipping VA-API package selection"
      ;;
  esac
}

base_set_defaults() {
  info "Setting default apps (Terminator, Firefox, VLC) and graphical target"
  if is_dry_run; then
    info "DRY-RUN: update-alternatives / mime / systemctl for desktop defaults"
    return 0
  fi

  # Preferred applications via exo / xdg
  mkdir -p /etc/xdg/xfce4
  cat > /etc/xdg/xfce4/helpers.rc <<'EOF'
TerminalEmulator=terminator
FileManager=Thunar
WebBrowser=firefox
EOF

  # MIME defaults system-wide
  mkdir -p /usr/local/share/applications
  cat > /usr/local/share/applications/mimeapps.list <<'EOF'
[Default Applications]
text/html=firefox.desktop
x-scheme-handler/http=firefox.desktop
x-scheme-handler/https=firefox.desktop
audio/mpeg=vlc.desktop
audio/mp4=vlc.desktop
audio/x-vorbis+ogg=vlc.desktop
video/mp4=vlc.desktop
video/x-matroska=vlc.desktop
video/webm=vlc.desktop
inode/directory=thunar.desktop
EOF

  systemctl enable lightdm.service 2>/dev/null || true
  systemctl set-default graphical.target 2>/dev/null || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

# MDE owns the labwc/shell package set.
# Legacy helpers kept as no-ops for older tools that still call them.
base_install_sway() {
  info "Sway Experience moved to MDE; use de_install_mde"
  de_install_mde
}

base_install_swayfx() {
  warn "SwayFX is no longer used; MDE installs Fedora labwc"
}

base_install_swayosd() {
  warn "SwayOSD is installed by the MDE tree when needed"
}

base_drop_miracle() { :; }

base_install_sway_plumbing() {
  warn "MDE plumbing is applied by mde/install.sh"
}

# LightDM's distro session-wrapper is /etc/X11/xinit/Xsession. That path is
# fine for XFCE, but for Wayland compositors it often drops XDG_RUNTIME_DIR
# (the compositor then cannot bind its Wayland socket). Run Wayland sessions
# directly after fixing the runtime dir; keep Xsession for X11 desktops.
base_install_lightdm_session_wrapper() {
  if is_dry_run; then
    info "DRY-RUN: install /usr/local/bin/ws-lightdm-session"
    return 0
  fi
  mkdir -p /usr/local/bin
  cat >/usr/local/bin/ws-lightdm-session <<'EOF'
#!/usr/bin/bash
# LightDM session-wrapper: ensure Wayland compositors get a valid runtime dir.
uid="$(id -u)"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/${uid}}"
if [[ ! -d "$XDG_RUNTIME_DIR" ]]; then
  mkdir -p "$XDG_RUNTIME_DIR"
  chmod 700 "$XDG_RUNTIME_DIR"
fi
if [[ -S "$XDG_RUNTIME_DIR/bus" ]]; then
  export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
fi

cmd="${1:-}"
shift || true
case "$cmd" in
  '' )
    echo "ws-lightdm-session: missing session command" >&2
    exit 1
    ;;
  *sway|*wayland*)
    export XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-wayland}"
    export XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP:-sway}"
    # Native Wayland for Firefox, Electron, Qt, SDL and Java apps; the mm
    # cursor and Qt theme for everything started from the session.
    export MOZ_ENABLE_WAYLAND=1 ELECTRON_OZONE_PLATFORM_HINT=auto
    export QT_QPA_PLATFORM="wayland;xcb" QT_QPA_PLATFORMTHEME=qt5ct
    export SDL_VIDEODRIVER="wayland,x11" _JAVA_AWT_WM_NONREPARENTING=1
    export XCURSOR_THEME="${XCURSOR_THEME:-Adwaita}" XCURSOR_SIZE="${XCURSOR_SIZE:-24}"
    systemctl --user import-environment \
      XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS XDG_SESSION_TYPE XDG_CURRENT_DESKTOP \
      >/dev/null 2>&1 || true
    exec "$cmd" "$@"
    ;;
  *)
    exec /etc/X11/xinit/Xsession "$cmd" "$@"
    ;;
esac
EOF
  chmod 0755 /usr/local/bin/ws-lightdm-session
}

base_set_lightdm_session() {
  local session greeter=lightdm-gtk-greeter
  session="$(de_session_name)"
  if [[ "${WS_DE:-xfce}" == mde || "${WS_DE:-xfce}" == sway ]] &&
    [[ "${WS_GREETER:-ws}" != gtk && -f /usr/share/xgreeters/ws-greeter.desktop ]]; then
    greeter=ws-greeter
  fi
  info "LightDM default session: $session"
  if is_dry_run; then
    info "DRY-RUN: write /etc/lightdm/lightdm.conf.d/99-ws-session.conf"
    return 0
  fi
  # MDE installs its own LightDM drop-in; skip duplicate for mde.
  if [[ "${WS_DE:-xfce}" == "mde" || "${WS_DE:-xfce}" == "sway" ]]; then
    info "LightDM session for MDE is configured by mde/install.sh"
    return 0
  fi
  base_install_lightdm_session_wrapper
  mkdir -p /etc/lightdm/lightdm.conf.d
  cat >/etc/lightdm/lightdm.conf.d/99-ws-session.conf <<EOF
[Seat:*]
greeter-session=${greeter}
user-session=${session}
session-wrapper=/usr/local/bin/ws-lightdm-session
EOF
  systemctl enable lightdm.service >/dev/null 2>&1 || true
  systemctl set-default graphical.target >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

base_apply() {
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    info "PiPedal appliance base is applied by the PiPedal steps"
    return 0
  fi
  WS_DE="$(de_canonical "${WS_DE:-xfce}")"
  base_enable_repos
  case "${WS_DE}" in
    mde)
      de_install_mde
      # Workstation profile already applies baseline; minimal skips codecs here
      # so shared hardening can still run from the setup wizard.
      if [[ "${WS_MDE_PROFILE}" == "workstation" ]]; then
        info "MDE workstation profile includes baseline; skipping duplicate base codecs step hooks that MDE owns"
      else
        base_install_codecs
        base_install_gpu_vaapi
        cfg_power_profiles
        cfg_dvd_playback
        base_set_defaults
      fi
      ;;
    *)
      base_install_xfce
      base_install_plumbing
      base_install_codecs
      base_install_gpu_vaapi
      cfg_power_profiles
      cfg_dvd_playback
      base_set_defaults
      base_set_lightdm_session
      ;;
  esac
}

base_summary_text() {
  local surface_line="Surface baseline: no"
  local env_line switch_line
  if [[ "${WS_SURFACE}" == "1" ]]; then
    surface_line="Surface baseline: yes (${WS_SURFACE_FAMILY}) — linux-surface kernel + iptsd"
  fi
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    if [[ "${WS_DE_SWITCH:-0}" == "1" ]]; then
      switch_line="Switch:       yes — remove the graphical desktop and display servers"
    else
      switch_line="Switch:       no"
    fi
    cat <<EOF
PiPedal appliance plan

  Environment:  Text console. No display manager, X11 server, or Wayland server.
  ${switch_line}
  Kernel:       PREEMPT_RT (kernel-vanilla, then Audinux kernel-rt-mao)
                Fedora kernel stays installed as a boot fallback
  Boot args:    mitigations=off usbcore.autosuspend=-1
  Boot splash:  Neon Tech Penguin
  CPU:          tuned latency-performance (no desktop power profiles)
  Audio:        PipeWire stopped. PiPedal opens the USB card through ALSA.
                48 kHz, 16 frames x 4 periods. ALSA MIDI.
  Web:          http://$(hostname -s).local/ (port 80 and mDNS opened)
  Service:      pipedal_d, rtprio 95, unlimited locked memory
  Plugins:      Pedalboard LV2 set, including TooB from Audinux
  Build:        Latest PiPedal release, installed to /opt/pipedal
  Rollback:     A failed source build restores the previous desktop and removes the RT kernel
  ${surface_line}

These steps run only after Begin Installation.
EOF
    return 0
  fi
  if [[ "${WS_DE:-xfce}" == "mde" || "${WS_DE:-xfce}" == "sway" ]]; then
    env_line="MDE ${WS_MDE_PROFILE} on Fedora sway (Win11-proportioned shell, dual Sway session)"
  else
    env_line="XFCE (spin set, then strip Office/games/extras)"
  fi
  if [[ "${WS_DE:-xfce}" == "mde" ]]; then
    switch_line="Coexist:      other desktops stay installed"
  elif [[ "${WS_DE_SWITCH:-0}" == "1" ]]; then
    switch_line="Switch:       yes — remove $(de_other "$WS_DE"), then autoremove"
  else
    switch_line="Switch:       no"
  fi
  cat <<EOF
Base desktop plan

  Environment:  ${env_line}
  Display mgr:  $([[ "${WS_DE:-xfce}" == "pipedal" ]] && echo "none (multi-user console)" || echo "LightDM (system default session)")
  ${switch_line}
  Apps:         Firefox, Terminator, VLC, Mousepad, Ristretto,
                Xarchiver, dnfdragora, NM/blueman/audio stack
  Hardening:    firewalld, dnf-automatic, zram+oomd, fwupd,
                fstrim, chrony, snapper on btrfs
  Codecs:       RPM Fusion Free/Nonfree + OpenH264
  Power:        tuned-ppd when present, otherwise install it
  DVD:          RPM Fusion tainted repo, then libdvdcss
  GPU VA-API:   ${WS_GPU}
  Boot splash:  Neon Tech Penguin
  ${surface_line}

These base steps are queued and apply only after final Confirm.
EOF
}
