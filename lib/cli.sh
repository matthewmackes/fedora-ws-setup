#!/usr/bin/env bash
# Text installer for a console with no graphical session.
# Ten multiple-choice questions, then the same apply path as the Anaconda window.
# whiptail (the newt package) is used when it can be installed. Numbered prompts
# are the fallback, including when newt cannot be installed.
# shellcheck disable=SC2034

WS_CLI_WHIPTAIL=0
WS_CLI_SSH="${WS_CLI_SSH:-}"
WS_CLI_MDNS="${WS_CLI_MDNS:-}"
WS_PIPEDAL_RT_ON_FAIL="${WS_PIPEDAL_RT_ON_FAIL:-}"
WS_PIPEDAL_MOK_CHOICE="${WS_PIPEDAL_MOK_CHOICE:-}"
WS_PIPEDAL_ALSA_CARD="${WS_PIPEDAL_ALSA_CARD:-}"

cli_say() {
  printf '%s\n' "$*" >/dev/tty 2>/dev/null || printf '%s\n' "$*"
}

cli_ensure_ui() {
  WS_CLI_WHIPTAIL=0
  if [[ "${WS_CLI_FORCE_BASH:-0}" == "1" ]]; then
    info "Numbered prompts selected"
    return 0
  fi
  if ! command -v whiptail >/dev/null 2>&1; then
    if [[ "$(id -u)" -eq 0 ]] && ! is_dry_run; then
      info "Installing newt for the text installer"
      dnf_yes install newt || true
    fi
  fi
  if command -v whiptail >/dev/null 2>&1 && [[ -e /dev/tty && "${TERM:-dumb}" != "dumb" ]]; then
    WS_CLI_WHIPTAIL=1
    info "Text installer: whiptail"
  else
    info "Text installer: numbered prompts"
  fi
}

cli_require_tty() {
  if [[ -r /dev/tty && -w /dev/tty ]]; then
    return 0
  fi
  die "The text installer needs a terminal. For an unattended install use --noninteractive --de=pipedal"
}

cli_whiptail_ok() {
  [[ "$WS_CLI_WHIPTAIL" == "1" ]]
}

# cli_choose TITLE PROMPT DEFAULT tag label [tag label...]
# Prints the chosen tag. Escape cancels the installer.
cli_choose() {
  local title="$1" prompt="$2" default="$3"
  shift 3
  local choice=""
  if cli_whiptail_ok; then
    choice="$(
      whiptail --title "$title" --backtitle "Fedora Workstation Setup" \
        --default-item "$default" --menu "$prompt" 20 74 8 \
        "$@" 3>&1 1>&2 2>&3
    )" || die "Installer cancelled"
    printf '%s\n' "$choice"
    return 0
  fi
  cli_choose_bash "$title" "$prompt" "$default" "$@"
}

cli_choose_bash() {
  local title="$1" prompt="$2" default="$3"
  shift 3
  local -a tags=() labels=()
  local i=0 n=0 ans="" pick=""
  while (($#)); do
    tags+=("$1")
    labels+=("$2")
    shift 2
  done
  n="${#tags[@]}"
  cli_say ""
  cli_say "$title"
  cli_say "$prompt"
  for ((i = 0; i < n; i++)); do
    cli_say "  $((i + 1))) ${labels[i]}"
  done
  printf 'Choice [%s]: ' "$default" >/dev/tty
  IFS= read -r ans </dev/tty || die "Installer cancelled"
  ans="${ans#"${ans%%[![:space:]]*}"}"
  ans="${ans%"${ans##*[![:space:]]}"}"
  if [[ -z "$ans" ]]; then
    printf '%s\n' "$default"
    return 0
  fi
  if [[ "$ans" =~ ^[0-9]+$ ]] && ((ans >= 1 && ans <= n)); then
    printf '%s\n' "${tags[ans - 1]}"
    return 0
  fi
  for pick in "${tags[@]}"; do
    if [[ "$ans" == "$pick" ]]; then
      printf '%s\n' "$pick"
      return 0
    fi
  done
  die "Unknown choice: $ans"
}

# 0 = yes, 1 = no or escape. Used while the install is already running.
cli_yesno() {
  local title="$1" prompt="$2"
  if cli_whiptail_ok; then
    if whiptail --title "$title" --backtitle "Fedora Workstation Setup" --yesno "$prompt" 12 70; then
      return 0
    fi
    return 1
  fi
  cli_say ""
  cli_say "$title"
  cli_say "$prompt"
  cli_say "  1) Yes"
  cli_say "  2) No"
  printf 'Choice [Yes]: ' >/dev/tty
  local ans=""
  IFS= read -r ans </dev/tty || return 1
  case "${ans,,}" in
    ""|y|yes|1) return 0 ;;
    *) return 1 ;;
  esac
}

# Escape cancels. Default is Yes. 0 = yes, 1 = no.
cli_ask_yes() {
  local title="$1" prompt="$2" choice=""
  choice="$(cli_choose "$title" "$prompt" yes \
    yes "Yes" \
    no "No")" || return 1
  if [[ "$choice" == "yes" ]]; then
    return 0
  fi
  return 1
}

cli_q() {
  printf 'Question %s of 10 — %s' "$1" "$2"
}

cli_add_items() {
  local id
  for id in "$@"; do
    [[ -n "$id" ]] || continue
    WS_SELECTED_ITEMS+=("$id")
  done
}

cli_set_users() {
  local choice="$1"
  WS_TARGET_USERS=()
  case "$choice" in
    all)
      if ((${#WS_LOCAL_USERS[@]})); then
        WS_TARGET_USERS=("${WS_LOCAL_USERS[@]}")
      fi
      ;;
    first)
      if ((${#WS_LOCAL_USERS[@]})); then
        WS_TARGET_USERS=("${WS_LOCAL_USERS[0]}")
      fi
      ;;
  esac
}

# Prints tag and label lines for USB cards: card<TAB>label
cli_usb_cards() {
  local dir card id
  for dir in /proc/asound/card[0-9]*; do
    [[ -f "$dir/usbid" ]] || continue
    card="${dir##*/card}"
    id="$(head -n 1 "$dir/id" 2>/dev/null || echo "USB")"
    printf '%s\tUSB card %s (%s)\n' "$card" "$card" "$id"
  done
}

cli_ask_audio() {
  local -a args=()
  local line card label default=""
  while IFS=$'\t' read -r card label; do
    [[ -n "$card" ]] || continue
    args+=("$card" "$label")
    [[ -n "$default" ]] || default="$card"
  done < <(cli_usb_cards)
  args+=("unset" "Leave the device unset. PiPedal's web page will ask.")
  args+=("alsa0" "Use ALSA card 0 even when it is not USB")
  [[ -n "$default" ]] || default="unset"
  cli_choose "$(cli_q 9 "Audio device")" \
    "Sample rate stays 48 kHz. Buffer stays 16 frames and 4 periods." \
    "$default" "${args[@]}"
}

cli_ask_desktop_extras() {
  local choice
  choice="$(cli_choose "$(cli_q 6 "Recommended")" \
    "Workstation extras. The base install already includes codecs, the look, and hardening." \
    all \
    all "Flathub, power profiles, printing, fonts, and Bluetooth audio" \
    noflathub "Power profiles, printing, fonts, and Bluetooth audio" \
    none "None of the recommended extras")"
  case "$choice" in
    all) cli_add_items rec-flathub rec-power rec-print rec-fonts rec-btaudio ;;
    noflathub) cli_add_items rec-power rec-print rec-fonts rec-btaudio ;;
  esac

  choice="$(cli_choose "$(cli_q 7 "DevOps")" \
    "Optional development tools." \
    none \
    none "No DevOps packages" \
    podman "Podman suite and the build toolchain" \
    docker "Docker, VS Code, and KVM")"
  case "$choice" in
    podman) cli_add_items devops-podman devops-toolchain ;;
    docker) cli_add_items devops-docker devops-vscode devops-kvm ;;
  esac

  choice="$(cli_choose "$(cli_q 8 "Browsers")" \
    "Firefox is already part of the graphical base." \
    none \
    none "No extra browser setup" \
    policy "Firefox policy pack" \
    chromium "Firefox policy pack and Chromium")"
  case "$choice" in
    policy) cli_add_items browser-firefox-policy ;;
    chromium) cli_add_items browser-firefox-policy browser-chromium ;;
  esac

  choice="$(cli_choose "$(cli_q 9 "Gaming")" \
    "Optional game launchers." \
    none \
    none "No gaming packages" \
    steam "Steam" \
    more "Steam, Lutris, and Wine")"
  case "$choice" in
    steam) cli_add_items game-steam ;;
    more) cli_add_items game-steam game-lutris game-wine ;;
  esac

  choice="$(cli_choose "$(cli_q 10 "Studio audio")" \
    "Optional audio applications. PiPedal's pedal plugins are not part of this desktop." \
    none \
    none "No studio audio packages" \
    reaper "REAPER and the realtime audio groups" \
    studio "REAPER, realtime audio, and the mixing, synth, and drum LV2 sets")"
  case "$choice" in
    reaper) cli_add_items audio-reaper audio-realtime ;;
    studio) cli_add_items audio-reaper audio-realtime audio-lv2-mix audio-lv2-synths audio-lv2-drums ;;
  esac
}

cli_ask_pipedal_extras() {
  local choice
  if cli_ask_yes "$(cli_q 6 "Pedal essentials")" \
    "Install TooB, Guitarix, Calf, x42, ZAM, and Dragonfly?"; then
    cli_add_items audio-pedal-core
  fi
  if cli_ask_yes "$(cli_q 7 "Amp models")" \
    "Install Neural Amp Modeler, AIDA-X, Ratatouille, Proteus, and BYOD?"; then
    cli_add_items audio-pedal-amp
  fi
  if cli_ask_yes "$(cli_q 8 "GxPlugins")" \
    "Install the GxPlugins guitar pedals?"; then
    cli_add_items audio-pedal-gx
  fi
  WS_PIPEDAL_ALSA_CARD="$(cli_ask_audio)"
  choice="$(cli_choose "$(cli_q 10 "Secure Boot")" \
    "This applies only when Secure Boot is on and a realtime kernel was installed." \
    ask \
    ask "Ask again at install time whether to queue MOK" \
    queue "Queue MOK enrolment. The password is pipedal." \
    skip "Leave Secure Boot alone. The Fedora kernel stays bootable.")"
  WS_PIPEDAL_MOK_CHOICE="$choice"
  WS_PIPEDAL_RT_ON_FAIL=ask
}

cli_ask() {
  local users choice default_de
  default_de="$(de_default_choice)"
  default_de="$(de_canonical "$default_de")"
  WS_DE="$(cli_choose "$(cli_q 1 "Desktop")" \
    "XFCE and PiPedal remove the other choice. MDE coexists with other desktops and is installed from the standalone mde/ tree." \
    "$default_de" \
    xfce "XFCE. LightDM, the mm panel, theme, and wallpaper." \
    mde "MDE. Mackes Desktop Environment 2026 on labwc (stacking windows, Win11-proportioned shell)." \
    pipedal "PiPedal. No display server. The pedal is used from a phone or browser.")"
  WS_DE="$(de_canonical "$WS_DE")"
  if [[ "$WS_DE" == "mde" ]]; then
    WS_MDE_PROFILE="$(cli_choose "$(cli_q 1b "MDE profile")" \
      "Minimal is the shell only. Workstation adds the baseline (codecs, firewalld, updates, plymouth, default apps)." \
      "${WS_MDE_PROFILE:-workstation}" \
      minimal "Minimal shell" \
      workstation "Workstation MDE")"
  fi
  de_mark_switch

  choice="$(cli_choose "$(cli_q 2 "Users")" \
    "Local users: ${WS_LOCAL_USERS[*]:-none}. PiPedal itself runs as the pipedal_d service account. XFCE and Sway write the look for the users you pick." \
    all \
    all "All local users" \
    first "Only the first local user (${WS_LOCAL_USERS[0]:-none})" \
    none "No per-user settings")"
  cli_set_users "$choice"

  if [[ "$WS_DE" == "pipedal" ]]; then
    users="The appliance is reached over the network."
  else
    users="OpenSSH is not part of the graphical base."
  fi
  WS_CLI_SSH="$(cli_choose "$(cli_q 3 "OpenSSH")" \
    "$users" \
    "$([[ "$WS_DE" == "pipedal" ]] && echo on || echo leave)" \
    on "Enable and start sshd" \
    firewall "Enable sshd and open ssh in the firewall" \
    leave "Leave SSH as it is")"

  WS_CLI_MDNS="$(cli_choose "$(cli_q 4 "mDNS")" \
    "Avahi publishes http://$(hostname -s 2>/dev/null || echo hostname).local/." \
    "$([[ "$WS_DE" == "pipedal" ]] && echo full || echo off)" \
    full "Avahi, and open http and mdns in the firewall" \
    avahi "Avahi only" \
    off "Leave discovery off")"

  WS_UPDATES="$(cli_choose "$(cli_q 5 "Updates")" \
    "Security updates install on their own. The weekly choice also runs a full upgrade." \
    weekly \
    weekly "Security updates and a weekly full upgrade" \
    security "Security updates only" \
    off "No automatic updates")"

  WS_SELECTED_ITEMS=()
  if [[ "$WS_DE" == "pipedal" ]]; then
    [[ "$WS_CLI_SSH" == "leave" ]] || cli_add_items rec-ssh
    [[ "$WS_CLI_MDNS" == "off" ]] || cli_add_items rec-mdns
    cli_ask_pipedal_extras
  else
    cli_ask_desktop_extras
  fi
}

cli_apply_extras() {
  [[ "${WS_CLI:-0}" == "1" ]] || return 0
  if [[ "${WS_CLI_SSH:-leave}" == "firewall" || ( "${WS_CLI_SSH:-leave}" == "on" && "${WS_DE:-}" != "pipedal" ) ]]; then
    cfg_sshd
  fi
  if [[ "${WS_CLI_SSH:-leave}" == "firewall" ]]; then
    if is_dry_run; then
      info "DRY-RUN: firewall-cmd add-service ssh"
    elif command -v firewall-cmd >/dev/null 2>&1; then
      firewall-cmd --permanent --add-service=ssh >/dev/null 2>&1 || true
      firewall-cmd --reload >/dev/null 2>&1 || true
    fi
  fi
  if [[ "${WS_DE:-}" == "pipedal" || "${WS_CLI_MDNS:-off}" == "off" ]]; then
    return 0
  fi
  cfg_avahi
  if [[ "$WS_CLI_MDNS" == "full" ]]; then
    if is_dry_run; then
      info "DRY-RUN: firewall-cmd add-service http mdns"
    elif command -v firewall-cmd >/dev/null 2>&1; then
      firewall-cmd --permanent --add-service=http >/dev/null 2>&1 || true
      firewall-cmd --permanent --add-service=mdns >/dev/null 2>&1 || true
      firewall-cmd --reload >/dev/null 2>&1 || true
    fi
  fi
}

cli_finish() {
  local host
  host="$(hostname -s 2>/dev/null || echo localhost)"
  if (( WS_FAILED > 0 )) || [[ "${WS_PIPEDAL_ABORT:-0}" == "1" ]]; then
    cli_say "Install did not finish. Failures: ${WS_FAILURES[*]:-see the log}"
    cli_say "Log: $WS_LOG_FILE"
    surface_mok_instructions || true
    pipedal_mok_instructions || true
    return 0
  fi
  if [[ "${WS_DE:-}" == "pipedal" ]]; then
    cli_say "PiPedal will answer at http://${host}.local/"
  fi
  surface_mok_instructions || true
  pipedal_mok_instructions || true
  if is_dry_run; then
    info "DRY-RUN: systemctl reboot"
    cli_say "Dry-run finished. The machine was not rebooted."
    return 0
  fi
  cli_say "Rebooting now."
  systemctl reboot
}

run_cli() {
  cli_require_tty
  cli_ensure_ui
  info "Text installer: ten questions, then apply (desktop default $(de_default_choice))"
  cli_ask
  info "CLI choices: de=$WS_DE users=${WS_TARGET_USERS[*]:-none} ssh=${WS_CLI_SSH:-leave} mdns=${WS_CLI_MDNS:-off} updates=${WS_UPDATES:-weekly} items=${WS_SELECTED_ITEMS[*]:-none} audio=${WS_PIPEDAL_ALSA_CARD:-auto} mok=${WS_PIPEDAL_MOK_CHOICE:-queue} rt-fail=${WS_PIPEDAL_RT_ON_FAIL:-stop}"
  cli_say ""
  cli_say "$(de_switch_summary)"
  cli_say "Applying. Package output is in $WS_LOG_FILE"
  wizard_apply
  cli_finish
}
