#!/usr/bin/env bash
# Microsoft Surface Laptop/Pro silent baseline via linux-surface (native reimplementation).
# shellcheck disable=SC2034

WS_FORCE_SURFACE=""
WS_FORCE_NO_SURFACE=0
WS_SIMULATE_SURFACE=""
WS_MOK_NEEDED=0

surface_parse_args() {
  # Called from entry with remaining argv fragments already parsed into globals
  :
}

surface_apply_overrides() {
  if [[ "$WS_FORCE_NO_SURFACE" == "1" ]]; then
    WS_SURFACE=0
    WS_SURFACE_FAMILY=""
    info "Surface path disabled by --no-surface"
    return 0
  fi
  if [[ -n "$WS_FORCE_SURFACE" ]]; then
    WS_SURFACE=1
    WS_SURFACE_FAMILY="$WS_FORCE_SURFACE"
    WS_SURFACE_PRODUCT="forced:${WS_FORCE_SURFACE}"
  fi
  if [[ -n "$WS_SIMULATE_SURFACE" ]]; then
    if ! is_dry_run; then
      die "--simulate-surface is only valid with --dry-run"
    fi
    WS_SURFACE=1
    WS_SURFACE_FAMILY="$WS_SIMULATE_SURFACE"
    WS_SURFACE_PRODUCT="simulated:${WS_SIMULATE_SURFACE}"
  fi
}

surface_add_repo() {
  if [[ -f /etc/yum.repos.d/linux-surface.repo ]]; then
    info "linux-surface repo already present"
    return 0
  fi
  dnf_add_repo_file "https://pkg.surfacelinux.com/fedora/linux-surface.repo"
}

surface_install_kernel() {
  # Full linux-surface Fedora recommended baseline
  run_or_skip "surface-kernel" dnf_yes install --allowerasing \
    kernel-surface iptsd libwacom-surface kernel-surface-default-watchdog
}

surface_set_default_kernel() {
  if is_dry_run; then
    info "DRY-RUN: enable linux-surface-default-watchdog + run watchdog.py"
    return 0
  fi
  systemctl enable --now linux-surface-default-watchdog.path 2>/dev/null || true
  if command -v linux-surface-default-watchdog.py >/dev/null 2>&1; then
    linux-surface-default-watchdog.py || warn "watchdog.py returned non-zero"
  fi
  WS_APPLIED=$((WS_APPLIED + 1))
}

surface_secureboot_key() {
  if [[ "$WS_SECURE_BOOT" != "1" ]]; then
    info "Secure Boot disabled; skipping surface-secureboot"
    return 0
  fi
  WS_MOK_NEEDED=1
  # Separate transaction as upstream recommends
  run_or_skip "surface-secureboot" dnf_yes install surface-secureboot
}

surface_family_extras() {
  case "$WS_SURFACE_FAMILY" in
    pro)
      ensure_pkgs "surface-pro-extras" florence iio-sensor-proxy
      ;;
    laptop)
      info "Surface Laptop profile: no extra packages beyond baseline"
      ;;
    *)
      :
      ;;
  esac
}

surface_mok_instructions() {
  if [[ "$WS_MOK_NEEDED" != "1" ]]; then
    return 0
  fi
  cat <<'EOF'

Secure Boot / MOK enrolment (required after reboot)
----------------------------------------------------
The linux-surface kernel was set as the GRUB default, but its signing key
still needs enrolment. On the next reboot:

  1. A blue MOK management screen should appear.
  2. Choose Enroll MOK → Continue → Yes.
  3. When asked for a password, enter:  surface
  4. Reboot again when prompted.

If you miss the MOK screen, boot the stock Fedora kernel from the GRUB
menu, then reinstall surface-secureboot and reboot to retry:

  dnf -y reinstall surface-secureboot

Until the key is enrolled, the surface kernel may fail to boot under
Secure Boot — use the Fedora kernel entry as a fallback.
EOF
}

surface_apply() {
  if [[ "$WS_SURFACE" != "1" ]]; then
    info "Not an allowlisted Surface; skipping Surface baseline"
    return 0
  fi
  info "Applying Surface ${WS_SURFACE_FAMILY} baseline (${WS_SURFACE_PRODUCT})"
  surface_add_repo
  surface_install_kernel
  surface_set_default_kernel
  surface_secureboot_key
  surface_family_extras
}
