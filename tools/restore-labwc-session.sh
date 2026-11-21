#!/usr/bin/bash
# Restore a working graphical session (labwc bridge) after the incomplete
# mde-comp DRM cutover left greetd with nothing to display.
set -euo pipefail
[[ $EUID -eq 0 ]] || { echo "run with sudo" >&2; exit 1; }

dnf install -y labwc nwg-bar wofi SwayNotificationCenter swayosd kanshi

install -d -m 0755 /usr/share/mde
cp -a /home/mm/.local/share/mde/wm.py /home/mm/.local/share/mde/labwc_cfg.py /usr/share/mde/
cp -a /home/mm/.local/share/mde/ws-settings /usr/share/mde/
cp -a /home/mm/.local/share/mde/ws-taskbar /usr/share/mde/

install -m 0755 /home/mm/fedora-ws-setup/mde/install/bin/mde-session /usr/bin/mde-session
install -m 0755 /home/mm/fedora-ws-setup/mde/install/bin/ws-sway-autostart /usr/bin/ws-sway-autostart

cat >/usr/bin/ws-drawer <<'EOD'
#!/usr/bin/env bash
exec ws-start "$@"
EOD
chmod 0755 /usr/bin/ws-drawer

# Keep greetd on agreety → mde-session (Fedora user greetd)
cat >/etc/greetd/config.toml <<'EOD'
[terminal]
vt = 1

[default_session]
command = "agreety --cmd mde-session"
user = "greetd"
EOD
cp -a /etc/greetd/config.toml /etc/greetd/mde-config.toml

usermod -aG seat,video,input greetd || true
usermod -aG seat mm || true

systemctl reset-failed greetd.service || true
systemctl restart greetd.service
systemctl --no-pager --full status greetd.service || true
echo "OK: log in on VT1 (Ctrl+Alt+F1). Session starts labwc + taskbar."
