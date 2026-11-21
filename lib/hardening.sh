#!/usr/bin/env bash
# Core reliability and security applied on every desktop.
# shellcheck disable=SC2034

harden_dnf_conf() {
  local conf="/etc/dnf/dnf.conf"
  if is_dry_run; then
    info "DRY-RUN: set fastestmirror and max_parallel_downloads in $conf"
    return 0
  fi
  [[ -f "$conf" ]] || touch "$conf"
  if grep -q '^max_parallel_downloads=' "$conf"; then
    sed -i 's/^max_parallel_downloads=.*/max_parallel_downloads=10/' "$conf"
  else
    printf '\nmax_parallel_downloads=10\n' >>"$conf"
  fi
  if grep -q '^fastestmirror=' "$conf"; then
    sed -i 's/^fastestmirror=.*/fastestmirror=True/' "$conf"
  else
    printf 'fastestmirror=True\n' >>"$conf"
  fi
  WS_APPLIED=$((WS_APPLIED + 1))
}

harden_firewall() {
  ensure_pkgs "firewalld" firewalld
  if is_dry_run; then
    info "DRY-RUN: enable firewalld and FedoraWorkstation zone"
    return 0
  fi
  systemctl enable --now firewalld.service >/dev/null 2>&1 || true
  if firewall-cmd --get-zones 2>/dev/null | grep -qw FedoraWorkstation; then
    firewall-cmd --set-default-zone=FedoraWorkstation >/dev/null 2>&1 || true
  fi
  firewall-cmd --permanent --add-service=mdns >/dev/null 2>&1 || true
  firewall-cmd --permanent --add-service=dhcpv6-client >/dev/null 2>&1 || true
  firewall-cmd --permanent --add-service=samba-client >/dev/null 2>&1 || true
  firewall-cmd --reload >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

harden_auto_updates() {
  # Unset keeps the previous behaviour: security updates plus a weekly full upgrade.
  # The text installer sets WS_UPDATES to weekly, security, or off.
  local mode="${WS_UPDATES:-weekly}"
  if [[ "$mode" == "off" ]]; then
    info "Automatic updates left off"
    return 0
  fi
  if ! pkg_installed dnf-automatic && ! pkg_installed dnf5-plugin-automatic; then
    if ! dnf_yes install dnf-automatic; then
      dnf_yes install dnf5-plugin-automatic || true
    fi
  fi
  if is_dry_run; then
    if [[ "$mode" == "security" ]]; then
      info "DRY-RUN: dnf-automatic security timer"
    else
      info "DRY-RUN: dnf-automatic security timer + weekly full upgrade timer"
    fi
    return 0
  fi
  if [[ -f /etc/dnf/automatic.conf ]]; then
    sed -i 's/^upgrade_type.*/upgrade_type = security/' /etc/dnf/automatic.conf || true
    sed -i 's/^apply_updates.*/apply_updates = yes/' /etc/dnf/automatic.conf || true
  fi
  systemctl enable --now dnf-automatic-install.timer >/dev/null 2>&1 \
    || systemctl enable --now dnf-automatic.timer >/dev/null 2>&1 \
    || systemctl enable --now dnf5-automatic.timer >/dev/null 2>&1 || true
  if [[ "$mode" == "security" ]]; then
    info "Weekly full upgrade timer left off"
    WS_APPLIED=$((WS_APPLIED + 1))
    return 0
  fi

  mkdir -p /etc/systemd/system
  cat >/etc/systemd/system/ws-dnf-weekly.service <<'EOF'
[Unit]
Description=Weekly full dnf upgrade
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/bin/dnf -y upgrade
EOF
  cat >/etc/systemd/system/ws-dnf-weekly.timer <<'EOF'
[Unit]
Description=Weekly full dnf upgrade

[Timer]
OnCalendar=Sun *-*-* 04:30
Persistent=true

[Install]
WantedBy=timers.target
EOF
  systemctl daemon-reload >/dev/null 2>&1 || true
  systemctl enable --now ws-dnf-weekly.timer >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

harden_install_snapper() {
  [[ "${WS_ROOT_FS:-}" == "btrfs" ]] || return 0
  ensure_pkgs "snapper" snapper
  if ! is_dry_run; then
    dnf_yes install snapper-plugins >/dev/null 2>&1 || true
  fi
}

harden_snapshots() {
  [[ "${WS_ROOT_FS:-}" == "btrfs" ]] || {
    info "Root is not btrfs; skipping snapper config"
    return 0
  }
  harden_install_snapper
  if is_dry_run; then
    info "DRY-RUN: snapper -c root create-config /"
    return 0
  fi
  if ! snapper -c root get-config >/dev/null 2>&1; then
    snapper -c root create-config / || true
  fi
  WS_APPLIED=$((WS_APPLIED + 1))
}

snapshot_create() {
  local desc="$1"
  [[ "${WS_ROOT_FS:-}" == "btrfs" ]] || return 0
  if is_dry_run; then
    info "DRY-RUN: snapper create --description \"$desc\""
    return 0
  fi
  if ! command -v snapper >/dev/null 2>&1; then
    warn "snapper is not installed; skipping snapshot ($desc)"
    return 0
  fi
  if snapper -c root create --description "$desc" >/dev/null 2>&1; then
    info "Snapshot: $desc"
    WS_APPLIED=$((WS_APPLIED + 1))
  else
    warn "Snapshot failed: $desc"
  fi
}

harden_memory() {
  ensure_pkgs "zram-generator" zram-generator
  if is_dry_run; then
    info "DRY-RUN: zram sized to RAM + enable systemd-oomd"
    return 0
  fi
  mkdir -p /etc/systemd
  cat >/etc/systemd/zram-generator.conf <<'EOF'
[zram0]
zram-size = ram
compression-algorithm = zstd
EOF
  systemctl daemon-reload >/dev/null 2>&1 || true
  systemctl start systemd-zram-setup@zram0.service >/dev/null 2>&1 || true
  systemctl enable --now systemd-oomd.service >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

harden_fwupd() {
  ensure_pkgs "fwupd" fwupd
  if is_dry_run; then
    info "DRY-RUN: enable fwupd and refresh metadata"
    return 0
  fi
  systemctl enable --now fwupd.service >/dev/null 2>&1 || true
  fwupdmgr refresh --force >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

harden_fstrim() {
  if is_dry_run; then
    info "DRY-RUN: systemctl enable fstrim.timer"
    return 0
  fi
  systemctl enable --now fstrim.timer >/dev/null 2>&1 || true
  WS_APPLIED=$((WS_APPLIED + 1))
}

harden_time_selinux() {
  ensure_pkgs "chrony" chrony
  if is_dry_run; then
    info "DRY-RUN: enable chronyd and keep SELinux enforcing"
    return 0
  fi
  systemctl enable --now chronyd.service >/dev/null 2>&1 || true
  if [[ -f /etc/selinux/config ]]; then
    sed -i 's/^SELINUX=.*/SELINUX=enforcing/' /etc/selinux/config || true
  fi
  if command -v getenforce >/dev/null 2>&1; then
    case "$(getenforce 2>/dev/null || true)" in
      Permissive|Disabled) setenforce 1 >/dev/null 2>&1 || true ;;
    esac
  fi
  WS_APPLIED=$((WS_APPLIED + 1))
}

hardening_apply() {
  harden_dnf_conf
  harden_firewall
  harden_auto_updates
  harden_snapshots
  harden_memory
  harden_fwupd
  harden_fstrim
  harden_time_selinux
}
