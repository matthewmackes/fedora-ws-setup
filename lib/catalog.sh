#!/usr/bin/env bash
# Upgrade catalog parser, preflight, and apply handlers.
# Format per line: id|label|kind|payload|guard|de
# kind: rpm | flatpak | copr | repo | config | mix
# payload: space-separated packages, flatpak id, copr project, repo URL, or function name
# guard: always | fedora>=NN | surface | surface-pro | btrfs | nvidia | intel | amd
# de: any | xfce | sway   (default any)
# shellcheck disable=SC2034

WS_CATALOG_DIR="${WS_CATALOG_DIR:-$WS_ROOT/catalog}"
WS_SELECTED_ITEMS=()
WS_REMOVE_ITEMS=()
WS_UNINSTALL_DE="${WS_UNINSTALL_DE:-}"
WS_APPLIED_FILE="${WS_APPLIED_FILE:-${WS_STATE_DIR:-/var/lib/fedora-ws-setup}/applied.json}"
declare -A WS_CATALOG_META=()
declare -A WS_APPLIED_IDS=()

# --- config recipe functions referenced by catalog -----------------------

cfg_firefox_policy() {
  mkdir -p /etc/firefox/policies
  cat > /etc/firefox/policies/policies.json <<'EOF'
{
  "policies": {
    "DisableTelemetry": true,
    "DisableFirefoxStudies": true,
    "DisablePocket": true,
    "FirefoxHome": {
      "SponsoredTopSites": false,
      "SponsoredPocket": false
    },
    "Preferences": {
      "media.ffmpeg.vaapi.enabled": { "Value": true, "Status": "user" }
    }
  }
}
EOF
}

cfg_keyboard_touchpad() {
  local u
  for u in "${WS_TARGET_USERS[@]}"; do
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/commands/custom/Super_L" string "xfce4-popup-meowmenu"
    # Fallback if meowmenu missing — appfinder
    if ! pkg_installed xfce4-meowmenu-plugin; then
      look_xfconf_create "$u" xfce4-keyboard-shortcuts "/commands/custom/Super_L" string "xfce4-popup-whiskermenu"
    fi
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/commands/custom/<Super>l" string "xflock4"
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/commands/custom/<Super>e" string "thunar"
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/xfwm4/custom/<Super>Left" string "tile_left_key"
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/xfwm4/custom/<Super>Right" string "tile_right_key"
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/xfwm4/custom/<Super>Up" string "tile_up_key"
    look_xfconf_create "$u" xfce4-keyboard-shortcuts "/xfwm4/custom/<Super>Down" string "tile_down_key"
  done
  # Tap-to-click via synaptics/libinput — best effort
  mkdir -p /etc/X11/xorg.conf.d
  cat > /etc/X11/xorg.conf.d/40-libinput-tap.conf <<'EOF'
Section "InputClass"
  Identifier "libinput touchpad catchall"
  MatchIsTouchpad "on"
  Driver "libinput"
  Option "Tapping" "on"
  Option "NaturalScrolling" "true"
EndSection
EOF
}

cfg_qt_match_gtk() {
  mkdir -p /etc/environment.d
  cat > /etc/environment.d/99-ws-qt.conf <<'EOF'
QT_QPA_PLATFORMTHEME=qt5ct
EOF
  local u home
  for u in "${WS_TARGET_USERS[@]}"; do
    home="$(getent passwd "$u" | cut -d: -f6)"
    mkdir -p "$home/.config/qt5ct" "$home/.config/qt6ct"
    cat > "$home/.config/qt5ct/qt5ct.conf" <<'EOF'
[Appearance]
style=Fusion
icon_theme=Mackes-Carbon
EOF
    cp "$home/.config/qt5ct/qt5ct.conf" "$home/.config/qt6ct/qt6ct.conf" 2>/dev/null || true
    chown -R "$u:$u" "$home/.config/qt5ct" "$home/.config/qt6ct" 2>/dev/null || true
  done
}

cfg_docker_group() {
  getent group docker >/dev/null || groupadd docker
  local u
  for u in "${WS_TARGET_USERS[@]}"; do
    usermod -aG docker "$u" 2>/dev/null || true
  done
  systemctl enable --now docker.service 2>/dev/null || true
}

cfg_libvirt_group() {
  local u
  for u in "${WS_TARGET_USERS[@]}"; do
    usermod -aG libvirt "$u" 2>/dev/null || true
  done
  systemctl enable --now libvirtd.service 2>/dev/null || true
}

cfg_vscode_inotify() {
  cat > /etc/sysctl.d/99-ws-inotify.conf <<'EOF'
fs.inotify.max_user_watches=524288
fs.inotify.max_user_instances=1024
EOF
  sysctl --system >/dev/null 2>&1 || true
}

cfg_dnf_automatic_security() {
  ensure_pkgs "dnf-automatic" dnf5-plugin-automatic || ensure_pkgs "dnf-automatic" dnf-automatic
  mkdir -p /etc/dnf/automatic.conf.d 2>/dev/null || true
  if [[ -f /etc/dnf/automatic.conf ]]; then
    sed -i 's/^upgrade_type.*/upgrade_type = security/' /etc/dnf/automatic.conf || true
    sed -i 's/^apply_updates.*/apply_updates = yes/' /etc/dnf/automatic.conf || true
  fi
  systemctl enable --now dnf5-automatic.timer 2>/dev/null \
    || systemctl enable --now dnf-automatic.timer 2>/dev/null || true
}

cfg_sysctl_harden() {
  cat > /etc/sysctl.d/99-ws-harden.conf <<'EOF'
kernel.kptr_restrict=2
kernel.dmesg_restrict=1
net.ipv4.conf.all.rp_filter=1
net.ipv4.conf.default.rp_filter=1
net.ipv4.tcp_syncookies=1
net.ipv4.conf.all.accept_redirects=0
net.ipv6.conf.all.accept_redirects=0
EOF
  sysctl --system >/dev/null 2>&1 || true
}

cfg_ssh_fail2ban() {
  ensure_pkgs "fail2ban" fail2ban
  mkdir -p /etc/fail2ban/jail.d
  cat > /etc/fail2ban/jail.d/sshd.local <<'EOF'
[sshd]
enabled = true
EOF
  if [[ -f /etc/ssh/sshd_config ]]; then
    sed -i 's/^#\?PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config || true
    sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config || true
  fi
  systemctl enable --now fail2ban 2>/dev/null || true
}

cfg_power_tuned() {
  systemctl enable --now tuned 2>/dev/null || true
  tuned-adm profile balanced 2>/dev/null || true
  systemctl enable --now thermald 2>/dev/null || true
}

cfg_fingerprint() {
  authselect enable-feature with-fingerprint 2>/dev/null || true
  systemctl enable --now fprintd 2>/dev/null || true
}

cfg_backup_stack() {
  if [[ "$WS_ROOT_FS" == "btrfs" ]]; then
    ensure_pkgs "backup-btrfs" snapper btrfs-assistant vorta
  else
    ensure_pkgs "backup-other" timeshift vorta
  fi
}

cfg_vscode_repo() {
  rpm --import https://packages.microsoft.com/keys/microsoft.asc 2>/dev/null || true
  cat > /etc/yum.repos.d/vscode.repo <<'EOF'
[code]
name=Visual Studio Code
baseurl=https://packages.microsoft.com/yumrepos/vscode
enabled=1
gpgcheck=1
gpgkey=https://packages.microsoft.com/keys/microsoft.asc
EOF
}

cfg_edge_repo() {
  rpm --import https://packages.microsoft.com/keys/microsoft.asc 2>/dev/null || true
  cat > /etc/yum.repos.d/microsoft-edge.repo <<'EOF'
[microsoft-edge]
name=microsoft-edge
baseurl=https://packages.microsoft.com/yumrepos/edge
enabled=1
gpgcheck=1
gpgkey=https://packages.microsoft.com/keys/microsoft.asc
EOF
}

cfg_chrome_repo() {
  dnf_yes install fedora-workstation-repositories
  dnf_yes config-manager setopt google-chrome.enabled=1 2>/dev/null \
    || dnf_yes config-manager --set-enabled google-chrome 2>/dev/null || true
}

cfg_flathub_full() {
  ensure_pkgs "flatpak" flatpak
  if is_dry_run; then
    info "DRY-RUN: flatpak remote-add flathub (unfiltered, system)"
    return 0
  fi
  flatpak remote-add --if-not-exists --system flathub \
    https://flathub.org/repo/flathub.flatpakrepo
}

cfg_print_enable() {
  systemctl enable --now cups.service >/dev/null 2>&1 || true
  systemctl enable --now avahi-daemon.service >/dev/null 2>&1 || true
}

# Fedora's tuned-ppd and power-profiles-daemon both provide ppd-service and conflict.
# Keep whichever is already installed. Otherwise use tuned-ppd, which the hardware
# catalog also installs.
cfg_power_profiles() {
  local unit="tuned-ppd.service"
  if pkg_installed power-profiles-daemon && ! pkg_installed tuned-ppd; then
    unit="power-profiles-daemon.service"
  elif ! pkg_installed tuned-ppd && ! pkg_installed power-profiles-daemon; then
    ensure_pkgs "power-profiles" tuned-ppd
  else
    info "Power profiles already provided by tuned-ppd"
  fi
  if is_dry_run; then
    info "DRY-RUN: systemctl enable --now $unit"
    return 0
  fi
  systemctl enable --now "$unit" >/dev/null 2>&1 || true
}

# libdvdcss is in the tainted repo, which does not exist until its release package
# is installed. One dnf transaction cannot see the package it is about to enable.
cfg_dvd_playback() {
  ensure_pkgs "rpmfusion-free-release-tainted" rpmfusion-free-release-tainted
  if is_dry_run; then
    info "DRY-RUN: install libdvdcss from the tainted repo"
    return 0
  fi
  if ! pkg_installed rpmfusion-free-release-tainted; then
    return 0
  fi
  ensure_pkgs "libdvdcss" libdvdcss
}

cfg_bt_audio() {
  ensure_pkgs "bt-audio" bluez blueman pipewire pipewire-pulseaudio wireplumber
  systemctl enable --now bluetooth.service >/dev/null 2>&1 || true
}

cfg_sshd() {
  ensure_pkgs "openssh-server" openssh-server
  if is_dry_run; then
    info "DRY-RUN: systemctl enable --now sshd"
    return 0
  fi
  systemctl enable --now sshd.service >/dev/null 2>&1 || true
}

cfg_avahi() {
  ensure_pkgs "avahi" avahi
  if is_dry_run; then
    info "DRY-RUN: systemctl enable --now avahi-daemon"
    return 0
  fi
  systemctl enable --now avahi-daemon.service >/dev/null 2>&1 || true
}

cfg_network_neighbourhood() {
  # Optional Network Neighbourhood (XFCE/MDE) — not for PiPedal.
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    info "Skipping Network Neighbourhood on PiPedal"
    return 0
  fi
  ensure_pkgs "network-neighbourhood-runtime" \
    fuse3 fuse-sshfs cifs-utils nfs-utils davfs2 avahi avahi-tools samba-client \
    gtk4 polkit
  ensure_pkgs "network-neighbourhood-build" \
    cargo rust gtk4-devel openssl-devel fuse3-devel gcc pkgconf-pkg-config
  if is_dry_run; then
    info "DRY-RUN: build/install Network Neighbourhood"
    return 0
  fi
  local installer="$WS_ROOT/mde/data/network-neighbourhood/install.sh"
  if [[ -x "$installer" ]]; then
    bash "$installer"
  elif [[ -f "$installer" ]]; then
    bash "$installer"
  else
    error "Network Neighbourhood installer missing: $installer"
    return 1
  fi
}

# --- audio production ------------------------------------------------------

WS_REAPER_FALLBACK_URL="https://www.reaper.fm/files/7.x/reaper785_linux_x86_64.tar.xz"
WS_AUDINUX_REPO="/etc/yum.repos.d/_copr:copr.fedorainfracloud.org:ycollet:audinux.repo"

reaper_latest_url() {
  local path
  path="$(curl -fsSL https://www.reaper.fm/download.php 2>/dev/null \
    | grep -oE 'files/[0-9]+\.x/reaper[0-9]+_linux_x86_64\.tar\.xz' | head -n1)"
  if [[ -n "$path" ]]; then
    printf 'https://www.reaper.fm/%s\n' "$path"
  else
    printf '%s\n' "$WS_REAPER_FALLBACK_URL"
  fi
}

# REAPER only scans /usr/lib/lv2 and /usr/local/lib/lv2 by default; Fedora
# installs LV2 bundles into /usr/lib64/lv2.
cfg_reaper_lv2_path() {
  local lv2path='~/.lv2;/usr/lib64/lv2;/usr/lib/lv2;/usr/local/lib64/lv2;/usr/local/lib/lv2'
  local u home ini
  for u in "${WS_TARGET_USERS[@]}"; do
    home="$(getent passwd "$u" | cut -d: -f6)"
    [[ -n "$home" && -d "$home" ]] || continue
    ini="$home/.config/REAPER/reaper.ini"
    mkdir -p "$home/.config/REAPER"
    if [[ ! -f "$ini" ]]; then
      printf '[REAPER]\nlv2path_linux=%s\n' "$lv2path" > "$ini"
    elif grep -q '^lv2path_linux=' "$ini"; then
      grep -q '/usr/lib64/lv2' "$ini" || sed -i "s|^lv2path_linux=.*|lv2path_linux=$lv2path|" "$ini"
    elif grep -q '^\[REAPER\]' "$ini"; then
      sed -i "/^\[REAPER\]/a lv2path_linux=$lv2path" "$ini"
    else
      printf '\n[REAPER]\nlv2path_linux=%s\n' "$lv2path" >> "$ini"
    fi
    chown -R "$u:$u" "$home/.config/REAPER" 2>/dev/null || true
  done
}

cfg_reaper() {
  ensure_pkgs "reaper-deps" curl tar xz xdg-utils gtk3 pipewire-jack-audio-connection-kit
  local url ver have=""
  url="$(reaper_latest_url)"
  ver="$(basename "$url" | grep -oE '[0-9]+' | head -n1)"
  [[ -f /opt/REAPER/.ws-version ]] && have="$(cat /opt/REAPER/.ws-version)"
  if [[ -x /opt/REAPER/reaper && "$have" == "$ver" ]]; then
    info "REAPER $ver already installed"
  else
    local tmp
    tmp="$(mktemp -d)"
    info "Downloading REAPER: $url"
    if ! curl -fsSL "$url" -o "$tmp/reaper.tar.xz" || ! tar -xJf "$tmp/reaper.tar.xz" -C "$tmp"; then
      rm -rf "$tmp"
      error "REAPER download failed: $url"
      return 1
    fi
    if ! sh "$tmp/reaper_linux_x86_64/install-reaper.sh" --install /opt \
        --integrate-sys-desktop --usr-local-bin-symlink --quiet >>"$WS_LOG_FILE" 2>&1; then
      rm -rf "$tmp"
      error "REAPER installer failed"
      return 1
    fi
    printf '%s\n' "$ver" > /opt/REAPER/.ws-version
    rm -rf "$tmp"
    info "Installed REAPER $ver to /opt/REAPER"
  fi
  cfg_reaper_lv2_path
}

# Audinux carries the LV2 plugins Fedora does not. Priority 100 (Fedora is 99)
# keeps Fedora's builds of anything both repos ship.
cfg_audinux_repo() {
  [[ -f "$WS_AUDINUX_REPO" ]] || ensure_copr ycollet/audinux
  if [[ -f "$WS_AUDINUX_REPO" ]] && ! grep -q '^priority=' "$WS_AUDINUX_REPO"; then
    sed -i '/^\[copr:copr.fedorainfracloud.org:ycollet:audinux\]/a priority=100' "$WS_AUDINUX_REPO"
  fi
}

cfg_audio_realtime() {
  ensure_pkgs "audio-realtime" realtime-setup pipewire-jack-audio-connection-kit qpwgraph
  getent group realtime >/dev/null || groupadd -r realtime
  local u
  for u in "${WS_TARGET_USERS[@]}"; do
    usermod -aG realtime,audio "$u" 2>/dev/null || true
  done
}

# Top 20 English Google Fonts. Repo packages are preferred; the rest are
# downloaded as family zips into /usr/share/fonts/google.
cfg_google_fonts() {
  ensure_pkgs "font-tools" curl unzip fontconfig
  local candidates=(
    google-roboto-fonts
    open-sans-fonts
    lato-fonts
    adobe-source-sans-3-fonts
    adobe-source-sans-pro-fonts
    rsms-inter-fonts
    paratype-pt-sans-fonts
    google-noto-sans-fonts
    google-noto-sans-cjk-fonts
    google-noto-color-emoji-fonts
    liberation-fonts
    cascadia-code-fonts
    fira-sans-fonts
    ubuntu-fonts
  )
  local p
  for p in "${candidates[@]}"; do
    if is_dry_run; then
      info "DRY-RUN: try font package $p"
    else
      dnf_yes install "$p" >/dev/null 2>&1 || true
    fi
  done
  local families=(
    "Roboto"
    "Open Sans"
    "Lato"
    "Montserrat"
    "Oswald"
    "Raleway"
    "Poppins"
    "Nunito"
    "Merriweather"
    "Playfair Display"
    "Source Sans 3"
    "Inter"
    "Work Sans"
    "Rubik"
    "Mukta"
    "PT Sans"
    "Noto Sans"
    "Ubuntu"
    "Fira Sans"
    "Karla"
  )
  local fam slug zip url
  mkdir -p /usr/share/fonts/google
  for fam in "${families[@]}"; do
    if fc-list : family 2>/dev/null | grep -qiF "$fam"; then
      info "Font already available: $fam"
      continue
    fi
    slug="$(printf '%s' "$fam" | tr '[:upper:]' '[:lower:]' | tr ' ' '-')"
    if is_dry_run; then
      info "DRY-RUN: download Google Font $fam"
      continue
    fi
    url="https://fonts.google.com/download?family=$(printf '%s' "$fam" | sed 's/ /%20/g')"
    zip="$(mktemp --suffix=.zip)"
    if curl -fsSL "$url" -o "$zip" && unzip -qo "$zip" -d "/usr/share/fonts/google/$slug"; then
      info "Installed Google Font: $fam"
    else
      warn "Could not download Google Font: $fam"
    fi
    rm -f "$zip"
  done
  fc-cache -f /usr/share/fonts/google >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

# --- config undo helpers (best-effort) -----------------------------------

cfg_firefox_policy_undo() {
  rm -f /etc/firefox/policies/policies.json
  rmdir /etc/firefox/policies 2>/dev/null || true
}

cfg_keyboard_touchpad_undo() {
  rm -f /etc/X11/xorg.conf.d/40-libinput-tap.conf
}

cfg_qt_match_gtk_undo() {
  rm -f /etc/environment.d/99-ws-qt.conf
}

cfg_docker_group_undo() {
  systemctl disable --now docker.service >/dev/null 2>&1 || true
}

cfg_libvirt_group_undo() {
  systemctl disable --now libvirtd.service >/dev/null 2>&1 || true
}

cfg_vscode_inotify_undo() {
  rm -f /etc/sysctl.d/99-ws-inotify.conf
  sysctl --system >/dev/null 2>&1 || true
}

cfg_dnf_automatic_security_undo() {
  systemctl disable --now dnf5-automatic.timer >/dev/null 2>&1 || true
  systemctl disable --now dnf-automatic.timer >/dev/null 2>&1 || true
}

cfg_sysctl_harden_undo() {
  rm -f /etc/sysctl.d/99-ws-harden.conf
  sysctl --system >/dev/null 2>&1 || true
}

cfg_ssh_fail2ban_undo() {
  systemctl disable --now fail2ban >/dev/null 2>&1 || true
}

cfg_power_tuned_undo() {
  systemctl disable --now tuned >/dev/null 2>&1 || true
  systemctl disable --now thermald >/dev/null 2>&1 || true
}

cfg_fingerprint_undo() {
  systemctl disable --now fprintd >/dev/null 2>&1 || true
}

cfg_flathub_full_undo() {
  if flatpak remotes --system 2>/dev/null | grep -qw flathub; then
    flatpak remote-delete --system flathub >/dev/null 2>&1 || true
  fi
}

cfg_print_enable_undo() {
  systemctl disable --now cups.service >/dev/null 2>&1 || true
}

cfg_power_profiles_undo() {
  systemctl disable --now tuned-ppd.service >/dev/null 2>&1 || true
  systemctl disable --now power-profiles-daemon.service >/dev/null 2>&1 || true
}

cfg_bt_audio_undo() {
  systemctl disable --now bluetooth.service >/dev/null 2>&1 || true
}

cfg_sshd_undo() {
  systemctl disable --now sshd.service >/dev/null 2>&1 || true
}

cfg_avahi_undo() {
  systemctl disable --now avahi-daemon.service >/dev/null 2>&1 || true
}

cfg_google_fonts_undo() {
  rm -rf /usr/share/fonts/google
  fc-cache -f >/dev/null 2>&1 || true
}

cfg_reaper_undo() {
  if [[ -x /opt/REAPER/install-reaper.sh ]]; then
    sh /opt/REAPER/install-reaper.sh --uninstall --quiet >/dev/null 2>&1 || true
  fi
  rm -rf /opt/REAPER
  rm -f /usr/local/bin/reaper /usr/share/applications/cockos-reaper.desktop 2>/dev/null || true
}

cfg_reaper_lv2_path_undo() {
  local u home ini
  for u in "${WS_TARGET_USERS[@]+"${WS_TARGET_USERS[@]}"}"; do
    home="$(getent passwd "$u" | cut -d: -f6)"
    ini="$home/.config/REAPER/reaper.ini"
    [[ -f "$ini" ]] || continue
    sed -i '/^lv2path_linux=/d' "$ini" 2>/dev/null || true
  done
}

cfg_audio_realtime_undo() {
  true
}

cfg_network_neighbourhood_undo() {
  local uninstaller="$WS_ROOT/mde/data/network-neighbourhood/uninstall.sh"
  if [[ -x "$uninstaller" ]]; then
    bash "$uninstaller" || true
  elif [[ -f "$uninstaller" ]]; then
    bash "$uninstaller" || true
  else
    warn "Network Neighbourhood uninstall helper missing"
  fi
}

cfg_vscode_repo_undo() {
  true
}

cfg_edge_repo_undo() {
  true
}

cfg_chrome_repo_undo() {
  true
}

cfg_dvd_playback_undo() {
  true
}

cfg_backup_stack_undo() {
  true
}

cfg_audinux_repo_undo() {
  true
}

# --- applied catalog state -----------------------------------------------

catalog_applied_load() {
  WS_APPLIED_IDS=()
  [[ -f "$WS_APPLIED_FILE" ]] || return 0
  local ids
  ids="$(python3 - "$WS_APPLIED_FILE" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
try:
    data = json.loads(path.read_text(encoding="utf-8"))
except Exception:
    data = {}
items = data.get("items") or {}
for key in items:
    print(key)
PY
)" || return 0
  local id
  while IFS= read -r id; do
    [[ -n "$id" ]] || continue
    WS_APPLIED_IDS["$id"]=1
  done <<<"$ids"
}

catalog_applied_save() {
  if is_dry_run; then
    info "DRY-RUN: write $WS_APPLIED_FILE (${#WS_APPLIED_IDS[@]} ids)"
    return 0
  fi
  mkdir -p "$(dirname "$WS_APPLIED_FILE")"
  local keys=()
  local k
  for k in "${!WS_APPLIED_IDS[@]}"; do
    keys+=("$k")
  done
  python3 - "$WS_APPLIED_FILE" "${keys[@]}" <<'PY'
import json, pathlib, sys, datetime
dest = pathlib.Path(sys.argv[1])
ids = sys.argv[2:]
now = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")
prev = {}
if dest.is_file():
    try:
        prev = (json.loads(dest.read_text(encoding="utf-8")).get("items") or {})
    except Exception:
        prev = {}
items = {}
for i in ids:
    items[i] = prev.get(i) or {"ts": now}
    if "ts" not in items[i]:
        items[i]["ts"] = now
dest.write_text(json.dumps({"items": items}, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
}

catalog_applied_mark() {
  local id="$1"
  [[ -n "$id" ]] || return 0
  WS_APPLIED_IDS["$id"]=1
  catalog_applied_save
}

catalog_applied_unmark() {
  local id="$1"
  [[ -n "$id" ]] || return 0
  unset "WS_APPLIED_IDS[$id]"
  catalog_applied_save
}

catalog_cfg_present() {
  local fn="$1"
  case "$fn" in
    cfg_firefox_policy) [[ -f /etc/firefox/policies/policies.json ]] ;;
    cfg_keyboard_touchpad) [[ -f /etc/X11/xorg.conf.d/40-libinput-tap.conf ]] ;;
    cfg_qt_match_gtk) [[ -f /etc/environment.d/99-ws-qt.conf ]] ;;
    cfg_docker_group) pkg_installed moby-engine || systemctl is-enabled docker.service >/dev/null 2>&1 ;;
    cfg_libvirt_group) systemctl is-enabled libvirtd.service >/dev/null 2>&1 ;;
    cfg_vscode_inotify) [[ -f /etc/sysctl.d/99-ws-inotify.conf ]] ;;
    cfg_dnf_automatic_security)
      systemctl is-enabled dnf5-automatic.timer >/dev/null 2>&1 \
        || systemctl is-enabled dnf-automatic.timer >/dev/null 2>&1
      ;;
    cfg_sysctl_harden) [[ -f /etc/sysctl.d/99-ws-harden.conf ]] ;;
    cfg_ssh_fail2ban) systemctl is-enabled fail2ban >/dev/null 2>&1 || pkg_installed fail2ban ;;
    cfg_power_tuned) systemctl is-enabled tuned >/dev/null 2>&1 || pkg_installed tuned ;;
    cfg_fingerprint) systemctl is-enabled fprintd >/dev/null 2>&1 || pkg_installed fprintd ;;
    cfg_flathub_full) flatpak remotes --system 2>/dev/null | grep -qw flathub ;;
    cfg_print_enable) systemctl is-enabled cups.service >/dev/null 2>&1 || pkg_installed cups ;;
    cfg_power_profiles)
      pkg_installed tuned-ppd || pkg_installed power-profiles-daemon \
        || systemctl is-enabled tuned-ppd.service >/dev/null 2>&1 \
        || systemctl is-enabled power-profiles-daemon.service >/dev/null 2>&1
      ;;
    cfg_bt_audio) pkg_installed bluez || systemctl is-enabled bluetooth.service >/dev/null 2>&1 ;;
    cfg_sshd) systemctl is-enabled sshd.service >/dev/null 2>&1 || pkg_installed openssh-server ;;
    cfg_avahi) systemctl is-enabled avahi-daemon.service >/dev/null 2>&1 || pkg_installed avahi ;;
    cfg_google_fonts) [[ -d /usr/share/fonts/google ]] && [[ -n "$(ls -A /usr/share/fonts/google 2>/dev/null)" ]] ;;
    cfg_reaper) [[ -x /opt/REAPER/reaper ]] ;;
    cfg_reaper_lv2_path) return 1 ;;
    cfg_audio_realtime) pkg_installed realtime-setup ;;
    cfg_network_neighbourhood) command -v mde-neighbourhood >/dev/null 2>&1 || pkg_installed fuse-sshfs ;;
    cfg_dvd_playback) pkg_installed libdvdcss ;;
    cfg_vscode_repo|cfg_edge_repo|cfg_chrome_repo|cfg_audinux_repo|cfg_backup_stack) return 1 ;;
    *) return 1 ;;
  esac
}

# --- catalog IO ----------------------------------------------------------

catalog_files() {
  find "$WS_CATALOG_DIR" -maxdepth 1 -name '*.conf' | sort
}

catalog_parse_line() {
  # sets: C_ID C_LABEL C_KIND C_PAYLOAD C_GUARD C_DE
  local line="$1"
  [[ "$line" =~ ^[[:space:]]*# ]] && return 1
  [[ -z "${line// }" ]] && return 1
  IFS='|' read -r C_ID C_LABEL C_KIND C_PAYLOAD C_GUARD C_DE <<<"$line"
  [[ -n "$C_ID" ]] || return 1
  C_GUARD="${C_GUARD:-always}"
  C_DE="${C_DE:-any}"
  C_DE="${C_DE//$'\r'/}"
  return 0
}

catalog_visible_file() {
  local base
  base="$(basename "$1")"
  if [[ "${WS_DE:-xfce}" != "pipedal" ]]; then
    [[ "$base" == "00-recommended-pipedal.conf" ]] && return 1
    return 0
  fi
  case "$base" in
    00-recommended-pipedal.conf|09-security.conf|10-hardware.conf|12-audio-production.conf) return 0 ;;
    *) return 1 ;;
  esac
}

catalog_de_ok() {
  local de="${1:-any}"
  case "$de" in
    any|"") return 0 ;;
    workstation) [[ "${WS_DE:-xfce}" != "pipedal" ]] ;;
    pipedal) [[ "${WS_DE:-xfce}" == "pipedal" ]] ;;
    xfce) [[ "${WS_DE:-xfce}" == "xfce" ]] ;;
    sway|mde) [[ "${WS_DE:-xfce}" == "mde" || "${WS_DE:-xfce}" == "sway" ]] ;;
    *) return 0 ;;
  esac
}

catalog_guard_ok() {
  local guard="$1"
  case "$guard" in
    always|"") return 0 ;;
    surface) [[ "$WS_SURFACE" == "1" ]] ;;
    surface-pro) [[ "$WS_SURFACE" == "1" && "$WS_SURFACE_FAMILY" == "pro" ]] ;;
    surface-laptop) [[ "$WS_SURFACE" == "1" && "$WS_SURFACE_FAMILY" == "laptop" ]] ;;
    btrfs) [[ "$WS_ROOT_FS" == "btrfs" ]] ;;
    not-btrfs) [[ "$WS_ROOT_FS" != "btrfs" ]] ;;
    nvidia) [[ "$WS_GPU" == "nvidia" ]] ;;
    intel) [[ "$WS_GPU" == "intel" ]] ;;
    amd) [[ "$WS_GPU" == "amd" ]] ;;
    # bash case uses [...], not {...}, for character classes
    fedora\>=[1-9]*)
      local need="${guard#fedora>=}"
      [[ "$need" =~ ^[0-9]+$ ]] || return 0
      (( WS_FEDORA_VER >= need ))
      ;;
    *)
      warn "Unknown guard '$guard' — treating as always"
      return 0
      ;;
  esac
}

catalog_item_available() {
  # Checklist load must stay instant: never call dnf/repoquery here.
  # Apply-time failures are reported in the log / results screen instead.
  case "$1" in
    rpm|mix|flatpak|copr|repo|config) return 0 ;;
    *) return 0 ;;
  esac
}

catalog_payload_probe() {
  local kind="$1" payload="$2"
  case "$kind" in
    rpm)
      local p
      for p in $payload; do
        pkg_installed "$p" || return 1
      done
      return 0
      ;;
    flatpak)
      flatpak_installed "$payload"
      ;;
    config)
      catalog_cfg_present "$payload"
      ;;
    copr)
      local pkgs
      pkgs="$(echo "$payload" | cut -d' ' -f2-)"
      local p
      for p in $pkgs; do
        pkg_installed "$p" || return 1
      done
      return 0
      ;;
    repo)
      local rest first
      first="$(echo "$payload" | awk '{print $1}')"
      rest="$(echo "$payload" | cut -d' ' -f2-)"
      if declare -F "$first" >/dev/null 2>&1; then
        local tok
        for tok in $rest; do
          if declare -F "$tok" >/dev/null 2>&1; then
            catalog_cfg_present "$tok" && return 0
          elif [[ "$tok" == *.* && "$tok" != */* ]]; then
            flatpak_installed "$tok" && return 0
          else
            pkg_installed "$tok" && return 0
          fi
        done
        return 1
      fi
      local p
      for p in $rest; do
        pkg_installed "$p" || return 1
      done
      return 0
      ;;
    mix)
      local tok any=0
      for tok in $payload; do
        if declare -F "$tok" >/dev/null 2>&1; then
          if catalog_cfg_present "$tok"; then any=1; else return 1; fi
        elif [[ "$tok" == *.* && "$tok" != */* ]]; then
          if flatpak_installed "$tok"; then any=1; else return 1; fi
        else
          if pkg_installed "$tok"; then any=1; else return 1; fi
        fi
      done
      (( any == 1 ))
      ;;
    *) return 1 ;;
  esac
}

# True when applied-state or heuristic probe says the item is present.
catalog_item_installed() {
  local kind="$1" payload="$2" id="${3:-}"
  if [[ -n "$id" && -n "${WS_APPLIED_IDS[$id]:-}" ]]; then
    return 0
  fi
  catalog_payload_probe "$kind" "$payload"
}

catalog_item_present_id() {
  local id="$1"
  catalog_lookup "$id" || return 1
  catalog_item_installed "$C_KIND" "$C_PAYLOAD" "$C_ID"
}

catalog_category_title() {
  local file="$1"
  local base
  base="$(basename "$file" .conf)"
  # 01-devops → DevOps
  base="${base#[0-9]*-}"
  local title
  title="$(echo "$base" | sed -E 's/-/ /g; s/\b(.)/\u\1/g')"
  case "$title" in
    Devops) printf '%s\n' "DevOps" ;;
    "Recommended Pipedal") printf '%s\n' "Recommended" ;;
    *) printf '%s\n' "$title" ;;
  esac
}

catalog_load_checklist_args() {
  # Prints checklist quads (tag, label, ON/OFF, PRESENT/ABSENT) for a conf file.
  # Defaults: present → ON [keep]; recommended absent → ON [install]; else OFF (leave).
  local file="$1"
  local line status label present=0
  catalog_applied_load
  while IFS= read -r line || [[ -n "$line" ]]; do
    catalog_parse_line "$line" || continue
    catalog_guard_ok "$C_GUARD" || continue
    catalog_de_ok "$C_DE" || continue
    catalog_item_available "$C_KIND" "$C_PAYLOAD" || continue
    present=0
    if catalog_item_installed "$C_KIND" "$C_PAYLOAD" "$C_ID"; then
      present=1
    fi
    status="OFF"
    label="$C_LABEL"
    if (( present == 1 )); then
      status="ON"
      label="$C_LABEL [keep]"
    else
      case "$(basename "$file")" in
        00-recommended.conf|00-recommended-pipedal.conf)
          status="ON"
          label="$C_LABEL [install]"
          ;;
      esac
    fi
    printf '%s\n%s\n%s\n%s\n' "$C_ID" "$label" "$status" "$([[ $present -eq 1 ]] && echo PRESENT || echo ABSENT)"
  done < "$file"
}

catalog_lookup() {
  local id="$1"
  local f line
  for f in $(catalog_files); do
    while IFS= read -r line || [[ -n "$line" ]]; do
      catalog_parse_line "$line" || continue
      if [[ "$C_ID" == "$id" ]]; then
        C_FILE="$f"
        return 0
      fi
    done < "$f"
  done
  return 1
}

catalog_apply_item() {
  local id="$1"
  if ! catalog_lookup "$id"; then
    error "Unknown catalog id: $id"
    WS_FAILURES+=("$id")
    WS_FAILED=$((WS_FAILED + 1))
    return 0
  fi
  if ! catalog_visible_file "${C_FILE:-}" || ! catalog_de_ok "$C_DE"; then
    error "Catalog item $id is not part of the ${WS_DE:-xfce} install"
    WS_FAILURES+=("$id")
    WS_FAILED=$((WS_FAILED + 1))
    return 0
  fi
  info "Catalog apply: $C_ID ($C_KIND) — $C_LABEL"
  if declare -F ui_progress_item >/dev/null 2>&1; then
    ui_progress_item apply "$C_ID" "$C_LABEL"
  fi
  local before_failed=$WS_FAILED
  case "$C_KIND" in
    rpm)
      # shellcheck disable=SC2086
      ensure_pkgs "$C_ID" $C_PAYLOAD
      ;;
    flatpak)
      ensure_flatpak "$C_PAYLOAD"
      ;;
    copr)
      # payload: "owner/project pkg1 pkg2..."
      local project pkgs
      project="$(echo "$C_PAYLOAD" | awk '{print $1}')"
      pkgs="$(echo "$C_PAYLOAD" | cut -d' ' -f2-)"
      ensure_copr "$project"
      # shellcheck disable=SC2086
      ensure_pkgs "$C_ID" $pkgs
      ;;
    repo)
      # payload starts with repo setup function or URL then packages
      local first rest
      first="$(echo "$C_PAYLOAD" | awk '{print $1}')"
      rest="$(echo "$C_PAYLOAD" | cut -d' ' -f2-)"
      if declare -F "$first" >/dev/null 2>&1; then
        if is_dry_run; then info "DRY-RUN: $first"; else "$first" || true; fi
        # shellcheck disable=SC2086
        ensure_pkgs "$C_ID" $rest
      else
        dnf_add_repo_file "$first"
        # shellcheck disable=SC2086
        ensure_pkgs "$C_ID" $rest
      fi
      ;;
    config)
      apply_config "$C_PAYLOAD"
      ;;
    mix)
      # Tokens: cfg_*/declared functions → config; reverse-dns → flatpak; else rpm
      local tok
      for tok in $C_PAYLOAD; do
        if declare -F "$tok" >/dev/null 2>&1; then
          apply_config "$tok"
        elif [[ "$tok" == *.* && "$tok" != */* ]]; then
          ensure_flatpak "$tok"
        else
          ensure_pkgs "$C_ID:$tok" "$tok"
        fi
      done
      ;;
    *)
      error "Unknown kind $C_KIND for $C_ID"
      WS_FAILURES+=("$C_ID")
      WS_FAILED=$((WS_FAILED + 1))
      ;;
  esac
  local ok=1
  if (( WS_FAILED == before_failed )); then
    catalog_applied_mark "$C_ID"
  else
    ok=0
  fi
  if declare -F ui_progress_item_done >/dev/null 2>&1; then
    ui_progress_item_done "$C_ID" "$ok"
  fi
}

# --- remove / keep-list / refcount ---------------------------------------

catalog_keep_pkgs() {
  local p
  for p in "${BASE_KEEP_APPS[@]+"${BASE_KEEP_APPS[@]}"}"; do printf '%s\n' "$p"; done
  for p in "${BASE_PLUMBING[@]+"${BASE_PLUMBING[@]}"}"; do printf '%s\n' "$p"; done
  for p in "${BASE_THEME_DEPS[@]+"${BASE_THEME_DEPS[@]}"}"; do printf '%s\n' "$p"; done
  for p in "${BASE_CODECS[@]+"${BASE_CODECS[@]}"}"; do printf '%s\n' "$p"; done
  # Session-critical stack — never stripped by catalog remove.
  printf '%s\n' \
    pipewire pipewire-pulseaudio pipewire-alsa pipewire-jack-audio-connection-kit \
    wireplumber bluez NetworkManager NetworkManager-wifi \
    mesa-dri-drivers mesa-vulkan-drivers \
    labwc seatd greetd
  # Protect the desktop we are installing/keeping unless uninstalling it.
  if [[ "${WS_UNINSTALL_DE:-}" != "xfce" && "${WS_UNINSTALL_DE:-}" != "all" ]]; then
    de_pkg_list xfce
  fi
  if [[ "${WS_UNINSTALL_DE:-}" != "mde" && "${WS_UNINSTALL_DE:-}" != "all" ]]; then
    de_pkg_list mde
  fi
  if [[ "${WS_DE:-xfce}" != "pipedal" ]]; then
    de_pkg_list display
  fi
}

catalog_id_in_list() {
  local needle="$1"; shift
  local x
  for x in "$@"; do
    [[ "$x" == "$needle" ]] && return 0
  done
  return 1
}

# Emit rpm package names referenced by a catalog payload (best-effort).
catalog_payload_rpms() {
  local kind="$1" payload="$2"
  case "$kind" in
    rpm)
      # shellcheck disable=SC2086
      printf '%s\n' $payload
      ;;
    copr)
      echo "$payload" | cut -d' ' -f2- | tr ' ' '\n'
      ;;
    repo)
      echo "$payload" | cut -d' ' -f2- | tr ' ' '\n' | while read -r tok; do
        [[ -n "$tok" ]] || continue
        declare -F "$tok" >/dev/null 2>&1 && continue
        [[ "$tok" == *.* && "$tok" != */* ]] && continue
        printf '%s\n' "$tok"
      done
      ;;
    mix)
      local tok
      for tok in $payload; do
        declare -F "$tok" >/dev/null 2>&1 && continue
        [[ "$tok" == *.* && "$tok" != */* ]] && continue
        printf '%s\n' "$tok"
      done
      ;;
    config)
      case "$payload" in
        cfg_bt_audio) printf '%s\n' bluez blueman pipewire pipewire-pulseaudio wireplumber ;;
        cfg_sshd) printf '%s\n' openssh-server ;;
        cfg_avahi) printf '%s\n' avahi ;;
        cfg_power_profiles) printf '%s\n' tuned-ppd power-profiles-daemon ;;
        cfg_dvd_playback) printf '%s\n' libdvdcss rpmfusion-free-release-tainted ;;
        cfg_audio_realtime) printf '%s\n' realtime-setup pipewire-jack-audio-connection-kit qpwgraph ;;
        cfg_dnf_automatic_security) printf '%s\n' dnf5-plugin-automatic dnf-automatic ;;
        cfg_ssh_fail2ban) printf '%s\n' fail2ban ;;
      esac
      ;;
  esac
}

catalog_payload_flatpaks() {
  local kind="$1" payload="$2"
  case "$kind" in
    flatpak) printf '%s\n' "$payload" ;;
    mix)
      local tok
      for tok in $payload; do
        if [[ "$tok" == *.* && "$tok" != */* ]] && ! declare -F "$tok" >/dev/null 2>&1; then
          printf '%s\n' "$tok"
        fi
      done
      ;;
  esac
}

catalog_wanted_pkgs() {
  # Packages still wanted by install picks + kept present items + base keep-list.
  local id
  catalog_keep_pkgs
  for id in "${WS_SELECTED_ITEMS[@]+"${WS_SELECTED_ITEMS[@]}"}"; do
    catalog_lookup "$id" || continue
    catalog_payload_rpms "$C_KIND" "$C_PAYLOAD"
  done
  # Kept: present and not queued for remove
  local f line
  for f in $(catalog_files); do
    catalog_visible_file "$f" || continue
    while IFS= read -r line || [[ -n "$line" ]]; do
      catalog_parse_line "$line" || continue
      catalog_guard_ok "$C_GUARD" || continue
      catalog_de_ok "$C_DE" || continue
      catalog_id_in_list "$C_ID" "${WS_REMOVE_ITEMS[@]+"${WS_REMOVE_ITEMS[@]}"}" && continue
      catalog_id_in_list "$C_ID" "${WS_SELECTED_ITEMS[@]+"${WS_SELECTED_ITEMS[@]}"}" && continue
      if catalog_item_installed "$C_KIND" "$C_PAYLOAD" "$C_ID"; then
        catalog_payload_rpms "$C_KIND" "$C_PAYLOAD"
      fi
    done < "$f"
  done
}

catalog_remove_item() {
  local id="$1"
  if ! catalog_lookup "$id"; then
    error "Unknown catalog id: $id"
    WS_FAILURES+=("remove:$id")
    WS_FAILED=$((WS_FAILED + 1))
    return 0
  fi
  local rid="$C_ID" rkind="$C_KIND" rpayload="$C_PAYLOAD" rlabel="$C_LABEL"
  info "Catalog remove: $rid ($rkind) — $rlabel"
  if declare -F ui_progress_item >/dev/null 2>&1; then
    ui_progress_item remove "$rid" "$rlabel"
  fi
  local before_failed=$WS_FAILED
  local wanted
  wanted="$(catalog_wanted_pkgs | sort -u)"
  local pkg_candidates=() flat_candidates=()
  local p
  while IFS= read -r p; do
    [[ -n "$p" ]] || continue
    pkg_candidates+=("$p")
  done < <(catalog_payload_rpms "$rkind" "$rpayload")
  while IFS= read -r p; do
    [[ -n "$p" ]] || continue
    flat_candidates+=("$p")
  done < <(catalog_payload_flatpaks "$rkind" "$rpayload")

  local to_remove=()
  for p in "${pkg_candidates[@]+"${pkg_candidates[@]}"}"; do
    if printf '%s\n' "$wanted" | grep -qxF "$p"; then
      info "Keep shared package: $p (still wanted)"
      continue
    fi
    to_remove+=("$p")
  done
  if ((${#to_remove[@]} > 0)); then
    remove_pkgs "remove:$rid" "${to_remove[@]}"
  fi
  for p in "${flat_candidates[@]+"${flat_candidates[@]}"}"; do
    remove_flatpak "$p"
  done

  case "$rkind" in
    config)
      undo_config "$rpayload"
      ;;
    mix)
      local tok
      for tok in $rpayload; do
        if declare -F "$tok" >/dev/null 2>&1; then
          undo_config "$tok"
        fi
      done
      ;;
    repo)
      local first
      first="$(echo "$rpayload" | awk '{print $1}')"
      if declare -F "$first" >/dev/null 2>&1; then
        undo_config "$first"
      fi
      local rest tok
      rest="$(echo "$rpayload" | cut -d' ' -f2-)"
      for tok in $rest; do
        if declare -F "$tok" >/dev/null 2>&1; then
          undo_config "$tok"
        fi
      done
      ;;
  esac

  local ok=1
  if (( WS_FAILED == before_failed )); then
    catalog_applied_unmark "$rid"
  else
    ok=0
  fi
  if declare -F ui_progress_item_done >/dev/null 2>&1; then
    ui_progress_item_done "$rid" "$ok"
  fi
}

catalog_remove_selected() {
  local id
  catalog_applied_load
  if ((${#WS_REMOVE_ITEMS[@]})) && declare -F ui_progress_info >/dev/null 2>&1; then
    ui_progress_info "Removing ${#WS_REMOVE_ITEMS[@]} catalog item(s)"
  fi
  for id in "${WS_REMOVE_ITEMS[@]+"${WS_REMOVE_ITEMS[@]}"}"; do
    [[ -n "$id" ]] || continue
    catalog_remove_item "$id"
  done
}

catalog_apply_selected() {
  local id
  catalog_applied_load
  if ((${#WS_SELECTED_ITEMS[@]})) && declare -F ui_progress_info >/dev/null 2>&1; then
    ui_progress_info "Installing ${#WS_SELECTED_ITEMS[@]} catalog item(s)"
  fi
  for id in "${WS_SELECTED_ITEMS[@]}"; do
    [[ -n "$id" ]] || continue
    catalog_apply_item "$id"
  done
}
