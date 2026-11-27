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

cli_add_remove_items() {
  local id
  for id in "$@"; do
    [[ -n "$id" ]] || continue
    WS_REMOVE_ITEMS+=("$id")
  done
}

# Tri-state intent per id: leave | install | keep | remove
declare -A WS_CLI_ITEM_STATE=()

cli_item_status_suffix() {
  case "$1" in
    install) printf '%s\n' "[install]" ;;
    keep) printf '%s\n' "[keep]" ;;
    remove) printf '%s\n' "[remove]" ;;
    *) printf '%s\n' "" ;;
  esac
}

cli_category_master() {
  local title="$1"
  cli_choose "$title" \
    "Category master. Per-item checklist follows unless you pick leave-as-is defaults only." \
    keep \
    leave "Leave defaults (recommended install / installed keep)" \
    install "Queue all items in this category for install" \
    keep "Keep all installed; leave absent unchecked" \
    remove "Remove all installed items in this category" \
    none "Uncheck everything (no install, no remove)"
}

cli_ask_category_items() {
  local file="$1" title="$2" master="$3"
  local -a ids=() labels=() presents=() defaults=()
  local line id label status present state args=()
  catalog_applied_load
  while IFS= read -r line || [[ -n "$line" ]]; do
    catalog_parse_line "$line" || continue
    catalog_guard_ok "$C_GUARD" || continue
    catalog_de_ok "$C_DE" || continue
    catalog_item_available "$C_KIND" "$C_PAYLOAD" || continue
    present=0
    catalog_item_installed "$C_KIND" "$C_PAYLOAD" "$C_ID" && present=1
    if (( present == 1 )); then
      state="keep"
    else
      case "$(basename "$file")" in
        00-recommended.conf|00-recommended-pipedal.conf) state="install" ;;
        *) state="leave" ;;
      esac
    fi
    case "$master" in
      install) state="install" ;;
      keep) (( present == 1 )) && state="keep" || state="leave" ;;
      remove) (( present == 1 )) && state="remove" || state="leave" ;;
      none) state="leave" ;;
    esac
    ids+=("$C_ID")
    presents+=("$present")
    defaults+=("$state")
    labels+=("$C_LABEL $(cli_item_status_suffix "$state")")
  done < "$file"
  ((${#ids[@]})) || return 0

  if cli_whiptail_ok; then
    args=()
    local i on
    for ((i = 0; i < ${#ids[@]}; i++)); do
      on="OFF"
      [[ "${defaults[i]}" == "install" || "${defaults[i]}" == "keep" ]] && on="ON"
      # Present+ON = keep; absent+ON = install; present+OFF will become remove after confirm path
      args+=("${ids[i]}" "${labels[i]}" "$on")
    done
    local selected=""
    selected="$(
      whiptail --title "$title" --backtitle "Fedora Workstation Setup" \
        --checklist "Checked = install or keep. Uncheck an installed item to remove it." \
        22 78 12 "${args[@]}" 3>&1 1>&2 2>&3
    )" || die "Installer cancelled"
    # whiptail returns quoted tags
    selected="${selected//\"/}"
    local -A picked=()
    local tok
    for tok in $selected; do
      picked["$tok"]=1
    done
    for ((i = 0; i < ${#ids[@]}; i++)); do
      id="${ids[i]}"
      if [[ -n "${picked[$id]:-}" ]]; then
        if (( presents[i] == 1 )); then
          WS_CLI_ITEM_STATE["$id"]=keep
        else
          WS_CLI_ITEM_STATE["$id"]=install
          cli_add_items "$id"
        fi
      else
        if (( presents[i] == 1 )); then
          WS_CLI_ITEM_STATE["$id"]=remove
          cli_add_remove_items "$id"
        else
          WS_CLI_ITEM_STATE["$id"]=leave
        fi
      fi
    done
    return 0
  fi

  # Numbered fallback: toggle each item
  local i choice
  for ((i = 0; i < ${#ids[@]}; i++)); do
    id="${ids[i]}"
    if (( presents[i] == 1 )); then
      choice="$(cli_choose "$title — ${labels[i]}" \
        "Item is installed." \
        keep \
        keep "Keep installed" \
        remove "Remove on apply" \
        leave "Leave alone (same as keep)")"
      case "$choice" in
        remove) WS_CLI_ITEM_STATE["$id"]=remove; cli_add_remove_items "$id" ;;
        *) WS_CLI_ITEM_STATE["$id"]=keep ;;
      esac
    else
      choice="$(cli_choose "$title — ${ids[i]}" \
        "${labels[i]}" \
        "$([[ "${defaults[i]}" == "install" ]] && echo install || echo leave)" \
        install "Install" \
        leave "Leave alone")"
      if [[ "$choice" == "install" ]]; then
        WS_CLI_ITEM_STATE["$id"]=install
        cli_add_items "$id"
      else
        WS_CLI_ITEM_STATE["$id"]=leave
      fi
    fi
  done
}

cli_confirm_removes() {
  ((${#WS_REMOVE_ITEMS[@]})) || return 0
  local list
  list="$(printf '%s, ' "${WS_REMOVE_ITEMS[@]}" | sed 's/, $//')"
  if ! cli_yesno "Confirm catalog removals" \
    "These catalog items will be removed: ${list}. Continue?"; then
    die "Installer cancelled"
  fi
}

cli_confirm_de_uninstall() {
  [[ -n "${WS_UNINSTALL_DE:-}" ]] || return 0
  if ! cli_yesno "Confirm desktop uninstall" \
    "This will uninstall the ${WS_UNINSTALL_DE} desktop/experience. Shared keep-apps stay. Continue?"; then
    die "Installer cancelled"
  fi
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
  # Per-category master + per-item tri-state checklist (full catalog parity).
  local file title master qn=6 n=0
  local -a files=()
  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    catalog_visible_file "$file" || continue
    files+=("$file")
  done < <(catalog_files)
  local total=${#files[@]}
  for file in "${files[@]}"; do
    title="$(catalog_category_title "$file")"
    master="$(cli_category_master "$(cli_q "$qn" "$title master")")"
    cli_ask_category_items "$file" "$(cli_q "$qn" "$title")" "$master"
    n=$((n + 1))
    # Keep question numbers in the 6–10 band for UX; wrap if many categories.
    if (( qn < 10 )); then
      qn=$((qn + 1))
    fi
  done
  (( total > 0 )) || true
}

cli_ask_pipedal_extras() {
  local choice file title master
  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    catalog_visible_file "$file" || continue
    title="$(catalog_category_title "$file")"
    master="$(cli_category_master "$(cli_q 6 "$title master")")"
    cli_ask_category_items "$file" "$(cli_q 6 "$title")" "$master"
  done < <(catalog_files)
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

  WS_UNINSTALL_DE=""
  choice="$(cli_choose "$(cli_q 1c "Uninstall experience")" \
    "Optionally remove a desktop already on this machine. Shared keep-apps are protected." \
    none \
    none "Do not uninstall a desktop" \
    mde "Uninstall MDE" \
    xfce "Uninstall XFCE" \
    pipedal "Uninstall PiPedal" \
    all "Uninstall all graphical experiences managed here")"
  [[ "$choice" == "none" ]] || WS_UNINSTALL_DE="$choice"

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
  WS_REMOVE_ITEMS=()
  WS_CLI_ITEM_STATE=()
  if [[ "$WS_DE" == "pipedal" ]]; then
    [[ "$WS_CLI_SSH" == "leave" ]] || cli_add_items rec-ssh
    [[ "$WS_CLI_MDNS" == "off" ]] || cli_add_items rec-mdns
    cli_ask_pipedal_extras
  else
    cli_ask_desktop_extras
  fi
  cli_confirm_removes
  cli_confirm_de_uninstall
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
  info "CLI choices: de=$WS_DE users=${WS_TARGET_USERS[*]:-none} ssh=${WS_CLI_SSH:-leave} mdns=${WS_CLI_MDNS:-off} updates=${WS_UPDATES:-weekly} items=${WS_SELECTED_ITEMS[*]:-none} remove=${WS_REMOVE_ITEMS[*]:-none} uninstall_de=${WS_UNINSTALL_DE:-none} audio=${WS_PIPEDAL_ALSA_CARD:-auto} mok=${WS_PIPEDAL_MOK_CHOICE:-queue} rt-fail=${WS_PIPEDAL_RT_ON_FAIL:-stop}"
  cli_say ""
  cli_say "$(de_switch_summary)"
  if ((${#WS_REMOVE_ITEMS[@]})); then
    cli_say "Catalog removals: ${WS_REMOVE_ITEMS[*]}"
  fi
  if [[ -n "${WS_UNINSTALL_DE:-}" ]]; then
    cli_say "Desktop uninstall: $WS_UNINSTALL_DE"
  fi
  cli_say "Applying. Package output is in $WS_LOG_FILE"
  wizard_apply
  cli_finish
}
