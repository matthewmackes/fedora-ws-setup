#!/usr/bin/env bash
# MDE look: Fedora labwc with a Windows 11–proportioned centered taskbar and
# Start menu, padOS apps, and Mackes-Carbon icons. Windows stack, with a
# server-side caption, slight corner radius, and a soft shadow. Sizes follow
# the Fluent 2 type ramp and 4 px grid.
# shellcheck disable=SC2034

SWAY_ACCENT="#0078d4"
SWAY_ACCENT_TEXT="#005fb8"
SWAY_MUTED="#606984"
SWAY_PAD_FG="#1a1a1a"
SWAY_PAD_FG2="#5d5d5d"
SWAY_PAD_BORDER="#e0e0e0"
SWAY_PANEL_BG="#f3f3f3"
SWAY_PANEL_HEIGHT=48
# Fluent type ramp in points: Caption 12px, Body 14px, Body Strong 14px semibold.
SWAY_FONT="${SWAY_FONT:-Inter}"
SWAY_FONT_CAPTION="${SWAY_FONT_CAPTION:-9}"
SWAY_FONT_BODY="${SWAY_FONT_BODY:-10.5}"
SWAY_FONT_MONO="${SWAY_FONT_MONO:-Cascadia Mono}"
# Keep shared LOOK_* in sync for GTK / greeter / xfconf paths.
LOOK_FONT="${SWAY_FONT} ${SWAY_FONT_BODY}"
LOOK_TITLE_FONT="${SWAY_FONT} ${SWAY_FONT_CAPTION}"
# Laptop panel scale is derived from WS_SCALE_TIER / EDID PPI (see sway_panel_scale).
SWAY_CURSOR="Adwaita"
SWAY_CURSOR_SIZE=24
SWAY_ICON_LIGHT="Mackes-Carbon"
SWAY_ICON_DARK="Mackes-Carbon-Light"
SWAY_ICON_DIR="/usr/share/icons/${LOOK_THEME_ICON:-Mackes-Carbon}/scalable/actions"
SWAY_HELPER_DIR="${MDE_BIN:-/usr/bin}"

# Apps that tile by default but read better as floating windows. Matched
# case-insensitively against the Wayland app_id.
SWAY_FLOAT_APP_IDS="ws-settings ws-mmc mde-control org.pulseaudio.pavucontrol pavucontrol blueman-manager blueman-adapters nm-connection-editor org.gnome.Calculator gnome-calculator galculator qalculate-gtk swappy wdisplays nwg-look xfce4-appfinder"

# XFCE session extras that crash or duplicate the Sway shell; hidden from
# Sway with NotShowIn so XFCE still starts them.
SWAY_HIDDEN_AUTOSTART="autorandr input-remapper-autoload org.mageia.dnfdragora-updater plank sealertauto"

# Helpers and units from the Miracle WM version of this look.
SWAY_LEGACY_HELPERS="ws-super-tap ws-miracle-helper ws-miracle-autostart ws-lock-watch ws-clock ws-updates ws-update-run"

sway_write() {
  local path="$1"
  mkdir -p "$(dirname "$path")"
  cat >"$path"
}

# --- Session helpers (system-wide, shared by every Sway user) -------------

look_sway_install_helpers() {
  if is_dry_run; then
    info "DRY-RUN: install Rust platform helpers to $SWAY_HELPER_DIR"
    return 0
  fi
  mkdir -p "$SWAY_HELPER_DIR" /usr/share/applications /usr/share/mde

  # Archive-only: do not ship Python/labwc shell in the default install.
  # W458: keep ws-mmc Python share for the Rust bridge until W495.
  rm -rf /usr/share/mde/ws-taskbar /usr/share/mde/ws-start /usr/share/mde/ws-settings \
    /usr/share/mde/ws-greeter /usr/share/mde/ws-regedit
  rm -f /usr/share/mde/wm.py /usr/share/mde/labwc_cfg.py \
    /usr/share/mde/ws-display-identify.py /usr/share/mde/ws-display-hotplug.py
  if [[ -d "$MDE_ROOT/shell/ws-mmc" ]]; then
    mkdir -p /usr/share/mde/ws-mmc
    install -m 0644 "$MDE_ROOT"/shell/ws-mmc/*.py /usr/share/mde/ws-mmc/ 2>/dev/null || true
    if [[ -f "$MDE_ROOT/shell/ws-mmc/mde-mmc-helper" ]]; then
      install -m 0755 "$MDE_ROOT/shell/ws-mmc/mde-mmc-helper" /usr/libexec/mde-mmc-helper
      # Compatibility name for older policies.
      install -m 0755 "$MDE_ROOT/shell/ws-mmc/mde-mmc-helper" /usr/libexec/ws-mmc-helper
    fi
    if [[ -f "$MDE_ROOT/shell/ws-mmc/org.fedoraws.mmc.policy" ]]; then
      install -m 0644 "$MDE_ROOT/shell/ws-mmc/org.fedoraws.mmc.policy" \
        /usr/share/polkit-1/actions/org.fedoraws.mmc.policy
    fi
    if [[ -f "$MDE_ROOT/shell/ws-mmc/ws-mmc-devices.service" ]]; then
      install -m 0644 "$MDE_ROOT/shell/ws-mmc/ws-mmc-devices.service" \
        /etc/systemd/system/ws-mmc-devices.service
      systemctl enable ws-mmc-devices.service >/dev/null 2>&1 || true
    fi
  fi

  local legacy
  for legacy in $SWAY_LEGACY_HELPERS; do
    rm -f "${SWAY_HELPER_DIR:?}/$legacy"
  done
  if [[ -f /usr/lib/systemd/user/ws-miracle-shell.service ]]; then
    systemctl --global disable ws-miracle-shell.service >/dev/null 2>&1 || true
    rm -f /usr/lib/systemd/user/ws-miracle-shell.service
  fi

  sway_write "$SWAY_HELPER_DIR/ws-run-if" <<'EOS'
#!/usr/bin/env bash
command -v "$1" >/dev/null 2>&1 || exec sleep infinity
exec "$@"
EOS

  sway_write "$SWAY_HELPER_DIR/ws-drawer" <<'EOS'
#!/usr/bin/env bash
# Start lives inside mde-shell.
case "${1:-}" in
  start|restart) exit 0 ;;
esac
exec mde-shell
EOS

  sway_write "$SWAY_HELPER_DIR/ws-start" <<'EOS'
#!/bin/sh
exec mde-shell
EOS
  chmod 0755 "$SWAY_HELPER_DIR/ws-start"

  sway_write "$SWAY_HELPER_DIR/ws-lock" <<'EOS'
#!/usr/bin/env bash
loginctl lock-session 2>/dev/null || true
EOS

  sway_write "$SWAY_HELPER_DIR/ws-screenshot" <<'EOS'
#!/usr/bin/env bash
# Standalone helper (labwc / manual). Live MDE prefers compositor → mde-shell modes.
set -euo pipefail
mode="${1:-screen}"
dir="$(xdg-user-dir PICTURES 2>/dev/null || echo "$HOME/Pictures")/Screenshots"
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/mde/config.toml"
if [[ -f "$cfg" ]]; then
  cfg_dir="$(sed -n 's/^[[:space:]]*screenshot_dir[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$cfg" | head -1)"
  if [[ -n "${cfg_dir:-}" ]]; then
    dir="${cfg_dir/#\~/$HOME}"
  fi
fi
mkdir -p "$dir"
file="$dir/Screenshot-$(date +%Y%m%d-%H%M%S).png"
case "$mode" in
  area|snip)
    geom="$(slurp)" || exit 0
    grim -g "$geom" "$file"
    wl-copy --type image/png <"$file" 2>/dev/null || true
    notify-send -i camera-photo "Screenshot saved" "${file/#$HOME/\~}" 2>/dev/null || true
    command -v swappy >/dev/null && swappy -f "$file" &
    ;;
  window)
    geom="${2:-}"
    if [[ -z "$geom" ]]; then
      echo "ws-screenshot window: pass grim geometry as \$2 (x,y WxH), or use Super+Print in MDE" >&2
      exit 2
    fi
    grim -g "$geom" "$file"
    wl-copy --type image/png <"$file" 2>/dev/null || true
    notify-send -i camera-photo "Screenshot saved" "${file/#$HOME/\~}" 2>/dev/null || true
    ;;
  screen|*)
    grim "$file"
    wl-copy --type image/png <"$file" 2>/dev/null || true
    notify-send -i camera-photo "Screenshot saved" "${file/#$HOME/\~}" 2>/dev/null || true
    ;;
esac
EOS

  sway_write "$SWAY_HELPER_DIR/ws-clip" <<'EOS'
#!/bin/sh
exec mde-shell
EOS

  sway_write "$SWAY_HELPER_DIR/ws-power-menu" <<'EOS'
#!/bin/sh
exec mde-control
EOS

  sway_write "$SWAY_HELPER_DIR/ws-settings" <<'EOS'
#!/bin/sh
exec mde-control "$@"
EOS

  sway_write "$SWAY_HELPER_DIR/ws-taskbar" <<'EOS'
#!/bin/sh
exec mde-shell
EOS

  sway_write "$SWAY_HELPER_DIR/ws-watermark" <<'EOS'
#!/bin/sh
exit 0
EOS

  sway_write "$SWAY_HELPER_DIR/ws-window-buttons" <<'EOS'
#!/bin/sh
exit 0
EOS
  sway_install_brand
  sway_remove_regedit
  mde_install_rust_platform
  install -m 0755 "$MDE_ROOT/install/bin/mde-session" "$SWAY_HELPER_DIR/mde-session"

  sway_write "$SWAY_HELPER_DIR/ws-show-desktop" <<'EOS'
#!/bin/sh
# Show-desktop is handled by mde-comp / mde-shell.
exit 0
EOS

  sway_write "$SWAY_HELPER_DIR/ws-winx" <<'EOS'
#!/bin/sh
exec mde-mmc compmgmt.msc
EOS
  chmod 0755 "$SWAY_HELPER_DIR/ws-winx"

  sway_write "$SWAY_HELPER_DIR/ws-mmc" <<'EOS'
#!/bin/sh
exec mde-mmc "$@"
EOS
  chmod 0755 "$SWAY_HELPER_DIR/ws-mmc"

  # Windows-style .msc / Linux CLI aliases → mde-mmc (W459).
  for name in compmgmt.msc devmgmt.msc diskmgmt.msc lusrmgr.msc services.msc \
              eventvwr.msc taskschd.msc perfmon.msc network.msc firewall.msc packages.msc; do
    sway_write "$SWAY_HELPER_DIR/$name" <<EOS
#!/bin/sh
exec mde-mmc $name "\$@"
EOS
    chmod 0755 "$SWAY_HELPER_DIR/$name"
  done

  sway_write "$SWAY_HELPER_DIR/ws-power" <<'EOS'
#!/bin/sh
case "${1:-}" in
  reboot|restart) exec mde-privileged power reboot ;;
  poweroff|shutdown) exec mde-privileged power poweroff ;;
  suspend|sleep) exec mde-privileged power suspend ;;
  *) echo "usage: ws-power reboot|poweroff|suspend" >&2; exit 2 ;;
esac
EOS
  chmod 0755 "$SWAY_HELPER_DIR/ws-power"

  sway_write "$SWAY_HELPER_DIR/ws-osd" <<'EOS'
#!/usr/bin/env bash
case "$1" in
  volume-up)   wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+ ;;
  volume-down) wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%- ;;
  mute)        wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle ;;
  mic-mute)    wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle ;;
  bright-up)   brightnessctl -q set 5%+ ;;
  bright-down) brightnessctl -q set 5%- ;;
esac
EOS

  sway_write "$SWAY_HELPER_DIR/mde-autostart" <<'EOS'
#!/bin/sh
# Session children are started by mde-session.
exit 0
EOS
  chmod 0755 "$SWAY_HELPER_DIR/mde-autostart" \
    "$SWAY_HELPER_DIR/ws-run-if" "$SWAY_HELPER_DIR/ws-drawer" \
    "$SWAY_HELPER_DIR/ws-lock" "$SWAY_HELPER_DIR/ws-settings" \
    "$SWAY_HELPER_DIR/ws-taskbar" "$SWAY_HELPER_DIR/ws-show-desktop" \
    "$SWAY_HELPER_DIR/ws-osd" "$SWAY_HELPER_DIR/ws-screenshot" \
    "$SWAY_HELPER_DIR/mde-session" 2>/dev/null || true

  WS_APPLIED=$((WS_APPLIED + 1))
}


# Management Console: Device Manager, Disk Management, Local Users and Groups,
# Services, Event Viewer, Task Scheduler and Performance, plus the .msc commands
# the Run box understands. The helper is the few actions with no D-Bus API.
sway_install_mmc() {
  local share=/usr/share/mde/ws-mmc f name
  mkdir -p "$share" /usr/libexec /usr/share/polkit-1/actions
  for f in "$MDE_ROOT"/shell/ws-mmc/*.py; do
    sed -e "s|@FONT@|${SWAY_FONT}|g" -e "s|@BODY@|${SWAY_FONT_BODY}|g" \
      -e "s|@CAPTION@|${SWAY_FONT_CAPTION}|g" -e "s|@ACCENT_TEXT@|${SWAY_ACCENT_TEXT}|g" \
      -e "s|@ACCENT@|${SWAY_ACCENT}|g" -e "s|@GTK_THEME@|${LOOK_THEME_GTK}|g" \
      "$f" | sway_write "$share/${f##*/}"
  done
  sway_write "$SWAY_HELPER_DIR/ws-mmc" <<'EOS'
#!/bin/sh
# Management Console. "ws-mmc devmgmt.msc" opens Device Manager; with no
# argument it opens an empty console (File > Add/Remove Snap-in).
exec python3 /usr/share/mde/ws-mmc/mmc.py "$@"
EOS
  for name in devmgmt.msc diskmgmt.msc compmgmt.msc eventvwr.msc services.msc \
              lusrmgr.msc taskschd.msc perfmon.msc mmc; do
    sway_write "$SWAY_HELPER_DIR/$name" <<EOS
#!/bin/sh
exec $SWAY_HELPER_DIR/ws-mmc $name "\$@"
EOS
    chmod 0755 "$SWAY_HELPER_DIR/$name"
  done
  sway_write "$SWAY_HELPER_DIR/msinfo32" <<'EOS'
#!/bin/sh
exec hardinfo2 "$@"
EOS
  chmod 0755 "$SWAY_HELPER_DIR/msinfo32"
  install -m 0755 "$MDE_ROOT/shell/ws-mmc/ws-mmc-helper" /usr/libexec/ws-mmc-helper
  install -m 0644 "$MDE_ROOT/shell/ws-mmc/org.fedoraws.mmc.policy" \
    /usr/share/polkit-1/actions/org.fedoraws.mmc.policy
  install -m 0644 "$MDE_ROOT/shell/ws-mmc/ws-mmc-devices.service" \
    /etc/systemd/system/ws-mmc-devices.service
  systemctl daemon-reload
  systemctl enable ws-mmc-devices.service >/dev/null 2>&1 || \
    warn "Could not enable ws-mmc-devices.service"

  sway_write /usr/share/applications/ws-mmc-devices.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Device Manager
Comment=View and update hardware settings and driver software
Exec=ws-mmc devmgmt.msc
Icon=computer
Categories=System;Settings;
Keywords=devmgmt;devices;drivers;hardware;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-disks.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Disk Management
Comment=Manage disks, partitions and volumes
Exec=ws-mmc diskmgmt.msc
Icon=drive-harddisk
Categories=System;Settings;
Keywords=diskmgmt;partitions;volumes;drives;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-computer.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Computer Management
Comment=System tools, storage and services on this computer
Exec=ws-mmc compmgmt.msc
Icon=computer
Categories=System;Settings;
Keywords=compmgmt;management;console;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-events.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Event Viewer
Comment=View events that have happened on this computer
Exec=ws-mmc eventvwr.msc
Icon=text-x-generic
Categories=System;Settings;
Keywords=eventvwr;logs;journal;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-services.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Services
Comment=Start, stop and configure services
Exec=ws-mmc services.msc
Icon=system-run
Categories=System;Settings;
Keywords=services;systemd;daemons;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-users.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Local Users and Groups
Comment=Manage local users and groups
Exec=ws-mmc lusrmgr.msc
Icon=system-users
Categories=System;Settings;
Keywords=lusrmgr;users;groups;accounts;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-tasks.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Task Scheduler
Comment=Schedule tasks to run automatically
Exec=ws-mmc taskschd.msc
Icon=x-office-calendar
Categories=System;Settings;
Keywords=taskschd;timers;scheduled tasks;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc-perf.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Performance Monitor
Comment=See how this computer is using its processor, memory, disks and network
Exec=ws-mmc perfmon.msc
Icon=utilities-system-monitor
Categories=System;Settings;
Keywords=perfmon;performance;cpu;memory;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/ws-mmc.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Management Console
Comment=An empty console. Add snap-ins from the File menu
Exec=ws-mmc
Icon=applications-system
Categories=System;Settings;
Keywords=mmc;snap-in;console;
StartupWMClass=ws-mmc
EOF
  sway_write /usr/share/applications/msinfo32.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=System Information
Comment=Hardware and software information
Exec=msinfo32
Icon=computer
Categories=System;Settings;
Keywords=msinfo32;hardinfo;system information;
StartupWMClass=hardinfo2
EOF
}

# Regedit removed — Config Editor in mde-control owns ~/.config/mde/config.toml.
sway_remove_regedit() {
  rm -rf /usr/share/mde/ws-regedit
  rm -f "$SWAY_HELPER_DIR/regedit" /usr/share/applications/regedit.desktop \
    /usr/share/mime/packages/mde-regedit.xml
  update-mime-database /usr/share/mime >/dev/null 2>&1 || true
}

# Build and install the Rust platform (compositor, shell, control, session).
mde_install_rust_platform() {
  local crates="$MDE_ROOT/crates" bindir="${MDE_BIN:-/usr/bin}"
  # Prefer a user-writable target dir (root-owned target/ breaks unprivileged builds).
  local target="${CARGO_TARGET_DIR:-$crates/target-user}"
  if is_dry_run; then
    info "DRY-RUN: cargo build --release mde-comp mde-shell mde-control mde-session mde-privileged"
    return 0
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    warn "cargo not found; skip Rust platform install"
    return 0
  fi
  (cd "$crates" && CARGO_TARGET_DIR="$target" cargo build --release \
      -p mde-config -p mde-theme -p mde-comp -p mde-shell \
      -p mde-control -p mde-session -p mde-privileged -p mde-neighbourhood \
      -p mde-greeter) \
    || { warn "Rust platform build failed"; return 0; }
  local b
  for b in mde-comp mde-shell mde-control mde-privileged mde-neighbourhood mde-greeter mde-lock; do
    if [[ -x "$target/release/$b" ]]; then
      install -m 0755 "$target/release/$b" "$bindir/$b"
    fi
  done
  # Avoid clobbering the shell wrapper named mde-session.
  if [[ -x "$target/release/mde-session" ]]; then
    install -m 0755 "$target/release/mde-session" "$bindir/mde-session-rs"
  fi
  install -m 0755 "$MDE_ROOT/install/bin/mde-session" "$bindir/mde-session"
  # Compatibility shims
  sway_write "$bindir/ws-settings" <<'EOS'
#!/bin/sh
exec mde-control "$@"
EOS
  sway_write "$bindir/regedit" <<'EOS'
#!/bin/sh
# Regedit removed. Opens the Config Editor (single MDE config store).
exec mde-control config
EOS
  chmod 0755 "$bindir/ws-settings" "$bindir/regedit"
  mkdir -p /usr/share/applications /usr/share/wayland-sessions /etc/greetd
  sway_write /usr/share/applications/mde-control.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Settings
Comment=MDE Settings, Management Console, and Config Editor
Exec=mde-control
Icon=preferences-system
Categories=Settings;X-MDE;
StartupWMClass=mde-control
EOF
  sway_write /usr/share/applications/mde-config-editor.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Config Editor
Comment=Edit the MDE configuration store
Exec=mde-control config
Icon=preferences-system
Categories=Settings;X-MDE;
Keywords=registry;regedit;config;toml;
StartupWMClass=mde-control
EOF
  sway_write /usr/share/wayland-sessions/mde.desktop <<'EOF'
[Desktop Entry]
Name=Mackes Desktop Environment
Comment=MDE Rust platform (mde-comp + mde-shell)
Exec=mde-session
Type=Application
DesktopNames=MDE
EOF
  # greetd + seatd session (replaces LightDM for the Rust platform).
  # Fedora user "greetd"; cage + mde-comp greeter when cage is installed.
  local greet_cmd='agreety --cmd /usr/bin/mde-session'
  if command -v cage >/dev/null 2>&1; then
    greet_cmd='cage -s -- /usr/bin/mde-comp greeter'
  fi
  sway_write /etc/greetd/mde-config.toml <<EOF
[terminal]
vt = 1

[default_session]
command = "${greet_cmd}"
user = "greetd"
EOF
  install -d -m 0755 /usr/share/polkit-1/actions
  sway_write /usr/share/polkit-1/actions/org.mde.privileged.policy <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>Mackes Desktop Environment</vendor>
  <action id="org.mde.privileged.power">
    <description>MDE power actions</description>
    <message>Authentication is required for power actions</message>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
  </action>
  <action id="org.mde.privileged.offline">
    <description>MDE offline update trigger</description>
    <message>Authentication is required to prepare offline updates</message>
    <defaults>
      <allow_any>auth_admin</allow_any>
      <allow_inactive>auth_admin</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
  </action>
</policyconfig>
EOF
  # Portals: prefer gtk + wlr screenshot/screencast for the MDE session.
  mkdir -p /usr/share/xdg-desktop-portal
  sway_write /usr/share/xdg-desktop-portal/mde-portals.conf <<'EOF'
[preferred]
default=gtk
org.freedesktop.impl.portal.Screenshot=wlr
org.freedesktop.impl.portal.ScreenCast=wlr
EOF
  mkdir -p /usr/share/xdg-desktop-portal-wlr
  sway_write /usr/share/xdg-desktop-portal-wlr/mde.conf <<'EOF'
[screencast]
chooser_type=simple
chooser_cmd=slurp -f %o -or
EOF
}

# Settings app: the Python modules from mde/shell/ws-settings with the theme values
# filled in, the light shell styles that Personalization > Colors restyles,
# and the page index that Start searches.
sway_install_settings_app() {
  local share=/usr/share/mde/ws-settings f
  mkdir -p "$share/base"
  for f in "$MDE_ROOT"/shell/ws-settings/*.py; do
    sed -e "s|@FONT@|${SWAY_FONT}|g" -e "s|@BODY@|${SWAY_FONT_BODY}|g" \
      -e "s|@CAPTION@|${SWAY_FONT_CAPTION}|g" -e "s|@ACCENT_TEXT@|${SWAY_ACCENT_TEXT}|g" \
      -e "s|@ACCENT@|${SWAY_ACCENT}|g" -e "s|@GTK_THEME@|${LOOK_THEME_GTK}|g" \
      "$f" | sway_write "$share/${f##*/}"
  done
  sway_swaync_css | sway_write "$share/base/swaync.css"
  sway_wofi_css | sway_write "$share/base/wofi.css"
  sway_swayosd_css | sway_write "$share/base/swayosd.css"
  sway_nwg_bar_css | sway_write "$share/base/nwg-bar.css"
  sway_qt_colors 5 | sway_write "$share/base/qt5.conf"
  sway_qt_colors 6 | sway_write "$share/base/qt6.conf"
  printf '%s\n' "# Window colours are the MDE Openbox theme, not Sway client.* lines." |
    sway_write "$share/base/sway-colors.conf"
  python3 "$share/app.py" --index 2>/dev/null | sway_write "$share/index.json"
  sway_write /usr/share/applications/ws-settings.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Settings
Comment=Display, sound, personalisation, apps, accounts, updates and more
Exec=ws-settings
Icon=preferences-system
Categories=Settings;System;
Keywords=control panel;preferences;options;
StartupWMClass=ws-settings
EOF
}

# Mackes mark used by the installer, the Start button and Settings > About.
sway_install_brand() {
  local dest=/usr/share/mde/brand
  mkdir -p "$dest"
  cp -a "$MDE_ROOT"/assets/brand/*.svg "$dest/"
}

# Taskbar: the Python modules from mde/shell/ws-taskbar with the fonts filled in.
sway_install_taskbar() {
  local share=/usr/share/mde/ws-taskbar f
  mkdir -p "$share"
  for f in "$MDE_ROOT"/shell/ws-taskbar/*.py; do
    sed -e "s|@FONT@|${SWAY_FONT}|g" -e "s|@BODY@|${SWAY_FONT_BODY}|g" \
      -e "s|@CAPTION@|${SWAY_FONT_CAPTION}|g" "$f" | sway_write "$share/${f##*/}"
  done
}

# Sign-in and lock screen (lib/ws-greeter). The greeter falls back to
# lightdm-gtk-greeter when it can't start; WS_GREETER=gtk keeps the stock one.
# /var/lib/ws-greeter carries each user's lock picture and display scale to the
# greeter, which runs as lightdm and can't read home folders.
sway_install_greeter() {
  local share=/usr/share/mde/ws-greeter f
  mkdir -p "$share"
  for f in "$MDE_ROOT"/shell/ws-greeter/*.py; do
    sed -e "s|@FONT@|${SWAY_FONT}|g" -e "s|@ACCENT@|${SWAY_ACCENT}|g" "$f" | sway_write "$share/${f##*/}"
  done
  install -d -m 1777 /var/lib/ws-greeter
  sway_write /usr/bin/ws-greeter <<'EOS'
#!/usr/bin/bash
# LightDM greeter: the Windows 11 sign-in screen, or lightdm-gtk-greeter if it fails.
export GDK_BACKEND=x11
trap 'kill -TERM "$pid" 2>/dev/null; exit 0' TERM INT HUP
python3 /usr/share/mde/ws-greeter/greeter.py &
pid=$!
wait "$pid"
rc=$?
if [[ $rc -eq 0 || $rc -gt 128 ]]; then
  exit 0
fi
echo "ws-greeter: exited with $rc; starting lightdm-gtk-greeter" >&2
exec lightdm-gtk-greeter
EOS
  chmod 0755 /usr/bin/ws-greeter
  sway_write /usr/share/xgreeters/ws-greeter.desktop <<'EOS'
[Desktop Entry]
Name=MDE sign-in
Comment=Sign-in screen matching the Windows 11 look, with lightdm-gtk-greeter as fallback
Exec=/usr/bin/ws-greeter
Type=Application
EOS
  sway_write /etc/lightdm/ws-greeter.conf <<EOF
# ws-greeter settings. scale: interface size (1.0, 1.25, 1.5, 2); empty picks it
# from the user's display scale or the screen DPI.
[greeter]
background=$(look_wallpaper_path)
scale=
EOF
  local conf=/etc/lightdm/lightdm.conf.d/99-ws-session.conf
  if [[ "${WS_GREETER:-ws}" != gtk && -f "$conf" ]]; then
    sed -i 's/^greeter-session=.*/greeter-session=ws-greeter/' "$conf"
  fi
}

# --- Per-user configuration ------------------------------------------------


mde_retire_sway_config() {
  local home="$1" cfg dest stamp
  cfg="$home/.config"
  [[ -d "$cfg/mde/sway" || -e "$cfg/sway/config" ]] || return 0
  stamp="$(date +%Y%m%d-%H%M%S)"
  dest="$home/ws-setup-backup/mde-sway-$stamp"
  mkdir -p "$dest"
  [[ -d "$cfg/mde/sway" ]] && cp -a "$cfg/mde/sway" "$dest/mde-sway"
  [[ -e "$cfg/sway" ]] && cp -a "$cfg/sway" "$dest/sway"
  rm -rf "$cfg/mde/sway"
  rm -rf "$cfg/sway"
  chown -R "$(stat -c %U "$home"):$(stat -c %G "$home")" "$home/ws-setup-backup" 2>/dev/null || true
}

labwc_write_dir() {
  local dir="$1" wp="$2"
  python3 "$MDE_ROOT/shell/labwc_cfg.py" write-tree "$dir" \
    --wallpaper "$wp" \
    --accent "$SWAY_ACCENT" \
    --cursor "$SWAY_CURSOR" \
    --cursor-size "$SWAY_CURSOR_SIZE"
}

# Ship fixed compositor helpers under ~/.local/share/mde so Settings/taskbar
# keep working even if /usr/share/mde is briefly stale after a Sway→labwc move.
mde_install_user_overlay() {
  local home="$1" user="$2" share bin
  share="$home/.local/share/mde"
  bin="$home/.local/bin"
  mkdir -p "$share/ws-settings" "$share/ws-taskbar" "$bin"
  install -m 0644 "$MDE_ROOT/shell/labwc_cfg.py" "$share/labwc_cfg.py"
  install -m 0755 "$MDE_ROOT/shell/wm.py" "$share/wm.py"
  install -m 0755 "$MDE_ROOT/shell/ws-display-identify.py" "$share/ws-display-identify.py"
  install -m 0755 "$MDE_ROOT/shell/ws-display-hotplug.py" "$share/ws-display-hotplug.py"
  if [[ -d /usr/share/mde/ws-settings ]]; then
    cp -a /usr/share/mde/ws-settings/. "$share/ws-settings/"
  fi
  if [[ -d /usr/share/mde/ws-taskbar ]]; then
    cp -a /usr/share/mde/ws-taskbar/. "$share/ws-taskbar/"
  fi
  install -m 0644 "$MDE_ROOT/shell/ws-settings/p_system.py" "$share/ws-settings/p_system.py"
  install -m 0644 "$MDE_ROOT/shell/ws-settings/p_display.py" "$share/ws-settings/p_display.py"
  install -m 0644 "$MDE_ROOT/shell/ws-settings/wsui.py" "$share/ws-settings/wsui.py" 2>/dev/null || true
  # Keep processed CSS from the system copy of wsui when the source still has
  # @ACCENT@ placeholders; only patch the labwc loader path.
  if [[ -f /usr/share/mde/ws-settings/wsui.py ]]; then
    cp /usr/share/mde/ws-settings/wsui.py "$share/ws-settings/wsui.py"
    python3 - "$share/ws-settings/wsui.py" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
old = """def _labwc():
    root = os.path.abspath(os.path.join(os.path.dirname(os.path.realpath(__file__)), ".."))
    for path in (root, "/usr/share/mde"):
        if path not in sys.path:
            sys.path.insert(0, path)
    import labwc_cfg
    return labwc_cfg"""
new = """def _labwc():
    root = os.path.abspath(os.path.join(os.path.dirname(os.path.realpath(__file__)), ".."))
    local = os.path.join(os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share")), "mde")
    for path in (local, root, "/usr/share/mde"):
        if path and path not in sys.path:
            sys.path.insert(0, path)
    import labwc_cfg
    return labwc_cfg"""
if old in text:
    path.write_text(text.replace(old, new, 1))
PY
  fi
  install -m 0644 "$MDE_ROOT/shell/ws-taskbar/common.py" "$share/ws-taskbar/common.py"
  install -m 0644 "$MDE_ROOT/shell/ws-taskbar/taskbar.py" "$share/ws-taskbar/taskbar.py"
  install -m 0644 "$MDE_ROOT/shell/ws-taskbar/windowbuttons.py" "$share/ws-taskbar/windowbuttons.py"
  install -m 0644 "$MDE_ROOT/shell/ws-taskbar/notify.py" "$share/ws-taskbar/notify.py"
  install -m 0644 "$MDE_ROOT/shell/ws-taskbar/start.py" "$share/ws-taskbar/start.py"
  install -m 0644 "$MDE_ROOT/shell/ws-taskbar/quick.py" "$share/ws-taskbar/quick.py"
  for f in tray.py taskview.py watermark.py; do
    [[ -f "$MDE_ROOT/shell/ws-taskbar/$f" ]] && install -m 0644 "$MDE_ROOT/shell/ws-taskbar/$f" "$share/ws-taskbar/$f"
  done
  install -m 0755 "$MDE_ROOT/shell/ws-start.py" "$bin/ws-start"
  chown "$user:$user" "$bin/ws-start" 2>/dev/null || true
  cat > "$bin/ws-settings" <<EOF
#!/bin/sh
exec python3 "$share/ws-settings/app.py" "\$@"
EOF
  cat > "$bin/ws-taskbar" <<EOF
#!/bin/sh
exec python3 "$share/ws-taskbar/taskbar.py" "\$@"
EOF
  chmod 0755 "$bin/ws-settings" "$bin/ws-taskbar"
  chown -R "$user:$user" "$share" "$bin/ws-settings" "$bin/ws-taskbar" 2>/dev/null || true
}

# Toast-only SwayNC: Quick Settings and the taskbar notification centre own
# Wi-Fi/BT/volume/brightness. SwayNC still draws pop-ups and DND state.
sway_swaync_config() {
  cat <<EOF
{
  "\$schema": "/etc/xdg/swaync/configSchema.json",
  "positionX": "right",
  "positionY": "bottom",
  "layer": "overlay",
  "control-center-layer": "top",
  "cssPriority": "user",
  "control-center-margin-bottom": 12,
  "control-center-margin-right": 12,
  "control-center-width": 360,
  "control-center-height": 640,
  "notification-window-width": 364,
  "notification-icon-size": 32,
  "timeout": 5,
  "timeout-low": 5,
  "timeout-critical": 0,
  "transition-time": 250,
  "fit-to-screen": false,
  "hide-on-clear": true,
  "widgets": ["title", "dnd", "mpris", "notifications"],
  "widget-config": {
    "title": { "text": "Notifications", "clear-all-button": true, "button-text": "Clear All" },
    "dnd": { "text": "Do Not Disturb" },
    "mpris": { "show-album-art": "when-available", "autohide": true }
  }
}
EOF
}

sway_swaync_css() {
  cat <<EOF
/* mm notifications and quick settings — Fluent flyouts: 8px surfaces,
   4px controls, Body text with Body Strong titles, Windows blue */
* { font-family: "${SWAY_FONT}"; font-size: ${SWAY_FONT_BODY}pt; }
.notification-row { outline: none; background: transparent; }
.notification-row:focus, .notification-row:hover { background: transparent; }
.notification {
  background: #f9f9f9;
  color: ${SWAY_PAD_FG};
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 8px;
  margin: 6px 12px;
  padding: 0;
  box-shadow: 0 8px 16px alpha(#000000, 0.14);
}
.notification.critical { border-left: 4px solid #c42b1c; }
.notification-content { padding: 16px; }
.notification-default-action, .notification-action {
  background: transparent;
  color: ${SWAY_PAD_FG};
  border: none;
  border-radius: 8px;
}
.notification-action {
  margin: 0 4px 8px;
  border-radius: 4px;
  background: alpha(#ffffff, 0.7);
  border: 1px solid alpha(#000000, 0.06);
}
.notification-default-action:hover, .notification-action:hover { background: alpha(#000000, 0.04); }
.summary { font-weight: 600; color: ${SWAY_PAD_FG}; }
.time { color: ${SWAY_PAD_FG2}; font-size: ${SWAY_FONT_CAPTION}pt; }
.body { color: ${SWAY_PAD_FG2}; }
.close-button {
  background: transparent;
  color: ${SWAY_PAD_FG2};
  border-radius: 4px;
  min-width: 24px;
  min-height: 24px;
  margin: 8px;
  padding: 0;
}
.close-button:hover { background: alpha(#000000, 0.06); color: ${SWAY_PAD_FG}; }
.control-center {
  background: #f3f3f3;
  color: ${SWAY_PAD_FG};
  border: 1px solid alpha(#000000, 0.1);
  border-radius: 8px;
  box-shadow: 0 8px 16px alpha(#000000, 0.14);
}
.control-center-list { background: transparent; }
.widget-title { margin: 16px 16px 8px; font-size: ${SWAY_FONT_BODY}pt; font-weight: 600; }
.widget-title > button {
  background: alpha(#ffffff, 0.7);
  color: ${SWAY_PAD_FG};
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 4px;
  min-height: 30px;
  padding: 0 12px;
  font-size: ${SWAY_FONT_BODY}pt;
  font-weight: normal;
}
.widget-title > button:hover { background: alpha(#f9f9f9, 0.5); }
.widget-dnd { margin: 4px 16px 12px; }
.widget-dnd label { color: ${SWAY_PAD_FG}; font-size: ${SWAY_FONT_BODY}pt; }
.widget-dnd > switch {
  border-radius: 999px;
  background: transparent;
  border: 1px solid alpha(#000000, 0.45);
}
.widget-dnd > switch:checked { background: ${SWAY_ACCENT}; border-color: ${SWAY_ACCENT}; }
.widget-dnd > switch slider { background: alpha(#000000, 0.6); border-radius: 999px; min-width: 12px; min-height: 12px; margin: 3px; }
.widget-dnd > switch:checked slider { background: #ffffff; }
.widget-buttons-grid { margin: 0 12px 8px; }
.widget-buttons-grid flowboxchild > button {
  background: alpha(#ffffff, 0.7);
  color: ${SWAY_PAD_FG};
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 4px;
  min-height: 48px;
  padding: 0 4px;
  margin: 4px;
  font-size: ${SWAY_FONT_CAPTION}pt;
  font-weight: normal;
}
.widget-buttons-grid flowboxchild > button label { font-size: ${SWAY_FONT_CAPTION}pt; }
.widget-buttons-grid flowboxchild > button:hover { background: alpha(#f9f9f9, 0.5); }
.widget-buttons-grid flowboxchild > button.toggle:checked,
.widget-buttons-grid flowboxchild > button.active {
  background: ${SWAY_ACCENT};
  border-color: ${SWAY_ACCENT};
  color: #ffffff;
}
.widget-volume, .widget-backlight {
  margin: 4px 16px;
  padding: 8px 12px;
  background: transparent;
  border-radius: 4px;
}
.widget-volume label, .widget-backlight label { font-size: 12pt; margin-right: 12px; }
.widget-volume trough, .widget-backlight trough {
  min-height: 4px;
  border-radius: 999px;
  background: alpha(#000000, 0.45);
}
.widget-volume highlight, .widget-backlight highlight {
  border-radius: 999px;
  background: ${SWAY_ACCENT};
}
.widget-volume slider, .widget-backlight slider {
  min-width: 20px;
  min-height: 20px;
  margin: -8px 0;
  border-radius: 999px;
  background: ${SWAY_ACCENT};
  border: 1px solid alpha(#000000, 0.08);
  box-shadow: inset 0 0 0 4px #ffffff;
}
.widget-volume slider:hover, .widget-backlight slider:hover { box-shadow: inset 0 0 0 3px #ffffff; }
.widget-mpris .widget-mpris-player {
  margin: 8px 16px;
  border-radius: 8px;
  box-shadow: none;
}
.widget-mpris .widget-mpris-player .mpris-overlay {
  padding: 12px;
  background-color: alpha(#f9f9f9, 0.85);
  color: ${SWAY_PAD_FG};
}
.widget-mpris .widget-mpris-player .mpris-overlay button { border-radius: 4px; }
.widget-mpris .widget-mpris-player .mpris-overlay button:hover { background: alpha(#000000, 0.06); }
EOF
}

sway_wofi_config() {
  cat <<'EOF'
width=600
height=400
location=center
prompt=Search
insensitive=true
allow_images=true
image_size=20
gtk_dark=false
EOF
}

sway_wofi_css() {
  cat <<EOF
window {
  margin: 0;
  border: 1px solid alpha(#000000, 0.1);
  border-radius: 8px;
  background-color: #f3f3f3;
  font-family: "${SWAY_FONT}";
  font-size: ${SWAY_FONT_BODY}pt;
}
#input {
  margin: 12px 12px 8px;
  min-height: 30px;
  padding: 0 12px;
  border: 1px solid alpha(#000000, 0.06);
  border-bottom-color: alpha(#000000, 0.45);
  border-radius: 4px;
  color: ${SWAY_PAD_FG};
  background-color: alpha(#ffffff, 0.7);
  box-shadow: none;
}
#input:focus {
  background-color: #ffffff;
  border-bottom: 2px solid ${SWAY_ACCENT};
}
#outer-box { margin: 0; }
#scroll { margin: 0 8px 8px; }
#text { margin: 0 8px; color: ${SWAY_PAD_FG}; }
#entry {
  min-height: 36px;
  padding: 0 4px;
  border-radius: 4px;
}
#entry:selected {
  background-color: alpha(#000000, 0.04);
  background-image: linear-gradient(${SWAY_ACCENT}, ${SWAY_ACCENT});
  background-size: 3px 16px;
  background-position: 0 50%;
  background-repeat: no-repeat;
  outline: none;
}
#entry:selected #text { color: ${SWAY_PAD_FG}; }
EOF
}

sway_swaylock_config() {
  local wp="$1"
  cat <<EOF
image=${wp}
scaling=fill
color=${SWAY_PANEL_BG#\#}
font=${SWAY_FONT}
font-size=20
indicator-radius=100
indicator-thickness=7
indicator-idle-visible
indicator-caps-lock
show-failed-attempts
ignore-empty-password
ring-color=${SWAY_PAD_BORDER#\#}
ring-ver-color=${SWAY_ACCENT#\#}
ring-wrong-color=ff3b30
ring-clear-color=ffbd2e
ring-caps-lock-color=ffbd2e
key-hl-color=${SWAY_ACCENT#\#}
bs-hl-color=ff3b30
caps-lock-key-hl-color=${SWAY_ACCENT#\#}
caps-lock-bs-hl-color=ff3b30
inside-color=ffffffe6
inside-ver-color=ffffffe6
inside-wrong-color=ffffffe6
inside-clear-color=ffffffe6
inside-caps-lock-color=ffffffe6
line-color=00000000
line-ver-color=00000000
line-wrong-color=00000000
line-clear-color=00000000
line-caps-lock-color=00000000
separator-color=00000000
text-color=${SWAY_PAD_FG#\#}
text-ver-color=${SWAY_PAD_FG#\#}
text-wrong-color=ff3b30
text-clear-color=${SWAY_PAD_FG#\#}
text-caps-lock-color=${SWAY_PAD_FG#\#}
EOF
}

sway_swayosd_css() {
  cat <<EOF
window#osd {
  padding: 12px 20px;
  border-radius: 8px;
  border: 1px solid alpha(#000000, 0.06);
  background: alpha(#f9f9f9, 0.96);
}
window#osd #container { margin: 16px; }
window#osd image, window#osd label { color: ${SWAY_PAD_FG}; }
window#osd progressbar:disabled, window#osd image:disabled { opacity: 0.5; }
window#osd progressbar {
  min-height: 6px;
  border-radius: 999px;
  background: transparent;
  border: none;
}
window#osd trough {
  min-height: inherit;
  border-radius: inherit;
  border: none;
  background: ${SWAY_PAD_BORDER};
}
window#osd progress {
  min-height: inherit;
  border-radius: inherit;
  border: none;
  background: ${SWAY_ACCENT};
}
EOF
}

sway_nwg_bar_json() {
  cat <<EOF
[
  { "label": "Lock", "exec": "ws-lock", "icon": "${SWAY_ICON_DIR}/system-lock-screen.svg" },
  { "label": "Log Out", "exec": "ws-confirm logout", "icon": "${SWAY_ICON_DIR}/system-log-out.svg" },
  { "label": "Suspend", "exec": "systemctl suspend", "icon": "${SWAY_ICON_DIR}/system-suspend.svg" },
  { "label": "Restart", "exec": "ws-confirm reboot", "icon": "${SWAY_ICON_DIR}/system-reboot.svg" },
  { "label": "Shut Down", "exec": "ws-confirm poweroff", "icon": "${SWAY_ICON_DIR}/system-shutdown.svg" }
]
EOF
}

sway_nwg_bar_css() {
  cat <<EOF
window { background-color: alpha(#000000, 0.25); }
#outer-box { margin: 0; }
#inner-box {
  background-color: #f9f9f9;
  border: 1px solid alpha(#000000, 0.06);
  border-radius: 8px;
  padding: 4px;
  margin: 12px;
  box-shadow: 0 8px 16px alpha(#000000, 0.14);
}
button, image { background: none; border: none; box-shadow: none; }
button {
  padding: 12px 16px;
  margin: 2px;
  border-radius: 4px;
  color: ${SWAY_PAD_FG};
  font-family: "${SWAY_FONT}";
  font-size: ${SWAY_FONT_BODY}pt;
}
button:hover, button:focus { background-color: alpha(#000000, 0.04); }
button:active { background-color: alpha(#000000, 0.02); color: ${SWAY_PAD_FG2}; }
EOF
}

# Qt palette roles, in QPalette order. Qt 6 adds Accent as the 22nd role.
sway_qt_colors() {
  local roles=21 active disabled
  [[ "$1" == "6" ]] && roles=22
  active="#1a1a1a, #f5f5f7, #ffffff, #f0f0f2, #b8b8bd, #d0d0d5, #1a1a1a, #ffffff, #1a1a1a, #ffffff, #ffffff, #767676, #0078d4, #ffffff, #005fb8, #5856d6, #f5f5f7, #000000, #ffffff, #1a1a1a, #5d5d5d"
  disabled="#a0a0a5, #f5f5f7, #ffffff, #f0f0f2, #b8b8bd, #d0d0d5, #a0a0a5, #ffffff, #a0a0a5, #f5f5f7, #ffffff, #767676, #d0d0d5, #a0a0a5, #8fb8ff, #a8a6e6, #f5f5f7, #000000, #ffffff, #333333, #b8b8bd"
  if ((roles == 22)); then
    active+=", #0078d4"
    disabled+=", #8fb8ff"
  fi
  cat <<EOF
[ColorScheme]
active_colors=${active}
disabled_colors=${disabled}
inactive_colors=${active}
EOF
}

sway_qt_conf() {
  local ver="$1" home="$2" font fixed
  if [[ "$ver" == "6" ]]; then
    font="\"${SWAY_FONT},${SWAY_FONT_BODY},-1,5,400,0,0,0,0,0,0,0,0,0,0,1\""
    fixed="\"${SWAY_FONT_MONO},${SWAY_FONT_BODY},-1,5,400,0,0,0,0,0,0,0,0,0,0,1\""
  else
    font="\"${SWAY_FONT},${SWAY_FONT_BODY},-1,5,50,0,0,0,0,0\""
    fixed="\"${SWAY_FONT_MONO},${SWAY_FONT_BODY},-1,5,50,0,0,0,0,0\""
  fi
  cat <<EOF
[Appearance]
color_scheme_path=${home}/.config/qt${ver}ct/colors/padOS.conf
custom_palette=true
icon_theme=${LOOK_THEME_ICON}
standard_dialogs=default
style=Fusion

[Fonts]
fixed=${fixed}
general=${font}
EOF
}

# ClearType-like rendering for apps that read fontconfig rather than
# GSettings (Qt, Electron, Firefox). Segoe UI aliases map to the selected
# MDE UI font (default Inter).
sway_fontconfig() {
  cat <<EOF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <match target="font">
    <edit name="antialias" mode="assign"><bool>true</bool></edit>
    <edit name="hinting" mode="assign"><bool>true</bool></edit>
    <edit name="hintstyle" mode="assign"><const>hintslight</const></edit>
    <edit name="rgba" mode="assign"><const>rgb</const></edit>
    <edit name="lcdfilter" mode="assign"><const>lcddefault</const></edit>
  </match>
  <alias binding="same">
    <family>sans-serif</family>
    <prefer><family>${SWAY_FONT}</family></prefer>
  </alias>
  <alias binding="same">
    <family>monospace</family>
    <prefer><family>${SWAY_FONT_MONO}</family></prefer>
  </alias>
  <alias binding="same">
    <family>Segoe UI</family>
    <prefer><family>${SWAY_FONT}</family></prefer>
  </alias>
  <alias binding="same">
    <family>Segoe UI Variable</family>
    <prefer><family>${SWAY_FONT}</family></prefer>
  </alias>
</fontconfig>
EOF
}

# Resolve output scale from WS_SCALE_TIER or a quick EDID PPI probe.
# normal and dpi120 → 1, scale2 → 2. Fractional scales (1.25) make GTK3
# hit-test the wrong point in a window, so clicks miss the control.
sway_panel_scale() {
  local tier="${WS_SCALE_TIER:-}" ppi=0 conn edid mode w_px w_mm
  if [[ -z "$tier" ]]; then
    for conn in /sys/class/drm/card*-*/; do
      [[ -f "${conn}status" ]] || continue
      [[ "$(cat "${conn}status" 2>/dev/null)" == "connected" ]] || continue
      mode="$(head -1 "${conn}modes" 2>/dev/null || true)"
      [[ "$mode" =~ ^([0-9]+)x([0-9]+) ]] || continue
      w_px="${BASH_REMATCH[1]}"
      w_mm="$(cat "${conn}width" 2>/dev/null || echo 0)"
      (( w_mm > 0 )) || continue
      ppi=$(( (w_px * 254) / (w_mm * 10) ))
      break
    done
    if (( ppi >= 180 )); then
      tier=scale2
    elif (( ppi >= 140 )); then
      tier=dpi120
    else
      tier=normal
    fi
    WS_SCALE_TIER="$tier"
  fi
  case "$tier" in
    scale2) echo "2" ;;
    dpi120) echo "1" ;;
    *) echo "1" ;;
  esac
}

sway_kanshi_default() {
  local scale panel laptop_scale
  laptop_scale="$(sway_panel_scale)"
  case "${WS_SCALE_TIER:-normal}" in
    scale2) scale=" scale 2" ;;
    dpi120) scale="" ;;
    *) scale="" ;;
  esac
  panel="$(compgen -G '/sys/class/drm/card*-eDP-*' | head -1)"
  panel="${panel##*/}"
  panel="${panel#card*-}"
  cat <<EOF
# Display profiles. "Save Display Layout" (ws-display-save) adds a profile
# above these for each monitor combination you arrange in wdisplays.
# "ws-drawer restart" reloads the taskbar (and Start flyout) after a display change.
# Scale follows WS_SCALE_TIER / EDID PPI (1 or 2, never fractional).
EOF
  if [[ -n "$panel" ]]; then
    cat <<EOF
profile laptop {
  output ${panel} enable scale ${laptop_scale}
  exec ws-drawer restart
}

EOF
  fi
  cat <<EOF
profile default {
  output * enable${scale}
  exec ws-drawer restart
}
EOF
}

# Existing kanshi configs keep their profiles; each one only gains the
# start-menu restart hook.
sway_kanshi_add_drawer_hook() {
  local file="$1"
  grep -q 'ws-drawer restart' "$file" && return 0
  awk '/^profile / {inside=1} inside && /^}/ {print "  exec ws-drawer restart"; inside=0} {print}' \
    "$file" >"$file.tmp" && mv -f "$file.tmp" "$file"
}

# Hide XFCE autostart entries from MDE/Sway by adding MDE;sway to NotShowIn
# in a per-user copy; entries limited by OnlyShowIn are already hidden.
sway_hide_autostart() {
  local dir="$1/.config/autostart" name user_file sys_file base shown
  mkdir -p "$dir"
  for name in $SWAY_HIDDEN_AUTOSTART; do
    user_file="$dir/$name.desktop"
    sys_file="/etc/xdg/autostart/$name.desktop"
    base=""
    if [[ -f "$user_file" ]] && grep -q '^Exec=' "$user_file"; then
      base="$user_file"
    elif [[ -f "$sys_file" ]]; then
      base="$sys_file"
    fi
    [[ -n "$base" ]] || continue
    grep -q '^OnlyShowIn=' "$base" && continue
    shown="$(sed -n 's/^NotShowIn=//p' "$base" | head -1)"
    [[ -n "$shown" ]] && shown="${shown%;};"
    [[ ";$shown" == *";sway;"* || ";$shown" == *";labwc;"* || ";$shown" == *";MDE;"* ]] && continue
    {
      grep -v '^NotShowIn=' "$base"
      [[ -f "$user_file" && "$base" != "$user_file" ]] && grep -E '^X-' "$user_file"
      printf 'NotShowIn=%sMDE;labwc;\n' "$shown"
    } >"$user_file.tmp"
    mv -f "$user_file.tmp" "$user_file"
  done
}

# ws-sway-autostart starts swaync itself; skip the packaged user service in
# MDE/Sway so two copies do not fight over the bus name (XFCE is unaffected).
sway_swaync_dropin() {
  cat <<'EOF'
[Service]
ExecCondition=/bin/sh -c 'case ":$$XDG_CURRENT_DESKTOP:" in *:MDE:*|*:sway:*) exit 1 ;; esac'
EOF
}

# Default Start pins, as desktop file IDs: the Windows 11 staples (browser,
# File Explorer, Notepad, Calculator, Photos, Store, Settings) where installed.
sway_drawer_pins() {
  local id
  for id in mde-control org.mozilla.firefox firefox thunar terminator org.xfce.mousepad \
    org.gnome.Calculator galculator org.xfce.ristretto org.mageia.dnfdragora \
    network.cycles.wdisplays org.pulseaudio.pavucontrol ws-settings ws-shortcuts; do
    [[ -f "/usr/share/applications/$id.desktop" || -f "/usr/share/applications/$id.desktop" ]] &&
      printf '%s\n' "$id"
  done
}

# Background update check for Settings > Windows Update, every 6 hours.
sway_update_timer() {
  local units="$1/systemd/user"
  sway_write "$units/ws-update-check.service" <<'EOF'
[Unit]
Description=Check for updates (Settings > Windows Update)

[Service]
Type=oneshot
ExecStart=/usr/bin/ws-settings --check-updates
EOF
  sway_write "$units/ws-update-check.timer" <<'EOF'
[Unit]
Description=Check for updates (Settings > Windows Update)

[Timer]
OnBootSec=10min
OnUnitActiveSec=6h
Persistent=true

[Install]
WantedBy=timers.target
EOF
  mkdir -p "$units/timers.target.wants"
  ln -sf ../ws-update-check.timer "$units/timers.target.wants/ws-update-check.timer"
}

# Windows spotlight (Settings > Personalization > Background). The command
# does nothing until that background is selected.
sway_bing_timer() {
  local units="$1/systemd/user"
  sway_write "$units/ws-bing-wallpaper.service" <<'EOF'
[Unit]
Description=Windows spotlight wallpaper (Bing)

[Service]
Type=oneshot
ExecStart=/usr/bin/ws-settings --bing-wallpaper
EOF
  sway_write "$units/ws-bing-wallpaper.timer" <<'EOF'
[Unit]
Description=Windows spotlight wallpaper (Bing)

[Timer]
OnStartupSec=2min
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
EOF
  mkdir -p "$units/timers.target.wants"
  ln -sf ../ws-bing-wallpaper.timer "$units/timers.target.wants/ws-bing-wallpaper.timer"
}

sway_portals_conf() {
  cat <<'EOF'
[preferred]
default=gtk
org.freedesktop.impl.portal.Screenshot=wlr
org.freedesktop.impl.portal.ScreenCast=wlr
EOF
}

sway_xdpw_conf() {
  cat <<'EOF'
[screencast]
chooser_type=simple
chooser_cmd=slurp -f %o -or
EOF
}

sway_set_xresources() {
  local file="$1" key val
  touch "$file"
  for key in Xcursor.theme Xcursor.size; do
    [[ "$key" == Xcursor.theme ]] && val="$SWAY_CURSOR" || val="$SWAY_CURSOR_SIZE"
    if grep -q "^${key}:" "$file"; then
      sed -i "s|^${key}:.*|${key}: ${val}|" "$file"
    else
      printf '%s: %s\n' "$key" "$val" >>"$file"
    fi
  done
}

# GTK on Wayland reads GSettings rather than settings.ini for theme, icons,
# fonts, cursor, and the light/dark preference used by GTK 4 apps.
look_sway_gsettings() {
  local user="$1"
  if is_dry_run; then
    info "DRY-RUN: GSettings look for $user (padOS, prefer-light)"
    return 0
  fi
  runuser -u "$user" -- dbus-run-session -- sh -c "
    gsettings set org.gnome.desktop.interface gtk-theme '${LOOK_THEME_GTK}'
    gsettings set org.gnome.desktop.interface icon-theme '${LOOK_THEME_ICON}'
    gsettings set org.gnome.desktop.interface font-name '${LOOK_FONT}'
    gsettings set org.gnome.desktop.interface monospace-font-name '${SWAY_FONT_MONO} ${SWAY_FONT_BODY}'
    gsettings set org.gnome.desktop.interface font-antialiasing 'rgba'
    gsettings set org.gnome.desktop.interface font-rgba-order 'rgb'
    gsettings set org.gnome.desktop.interface font-hinting 'slight'
    gsettings set org.gnome.desktop.interface cursor-theme '${SWAY_CURSOR}'
    gsettings set org.gnome.desktop.interface cursor-size ${SWAY_CURSOR_SIZE}
    gsettings set org.gnome.desktop.interface color-scheme 'prefer-light'
    gsettings set org.gnome.desktop.wm.preferences button-layout ':minimize,maximize,close'
    gsettings set org.blueman.general symbolic-status-icons true 2>/dev/null || true
  " >/dev/null 2>&1 || warn "GSettings look not applied for $user"
}

look_apply_sway_user() {
  local user="$1" home cfg
  home="$(getent passwd "$user" | cut -d: -f6)"
  [[ -n "$home" ]] || return 0
  cfg="$home/.config"
  if is_dry_run; then
    info "DRY-RUN: seed MDE Rust config for $user (~/.config/mde + portals)"
    look_apply_gtk_ini "$user"
    look_sway_gsettings "$user"
    return 0
  fi

  mde_retire_sway_config "$home"
  mkdir -p "$cfg/mde/profiles" "$cfg/xdg-desktop-portal" "$cfg/xdg-desktop-portal-wlr"
  if [[ ! -f "$cfg/mde/config.toml" ]]; then
    # Minimal seed; mde-session migrates/writes on first login.
    printf '# MDE config (managed by mde-control)\n' >"$cfg/mde/config.toml"
  fi
  sway_portals_conf | sway_write "$cfg/xdg-desktop-portal/mde-portals.conf"
  sway_xdpw_conf | sway_write "$cfg/xdg-desktop-portal-wlr/config"
  look_apply_gtk_ini "$user"
  look_sway_gsettings "$user"
  chown -R "$user:$user" "$cfg/mde" "$cfg/xdg-desktop-portal" "$cfg/xdg-desktop-portal-wlr" 2>/dev/null || true
  WS_APPLIED=$((WS_APPLIED + 1))
  info "MDE look applied for $user (Rust session: mde-comp + mde-shell)"
}

look_apply_sway_users() {
  look_sway_install_helpers
  look_theme_greeter
  local u
  for u in "${WS_TARGET_USERS[@]}"; do
    look_apply_sway_user "$u"
  done
}
