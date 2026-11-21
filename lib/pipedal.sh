#!/usr/bin/env bash
# PiPedal appliance: no display server, PREEMPT_RT kernel, ALSA, source build.
# shellcheck disable=SC2034

WS_PIPEDAL_PREFIX="${WS_PIPEDAL_PREFIX:-/opt/pipedal}"
WS_PIPEDAL_ROLLBACK="${WS_PIPEDAL_ROLLBACK:-${WS_STATE_DIR:-/var/lib/fedora-ws-setup}/pipedal-rollback}"
WS_PIPEDAL_CMDLINE_ARGS="mitigations=off usbcore.autosuspend=-1"
WS_PIPEDAL_VANILLA_KEY="https://download.copr.fedorainfracloud.org/results/@kernel-vanilla/fedora/pubkey.gpg"
WS_PIPEDAL_RT_PKGS=(kernel-rt kernel-rt-core kernel-rt-modules kernel-rt-modules-core kernel-rt-modules-extra)
WS_PIPEDAL_MOK=0
WS_PIPEDAL_ABORT=0

pipedal_remove_graphical() {
  local pkgs=() p
  mapfile -t pkgs < <(
    de_installed_remove_list xfce
    de_installed_remove_list sway
    de_installed_remove_list display
  )
  de_protect_keep
  if ((${#pkgs[@]} == 0)); then
    info "No graphical desktop packages to remove"
  else
    info "Removing graphical desktop and display servers: ${pkgs[*]}"
    run_or_skip "remove-graphical" dnf_yes remove "${pkgs[@]}"
  fi
  run_or_skip "autoremove-graphical" dnf_yes autoremove
  if is_dry_run; then
    info "DRY-RUN: systemctl disable lightdm; systemctl set-default multi-user.target"
    return 0
  fi
  systemctl disable lightdm.service >/dev/null 2>&1 || true
  systemctl set-default multi-user.target >/dev/null 2>&1 || true
  info "Next boot is a text console"
}

pipedal_fedora_kernel() {
  local img
  for img in /boot/vmlinuz-*; do
    [[ -f "$img" ]] || continue
    [[ "$img" == *+rt || "$img" == *.rt* ]] && continue
    printf '%s\n' "$img"
  done | sort -V | tail -n 1
}

pipedal_restore_workstation_boot() {
  local fedora
  fedora="$(pipedal_fedora_kernel)"
  if is_dry_run; then
    info "DRY-RUN: boot Fedora kernel again (${fedora:-unknown}) and drop appliance kernel args"
    return 0
  fi
  if [[ -n "$fedora" ]] && command -v grubby >/dev/null 2>&1; then
    grubby --set-default "$fedora" >>"$WS_LOG_FILE" 2>&1 || warn "Could not restore the Fedora kernel as default"
  fi
  if command -v grubby >/dev/null 2>&1; then
    grubby --update-kernel=ALL --remove-args="$WS_PIPEDAL_CMDLINE_ARGS" >>"$WS_LOG_FILE" 2>&1 || true
  fi
  pipedal_unmask_pipewire
  rm -f /etc/NetworkManager/conf.d/99-pipedal-wifi-powersave.conf
  if command -v tuned-adm >/dev/null 2>&1 && [[ -f "$WS_PIPEDAL_ROLLBACK/tuned.txt" ]]; then
    tuned-adm profile "$(cat "$WS_PIPEDAL_ROLLBACK/tuned.txt")" >>"$WS_LOG_FILE" 2>&1 || true
  fi
}

pipedal_remove_service() {
  info "Removing the PiPedal service. The RT kernel stays installed."
  if is_dry_run; then
    info "DRY-RUN: pipedalconfig --uninstall; remove $WS_PIPEDAL_PREFIX; Fedora kernel becomes default"
    return 0
  fi
  if [[ -x "$WS_PIPEDAL_PREFIX/bin/pipedalconfig" ]]; then
    "$WS_PIPEDAL_PREFIX/bin/pipedalconfig" --uninstall >>"$WS_LOG_FILE" 2>&1 || true
  fi
  systemctl disable --now pipedald.service pipedaladmind.service pipedal_nm_p2pd.service >/dev/null 2>&1 || true
  rm -f /etc/systemd/system/pipedald.service /etc/systemd/system/pipedaladmind.service \
    /etc/systemd/system/pipedal_nm_p2pd.service
  rm -rf /etc/systemd/system/pipedald.service.d
  rm -rf "$WS_PIPEDAL_PREFIX"
  systemctl daemon-reload >/dev/null 2>&1 || true
  pipedal_restore_workstation_boot
}

pipedal_snapshot_rpms() {
  if is_dry_run; then
    info "DRY-RUN: record package set, boot target, and kernel for rollback"
    return 0
  fi
  mkdir -p "$WS_PIPEDAL_ROLLBACK"
  rpm -qa --qf '%{NAME}\n' | sort -u >"$WS_PIPEDAL_ROLLBACK/rpms.txt"
  systemctl get-default >"$WS_PIPEDAL_ROLLBACK/target.txt" 2>/dev/null || echo graphical.target >"$WS_PIPEDAL_ROLLBACK/target.txt"
  if command -v grubby >/dev/null 2>&1; then
    grubby --default-kernel >"$WS_PIPEDAL_ROLLBACK/kernel.txt" 2>/dev/null || true
  fi
  if command -v tuned-adm >/dev/null 2>&1; then
    tuned-adm active 2>/dev/null | awk '{print $NF}' >"$WS_PIPEDAL_ROLLBACK/tuned.txt" || true
  fi
}

pipedal_note_rt_packages() {
  [[ -f "$WS_PIPEDAL_ROLLBACK/rpms.txt" ]] || return 0
  local now
  now="$(mktemp)"
  rpm -qa --qf '%{NAME}\n' | sort -u >"$now"
  comm -13 "$WS_PIPEDAL_ROLLBACK/rpms.txt" "$now" | grep -E '^(kernel-rt|kernel-rt-mao)' >"$WS_PIPEDAL_ROLLBACK/rt-added.txt" || true
  rm -f "$now"
}

pipedal_enable_vanilla_repo() {
  if is_dry_run; then
    info "DRY-RUN: enable @kernel-vanilla/fedora, includepkgs=kernel-rt*"
    return 0
  fi
  local repo="https://download.copr.fedorainfracloud.org/results/@kernel-vanilla/fedora/fedora-${WS_FEDORA_VER}-x86_64/"
  cat >/etc/yum.repos.d/kernel-vanilla-fedora.repo <<EOF
[kernel-vanilla-fedora]
name=Fedora kernel-vanilla RT
baseurl=$repo
type=rpm-md
skip_if_unavailable=True
gpgcheck=1
gpgkey=$WS_PIPEDAL_VANILLA_KEY
repo_gpgcheck=0
enabled=1
enabled_metadata=1
includepkgs=kernel-rt kernel-rt-core kernel-rt-modules kernel-rt-modules-core kernel-rt-modules-extra kernel-rt-modules-internal kernel-rt-devel kernel-rt-devel-matched
EOF
}

pipedal_install_rt_kernel() {
  pipedal_enable_vanilla_repo
  if is_dry_run; then
    info "DRY-RUN: dnf install ${WS_PIPEDAL_RT_PKGS[*]} (fall back to kernel-rt-mao)"
    return 0
  fi
  if dnf_yes install "${WS_PIPEDAL_RT_PKGS[@]}"; then
    info "Installed kernel-rt from @kernel-vanilla/fedora"
    pipedal_note_rt_packages
    return 0
  fi
  warn "kernel-rt from @kernel-vanilla/fedora did not install. Trying Audinux kernel-rt-mao."
  cfg_audinux_repo || true
  if dnf_yes install kernel-rt-mao; then
    info "Installed kernel-rt-mao from ycollet/audinux"
    pipedal_note_rt_packages
    return 0
  fi
  error "No PREEMPT_RT kernel could be installed"
  return 1
}

# 0 = keep going on the Fedora kernel. 1 = stop before the desktop is removed.
pipedal_rt_continue() {
  case "${WS_PIPEDAL_RT_ON_FAIL:-stop}" in
    continue) return 0 ;;
    ask)
      if ! declare -F cli_yesno >/dev/null 2>&1; then
        return 1
      fi
      if cli_yesno "Realtime kernel" "No PREEMPT_RT kernel could be installed. Continue on the Fedora kernel? The desktop is still in place."; then
        return 0
      fi
      return 1
      ;;
    *) return 1 ;;
  esac
}

pipedal_set_rt_default() {
  local img
  if is_dry_run; then
    info "DRY-RUN: grubby --set-default to the RT kernel"
    return 0
  fi
  img="$(ls -1 /boot/vmlinuz-*+rt /boot/vmlinuz-*.rt* 2>/dev/null | sort -V | tail -n 1 || true)"
  if [[ -z "$img" ]]; then
    warn "RT kernel image was not found in /boot. The boot menu default was left unchanged."
    return 0
  fi
  command -v grubby >/dev/null 2>&1 || ensure_pkgs "grubby" grubby
  grubby --set-default "$img" >>"$WS_LOG_FILE" 2>&1 || warn "Could not set $img as the default kernel"
  info "Default kernel: $img"
}

pipedal_cmdline() {
  if is_dry_run; then
    info "DRY-RUN: grubby --update-kernel=ALL --args='$WS_PIPEDAL_CMDLINE_ARGS'"
    return 0
  fi
  command -v grubby >/dev/null 2>&1 || ensure_pkgs "grubby" grubby
  grubby --update-kernel=ALL --args="$WS_PIPEDAL_CMDLINE_ARGS" >>"$WS_LOG_FILE" 2>&1 \
    || warn "Could not add appliance kernel arguments"
}

pipedal_enroll_mok() {
  local cert
  WS_PIPEDAL_MOK=0
  if [[ "${WS_SECURE_BOOT:-0}" != "1" ]]; then
    info "Secure Boot is off. No MOK enrolment."
    return 0
  fi
  case "${WS_PIPEDAL_MOK_CHOICE:-queue}" in
    skip)
      warn "Secure Boot is on. MOK enrolment was skipped. The Fedora kernel remains bootable."
      return 0
      ;;
    ask)
      if ! declare -F cli_yesno >/dev/null 2>&1; then
        warn "No text prompt is available. MOK enrolment was skipped."
        return 0
      fi
      if ! cli_yesno "Secure Boot" "Queue MOK enrolment for the RT kernel? The blue-screen password is pipedal. No leaves Secure Boot alone."; then
        warn "MOK enrolment skipped. The Fedora kernel remains bootable."
        return 0
      fi
      ;;
  esac
  WS_PIPEDAL_MOK=1
  cert="$(find /usr/share /etc/pki -type f \( -name '*.cer' -o -name '*.der' \) 2>/dev/null | head -n 1 || true)"
  if [[ -z "$cert" ]]; then
    warn "Secure Boot is on and the RT kernel package did not ship a certificate to enrol"
    return 0
  fi
  if is_dry_run; then
    info "DRY-RUN: mokutil --import $cert"
    return 0
  fi
  printf 'pipedal\npipedal\n' | mokutil --import "$cert" >>"$WS_LOG_FILE" 2>&1 \
    || warn "mokutil --import failed for $cert"
  info "MOK import queued for $cert (enrolment password: pipedal)"
}

pipedal_mok_instructions() {
  [[ "${WS_PIPEDAL_MOK:-0}" == "1" ]] || return 0
  cat <<'EOF'

PiPedal RT kernel / Secure Boot
-------------------------------
Secure Boot is on. The Fedora kernel stays on the boot menu and will
boot if the RT kernel's key is not enrolled yet.

If a blue MOK screen appears:

  1. Choose Enroll MOK → Continue → Yes.
  2. Enter the password:  pipedal
  3. Reboot when prompted.

If the RT package did not ship a certificate, enrol one before relying
on the RT kernel. Until then, choose the Fedora kernel from the GRUB menu.
EOF
}

pipedal_mask_pipewire() {
  local unit
  local units=(pipewire.socket pipewire.service pipewire-pulse.socket pipewire-pulse.service wireplumber.service)
  if is_dry_run; then
    info "DRY-RUN: mask PipeWire and WirePlumber system-wide"
    return 0
  fi
  for unit in "${units[@]}"; do
    systemctl --global mask "$unit" >/dev/null 2>&1 || true
    systemctl mask "$unit" >/dev/null 2>&1 || true
  done
  info "PipeWire and WirePlumber are masked. PiPedal opens ALSA itself."
}

pipedal_unmask_pipewire() {
  local unit
  local units=(pipewire.socket pipewire.service pipewire-pulse.socket pipewire-pulse.service wireplumber.service)
  for unit in "${units[@]}"; do
    systemctl --global unmask "$unit" >/dev/null 2>&1 || true
    systemctl unmask "$unit" >/dev/null 2>&1 || true
  done
}

pipedal_latency_profile() {
  ensure_pkgs "tuned" tuned
  if is_dry_run; then
    info "DRY-RUN: tuned-adm profile latency-performance; disable tuned-ppd"
    return 0
  fi
  systemctl disable --now tuned-ppd.service >/dev/null 2>&1 || true
  systemctl enable --now tuned.service >/dev/null 2>&1 || true
  tuned-adm profile latency-performance >>"$WS_LOG_FILE" 2>&1 || warn "latency-performance profile was not applied"
}

pipedal_wifi_powersave() {
  if is_dry_run; then
    info "DRY-RUN: NetworkManager wifi.powersave=2"
    return 0
  fi
  mkdir -p /etc/NetworkManager/conf.d
  cat >/etc/NetworkManager/conf.d/99-pipedal-wifi-powersave.conf <<'EOF'
[connection]
wifi.powersave = 2
EOF
  systemctl reload NetworkManager >/dev/null 2>&1 || true
}

pipedal_firewall_and_mdns() {
  # Unset matches the appliance default: Avahi plus http and mdns in the firewall.
  case "${WS_CLI_MDNS:-full}" in
    off)
      info "mDNS left off"
      return 0
      ;;
  esac
  ensure_pkgs "avahi" avahi
  if is_dry_run; then
    if [[ "${WS_CLI_MDNS:-full}" == "avahi" ]]; then
      info "DRY-RUN: enable avahi"
    else
      info "DRY-RUN: enable avahi; firewall-cmd add http and mdns"
    fi
    return 0
  fi
  systemctl enable --now avahi-daemon.service >/dev/null 2>&1 || true
  if [[ "${WS_CLI_MDNS:-full}" == "avahi" ]]; then
    info "Avahi enabled. Firewall services were not changed."
    return 0
  fi
  if command -v firewall-cmd >/dev/null 2>&1; then
    firewall-cmd --permanent --add-service=http >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-service=mdns >/dev/null 2>&1 || true
    firewall-cmd --reload >/dev/null 2>&1 || true
  fi
}

pipedal_realtime_limits() {
  ensure_pkgs "realtime-setup" realtime-setup
  if is_dry_run; then
    info "DRY-RUN: rtprio 95 and unlimited memlock for pipedal_d"
    return 0
  fi
  cat >/etc/security/limits.d/90-pipedal.conf <<'EOF'
# Realtime priority for the PiPedal service account.
@pipedal_d   -  rtprio     95
@pipedal_d   -  memlock    unlimited
EOF
  mkdir -p /etc/systemd/system/pipedald.service.d
  cat >/etc/systemd/system/pipedald.service.d/limits.conf <<'EOF'
[Service]
LimitRTPRIO=95
LimitMEMLOCK=infinity
EOF
}

pipedal_base() {
  ensure_pkgs "pipedal-alsa" alsa-utils alsa-lib
  ensure_pkgs "pipedal-net" NetworkManager avahi authbind
  pipedal_mask_pipewire
  pipedal_latency_profile
  pipedal_wifi_powersave
  pipedal_firewall_and_mdns
  pipedal_cmdline
  pipedal_realtime_limits
  if is_dry_run; then
    info "DRY-RUN: systemctl set-default multi-user.target"
    return 0
  fi
  systemctl enable --now NetworkManager.service >/dev/null 2>&1 || true
  systemctl set-default multi-user.target >/dev/null 2>&1 || true
}

pipedal_latest_tag() {
  local tag
  tag="$(curl -fsSL https://api.github.com/repos/rerdavies/pipedal/releases/latest \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name",""))' 2>/dev/null || true)"
  [[ -n "$tag" ]] || tag="v2.0.111"
  printf '%s\n' "$tag"
}

pipedal_configure_files() {
  local card="" name="" id=""
  local dir mode="${WS_PIPEDAL_ALSA_CARD:-auto}"
  case "$mode" in
    unset)
      info "PiPedal audio device left unset for web onboarding"
      ;;
    alsa0)
      if [[ -d /proc/asound/card0 ]]; then
        card=0
        id="$(head -n 1 /proc/asound/card0/id 2>/dev/null || echo "ALSA")"
        name="$id"
      fi
      ;;
    [0-9]|[0-9][0-9])
      card="$mode"
      id="$(head -n 1 "/proc/asound/card${card}/id" 2>/dev/null || echo "USB")"
      name="$id"
      ;;
    *)
      for dir in /proc/asound/card[0-9]*; do
        [[ -f "$dir/usbid" ]] || continue
        card="${dir##*/card}"
        id="$(head -n 1 "$dir/id" 2>/dev/null || echo "USB")"
        name="$id"
        break
      done
      ;;
  esac
  if is_dry_run; then
    info "DRY-RUN: write /var/pipedal/config/config.json lv2_path and AudioConfig.json (usb=${card:-unset}, 48000, 16x4)"
    return 0
  fi
  mkdir -p /var/pipedal/config /var/pipedal/audio_uploads/ToobMlModels
  python3 - "$card" "$name" <<'PY'
import pathlib, sys
card, name = sys.argv[1], sys.argv[2]
cfg = pathlib.Path("/var/pipedal/config/config.json")
lv2 = "/usr/lib64/lv2:/usr/local/lib64/lv2:/usr/lib/lv2:/usr/local/lib/lv2"
text = cfg.read_text(encoding="utf-8") if cfg.exists() else "{\n}\n"
line = f'    "lv2_path": "{lv2}",'
if '"lv2_path"' in text:
    out = []
    for raw in text.splitlines():
        out.append(line if '"lv2_path"' in raw else raw)
    text = "\n".join(out) + "\n"
else:
    text = text.rstrip()
    if text.endswith("}"):
        text = text[:-1].rstrip().rstrip(",") + ",\n" + line + "\n}\n"
    else:
        text = "{\n" + line + "\n}\n"
cfg.write_text(text, encoding="utf-8")
if card:
    audio = f'''{{
  "valid": true,
  "isOnboarding": false,
  "rebootRequired": false,
  "isJackAudio": false,
  "alsaDevice": "",
  "alsaInputDevice": "hw:{card},0",
  "alsaOutputDevice": "hw:{card},0",
  "alsaInputDeviceName": "{name}",
  "alsaOutputDeviceName": "{name}",
  "sampleRate": 48000,
  "bufferSize": 16,
  "numberOfBuffers": 4
}}
'''
else:
    audio = '''{
  "valid": true,
  "isOnboarding": true,
  "rebootRequired": false,
  "isJackAudio": false,
  "alsaDevice": "",
  "alsaInputDevice": "",
  "alsaOutputDevice": "",
  "alsaInputDeviceName": "",
  "alsaOutputDeviceName": "",
  "sampleRate": 48000,
  "bufferSize": 16,
  "numberOfBuffers": 4
}
'''
pathlib.Path("/var/pipedal/AudioConfig.json").write_text(audio, encoding="utf-8")
PY
  if id pipedal_d >/dev/null 2>&1; then
    chown -R pipedal_d:pipedal_d /var/pipedal 2>/dev/null || true
    usermod -aG realtime,audio pipedal_d 2>/dev/null || true
  fi
  info "PiPedal audio default: 48000 Hz, 16x4, USB card ${card:-unset}"
}

pipedal_nam_models() {
  # Tone3000's model API requires a signed-in OAuth session. CC0 is the
  # license PiPedal treats as free for any use. Without that session the
  # installer records the skip instead of downloading models it cannot license.
  if is_dry_run; then
    info "DRY-RUN: Tone3000 CC0 starter set (skipped when the API requires a login)"
    return 0
  fi
  local code
  code="$(curl -sS -o /tmp/pipedal-t3k.json -w '%{http_code}' -A 'fedora-ws-setup' \
    -H 'Accept: application/json' 'https://www.tone3000.com/api/v1/models?license=cco' || true)"
  if [[ "$code" != "200" ]]; then
    warn "Tone3000 starter set skipped (HTTP ${code:-none}). The model API requires a signed-in session. In PiPedal's web UI, download models whose license is CC0."
    rm -f /tmp/pipedal-t3k.json
    return 0
  fi
  python3 - <<'PY'
import json, pathlib, urllib.request
data = json.load(open("/tmp/pipedal-t3k.json"))
items = data if isinstance(data, list) else data.get("models") or data.get("data") or []
kept = []
for item in items:
    if not isinstance(item, dict):
        continue
    if str(item.get("license", "")).lower() not in {"cco", "cc0"}:
        continue
    url = item.get("url") or item.get("download_url") or ""
    if url.startswith("https://") and "tone3000.com" in url:
        kept.append((item.get("title") or item.get("name") or "model", url))
    if len(kept) >= 3:
        break
dest = pathlib.Path("/var/pipedal/audio_uploads/ToobMlModels")
dest.mkdir(parents=True, exist_ok=True)
print(f"Tone3000 CC0 models selected: {len(kept)}")
for title, url in kept:
    print(f"Tone3000 model: {title} {url}")
    name = url.rstrip("/").split("/")[-1] or "model.nam"
    urllib.request.urlretrieve(url, dest / name)
PY
  rm -f /tmp/pipedal-t3k.json
}

pipedal_build() {
  local tag have src node_major
  tag="$(pipedal_latest_tag)"
  have=""
  [[ -f "$WS_PIPEDAL_PREFIX/.ws-version" ]] && have="$(cat "$WS_PIPEDAL_PREFIX/.ws-version")"
  if [[ -x "$WS_PIPEDAL_PREFIX/sbin/pipedald" && "$have" == "$tag" ]]; then
    info "PiPedal $tag is already installed"
    pipedal_configure_files
    return 0
  fi
  ensure_pkgs "pipedal-build" git cmake ninja-build gcc-c++ nodejs npm curl python3 \
    lilv-devel boost-devel systemd-devel catch-devel alsa-lib-devel libuuid-devel \
    avahi-devel NetworkManager-libnm-devel libicu-devel sdbus-cpp-devel libzip-devel \
    gperftools-devel pipewire-devel openssl-devel librsvg2-devel bzip2-devel authbind \
    pkgconf-pkg-config
  if is_dry_run; then
    info "DRY-RUN: clone rerdavies/pipedal $tag, build, install to $WS_PIPEDAL_PREFIX, delete the build tree"
    pipedal_configure_files
    return 0
  fi
  src="$(mktemp -d)"
  if ! git clone --depth 1 --branch "$tag" https://github.com/rerdavies/pipedal.git "$src/pipedal" >>"$WS_LOG_FILE" 2>&1; then
    rm -rf "$src"
    error "Could not clone PiPedal $tag"
    return 1
  fi
  if ! git -C "$src/pipedal" submodule update --init --recursive >>"$WS_LOG_FILE" 2>&1; then
    rm -rf "$src"
    error "PiPedal submodules did not update"
    return 1
  fi
  (
    cd "$src/pipedal" || exit 1
    ./react-config >>"$WS_LOG_FILE" 2>&1 || exit 1
    node_major="$(node -p 'process.versions.node.split(".")[0]')"
    if (( node_major >= 24 )); then
      (cd vite && npm approve-scripts esbuild) >>"$WS_LOG_FILE" 2>&1 || true
    fi
    sed -i 's|set *(CMAKE_INSTALL_PREFIX *"/usr/")|set (CMAKE_INSTALL_PREFIX "/opt/pipedal")|' src/CMakeLists.txt CMakeLists.txt || true
    cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DPIPEDAL_DISABLE_COPYRIGHT_BUILD=ON >>"$WS_LOG_FILE" 2>&1 || exit 1
    cmake --build build --parallel "$(nproc)" >>"$WS_LOG_FILE" 2>&1 || exit 1
    cmake --install build >>"$WS_LOG_FILE" 2>&1 || exit 1
  ) || {
    rm -rf "$src"
    error "PiPedal build failed"
    return 1
  }
  printf '%s\n' "$tag" >"$WS_PIPEDAL_PREFIX/.ws-version"
  rm -rf "$src"
  if ! "$WS_PIPEDAL_PREFIX/bin/pipedalconfig" --install --port 80 >>"$WS_LOG_FILE" 2>&1; then
    error "pipedalconfig --install failed"
    return 1
  fi
  pipedal_realtime_limits
  systemctl daemon-reload >/dev/null 2>&1 || true
  pipedal_configure_files
  pipedal_nam_models || true
  systemctl enable --now pipedald.service >>"$WS_LOG_FILE" 2>&1 || warn "pipedald did not start"
  info "Installed PiPedal $tag to $WS_PIPEDAL_PREFIX"
}

pipedal_rollback() {
  local removed now
  error "Rolling back the PiPedal appliance"
  if is_dry_run; then
    info "DRY-RUN: restore removed packages, remove the RT kernel, restore the Fedora boot"
    return 0
  fi
  if [[ ! -f "$WS_PIPEDAL_ROLLBACK/rpms.txt" ]]; then
    error "No rollback snapshot. The desktop was not restored automatically."
    return 1
  fi
  if [[ -x "$WS_PIPEDAL_PREFIX/bin/pipedalconfig" ]]; then
    "$WS_PIPEDAL_PREFIX/bin/pipedalconfig" --uninstall >>"$WS_LOG_FILE" 2>&1 || true
  fi
  systemctl disable --now pipedald.service pipedaladmind.service >/dev/null 2>&1 || true
  rm -rf "$WS_PIPEDAL_PREFIX" /etc/systemd/system/pipedald.service.d
  now="$(mktemp)"
  rpm -qa --qf '%{NAME}\n' | sort -u >"$now"
  removed="$(mktemp)"
  comm -23 "$WS_PIPEDAL_ROLLBACK/rpms.txt" "$now" >"$removed" || true
  if [[ -s "$removed" ]]; then
    info "Reinstalling packages removed for the appliance"
    dnf_yes install --skip-unavailable $(cat "$removed") || warn "Some desktop packages could not be reinstalled"
  fi
  if [[ -s "$WS_PIPEDAL_ROLLBACK/rt-added.txt" ]]; then
    info "Removing the RT kernel packages added for PiPedal"
    dnf_yes remove $(cat "$WS_PIPEDAL_ROLLBACK/rt-added.txt") || warn "RT kernel packages could not all be removed"
  fi
  rm -f "$now" "$removed"
  pipedal_restore_workstation_boot
  if [[ -f "$WS_PIPEDAL_ROLLBACK/target.txt" ]]; then
    systemctl set-default "$(cat "$WS_PIPEDAL_ROLLBACK/target.txt")" >/dev/null 2>&1 || true
  fi
  if [[ -f "$WS_PIPEDAL_ROLLBACK/kernel.txt" ]]; then
    grubby --set-default "$(cat "$WS_PIPEDAL_ROLLBACK/kernel.txt")" >>"$WS_LOG_FILE" 2>&1 || true
  fi
  systemctl daemon-reload >/dev/null 2>&1 || true
  info "Rollback finished. The previous desktop packages were requested for reinstall."
}

pipedal_wizard_apply() {
  local steps=(
    "Snapshot before changes"
    "Realtime kernel"
    "Remove graphical desktop"
    "Appliance base"
    "Core hardening"
    "Surface baseline"
    "Selected upgrades"
    "Build PiPedal"
    "Boot splash"
    "Snapshot after changes"
    "Save desktop state"
  )
  local i n ok
  info "=== PIPEDAL APPLY START ==="
  for i in "${!steps[@]}"; do
    n=$((i + 1))
    ok=1
    ui_phase "$n" "${#steps[@]}" "${steps[i]}"
    info "APPLY STEP $n/${#steps[@]}: ${steps[i]}"
    case "$i" in
      0) harden_install_snapper; snapshot_create "ws-setup pre"; pipedal_snapshot_rpms ;;
      1)
        if ! pipedal_install_rt_kernel; then
          if pipedal_rt_continue; then
            warn "Continuing on the Fedora kernel without PREEMPT_RT. The desktop will still be removed."
          else
            WS_FAILURES+=("pipedal-rt-kernel")
            WS_FAILED=$((WS_FAILED + 1))
            WS_PIPEDAL_ABORT=1
            ok=0
            error "PiPedal stopped before the desktop was removed because no PREEMPT_RT kernel installed."
          fi
        else
          pipedal_set_rt_default
          pipedal_enroll_mok
        fi
        ;;
      2) [[ "$WS_PIPEDAL_ABORT" == "1" ]] || pipedal_remove_graphical ;;
      3) [[ "$WS_PIPEDAL_ABORT" == "1" ]] || pipedal_base ;;
      4) [[ "$WS_PIPEDAL_ABORT" == "1" ]] || hardening_apply ;;
      5)
        if [[ "$WS_PIPEDAL_ABORT" != "1" ]]; then
          surface_apply
          pipedal_set_rt_default
        fi
        ;;
      6) [[ "$WS_PIPEDAL_ABORT" == "1" ]] || { cli_apply_extras; catalog_apply_selected; } ;;
      7)
        if [[ "$WS_PIPEDAL_ABORT" != "1" ]]; then
          if ! pipedal_build; then
            pipedal_rollback || true
            WS_FAILURES+=("pipedal-build")
            WS_FAILED=$((WS_FAILED + 1))
            WS_PIPEDAL_ABORT=1
            ok=0
          fi
        fi
        ;;
      8) look_install_boot_splash ;;
      9) [[ "$WS_PIPEDAL_ABORT" == "1" ]] || snapshot_create "ws-setup post" ;;
      10) [[ "$WS_PIPEDAL_ABORT" == "1" ]] || de_save_state ;;
    esac
    ui_phase_done "${steps[i]}" "$ok"
  done
  info "=== PIPEDAL APPLY DONE applied=$WS_APPLIED skipped=$WS_SKIPPED failed=$WS_FAILED ==="
  ui_write_result
}
