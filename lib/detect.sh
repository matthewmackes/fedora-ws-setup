#!/usr/bin/env bash
# Host detection: Fedora release, GPU, Surface, HiDPI, users, Secure Boot.
# shellcheck disable=SC2034

WS_FEDORA_VER=0
WS_ARCH=""
WS_GPU=""          # intel|amd|nvidia|unknown
WS_ROOT_FS=""
WS_SECURE_BOOT=0   # 1=enabled
WS_SURFACE=0       # 1=detected allowlisted Surface
WS_SURFACE_FAMILY=""  # laptop|pro|unknown|""
WS_SURFACE_PRODUCT=""
WS_PPI=0
WS_SCALE_TIER="normal"  # normal|dpi120|scale2
WS_LOCAL_USERS=()
WS_DMI_VENDOR=""
WS_DMI_PRODUCT=""

# Allowlist patterns: product name regexes for Laptop and Pro families.
# Built from common linux-surface supported devices (x86_64).
SURFACE_LAPTOP_RE='Surface Laptop( [0-9]+| Go|[ ]*Studio|$)?'
SURFACE_PRO_RE='Surface Pro( [0-9]+| X| 11|)$'

detect_fedora() {
  if [[ -f /etc/os-release ]]; then
    # shellcheck source=/dev/null
    . /etc/os-release
    WS_FEDORA_VER="${VERSION_ID%%.*}"
  fi
  WS_ARCH="$(uname -m)"
  if [[ "${ID:-}" != "fedora" ]]; then
    warn "OS ID is '${ID:-unknown}', expected fedora."
  fi
  if (( WS_FEDORA_VER < 40 )); then
    die "Fedora ${WS_FEDORA_VER} is below the minimum supported release (40)."
  fi
  if (( WS_FEDORA_VER < 43 )); then
    warn "Fedora ${WS_FEDORA_VER}: some COPR/linux-surface packages may be unavailable (best on 43+)."
  fi
}

detect_gpu() {
  WS_GPU="unknown"
  if lspci 2>/dev/null | grep -qiE 'VGA|3D|Display'; then
    local pci
    pci="$(lspci 2>/dev/null | grep -iE 'VGA|3D|Display' || true)"
    if echo "$pci" | grep -qi nvidia; then
      WS_GPU="nvidia"
    elif echo "$pci" | grep -qiE 'AMD|ATI|Advanced Micro Devices'; then
      WS_GPU="amd"
    elif echo "$pci" | grep -qiE 'Intel'; then
      WS_GPU="intel"
    fi
  fi
  # Fallback via /sys
  if [[ "$WS_GPU" == "unknown" ]]; then
    if ls /sys/module/i915 >/dev/null 2>&1 || ls /sys/module/xe >/dev/null 2>&1; then
      WS_GPU="intel"
    elif ls /sys/module/amdgpu >/dev/null 2>&1; then
      WS_GPU="amd"
    elif ls /sys/module/nvidia >/dev/null 2>&1; then
      WS_GPU="nvidia"
    fi
  fi
}

detect_root_fs() {
  WS_ROOT_FS="$(findmnt -no FSTYPE / 2>/dev/null || echo unknown)"
}

detect_secure_boot() {
  WS_SECURE_BOOT=0
  if command -v mokutil >/dev/null 2>&1; then
    if mokutil --sb-state 2>/dev/null | grep -qi 'SecureBoot enabled'; then
      WS_SECURE_BOOT=1
    fi
  elif [[ -d /sys/firmware/efi ]]; then
    if [[ -f /sys/firmware/efi/efivars/SecureBoot-* ]] 2>/dev/null; then
      # Best-effort: presence alone is not enough; leave 0 if unclear
      :
    fi
  fi
}

detect_surface() {
  WS_SURFACE=0
  WS_SURFACE_FAMILY=""
  WS_SURFACE_PRODUCT=""
  WS_DMI_VENDOR="$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || true)"
  WS_DMI_PRODUCT="$(cat /sys/class/dmi/id/product_name 2>/dev/null || true)"

  # CLI overrides applied later by surface_apply_overrides
  if [[ "${WS_FORCE_NO_SURFACE:-0}" == "1" ]]; then
    return 0
  fi
  if [[ -n "${WS_FORCE_SURFACE:-}" ]]; then
    WS_SURFACE=1
    WS_SURFACE_FAMILY="$WS_FORCE_SURFACE"
    WS_SURFACE_PRODUCT="forced:${WS_FORCE_SURFACE}"
    return 0
  fi
  if [[ -n "${WS_SIMULATE_SURFACE:-}" ]]; then
    WS_SURFACE=1
    WS_SURFACE_FAMILY="$WS_SIMULATE_SURFACE"
    WS_SURFACE_PRODUCT="simulated:${WS_SIMULATE_SURFACE}"
    return 0
  fi

  local vendor_lc product
  vendor_lc="$(echo "$WS_DMI_VENDOR" | tr '[:upper:]' '[:lower:]')"
  product="$WS_DMI_PRODUCT"
  if [[ "$vendor_lc" != *microsoft* ]]; then
    return 0
  fi
  if [[ ! "$product" =~ ^Surface ]]; then
    return 0
  fi

  WS_SURFACE_PRODUCT="$product"
  if [[ "$product" =~ $SURFACE_LAPTOP_RE ]]; then
    WS_SURFACE=1
    WS_SURFACE_FAMILY="laptop"
  elif [[ "$product" =~ $SURFACE_PRO_RE ]]; then
    WS_SURFACE=1
    WS_SURFACE_FAMILY="pro"
  else
    # Unknown Surface SKU → warn, stay on stock path
    warn "Detected Microsoft Surface '$product' but it is not in the Laptop/Pro allowlist; using stock kernel path."
    WS_SURFACE=0
    WS_SURFACE_FAMILY="unknown"
  fi
}

# Approximate PPI from first connected DRM connector EDID mode.
detect_display_scale() {
  WS_PPI=0
  WS_SCALE_TIER="normal"
  local edid conn w_mm h_mm w_px h_px
  for conn in /sys/class/drm/card*-*/; do
    [[ -f "${conn}status" ]] || continue
    [[ "$(cat "${conn}status" 2>/dev/null)" == "connected" ]] || continue
    edid="${conn}edid"
    [[ -r "$edid" ]] || continue
    # Preferred mode from modes file (first line often preferred)
    local mode
    mode="$(head -1 "${conn}modes" 2>/dev/null || true)"
    if [[ "$mode" =~ ^([0-9]+)x([0-9]+) ]]; then
      w_px="${BASH_REMATCH[1]}"
      h_px="${BASH_REMATCH[2]}"
    else
      continue
    fi
    # Physical size from DRM
    w_mm="$(cat "${conn}width" 2>/dev/null || echo 0)"
    h_mm="$(cat "${conn}height" 2>/dev/null || echo 0)"
    # Some kernels expose width_mm/height_mm differently; try alternate
    if [[ "$w_mm" -eq 0 || "$h_mm" -eq 0 ]]; then
      # Parse EDID bytes 21-22 width cm, 22 height cm (approx)
      if command -v edid-decode >/dev/null 2>&1; then
        :
      fi
      continue
    fi
    if (( w_mm > 0 )); then
      # PPI ≈ pixels / (mm / 25.4)
      WS_PPI=$(( (w_px * 254) / (w_mm * 10) ))
      break
    fi
  done

  # Fallback heuristic via xrandr if available
  if (( WS_PPI == 0 )) && command -v xrandr >/dev/null 2>&1; then
    local line
    line="$(xrandr 2>/dev/null | grep -E ' connected' | head -1 || true)"
    if [[ "$line" =~ ([0-9]+)x([0-9]+).*([0-9]+)mm\ x\ ([0-9]+)mm ]]; then
      w_px="${BASH_REMATCH[1]}"
      w_mm="${BASH_REMATCH[3]}"
      if (( w_mm > 0 )); then
        WS_PPI=$(( (w_px * 254) / (w_mm * 10) ))
      fi
    fi
  fi

  if (( WS_PPI >= 180 )); then
    WS_SCALE_TIER="scale2"
  elif (( WS_PPI >= 140 )); then
    WS_SCALE_TIER="dpi120"
  else
    WS_SCALE_TIER="normal"
  fi
}

detect_users() {
  WS_LOCAL_USERS=()
  local user uid home
  while IFS=: read -r user _ uid _ _ home _; do
    [[ "$uid" =~ ^[0-9]+$ ]] || continue
    (( uid >= 1000 )) || continue
    [[ "$user" == "nobody" ]] && continue
    [[ -d "$home" ]] || continue
    WS_LOCAL_USERS+=("$user")
  done < /etc/passwd
}

detect_all() {
  detect_fedora
  detect_gpu
  detect_root_fs
  detect_secure_boot
  detect_surface
  detect_display_scale
  detect_users
  info "Detected: Fedora ${WS_FEDORA_VER}/${WS_ARCH} GPU=${WS_GPU} FS=${WS_ROOT_FS} SB=${WS_SECURE_BOOT} Surface=${WS_SURFACE}/${WS_SURFACE_FAMILY:-none} PPI=${WS_PPI} scale=${WS_SCALE_TIER} users=${WS_LOCAL_USERS[*]}"
}

preflight_text() {
  cat <<EOF
Preflight summary

  Fedora:       ${WS_FEDORA_VER} (${WS_ARCH})
  GPU:          ${WS_GPU}
  Root FS:      ${WS_ROOT_FS}
  Secure Boot:  $([[ $WS_SECURE_BOOT -eq 1 ]] && echo enabled || echo disabled)
  Surface:      $([[ $WS_SURFACE -eq 1 ]] && echo "yes (${WS_SURFACE_FAMILY}: ${WS_SURFACE_PRODUCT})" || echo "no${WS_SURFACE_FAMILY:+ (skipped: $WS_SURFACE_FAMILY ${WS_SURFACE_PRODUCT})}")
  Display:      PPI≈${WS_PPI} → ${WS_SCALE_TIER}
  Desktop now:  ${WS_DE_PRESENT:-unknown}
  Local users:  ${WS_LOCAL_USERS[*]:-none}
  Dry-run:      $(is_dry_run && echo yes || echo no)

Press Next to continue.
EOF
}
