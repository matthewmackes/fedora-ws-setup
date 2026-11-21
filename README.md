# Fedora Workstation Setup

Fullscreen **Anaconda** installer that turns a **minimal Fedora** install into a lean workstation or a headless PiPedal appliance. Pick **one** desktop — **XFCE**, **MDE** (Mackes Desktop Environment 2026), or **PiPedal** — then apply the matching baseline and optional upgrades. MDE is a standalone DE under [`mde/`](mde/); this setup delegates to it (`--de=mde`). XFCE gets the mm look, codecs, and LightDM. PiPedal removes the display server.

The window is the same hub-and-spoke installer Anaconda uses: a summary screen, one spoke per choice, and **Begin Installation**. Package output stays in the log and on the progress screen.

## Requirements

- Fedora 40+ (43+ recommended; linux-surface and curated COPRs target recent releases)
- A graphical session for the fullscreen installer, or a text console for `--cli`
- Root for real applies; `--dry-run` works unprivileged
- Network access for repos, Flathub, COPR, and Google Fonts
- `anaconda-widgets` (Anaconda's GTK hub, spoke, and selector widgets), installed with `dnf install anaconda-widgets python3-gobject python3-dbus gtk3` on first root run

## Quick start

```bash
cd /home/mm/fedora-ws-setup
sudo ./fedora-ws-setup.sh
```

Run it from a terminal on the desktop so the fullscreen installer can open on your display. The first root run installs the Anaconda widgets if they are missing.

On a text console, including a base Fedora Server install with no display, the same command opens the text installer. It asks ten questions, then applies. `whiptail` is used when `newt` can be installed. Numbered prompts are used when it cannot. These start that installer directly:

```bash
sudo ./fedora-ws-setup.sh --cli
sudo ./fedora-ws-setup.sh pipedal
sudo ./fedora-ws-setup.sh --cli --de=mde
```

`--cli` and `--tui` are the same option. `cli`, `tui`, `xfce`, `sway`, and `pipedal` are accepted as words after the flags. `WS_CLI=1` does the same. `--noninteractive` never prompts.

Dry-run (no changes):

```bash
./fedora-ws-setup.sh --dry-run --noninteractive
./fedora-ws-setup.sh --dry-run --noninteractive --de=mde
./fedora-ws-setup.sh --dry-run --simulate-surface=pro --noninteractive
```

## Flags

| Flag | Meaning |
|------|---------|
| `--dry-run` | Print actions only |
| `--noninteractive` | Skip the questions; all users; no catalog upgrades; apply immediately |
| `--cli`, `--tui` | Text installer. Ten questions. Used automatically when there is no display. |
| `--de=xfce\|mde\|pipedal` | Desktop to install. In either installer this is the preselected choice. In `--noninteractive` it is the choice. `--de=sway` and `--de=miracle` are rejected. |
| `--force-surface=laptop\|pro` | Force Surface baseline |
| `--no-surface` | Disable Surface path |
| `--simulate-surface=laptop\|pro` | Dry-run only: pretend host is Surface |

Noninteractive desktop when `--de` is omitted: keep the desktop that is already installed. If both XFCE and MDE are installed, keep XFCE. A previous Sway or Miracle install counts as MDE and is replaced by labwc. If neither is installed, install XFCE.

## Text installer

The text installer asks ten questions. Press Enter to take the bracketed default.

1. Desktop: XFCE, MDE, or PiPedal. The other desktop is removed with no second prompt.
2. Users: all local users, the first local user, or none.
3. OpenSSH: enable sshd, enable sshd and open the firewall, or leave SSH as it is. PiPedal defaults to enabling sshd.
4. mDNS: Avahi plus http and mdns in the firewall, Avahi only, or off. PiPedal defaults to Avahi plus the firewall.
5. Updates: security updates and a weekly full upgrade, security updates only, or off.
6. PiPedal: pedal essentials (TooB, Guitarix, Calf, x42, ZAM, Dragonfly), default yes. XFCE and Sway: the recommended extras.
7. PiPedal: amp models (Neural Amp Modeler, AIDA-X, Ratatouille, Proteus, BYOD), default yes. XFCE and Sway: DevOps tools.
8. PiPedal: GxPlugins, default yes. XFCE and Sway: browser extras.
9. PiPedal: the USB card to use, or leave the device unset, or use ALSA card 0. The rate stays 48 kHz and the buffer stays 16 frames by 4 periods. XFCE and Sway: gaming.
10. PiPedal: Secure Boot. Ask at install time, queue MOK with password `pipedal`, or leave Secure Boot alone. XFCE and Sway: studio audio.

If the realtime kernel cannot be installed, PiPedal asks whether to continue on the Fedora kernel. No leaves the desktop in place. After a successful install the script prints `http://<hostname>.local/` for PiPedal and reboots. A failed install does not reboot.

## Desktops

Exactly one choice is installed.

- **PiPedal** — a console appliance with no display server. See [PiPedal](#pipedal) below.

- **XFCE** — spin-style group, then Office/mail/games extras are removed. LightDM. mm panel, theme, wallpaper.
- **MDE** — [Mackes Desktop Environment 2026](mde/README.md): Windows-style stacking shell on Fedora `labwc` (Win11-proportioned taskbar/Start/Settings, server-side title bars). Profiles: `--mde-profile=minimal|workstation`.

Switching desktops removes the other desktop's packages (shared apps such as Firefox and Terminator are kept), runs `dnf autoremove`, and backs up that desktop's user config to `~/ws-setup-backup/` before deleting it. LightDM's system default session follows the choice. The last applied desktop is stored in `/var/lib/fedora-ws-setup/state`.

## MDE (formerly Sway Experience)

The Sway Desktop Experience is now the standalone **Mackes Desktop Environment** under [`mde/`](mde/). Install with `sudo mde/install.sh` or `sudo ./fedora-ws-setup.sh --de=mde`. The compositor is **Fedora `labwc`**. Login offers one session, **Mackes Desktop Environment**. `--de=sway` and `--de=miracle` are rejected. See [`mde/README.md`](mde/README.md).

### Shell behaviour (MDE)

MDE runs on Wayland. LightDM starts the `mde` session (`mde-session`), which runs labwc. The keyboard layout comes from localectl, and the shell autostart starts the taskbar, Start menu, and tray helpers. Miracle WM and the Sway compositor are removed. swaybg, swayidle, swaylock, swaync, and swayosd stay; they are not the window manager.

- **Taskbar**: `ws-taskbar`, a native Windows 11 taskbar (Python GTK). Start, Task View (and optionally Search) sit in the centre with your pinned apps and open windows grouped by app, in full colour, with the running and active indicators. Click an app to open, switch to or minimise it; with several windows a list appears. Middle-click or Shift-click opens a new window. Right-click for the jump list: the app, its actions, Pin/Unpin from taskbar, and Close window. Drag icons to reorder pins. Start's "Pin to taskbar" adds apps. Taskbar pins live in `~/.config/ws-taskbar/pins`. On the right are the "Show hidden icons" chevron and the system tray (the taskbar is the StatusNotifier watcher), an updates icon when updates are waiting, the keyboard layout when there is more than one, the Quick Settings button (network, volume, battery), the clock with the notification bell, and the show-desktop corner. Quick Settings has Wi-Fi (with a network list), Bluetooth, Airplane mode, Night light, Do not disturb and Power mode tiles, brightness and volume sliders (with a sound output picker), battery and a Settings gear. The clock opens the notification centre: every notification grouped by app (swaync still shows the pop-ups), Clear all, Do not disturb, and a calendar with Focus session controls. Task View (Super+Tab) shows the windows of a desktop as cards above a desktop strip: switch, add, rename and close desktops, and drag windows between them. Right-click the empty taskbar for Task Manager and Taskbar settings. Settings › Personalization › Taskbar sets alignment, Search style, Task View, tray icons, auto-hide, badges, flashing, all displays, the show-desktop corner, seconds and combining buttons.
- **Start menu**: tap Super alone, or click the start button. Start is a Windows 11-style flyout inside `ws-taskbar` (`ws-start` / `ws-drawer` are thin clients): a search box, Pinned apps in pages of six columns (right-click to move to the front or unpin), Recommended (recently added and most used apps, recent files), All apps with letter headings and a letter jump grid (right-click an app there and choose Move to Advanced to tag it: it leaves All apps, Start pins, the taskbar pin, Recommended, Recently added and Most used, and appears at the top of Settings › Advanced under Moved from Start; Start search still finds it; Remove from Advanced in that section puts it back on All apps without restoring pins), and the account and power buttons at the bottom. Typing searches apps, settings and files in your home folder, with a Best match and a preview pane. Right-click inside Start, or run `ws-start --settings`, to open Settings › Personalization › Start. When an update is waiting for a restart, the power button shows a dot and offers Update and shut down or Update and restart. Apps use padOS (light); GTK 4 is bridged through adw-gtk3 so the same theme name applies. Dark mode switches GTK to adw-gtk3-dark and icons to Mackes-Carbon-Light. Qt apps use qt5ct/qt6ct with Fusion and a padOS palette. The pointer theme is Adwaita (Mackes-Carbon has no cursors).
- **Windows**: one desktop. Windows overlap and open centered. labwc draws a Windows 11 caption (icon, title, minimize, maximize, close) in the accent colour, with a slight corner radius and a soft shadow. Double-click the title bar to maximize, middle-click to close, right-click for the window menu. Super+Left/Right snap to half the screen, Super+Up maximizes, and dragging to a screen edge snaps or maximizes. Settings updates the caption colours immediately.
- **Motion**: Windows 11 timings and curves. Start and the flyouts rise from behind the taskbar (250 ms, decelerating) and drop back (167 ms, accelerating), and reverse smoothly if reopened mid-way. Task View and the desktop name fade. Hover fills fade in 83 ms, taskbar and Start icons shrink while pressed, and the active-app pill stretches from the running dot. Keys repeat after 500 ms at 30 per second, touchpads scroll the content, and notifications stay 5 seconds. Settings › Accessibility › Visual effects › Animation effects turns the motion off. Dialogs float, and so do Pavucontrol, Blueman, the network editor, calculators, swappy and wdisplays. Click to focus.
- **Settings**: Super+I, the Settings pin in Start or Win+X opens `ws-settings`, a native Windows 11 Settings app. It has System (display with a 15-second "Keep these display settings?" countdown saved to kanshi, night light, sound, notifications and focus, power, storage, multitasking, clipboard, about), Personalization (background, colours with light/dark/custom modes and an accent from the Windows palette or the wallpaper, themes, lock screen, fonts, Start, taskbar), Apps, Accounts, Time & language, Gaming, Accessibility, Privacy & security, Windows Update, and Advanced (an icon grid of every settings tool installed, with apps moved from Start in a Moved from Start section at the top). System changes go through D-Bus services (hostname1, timedate1, locale1, AccountsService, PackageKit, power profiles, firewalld), so polkit asks for a password only when needed. Windows Update checks every 6 hours, badges the taskbar, and installs core updates on restart through PackageKit offline updates. `ws-settings display` (or `ms-settings:display`) opens a page directly, and Start search finds every page. Choices are stored in `~/.config/ws-settings/settings.json` and re-applied when the setup runs again.
- **Management Console**: `mde-mmc` is the primary administrator console (Computer Management plus separate Linux snap-ins). Device Manager lists PCI/USB and can disable/enable/uninstall via polkit. Disk Management shows volumes, LVM/RAID/LUKS/remote inventory, and TRIM. Local Users and Groups lists accounts/groups with password-aging actions. Services maps systemd startup types correctly (Automatic / Manual / Disabled / Masked) and elevates start/stop/enable through `mde-mmc-helper`. Event Viewer covers Application/System/Security/Setup and SELinux AVC. Task Scheduler lists systemd timers and cron. Performance Monitor samples CPU/memory/disk/network. Network, Firewall, and Package Management are separate snap-ins. Win+X and Run (`compmgmt.msc`, `devmgmt.msc`, `diskmgmt.msc`, `lusrmgr.msc`, `services.msc`, `eventvwr.msc`, `taskschd.msc`, `perfmon.msc`, `network.msc`, `firewall.msc`, `packages.msc`, `mmc`) open `mde-mmc`. Settings › Advanced only launches the console. Privileged actions use one polkit action and write `/var/log/mde-mmc-audit.log`.
- **Advanced menu**: right-clicking the Start button or pressing Super+X opens the Windows 10 power-user menu (Installed apps, Power Options, Event Viewer, System, System Information, Device Manager, Network Connections, Disk Management, Computer Management, Terminal and Terminal (Admin), Task Manager, Settings, File Explorer, Search, Run, Shut down or sign out, Desktop). Underlined letters pick an item. Super+D shows the desktop and brings the windows back.
- **Shortcuts**: Super+/ lists them all. Super+I opens Settings, Super+A Quick Settings, Super+N notifications, and Super+1…9 opens the Nth taskbar app. Alt+Tab picks a window from a list showing app names and window titles. Super+Left/Right snap, Super+Up maximizes, Super+Down restores or minimizes. Super+Shift+V opens clipboard history. Also Super+E (Thunar), Super+L (lock), Super+D (show desktop), Alt+F4 (close), Ctrl+Alt+T (terminal) and Ctrl+Alt+Del (power menu). Super+Shift+E asks before logging out. Super+Shift+R reloads the labwc config.
- **Screenshots**: Print, Shift+Print, and Super+Shift+S open the Win11-style snipping overlay (Rectangular / Window / Full screen). Super+Print opens the same overlay with Window preselected. Captures save under `~/Pictures/Screenshots` (changeable in Settings › Gaming › Captures), copy to the clipboard, and show a thumbnail toast — click the toast to edit in swappy.
- **Media keys**: volume, mic, and brightness show a SwayOSD popup, including on the lock screen. SwayOSD comes from the `erikreider/swayosd` COPR. If that fails, the keys still work without a popup.
- **Notifications**: SwayNotificationCenter draws the toast pop-ups (and keeps DND / MPRIS state). The taskbar bell opens the MDE notification centre and calendar; Quick Settings (and Win+X Mobility Center) owns Wi-Fi, Bluetooth, volume and brightness. Low-battery warnings appear at 20% and 10%.
- **Session**: swayidle locks the screen after 5 minutes idle and turns it off 30 seconds later (changeable in Settings › System › Power & battery), but not while something is fullscreen. wlsunset provides night light. It also locks before suspend and on lid close. cliphist keeps text and image clipboard history. lxpolkit, gnome-keyring, nm-applet and blueman-applet (behind the hidden-icons chevron), udiskie automounts USB drives with an eject icon, XDG autostart (autorandr, input-remapper, dnfdragora and Plank are hidden in MDE), and portals (wlr for screen sharing and screenshots, GTK for file pickers). Firefox, Electron, Qt, and SDL apps run natively on Wayland. XWayland handles the rest. `DesktopNames` / `XDG_CURRENT_DESKTOP` is `MDE:sway`.
- **Displays**: laptop and external scales follow `WS_SCALE_TIER` / EDID PPI (1, 1.25, or 2 — not a fixed 1.5). Arrange monitors in wdisplays, then run **Save Display Layout** to store them as a kanshi profile that is applied whenever the same screens are connected.
- **Power menu**: nwg-bar with Lock, Log Out, Suspend, Restart, and Shut Down. Log Out, Restart, and Shut Down ask first.

To install or re-apply MDE, run `sudo tools/apply-sway-look.sh` (or `sudo mde/install.sh`). It installs Fedora labwc, removes the Sway compositor, writes helpers under `/usr/bin/ws-*` and `/usr/share/mde/`, seeds `~/.config/mde/labwc`, backs up and removes `~/.config/mde/sway`, makes MDE the LightDM default, and records the choice. Edit the look in `mde/install/lib/look_mde.sh`.

Helpers live in `/usr/bin/ws-*`. The labwc config is `~/.config/mde/labwc` (rc.xml, autostart, menu.xml, environment, theme), with the same tree at `/etc/xdg/mde/labwc`. Shell settings stay in `ws-taskbar`, `ws-start`, `nwg-bar`, `swaync`, `wofi`, `swaylock`, `swayosd`, `kanshi`, `ws-settings`, `qt5ct`, `qt6ct`, and portals. Re-applying the look rewrites the labwc files.

## What the graphical base always applies

PiPedal does not apply this list. Its plan is the console appliance described below.

- LightDM, Firefox, Terminator, VLC, Mousepad, Ristretto, Xarchiver, dnfdragora
- RPM Fusion Free+Nonfree, OpenH264, codecs, GPU VA-API (Intel/AMD)
- Power profiles through `tuned-ppd` (left in place when it is already installed; `power-profiles-daemon` is not installed beside it)
- DVD playback: the RPM Fusion tainted repository, then `libdvdcss`
- firewalld (FedoraWorkstation zone when present)
- `dnf-automatic` security updates and a weekly full-upgrade timer
- zram sized to RAM and `systemd-oomd`
- fwupd metadata refresh, `fstrim.timer`, chronyd, SELinux left enforcing
- dnf `fastestmirror=True` and `max_parallel_downloads=10`
- snapper pre/post snapshots when root is btrfs
- Themes: **padOS** / **Mackes-Carbon** / **Arctodon-Dark** (XFCE). MDE gets padOS (GTK 4 bridged via adw-gtk3), Mackes-Carbon (Mackes-Carbon-Light in dark mode), Adwaita cursors, Selawik (Microsoft's open Segoe UI stand-in) and Cascadia Mono sized to the Fluent type ramp, Windows blue, and the same wallpaper.

NVIDIA drivers stay a manual Hardware catalog item.

## Boot splash

Every install — XFCE, Sway, and PiPedal — sets Plymouth's theme to **Neon Tech Penguin** and rebuilds the initrd. The image is `assets/plymouth/neon-tech-penguin/splash.png`. It is scaled to fit the screen on black. An encrypted disk still gets a password box over the splash. `rhgb quiet` stays on the kernel command line so the splash is shown. A later run skips the initrd rebuild when that theme and image are already current. `sudo tools/apply-sway-look.sh` sets the same splash.

## Recommended upgrades

`catalog/00-recommended.conf` is pre-checked. Uncheck anything you do not want.

- Full unfiltered Flathub (system remote)
- Power profiles, if the base step has not already enabled `tuned-ppd`
- CUPS, SANE, Avahi
- Noto CJK, Noto emoji, metric-compatible fonts, and the top 20 English Google Fonts (Fedora packages when they exist, otherwise a download into `/usr/share/fonts/google`)
- Bluetooth plus PipeWire/WirePlumber

`--noninteractive` does **not** apply this group.

## Other upgrade categories

Desktop Polish (XFCE-only items hidden on MDE) · DevOps · Browsers · Communication · Media · Creative · Data/Science · Gaming · Security · Hardware · Audio Production.

## PiPedal

PiPedal is the third desktop choice. It is a guitar appliance: the next boot is a text console, and the pedal is used from a phone or browser at `http://<hostname>.local/`.

- The desktop, display manager, and X11/Wayland servers are removed. PipeWire and WirePlumber are masked so PiPedal can open the USB audio device through ALSA.
- A PREEMPT_RT kernel is required. The installer tries `kernel-rt` from the Fedora kernel team's `@kernel-vanilla/fedora` COPR, then `kernel-rt-mao` from Audinux. The Fedora kernel stays on the boot menu. The RT kernel is the default. Boot arguments are `mitigations=off` and `usbcore.autosuspend=-1`.
- If Secure Boot is on and the kernel package ships a certificate, that certificate is queued for MOK enrolment with the password `pipedal`.
- CPU tuning is tuned's `latency-performance` profile. Wi-Fi power save is turned off. The service account is `pipedal_d` (the account PiPedal's own installer creates), with realtime priority 95 and unlimited locked memory.
- PiPedal is compiled from the latest GitHub release into `/opt/pipedal`. The build tree is deleted after a successful install. A later run rebuilds only when a newer release exists.
- Audio starts at 48 kHz and 16 frames × 4 periods. The first USB audio interface is selected when one is plugged in. ALSA MIDI is available for a USB foot controller.
- If the source build fails, the installer rolls back: removed desktop packages are reinstalled, the RT kernel packages added for PiPedal are removed, and the previous boot target is restored.
- Choosing XFCE or Sway later removes the PiPedal service, makes the Fedora kernel the default again, and leaves the RT kernel installed.
- The hub hides workstation categories. What remains is a slim Recommended group (SSH, mDNS, security updates), Security, Hardware, and a pedalboard LV2 set: TooB, Guitarix, Calf, x42, ZAM, Dragonfly, Neural Amp Modeler, AIDA-X, Ratatouille, Proteus, BYOD, and the GxPlugins pedals. REAPER, synths, drums, and studio mixing plugins stay on the graphical desktops only.
- A Tone3000 starter set is downloaded only when the API returns models licensed CC0 (free for any use) without asking for an account. Tone3000 currently requires a signed-in session, so the installer records that skip and the models can be downloaded from PiPedal's web UI.

## Audio Production

- **REAPER** is installed from the official reaper.fm tarball (latest version scraped from the download page) into `/opt/REAPER`, with a system menu entry and `/usr/local/bin/reaper`. REAPER is paid software with a 60-day evaluation. REAPER only scans `/usr/lib/lv2` by default, so each selected user's `~/.config/REAPER/reaper.ini` gets `lv2path_linux` including Fedora's `/usr/lib64/lv2`.
- **Realtime audio** installs `realtime-setup`, PipeWire JACK and qpwgraph, and adds the selected users to the `realtime` and `audio` groups. Log out and back in to pick this up.
- **LV2 plugins** are the ones r/linuxaudio and r/Reaper recommend again and again. Fedora builds are used where they exist: Calf, LSP, x42, ZAM, SWH, Guitarix, ZynAddSubFX, setBfree and the rncbc synths. Everything else comes from the [Audinux COPR](https://copr.fedorainfracloud.org/coprs/ycollet/audinux/): Dragonfly, Airwindows, Vitalium, OB-Xd, Dexed, TAL, Vaporizer2, Odin2, Cardinal, Geonkick, DrumGizmo, sfizz, GxPlugins, Neural Amp Modeler, ChowDSP, the B-series and more. Audinux is enabled at priority 100 so Fedora's packages always win.
- **Surge XT** has no LV2 build in Audinux, so it installs as VST3 + CLAP. REAPER loads both.

## Surface

When a Surface **Laptop** or **Pro** is detected (or forced):

1. Add the linux-surface repo
2. Install `kernel-surface`, `iptsd`, `libwacom-surface`, and the default-kernel watchdog
3. If Secure Boot is on: install `surface-secureboot` and show MOK steps (password: `surface`) at the end
4. Pro also gets `florence` and `iio-sensor-proxy`

## Layout

```
fedora-ws-setup.sh     # entry; opens the Anaconda installer, or the text installer
lib/anaconda_ui.py     # fullscreen hub, spokes, and progress
lib/cli.sh             # ten-question text installer
lib/                   # core, ui, detect, de, base, look, surface, catalog, hardening
catalog/               # 00 recommended, 01–10 roles, 11 sway, 12 audio, PiPedal recommended
assets/                # themes, icons, fonts, wallpaper, boot splash, meowmenu RPM
tools/apply-sway-look.sh  # switch to / re-apply the MDE labwc session
tools/capture-profile.sh
logs/
```

## Logs

Each run writes `logs/ws-setup-YYYYMMDD-HHMMSS.log`. The progress spoke shows the current phase and the latest log lines. Failures are listed on the completion screen. After a real apply the installer offers a reboot.
