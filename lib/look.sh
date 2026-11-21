#!/usr/bin/env bash
# Apply mm profile look: themes, icons, fonts, wallpaper, panel recipes.
# shellcheck disable=SC2034

LOOK_THEME_GTK="padOS-master"
LOOK_THEME_ICON="Mackes-Carbon"
LOOK_THEME_WM="Arctodon-Dark"
LOOK_FONT="Selawik 10.5"
LOOK_TITLE_FONT="Selawik 9"
LOOK_BUTTON_LAYOUT="O|SHMC"
LOOK_WORKSPACES=4
LOOK_WALLPAPER_NAME="ws-wallpaper.jpg"
LOOK_BOOT_SPLASH_THEME="neon-tech-penguin"
WS_TARGET_USERS=()

look_assets_dir() {
  if [[ -d "$WS_ROOT/assets/themes" ]]; then
    echo "$WS_ROOT/assets"
  elif [[ -f "$WS_ROOT/assets.tar.xz" ]]; then
    echo "$WS_ROOT/assets"
  else
    echo "$WS_ROOT/assets"
  fi
}

look_extract_assets_if_needed() {
  local adir="$WS_ROOT/assets"
  if [[ ! -d "$adir/themes" ]] && [[ -f "$WS_ROOT/assets.tar.xz" ]]; then
    info "Extracting assets.tar.xz"
    run_or_skip "extract-assets" tar -C "$WS_ROOT" -xJf "$WS_ROOT/assets.tar.xz"
  fi
}

look_install_system_assets() {
  look_extract_assets_if_needed
  local adir="$WS_ROOT/assets"
  if is_dry_run; then
    info "DRY-RUN: install themes/icons/fonts/wallpaper to /usr/local/share"
    return 0
  fi

  mkdir -p /usr/local/share/{themes,icons,fonts,backgrounds/fedora-ws-setup}

  if [[ -d "$adir/themes" ]]; then
    cp -a "$adir/themes/." /usr/local/share/themes/
    look_patch_padOS /usr/local/share/themes/padOS-master
  fi
  if [[ -d "$adir/icons" ]]; then
    cp -a "$adir/icons/." /usr/local/share/icons/
  fi
  if [[ -d "$adir/fonts" ]]; then
    cp -a "$adir/fonts/." /usr/local/share/fonts/
    fc-cache -f /usr/local/share/fonts >/dev/null 2>&1 || true
  fi
  if [[ -f "$adir/wallpapers/$LOOK_WALLPAPER_NAME" ]]; then
    cp -a "$adir/wallpapers/$LOOK_WALLPAPER_NAME" \
      /usr/local/share/backgrounds/fedora-ws-setup/
  elif compgen -G "$adir/wallpapers/*" >/dev/null; then
    local first
    first="$(find "$adir/wallpapers" -type f | head -1)"
    cp -a "$first" "/usr/local/share/backgrounds/fedora-ws-setup/$LOOK_WALLPAPER_NAME"
  fi

  # Refresh icon caches if gtk-update-icon-cache exists
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    for d in /usr/local/share/icons/*; do
      [[ -d "$d" ]] || continue
      gtk-update-icon-cache -f "$d" >/dev/null 2>&1 || true
    done
  fi
  WS_APPLIED=$((WS_APPLIED + 1))
}

look_install_menu_plugin() {
  local rpm
  rpm="$(find "$WS_ROOT/assets/rpms" -name 'xfce4-meowmenu-plugin*.rpm' 2>/dev/null | head -1)"
  if [[ "$WS_FEDORA_VER" == "44" && "$WS_ARCH" == "x86_64" && -n "$rpm" ]]; then
    if pkg_installed xfce4-meowmenu-plugin; then
      info "meowmenu already installed"
      return 0
    fi
    run_or_skip "meowmenu" dnf_yes install "$rpm"
  else
    info "Using whiskermenu fallback (meowmenu only for fc44/x86_64)"
    ensure_pkgs "whiskermenu" xfce4-whiskermenu-plugin
  fi
}

# Run xfconf-query as a target user with a session bus.
look_xfconf() {
  local user="$1"; shift
  if is_dry_run; then
    info "DRY-RUN: xfconf as $user: $*"
    return 0
  fi
  runuser -u "$user" -- dbus-run-session -- xfconf-query "$@"
}

look_xfconf_create() {
  local user="$1" channel="$2" prop="$3" type="$4" value="$5"
  look_xfconf "$user" -c "$channel" -p "$prop" -n -t "$type" -s "$value" 2>/dev/null \
    || look_xfconf "$user" -c "$channel" -p "$prop" -t "$type" -s "$value"
}

look_apply_user_theme() {
  local user="$1"
  info "Applying theme/fonts/WM to user $user"
  look_xfconf_create "$user" xsettings /Net/ThemeName string "$LOOK_THEME_GTK"
  look_xfconf_create "$user" xsettings /Net/IconThemeName string "$LOOK_THEME_ICON"
  look_xfconf_create "$user" xsettings /Gtk/FontName string "$LOOK_FONT"
  look_xfconf_create "$user" xsettings /Gtk/DecorationLayout string "icon,menu:minimize,maximize,close"
  look_xfconf_create "$user" xsettings /Xfce/SyncThemes bool true

  look_xfconf_create "$user" xfwm4 /general/theme string "$LOOK_THEME_WM"
  look_xfconf_create "$user" xfwm4 /general/title_font string "$LOOK_TITLE_FONT"
  look_xfconf_create "$user" xfwm4 /general/button_layout string "$LOOK_BUTTON_LAYOUT"
  look_xfconf_create "$user" xfwm4 /general/workspace_count int "$LOOK_WORKSPACES"
  look_xfconf_create "$user" xfwm4 /general/use_compositing bool true
}

look_apply_hidpi() {
  local user="$1"
  case "$WS_SCALE_TIER" in
    scale2)
      look_xfconf_create "$user" xsettings /Gdk/WindowScalingFactor int 2
      look_xfconf_create "$user" xsettings /Gtk/CursorThemeSize int 48
      ;;
    dpi120)
      look_xfconf_create "$user" xsettings /Xft/DPI int 120
      ;;
    *)
      :
      ;;
  esac
}

look_apply_panel() {
  local user="$1"
  info "Applying portable panel recipe for $user"
  local menu_plugin="whiskermenu"
  if pkg_installed xfce4-meowmenu-plugin; then
    menu_plugin="meowmenu"
  fi

  # Reset to a single bottom panel via xfconf
  look_xfconf "$user" -c xfce4-panel -p /panels -t int -s 1 -a 2>/dev/null || true
  look_xfconf_create "$user" xfce4-panel /panels/dark-mode bool true
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/position string "p=8;x=0;y=0"
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/position-locked bool true
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/length double 100
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/size uint 30
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/mode uint 0
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/span-monitors bool false
  look_xfconf_create "$user" xfce4-panel /panels/panel-1/enable-struts bool true

  # Plugin IDs: 1=menu, 2=tasklist, 3=sep(expand), 4=systray, 5=pulse, 6=notify, 7=sep, 8=clock
  look_xfconf "$user" -c xfce4-panel -p /panels/panel-1/plugin-ids \
    -t int -s 1 -t int -s 2 -t int -s 3 -t int -s 4 -t int -s 5 -t int -s 6 -t int -s 7 -t int -s 8 -a 2>/dev/null \
    || true

  look_xfconf_create "$user" xfce4-panel /plugins/plugin-1 string "$menu_plugin"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-2 string "tasklist"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-2/flat-buttons bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-2/show-labels bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-2/grouping bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-2/include-all-workspaces bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-3 string "separator"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-3/expand bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-3/style uint 0
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-4 string "systray"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-4/symbolic-icons bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-4/square-icons bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-5 string "pulseaudio"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-5/enable-keyboard-shortcuts bool true
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-6 string "notification-plugin"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-7 string "separator"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-7/style uint 0
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-8 string "clock"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-8/digital-time-format string "%I:%M %p"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-8/digital-date-format string "%a, %b %d, %Y"
  look_xfconf_create "$user" xfce4-panel /plugins/plugin-8/mode uint 2
}

look_apply_wallpaper() {
  local user="$1"
  local wp="/usr/local/share/backgrounds/fedora-ws-setup/$LOOK_WALLPAPER_NAME"
  if [[ ! -f "$wp" ]] && is_dry_run; then
    info "DRY-RUN: would set wallpaper $wp for $user"
    return 0
  fi
  [[ -f "$wp" ]] || { warn "Wallpaper missing: $wp"; return 0; }

  # Generic last-image; first-login oneshot fixes per-monitor keys
  look_xfconf_create "$user" xfce4-desktop /backdrop/screen0/monitor0/workspace0/last-image string "$wp"
  look_xfconf_create "$user" xfce4-desktop /backdrop/screen0/monitor0/workspace0/image-style int 5
}

look_write_first_login() {
  local user="$1"
  local home
  home="$(getent passwd "$user" | cut -d: -f6)"
  [[ -n "$home" ]] || return 0
  if is_dry_run; then
    info "DRY-RUN: write first-login wallpaper oneshot for $user"
    return 0
  fi
  mkdir -p "$home/.config/autostart" "$home/.local/bin"
  cat > "$home/.local/bin/ws-first-login-look.sh" <<'EOS'
#!/usr/bin/env bash
WP="/usr/local/share/backgrounds/fedora-ws-setup/ws-wallpaper.jpg"
[[ -f "$WP" ]] || exit 0
# Apply wallpaper to every known monitor workspace0 key
while IFS= read -r prop; do
  xfconf-query -c xfce4-desktop -p "$prop" -n -t string -s "$WP" 2>/dev/null \
    || xfconf-query -c xfce4-desktop -p "$prop" -t string -s "$WP" 2>/dev/null || true
  style="${prop%/last-image}/image-style"
  xfconf-query -c xfce4-desktop -p "$style" -n -t int -s 5 2>/dev/null \
    || xfconf-query -c xfce4-desktop -p "$style" -t int -s 5 2>/dev/null || true
done < <(xfconf-query -c xfce4-desktop -l 2>/dev/null | grep '/last-image$' || true)
# Self-disable
rm -f "$HOME/.config/autostart/ws-first-login-look.desktop"
EOS
  chmod +x "$home/.local/bin/ws-first-login-look.sh"
  cat > "$home/.config/autostart/ws-first-login-look.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=WS First Login Look
Exec=$home/.local/bin/ws-first-login-look.sh
OnlyShowIn=XFCE;
X-GNOME-Autostart-enabled=true
Hidden=false
EOF
  chown -R "$user:$user" "$home/.local/bin/ws-first-login-look.sh" \
    "$home/.config/autostart/ws-first-login-look.desktop" 2>/dev/null || true
}

look_theme_greeter() {
  if is_dry_run; then
    info "DRY-RUN: theme lightdm-gtk-greeter"
    return 0
  fi
  local conf="/etc/lightdm/lightdm-gtk-greeter.conf"
  mkdir -p /etc/lightdm
  if [[ ! -f "$conf" ]]; then
    printf '[greeter]\n' > "$conf"
  fi
  # Ensure keys under [greeter]
  if ! grep -q '^\[greeter\]' "$conf"; then
    printf '\n[greeter]\n' >> "$conf"
  fi
  local wp="/usr/local/share/backgrounds/fedora-ws-setup/$LOOK_WALLPAPER_NAME"
  local -A kv=(
    [theme-name]="$LOOK_THEME_GTK"
    [icon-theme-name]="$LOOK_THEME_ICON"
    [font-name]="$LOOK_FONT"
    [background]="$wp"
    [user-background]="false"
  )
  if [[ "$WS_SCALE_TIER" == "scale2" ]]; then
    kv[xft-dpi]="192"
  elif [[ "$WS_SCALE_TIER" == "dpi120" ]]; then
    kv[xft-dpi]="120"
  fi
  local k
  for k in "${!kv[@]}"; do
    if grep -q "^${k}=" "$conf"; then
      sed -i "s|^${k}=.*|${k}=${kv[$k]}|" "$conf"
    else
      # Insert after [greeter]
      sed -i "/^\[greeter\]/a ${k}=${kv[$k]}" "$conf"
    fi
  done
  WS_APPLIED=$((WS_APPLIED + 1))
}

# padOS ships CSS typos (unitless radius, a doubled property name) that every
# GTK 3 app logs at startup.
look_patch_padOS() {
  local css="$1/gtk-3.0/gtk.css"
  [[ -f "$css" ]] || return 0
  sed -i -e 's/border-radius: 999;/border-radius: 999px;/g' \
    -e 's/background-color: background-color: /background-color: /g' "$css"
}

look_wallpaper_path() {
  printf '%s\n' "/usr/local/share/backgrounds/fedora-ws-setup/${LOOK_WALLPAPER_NAME}"
}

look_backup_old_configs() {
  [[ "${WS_DE_SWITCH:-0}" == "1" ]] || return 0
  local u
  if ((${#WS_TARGET_USERS[@]} == 0)); then
    return 0
  fi
  for u in "${WS_TARGET_USERS[@]}"; do
    look_backup_user "$u"
  done
}

look_backup_user() {
  local user="$1" home stamp dest d
  home="$(getent passwd "$user" | cut -d: -f6)"
  [[ -n "$home" && -d "$home" ]] || return 0
  stamp="$(date +%Y%m%d-%H%M%S)"
  dest="$home/ws-setup-backup/$stamp"
  local dirs=()
  if [[ "${WS_DE:-xfce}" == "mde" || "${WS_DE:-xfce}" == "sway" ]]; then
    dirs=(.config/xfce4 .cache/sessions .config/miracle-wm .config/DankMaterialShell)
  else
    dirs=(
      .config/sway .config/mde .config/miracle-wm .config/nwg-bar .config/nwg-dock .config/nwg-drawer
      .config/nwg-panel .config/waybar .config/wofi .config/swaync
      .config/swaylock .config/swayosd .config/kanshi
      .config/xdg-desktop-portal .config/xdg-desktop-portal-wlr
      .config/DankMaterialShell
    )
  fi
  local existing=()
  for d in "${dirs[@]}"; do
    [[ -e "$home/$d" ]] && existing+=("$d")
  done
  if ((${#existing[@]} == 0)); then
    info "No ${user} desktop config to back up"
    return 0
  fi
  if is_dry_run; then
    info "DRY-RUN: tar ${existing[*]} -> $dest then remove"
    return 0
  fi
  mkdir -p "$dest"
  if tar -C "$home" -czf "$dest/desktop-config.tar.gz" "${existing[@]}"; then
    info "Backed up $user desktop config to $dest"
    for d in "${existing[@]}"; do
      rm -rf "$home/$d"
    done
    chown -R "$user:$user" "$home/ws-setup-backup" 2>/dev/null || true
    WS_APPLIED=$((WS_APPLIED + 1))
  else
    warn "Backup failed for $user; leaving config in place"
  fi
}

look_apply_gtk_ini() {
  local user="$1" home
  home="$(getent passwd "$user" | cut -d: -f6)"
  [[ -n "$home" ]] || return 0
  if is_dry_run; then
    info "DRY-RUN: GTK theme ini for $user"
    return 0
  fi
  mkdir -p "$home/.config/gtk-3.0" "$home/.config/gtk-4.0"
  # padOS is light and has no GTK 4 version, so GTK 4 keeps its own light theme.
  cat >"$home/.config/gtk-4.0/settings.ini" <<EOF
[Settings]
gtk-icon-theme-name=${LOOK_THEME_ICON}
gtk-font-name=${LOOK_FONT}
gtk-cursor-theme-name=Adwaita
gtk-cursor-theme-size=24
gtk-application-prefer-dark-theme=0
gtk-decoration-layout=menu:close
EOF
  {
    printf '%s\n' "[Settings]" "gtk-theme-name=${LOOK_THEME_GTK}"
    tail -n +2 "$home/.config/gtk-4.0/settings.ini"
  } >"$home/.config/gtk-3.0/settings.ini"
  chown -R "$user:$user" "$home/.config/gtk-3.0" "$home/.config/gtk-4.0"
}

look_apply_xfce_users() {
  look_install_menu_plugin
  look_theme_greeter
  local u
  for u in "${WS_TARGET_USERS[@]}"; do
    look_apply_user_theme "$u"
    look_apply_hidpi "$u"
    look_apply_panel "$u"
    look_apply_wallpaper "$u"
    look_write_first_login "$u"
  done
}

# Plymouth theme for every install: XFCE, Sway, and PiPedal.
look_install_boot_splash() {
  local src dest stamp stamp_file installed current
  src="$(look_assets_dir)/plymouth/${LOOK_BOOT_SPLASH_THEME}"
  dest="/usr/share/plymouth/themes/${LOOK_BOOT_SPLASH_THEME}"
  stamp_file="${WS_STATE_DIR:-/var/lib/fedora-ws-setup}/boot-splash.stamp"
  if [[ ! -f "$src/splash.png" || ! -f "$src/${LOOK_BOOT_SPLASH_THEME}.script" || ! -f "$src/${LOOK_BOOT_SPLASH_THEME}.plymouth" ]]; then
    warn "Boot splash assets missing ($src)"
    return 0
  fi
  ensure_pkgs "boot-splash" \
    plymouth plymouth-scripts plymouth-plugin-script plymouth-plugin-label grubby
  if is_dry_run; then
    info "DRY-RUN: install Plymouth theme ${LOOK_BOOT_SPLASH_THEME}, keep rhgb quiet, rebuild initrd"
    return 0
  fi
  stamp="$(
    cd "$src" && find . -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum | awk '{print $1}'
  )"
  installed=""
  [[ -f "$stamp_file" ]] && installed="$(tr -d '[:space:]' <"$stamp_file")"
  current="$(plymouth-set-default-theme 2>/dev/null || true)"
  current="${current//$'\n'/}"
  if [[ "$current" == "$LOOK_BOOT_SPLASH_THEME" && -n "$stamp" && "$stamp" == "$installed" ]]; then
    info "Boot splash already current (${LOOK_BOOT_SPLASH_THEME})"
    grubby --update-kernel=ALL --args="rhgb quiet" >>"$WS_LOG_FILE" 2>&1 \
      || warn "Could not keep rhgb quiet on the kernel command line"
    return 0
  fi
  info "Installing Plymouth theme ${LOOK_BOOT_SPLASH_THEME}"
  mkdir -p "$dest"
  cp -a "$src/." "$dest/"
  if ! plymouth-set-default-theme "$LOOK_BOOT_SPLASH_THEME" >>"$WS_LOG_FILE" 2>&1; then
    error "Could not set Plymouth theme ${LOOK_BOOT_SPLASH_THEME}"
    WS_FAILURES+=("boot-splash")
    WS_FAILED=$((WS_FAILED + 1))
    return 0
  fi
  grubby --update-kernel=ALL --args="rhgb quiet" >>"$WS_LOG_FILE" 2>&1 \
    || warn "Could not keep rhgb quiet on the kernel command line"
  info "Rebuilding initrd so the boot splash is used on the next boot"
  if plymouth-set-default-theme --rebuild-initrd >>"$WS_LOG_FILE" 2>&1; then
    mkdir -p "$(dirname "$stamp_file")"
    printf '%s\n' "$stamp" >"$stamp_file"
    info "Boot splash set to ${LOOK_BOOT_SPLASH_THEME}"
    WS_APPLIED=$((WS_APPLIED + 1))
  else
    error "Plymouth theme installed but the initrd rebuild failed"
    WS_FAILURES+=("boot-splash-initrd")
    WS_FAILED=$((WS_FAILED + 1))
  fi
}

look_apply_all_users() {
  look_install_boot_splash
  if [[ "${WS_DE:-xfce}" == "pipedal" ]]; then
    info "PiPedal appliance: skipping desktop themes"
    return 0
  fi
  # MDE owns themes, helpers, and per-user look via mde/install.sh.
  if [[ "${WS_DE:-xfce}" == "mde" || "${WS_DE:-xfce}" == "sway" ]]; then
    info "MDE look applied by the MDE installer; skipping inline Sway look"
    return 0
  fi
  look_install_system_assets
  look_backup_old_configs
  if ((${#WS_TARGET_USERS[@]} == 0)); then
    warn "No target users selected for look application"
    return 0
  fi
  look_apply_xfce_users
}
