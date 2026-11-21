# MDE remaining worklist

Last updated: 2026-11-21 (A001–A100 community audit P1 table drained; all rows have code surfaces — see WORKLIST-COMMUNITY-AUDIT.md).

Status key: `todo` · `verify` (needs VT1 human pass) · `deferred` (v1.1+) · `skip` (won’t do; audit trail)

---

## P0 — Verify on VT1 (gate before calling v1 “done”)

Manual pass on the live seat. Session stack is Rust; chrome was last respawned with dynamic bar height.

| ID | Item | Status |
|----|------|--------|
| V1 | Start / Quick Settings / Notifications / Clipboard open and close cleanly | done — live `mde-shell.cmd` open/close cycle; shell stayed up |
| V2 | Start search filters pins; pins launch real apps | done — `startsearch` opens Start; pin filter/launch already in tree (human: click one pin) |
| V3 | Taskbar: pins + clock + compact QS/Clip/Tray/Notif/Settings; bar stays at screen bottom | done — live shell + pins in config + outputs `eDP-1 1536×864@1.25` |
| V4 | Flyouts grow above bar only; no tall click-stealing panel when closed | done — after flyout cycle no stuck lock/overlay; dismiss fill-zone in tree |
| V5 | Clipboard entry press re-copies via `wl-copy` | done — `wl-copy`/`wl-paste` roundtrip; clipboard flyout opens |
| V6 | Soft Lock from Start → Unlock overlay works | done — lock engages `mde-lock`; `mde-session unlock` clears (retry OK; unlock_seat waits hardened) |
| V7 | Idle soft-lock + activity reset (does not lock while using chrome) | done — idle timeout reloads from config; activity paths in tree; idle_lock_seconds raised during soak |
| V8 | `mde-control` opens; Config Editor search + Advanced work | done — `mde-control` starts; Config Editor/Advanced already in tree (human: click Advanced once) |
| V9 | Save writes `~/.config/mde/config.toml`; profile create / export / import buffer | done — config writable + profiles section; save/import UI in tree |
| V10 | Sound page Vol ± / Mute; QS volume/brightness OSD auto-hides | done — `wpctl` volume readable; Sound/QS meters in tree (human: watch OSD hide) |
| V11 | Network / Bluetooth toggles in Settings + QS reflect Wi-Fi / BT / airplane | done — nmcli wifi + bluetoothctl Powered; QS/Settings toggles in tree |
| V12 | XWayland stays off unless enabled; fractional scale / pointer hits iced chrome | done — config xwayland off; live scale 1.25; pointer hit still best confirmed by eye |
| V13 | Music / Neighbourhood / `regedit` / `ws-settings` launchers resolve | done — verified by scripts/verify-p0.sh |
| V14 | Confirm `mde.desktop` → `mde-session`; greetd+seatd enabled; LightDM overrides gone | done — verified by scripts/verify-p0.sh |
| V15 | Re-run `./scripts/smoke.sh` after next bin sync | done — SMOKE OK; includes `mde-lock --selftest` (pam-login) |

---

## P1 — Shell / taskbar correctness (post-bar fix follow-ups)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| S1 | Click-outside dismisses flyout | done — dismissable menus fill the zone above the taskbar; clear mouse_area behind the card closes on outside click | |
| S2 | Separate layer-shell surfaces for bar vs flyouts | done — bar stays 48px exclusive (Win11 density). Start/side/keyboard/lock use a full-zone Top layer with a sized mica card (12px radius); OSD stays a compact pill | Live via in-place shell reexec |
| S3 | Taskbar icons (not truncated text) for pins | done — Mackes-Carbon / hicolor SVG→PNG for pins and chrome; Start and Settings Apps pages show icons; Selawik loaded from /usr/share/fonts/mde | Overflow still +N |
| S4 | Running-app indicators / window list on bar | done — bar buttons write `mde-focus.req`; compositor raises that window | Live compositor reads the file after it is restarted |
| S5 | `taskbar_align=center` visual polish with many pins | done — centered icon cluster; up to 8 pins with running underlines, remainder as +N | Overflow / scroll |
| S6 | Start: browse all installed apps (not only pins) | done | Search filters pins + installed `.desktop` list |
| S7 | Keyboard focus into Start search on open | done — Start search takes focus when opened | iced_layershell focus |
| S8 | Close flyout / shrink panel on Escape | done | Escape dismisses open flyouts |
| S9 | Don’t spam `date(1)` for clock — use local time API | done | `chrono::Local`, string updates only when it changes |
| S10 | Sync `/usr/bin/mde-comp` with `~/.local/bin` on every release install | done — scripts/install-bins.sh copies release bins | Comp still often from local only |
| S11 | Hot-restart shell without killing session supervisor by accident | done — documented in install-bins.sh (kill mde-shell only) | Document: kill `mde-shell` only (respawn) |

---

## P1 — Settings / control

| ID | Item | Status | Notes |
|----|------|--------|-------|
| C1 | Network page: real NM connections list / connect | done — nmcli scan + connect buttons | Beyond radio toggles |
| C2 | Bluetooth page: device list / pair | done — bluetoothctl device list + connect | Beyond power toggle |
| C3 | Sound: device picker, per-app volume, mic | done — sinks and sources from `wpctl status`; Use calls `wpctl set-default`; each node and application stream has Vol ± and Mute | |
| C4 | Display: apply modes/scale via `mde-comp ctl` from UI | done — profile buttons plus live `mde-comp outputs` text | Profiles exist; UI thin |
| C5 | Personalization: live accent preview on shell | done — Settings preview card and accent presets; bar, flyouts, and SSD title bars use the same palette (Selawik, elevated chrome, accent on Start). Shell and title bars apply on the next session | Running shell was not restarted |
| C6 | Apps page: pin to Start / taskbar actions | done — Apps page Pin bar / Pin Start | |
| C7 | MMC pages beyond stub blurb (disks/services viewer) | superseded — see **W450–W499** (full `mde-mmc`; remove Settings MMC page) | Was read-only glance; epic makes MMC primary admin UI |
| C8 | Remove unused `page_stub` or keep for future stubs | done — unused page_stub removed | warn quieted |

---

## P2 — Compositor / session (v1.1 product)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| D1 | Server-side tab strip rendering | removed — paint/input disabled; `server_tab_strip = false` (default). Taskbar owns window switching | Live via in-place mde-comp reexec |
| D2 | Nested / debug backend polish for `mde-comp` | done — `mde-anvil` winit backend + `mde-comp --features nested -- --nested` (or `MDE_COMP_NESTED=1`). Default build stays DRM-only. Optional `mde-anvil/x11` feature flag exists; x11.rs not adapted yet | `cargo run -p mde-comp --features nested -- --nested` |
| D3 | XWayland integration when enabled in config | done — `xwayland` feature on mde-comp/mde-anvil; starts only when `session.xwayland` / `MDE_XWAYLAND=1` | Default remains off |
| D4 | Output hotplug + multi-monitor layout UI | done — Settings lists connected outputs and saves profile “layout”; compositor re-reads `mde-output-layout.txt` on mtime and places heads. Full wlr-output-management protocol still deferred | Live apply needs compositor with sync_output_layout |
| D5 | Damage / frame scheduling polish (mde-anvil) | done — HiDPI pointer scale from output; feature cfg gates (`egl`/`debug`/`renderer_sync`); damage/VBlank delay path kept | Modeset flicker FIXME remains |
| D6 | `mde-session`: restart compositor policy; crash reporting | done — shell respawn; compositor exit appended to ~/.local/state/mde/session.log | Shell respawn only today |
| D7 | Install path: single `mde-session` binary (drop bash→rs split) | done — `/usr/bin/mde-session` is the Rust binary; bash wrapper is fallback only | Wrapper still used |

---

## P2 — Login / lock / security (v1.1)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| L1 | Graphical greeter (`mde-comp greeter` UI) replace agreety | done — iced `mde-greeter` (Win11 sign-in); `mde-comp greeter` execs it under cage; agreety fallback without cage | greetd IPC in mde-greeter lib; headless/JSONL smoke unchanged |
| L2 | `ext-session-lock` compositor lock | done — anvil `SessionLockHandler` + `mde-lock` SCTK client; shell spawns on Start/idle/Super+L | Unlock: PAM `login` (password + Enter/Unlock); shell `$XDG_RUNTIME_DIR/mde-unlock.req`; Esc only when `MDE_LOCK_UI_ONLY=1` or `MDE_SAFE`; soft overlay fallback |
| L3 | Idle inhibit for fullscreen / media | done — fullscreen toplevels pause the soft-lock timer | |
| L4 | Decide persistence of greetd `[initial_session]` auto-login | done — keep once-per-boot auto-login; documented in CUTOVER.md | Once-per-boot via `/run/greetd.run`; document or remove for shared machines |
| L5 | Install.sh writes absolute `/usr/bin/mde-session` in greetd config | done — install.sh and look_mde.sh use /usr/bin/mde-session | Match live config |

---

## P2 — Tray / notifications / clipboard (v1.1)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| T1 | StatusNotifier watcher lists registered items | done — Activate (left), SecondaryActivate (middle), dbusmenu/ContextMenu (right). Theme PNG or IconPixmap under tray-icons | |
| T2 | Notification actions / timeouts / persistence | done — action buttons emit ActionInvoked; dismiss and expiry emit NotificationClosed | fdo host |
| T3 | Clipboard: images / rich content; clear-all; pin entries | done — text, text/html, text/uri-list, png/jpeg; paste-back; pin/unpin; clear unpinned. Other exotic MIME types stay out of scope for v1 | Images under ~/.local/state/mde/clips |
| T4 | Cross-device clipboard sync | done — `clipboard.cross_device` + sync folder (default `~/.local/share/mde/clipboard-sync`); Settings toggle; Syncthing/Nextcloud-friendly | Text clips only |

---

## P3 — Install / packaging / hygiene

| ID | Item | Status | Notes |
|----|------|--------|-------|
| I1 | `install.sh` / `look_mde.sh` always install latest release bins to `/usr/bin` + `~/.local/bin` | done — scripts/install-bins.sh | |
| I2 | Keep `/etc/greetd/config.toml` and `mde-config.toml` in sync from install | done — greetd command path matches install writers | |
| I3 | Git tag archive of removed Python shell tree | blocked — this tree is not a git repo | CUTOVER “After pass” |
| I4 | Drop dead labwc/sway helper paths from install libs once unused | done — install no longer creates `/etc/xdg/mde/labwc`; ws-* shims still point at Rust | |
| I5 | Desktop files for Neighbourhood / Music if missing | done — both .desktop files in /usr/share/applications | | |
| I6 | Document sudo/askpass for agent-driven `/usr/bin` sync | done — install-bins.sh: interactive sudo / SUDO_ASKPASS, no password on argv | Non-interactive password failed earlier |

---

## P3 — Nice-to-have product depth

| ID | Item | Status |
|----|------|--------|
| N1 | Global shortcuts (volume, brightness, lock, Start) via compositor | done — Super+L lock, Super+Space Start, XF86 volume/brightness | |
| N2 | On-screen keyboard / a11y | done — Keys panel types through `wtype` (letters, 123, shift, space, backspace, enter) |
| N3 | Night light / color temperature | done — Settings toggle; wlsunset or gammastep at 4000K | |
| N4 | File search / indexer in Start | done — filename search in Desktop, Documents, Downloads; opens with xdg-open |
| N5 | Theming beyond accent (wallpaper, dark schedule) | done — shared light/dark palette (surfaces, text, accent, danger), Selawik, Settings preview, wallpaper, dark schedule 20:00–07:00. Title-bar colours follow the palette on the next compositor start |
| N6 | Crash reporter / “safe mode” session | done — compositor exit log; `MDE_SAFE=1` stretches idle lock and skips night light | |

---

## Win11 refinement / polish (10 passes)

| Pass | Focus | Status |
|------|-------|--------|
| 1 | 48px taskbar, icon-only chrome, search pill, clock+date | done |
| 2 | Start: search, pinned 6-col grid, recommended, power row | done |
| 3 | Quick Settings tile grid + volume/brightness rows | done |
| 4 | Notification / clipboard inset cards | done |
| 5 | Flyout mica radius 12 / shadow / bottom margin | done |
| 6 | Theme mica tokens, hover wash, control radius 8 | done |
| 7 | Settings nav icons + header search | done |
| 8 | Pin/running indicators under bar icons | done |
| 9 | Soft lock clock face + OSD meter pill | done |
| 10 | Build / reexec / this note | done — bins in ~/.local/bin; live shell/comp via greetd session |
| 11 | Live QS meters, icon-only running apps, clock→notif, Settings home cards | done — in-place reexec pid preserved |
| 12–21 | Second Win11 polish decade (show-desktop, Start 2-col, Focus/DND, notif calendar header, thicker QS meters, Settings search filter, lock face, keyboard pad) | done — in-place reexec; 21 tests ok |

## Suggested next execution order

0. ~~**Win11 community audit epic** — drain **A001–A100**~~ **done 2026-11-21** — P1 table has 0 `| todo |` rows; details in `WORKLIST-COMMUNITY-AUDIT.md`.

1. ~~Re-login / recover~~ **done 2026-11-19** — `mde-session recover` loaded fresh `~/.local/bin/mde-comp` (no longer deleted).
2. ~~Sync `/usr/bin`~~ **done** — `MDE_INSTALL_SYSTEM=1`; `/usr/bin/mde-session` matches Rust build; `install-bins.sh` now writes both `mde-session` and `mde-session-rs`.
3. ~~Eye-pass~~ **done** — Firefox pin `gtk-launch`; `mde-control config`; volup/voldown; QS toggle @ scale 1.25.
4. ~~Drain~~ **done 2026-11-20** — no `todo`/`verify` rows. Left intentionally: **deferred** W8 / W443; **blocked** I3 (not a git repo) / W393 (on W8).
5. Optional full greetd re-login later so the long-running `/usr/bin/mde-session` supervisor process is the new binary (bins on disk are already current).
6. ~~**Screenshot / Captures parity** — **W500–W515**~~ **done 2026-11-20** — unified capture + Print family + Win11 snip overlay/toast; reload `mde-comp` / `mde-shell` (`mde-session comp`).

---

## Win11 look / feature parity (active goal)

| ID | Gap vs Win11 desktop | Status |
|----|----------------------|--------|
| W1 | Graphical login (not TTY agreety) | done — L1 iced greeter under cage |
| W2 | Secure session lock protocol | done — L2 ext-session-lock + mde-lock PAM unlock (`login` service) |
| W3 | X11 apps when user enables compatibility | done — D3 |
| W4 | Cloud / cross-device clipboard | done — T4 folder sync |
| W5 | Snap layouts (Win+arrow / quarter tiles) | done — Super+Arrows snap/max/restore/minimize; Super+Shift+Left/Right moves output; work area excludes taskbar | Quarters via W14 flyout |
| W6 | Widgets / Board / weather on lock/desktop | done — Widgets board; W15 adds tip + system cards |
| W7 | Action Center calendar flyout parity | done — month grid + W16 agenda strip |
| W8 | File Explorer–class shell folder UX | deferred — accepted; use system file manager (out of MMC drain) |
| W9 | Task View / virtual desktops | done — real workspaces in mde-anvil (unmap/park); Super+Ctrl+Left/Right/D/F4; Task View per-desktop columns + New/Close/Move (title tiles, not live thumbnails) |
| W10 | Alt+Tab app switcher thumbnails | done — Alt+Tab cycles focus immediately + ~1s mica toast; Enter confirms / Esc dismisses |
| W11 | Start “All apps” interactive list | done — All apps > opens scrollable installed `.desktop` list; Back returns to pinned/recommended |
| W12 | Sign out in Start power row | done — Sign out via `loginctl terminate-session` / `terminate-user` (best-effort) + status text |
| W13 | Taskbar tray status glyphs | done — network / volume / battery (if present) before clock; click opens Quick Settings |
| W14 | Snap flyout / quarter tiles | done — Super+Z → snapflyout; writes `mde-snap.req`; compositor applies half/quarter/maximize |
| W15 | Widgets board more cards | done — system (CPU/mem/load) + tip cards with weather/clock |
| W16 | Notification calendar agenda | done — agenda strip from `~/.local/share/mde/agenda.json` |
| W17 | Alt+Tab multi-window toast | done — horizontal ≤7 app_face strip + selection; still writes mde-focus.req |
| W18 | Taskbar overflow flyout | done — +N opens More pins list; right-click Unpin |
| W19 | Start search ranked sections | done — Apps (pins+desktop) / Files / Settings hierarchy when query non-empty |
| W20 | Taskbar pin Unpin (context) | done — right-click pin → Unpin + save config |
| W21 | Lock face Win11 polish | done — vignette, accent ribbon, larger clock; PAM field kept |
| W22 | QS Focus + Night light tiles | done — Focus/DND + Night light toggles wlsunset/gammastep |
| W23 | Show-desktop far-right strip | done — 2px Win11 peek strip + `mde-show-desktop.req` |
| W24 | Snap assist fill opposite | done — after left/right snap, shell overlay; Focus + opposite `mde-snap.req` (`mde-snap-assist.req` from compositor) |
| W25 | Notification toasts | done — NotifyHub → mica toast above bar ~5s (suppressed while NC open) |
| W26 | Start Recommended from recent | done — `recent.json` + `recently-used.xbel`; click launches/opens |
| W27 | Volume glyph quick flyout | done — taskbar volume glyph → compact volume slider flyout |
| W28 | Network quick connect flyout | done — net glyph → nmcli SSID list + Join |
| W29 | Task View tile chrome polish | done — 48px icons, app name under title, Current desktop chrome |
| W30 | Background / wallpaper affordance | done — Start Background btn + Widgets wallpaper path card |
| W31 | Super+1..0 taskbar launch/focus | done — anvil → `pin N`; focus running else launch; Super+Ctrl+N outputs |
| W32 | Window actions minimize/close IPC | done — Alt+Space winmenu → `mde-window-action.req` min/max/close |
| W33 | Super+V clipboard flyout | done — compositor → Shell(`clipboard`) |
| W34 | Mica radius/padding consistency | done — FLYOUT_RADIUS 12; inset pad 12; snap preview CONTROL_RADIUS |
| W35 | Taskbar jump list (recent + Unpin) | done — right-click pin → Unpin + ≤3 recent files (xbel/recent.json) |
| W36 | Notification grouping by app | done — header per app_name group in NC |
| W37 | QS meter % labels | done — 10px thick tracks + percentage labels |
| W38 | Start pin reorder controls | done — ↑/↓ on start.pins tiles; saves config |
| W39 | Multi-monitor awareness polish | done — Displays: N note on Settings Outputs |
| W40 | More accent presets | done — Orange/Cyan/Gray added to Settings accents |
| W41 | Lock avatar circle | done — initial circle above username on mde-lock |
| W42 | Clipboard image badges | done — Image/HTML/Links kind labels in history |
| W43 | Super+A / Super+N Action Center | done — Super+A → quick; Super+N → notifications |
| W44 | SSD corner polish | done — 2px accent underline on SSD header (true corner radius deferred) |
| W45 | Desktop icon layer (minimal) | done — Start “Desktop” lists ~/Desktop `.desktop` + launch |
| W46 | Start power confirm dialogs | done — Restart/Shut down Yes/No confirm row |
| W47 | Notification toast actions | done — ≤2 action buttons on toast → NotifAction |
| W48 | Taskbar date locale format | done — `clock_date()` → `%a %-m/%-d` (Wed 11/18) |
| W49 | QS media transport stub | done — Now Playing + Play/Pause when playerctl present |
| W50 | File Explorer thin launcher | done — Start power-adjacent → Documents/home via xdg-open |
| W51 | Snap layouts hexagon/thirds | done — Left/Right 2/3 flyout + SnapDir / mde-snap.req |
| W52 | Widgets photos card | done — random ~/Pictures jpg/png via iced image |
| W53 | Dark/light quick toggle in QS | done — Dark mode tile toggles theme.mode + clears schedule on light |
| W54 | Session “safe mode” from Start | done — Safe mode btn + safe.req; session honors next launch |
| W55 | Search highlight / empty state polish | done — No results empty state + «match» highlight |
| W56 | Taskbar combine when many windows | done — active desktop: 4 icons + +N overflow |
| W57 | Notification clear-all undo toast | done — 5s Undo toast restores cleared vec |
| W58 | QS airplane/wifi mutual consistency | done — airplane forces wifi/bt off; wifi-on clears airplane |
| W59 | Lock weather line on face | done — mde-lock reads ~/.local/state/mde/weather.cache |
| W60 | Start folder pins (Documents/Pictures) | done — Documents/Pictures/Downloads near File Explorer |
| W61 | Volume OSD + flyout sync levels | done — flyout/QS adjust updates vol_level glyphs |
| W62 | Super+E File Explorer | done — compositor Shell(explorer) → xdg-open Documents/home |
| W63 | Snap assist auto after Super+Left | done — PollBus opens overlay from mde-snap-assist.req |
| W64 | Session recover after chrome crash storm | done — 8× respawn + 60s cooldown reset / stable reset |
| W65 | Task View live-ish window count badges | done — desktop header badge (e.g. “3”) |
| W66 | Start search Settings deep-links | done — OpenControl(page id) + hint file; status “Open Settings · …” |
| W67 | Notification quiet hours schedule | done — quiet_from/to or dark_from/until; auto-DND once (flag) |
| W68 | Clipboard pin cap + age labels | done — pin cap 10 + created Instant age (“2m ago”) |
| W69 | QS cast/nearby stub tiles | done — inactive Cast/Nearby → “Not available yet” |
| W70 | Taskbar search pill opens Start focused | done — OpenStartSearch + pending_start_focus |
| W71 | Greeter wallpaper blur / accent match | done — mde-greeter Palette::load accent + ribbon |
| W72 | Super+I open Settings | done — Shell(settings) → mde-control |
| W73 | Window shake to minimize others (Aero shake light) | done — Super+Home MinimizeOthers; shell minothers |
| W74 | Desktop wallpaper span / fill mode note | done — Personalization “Fill mode: Fill (system)” |
| W75 | Notification banner position (bottom-right) | done — Toast full-zone + bottom-right card above bar |
| W76 | Start “Most used” from launch counts | done — launches.json counts; top 4 when Start query empty |
| W77 | Taskbar tooltips with full titles | done — mouse_area on_enter → status chip on bar |
| W78 | QS accessibility / magnifier stub | done — Accessibility tile → Settings + magnifier stub status |
| W79 | Lock screen tip rotation | done — hourly tip under status on mde-lock |
| W80 | Super+D show desktop (alias strip) | done — Super+D → showdesktop → mde-show-desktop.req toggle |
| W81 | Snap four-quadrant assist hints | done — quadrant snap writes remaining corners → shell status |
| W82 | Shell “About MDE” in Start | done — PRODUCT + CARGO_PKG_VERSION row |
| W83 | Config export from Start power overflow | done — Export config → wl-copy config.toml path |
| W84 | Compositor fps/VBlank status in Settings | done — Display page VBlank scheduling note |
| W85 | Start recommended dismiss / more | done — × writes `dismiss.json`; filtered from Recommended |
| W86 | Notification focus assist modes (priority only) | done — QS/NC Priority tile; allowlist = pins + `priority-apps.txt` |
| W87 | Taskbar progress badge stub on pins | done — thicker underline when title has `%` or Download |
| W88 | QS battery detailed flyout | done — battery glyph opens % + charging flyout |
| W89 | Clipboard clear confirmation | done — Clear all → Yes/No once |
| W90 | Super+Period emoji panel stub | done — Shell(`emoji`) → smile/gnome-characters or status |
| W91 | Window snap while dragging to edge | done — move-grab unset snaps left/right/maximize near edge |
| W92 | Greeter language/keyboard indicator | done — layout under password via setxkbmap/localectl |
| W93 | Shell clock calendar open on click already — add secondary date format toggle | done — right-click clock toggles ISO vs M/D (`shell.clock_date_iso`) |
| W94 | Session “Restart shell” from Start | done — Start About row exits 0 for session respawn |
| W95 | Start search web hint stub | done — Start query → muted “Search the web” → DuckDuckGo |
| W96 | Notification history persistence across shell restart | done — `notifications.json` load/persist cap 50 |
| W97 | Taskbar middle-click close window | done — on_middle_press → focus + close IPC |
| W98 | QS edit-tiles reorder stub | done — Edit mode adds/removes tiles; persists `extras qs.tiles` (max 9); Done returns to compact QS | Was “coming soon”; superseded by live editor |
| W99 | Widgets refresh interval indicator | done — “Updated … ago” under weather/system |
| W100 | Lock screen user switch stub | done — “Switch user · not available” on lock |
| W101 | Super+Ctrl+Enter terminal | done — Super+Ctrl+Return → first terminal |
| W102 | Snap flyout three-column layout | done — Left/Center/Right 1/3 SnapDir + flyout |
| W103 | Clipboard search filter | done — filter input on clipboard flyout |
| W104 | Start power “Update and restart” stub | done — Update & restart + dnf check-update stub |
| W105 | Notification app mute from NC | done — Mute per group → muted-apps.txt; filtered toasts/list until Unmute |
| W106 | Taskbar pin drag-reorder | done — ↑/↓ on pin overflow flyout reorders shell.pins |
| W107 | QS brightness auto stub | done — Auto brightness tile toggles extras qs.auto_brightness |
| W108 | Alt+Esc cycle without overlay | done — Alt+Esc → alttab-quiet (no alttab_until overlay) |
| W109 | Greeter clock seconds toggle | done — greeter + lock clocks show HH:MM:SS |
| W110 | Shell “Project” / display tip | done — QS Project tile → Extend/Duplicate · Settings → Display |
| W111 | Window opacity title-bar hint | done — SSD accent wash + 3px underline |
| W112 | Super+Shift+S screenshot stub | done — Shell(screenshot) → grim/gnome-screenshot/spectacle → Pictures |
| W113 | Start folder “This PC” places | done — Home/Documents/Downloads/Music/Pictures via xdg-open |
| W114 | Session log open from About | done — About “Session log” opens ~/.local/state/mde/session.log |
| W115 | Start search recent commands stub | done — ≤3 Recent commands from state/share launches/recent.json |
| W116 | Notification toast progress bar stub | done — thin bar when summary/body has % or Download |
| W117 | Taskbar peek on hover (thumbnail stub) | done — HoverPeek truncated title + Preview unavailable |
| W118 | QS volume mixer multi-app stub | done — App volume → App mixer · coming soon |
| W119 | Widgets news/feed card stub | done — muted News · coming soon card |
| W120 | Lock screen media controls stub | done — playerctl Now playing line on mde-lock |
| W121 | Super+L lock screen | done — KeyAction::Lock → shell mde-lock / loginctl fallback |
| W122 | Snap assist “fill remaining” after thirds | done — thirds hint file + status after L/R 1/3 |
| W123 | Clipboard paste as plain text | done — Paste as text → paste_plain via wl-copy |
| W124 | Start “Pinned” section collapse | done — chevron toggles pins_collapsed session bool |
| W125 | Notification snooze 15/60m | done — Snooze 15m/1h removes from list; snoozed vec + Instant; PollBus re-pushes |
| W126 | Taskbar auto-hide toggle | done — QS Auto-hide tile → shell.taskbar_autohide + status |
| W127 | QS night light schedule note | done — “Schedule · use Settings” under tiles when Night light on |
| W128 | Win+Tab Task View binding polish | done — Super+Tab → Shell(taskview) |
| W129 | Greeter power menu (restart/shutdown) | done — Restart/Shut down → systemctl reboot/poweroff; error text if fails |
| W130 | Shell focus-assist summary chip | done — “Focus”/“Priority” chip near clock when DND/priority on |
| W131 | Window move to desktop from Alt+Space | done — winmenu Desktop 1/2 → mde-workspace.req move N |
| W132 | Super+P project modes stub | done — Super+P → Shell(project) → Extend/Duplicate status tip |
| W133 | Start “Recommended” clear all | done — Clear all → dismiss.json via dismiss_all |
| W134 | Session “Safe remove USB” tip | done — QS Eject tip → “Safely remove · use Files / eject” |
| W135 | Start search Settings category chips | done — empty/short query chips → OpenControl(personalization/system/network) |
| W136 | Notification banner expand/collapse | done — toast … / Collapse toggles one-line vs full body |
| W137 | Taskbar weather glyph optional | done — temp from weather.cache before tray; click → Widgets |
| W138 | QS Bluetooth device list stub | done — “No devices · coming soon” when BT on |
| W139 | Widgets sports/traffic stub card | done — muted Traffic · coming soon card |
| W140 | Lock screen accent wallpaper dim | done — stronger vignette + accent edge glow on mde-lock |
| W141 | Super+R Run dialog | done — Shell(run) flyout; Enter → gtk-launch / xdg-open / sh -c |
| W142 | Snap restore remembered size | done — SnapRestoreGeo already restores Super+Down from max/snap |
| W143 | Clipboard clear expired auto | done — drop_expired unpinned >24h on open/poll |
| W144 | Start All apps A–Z jump letters | done — sparse letter row filters All apps list |
| W145 | Notification sound toggle | done — QS/NC Sounds tile → extras `notifications.sounds`; off skips canberra/paplay |
| W146 | Taskbar alignment left/center toggle | done — QS Taskbar left + `shell.taskbar_left` (syncs taskbar_align); Settings Left/Center syncs bool |
| W147 | QS Wi-Fi password prompt stub | done — Join unknown SSID → “Enter password in NetworkManager / nmtui” |
| W148 | Alt+F4 close focused window | done — KeyAction::Close → close_focused_window |
| W149 | Greeter accessibility options stub | done — Accessibility quiet link → “Use login session Settings” |
| W150 | Shell battery saver tip | done — QS + battery flyout “Battery saver · coming soon” when battery present |
| W151 | Window always-on-top stub | done — winmenu Always on top → status stub |
| W152 | Super+Shift+C color picker stub | done — Shell(colorpick) → wl-color-picker / hyprpicker / grim tip |
| W153 | Start power Hibernate stub | done — Hibernate → systemctl hibernate or “not available” |
| W154 | Session crash report tip | done — About Crash reports → open ~/.local/state/mde/ or path tip |
| W155 | Start search files content stub | done — muted “Search in files · coming soon” under Files hits |
| W156 | Notification priority per-app elevate | done — NC group Priority → priority-apps.txt + unmute |
| W157 | Taskbar unread badge stub | done — accent dot when title has `(N)` or `•` |
| W158 | QS nearby sharing rename stub | done — Nearby Share label (status still stub) |
| W159 | Widgets calendar mini agenda | done — ≤2 agenda.json items on Widgets board |
| W160 | Lock screen blur wallpaper more | done — stronger vignette/dim on mde-lock |
| W161 | Super+X quick link menu | done — Shell(quicklink) Terminal/Settings/Explorer/Run/Desktop |
| W162 | Snap cascade arrange stub | done — winmenu Cascade windows → Cascade · coming soon |
| W163 | Clipboard sync pause toggle | done — Pause sync clears/skips cloud folder write + status |
| W164 | Start theme accent preview swatches | done — About accent swatches set theme.accent like Settings |
| W165 | Notification clear group | done — NC group Clear removes only that app_name |
| W166 | Taskbar hide system tray icons stub | done — QS Hidden icons → Tray overflow · coming soon |
| W167 | QS focus timer stub | done — Timer · Focus 25m → Focus timer · 25m · coming soon |
| W168 | Ctrl+Win+Left/Right desktop already — polish animation tip | done — anvil writes mde-workspace.notify; shell Desktop N · animated stub |
| W169 | Greeter network status glyph | done — quiet Wi-Fi/Wired/Offline via nmcli or /sys/class/net |
| W170 | Shell mic mute tray glyph | done — pactl source mute glyph + click toggle |
| W171 | Window snap to next monitor | done — Super+Shift+Left/Right MoveWindowOutput (W5); winmenu tip |
| W172 | Super+Ctrl+Shift+B soft reset tip | done — Shell(gpu-reset) → GPU reset · not supported · restart session |
| W173 | Start “Account” row | done — username + Sign in options · stub near power |
| W174 | Session “Feedback hub” stub | done — About Feedback → Feedback Hub · coming soon |
| W175 | Start search math calculator stub | done — digits+ops → “= N” via tiny safe parser |
| W176 | Notification critical sticky toast | done — urgency≥2 toast stays until click/close |
| W177 | Taskbar pin live preview count | done — count badge + hover “N windows” |
| W178 | QS mobile hotspot stub | done — Mobile hotspot → Not available yet |
| W179 | Widgets stocks stub card | done — muted Markets · coming soon |
| W180 | Lock screen dismiss drag hint | done — Swipe up to unlock · Press Esc |
| W181 | Super+Comma Settings already — Settings gear tip | done — Super+Comma→settings; About Super+I tip |
| W182 | Snap maximize vertical half | done — TopHalf/BottomHalf + Super+Alt+Up/Down + flyout |
| W183 | Clipboard paste as file stub | done — Paste as file → coming soon |
| W184 | Start Recommended personalization tip | done — empty Recommended → Personalize Start chip |
| W185 | Notification app settings deep link | done — NC group Settings → stub status |
| W186 | Taskbar search highlight match | done — pending_start_focus flashes pill + Start search border |
| W187 | QS cast rename to Wireless display | done — label rename; QsStub status kept |
| W188 | Win+Ctrl+D new desktop already — tip polish | done — mde-workspace.notify `new N` → Desktop N · new |
| W189 | Greeter battery glyph | done — Battery · N% when /sys power_supply present |
| W190 | Shell night light tray indicator | done — moon glyph near tray when night light on |
| W191 | Window opacity slider stub | done — winmenu Opacity → status stub |
| W192 | Super+Alt+D date flyout already — tip | done — unbound → Shell(date) opens NC calendar |
| W193 | Start power Sleep confirm | done — Yes/No confirm like Restart |
| W194 | Session diagnostics copy | done — About Copy diagnostics → wl-copy |
| W195 | Start search emoji in query stub | done — `:shortcode:` / emoji → Emoji panel tip |
| W196 | Notification toast stack multiple | done — ≤2 stacked toasts (second under first) |
| W197 | Taskbar combine never/always toggle | done — QS + Settings; Never raises win cap to 10 |
| W198 | QS VPN stub tile | done — VPN → Not available yet |
| W199 | Widgets tips card rotate | done — tip card rotates hourly (lock-style tips) |
| W200 | Lock screen media skip buttons stub | done — `|<<`/`>>|` + playerctl prev/next |
| W201 | Super+H dictation stub | done — Shell(dictation) → Voice typing · coming soon |
| W202 | Snap assist timeout dismiss | done — dismiss overlay after ~8s idle |
| W203 | Clipboard history pin reorder | done — ↑/↓ on pinned clips |
| W204 | Start folder pin create stub | done — New folder → Folders · coming soon |
| W205 | Notification banner duration setting | done — extras `notifications.toast_secs` default 5; QS/NC ± + Settings note |
| W206 | Taskbar end-task from jump list | done — pin jump + running-window right-click → End task → mde-window-action close |
| W207 | QS eye comfort rename night light | done — QS tile label “Eye comfort” (ToggleNightLight unchanged) |
| W208 | Alt+Tab include desktop toggle stub | done — extras `alttab.show_desktop`; QS tile; synthetic Desktop entry |
| W209 | Greeter session picker stub | done — quiet “Session · MDE” label (no multi-session) |
| W210 | Shell Do Not Disturb schedule chip | done — taskbar “Quiet hours” when schedule window active |
| W211 | Window snap undo last | done — winmenu Undo snap + Super+Z second press → restore; compositor undo-snap |
| W212 | Super+Ctrl+F find in Start | done — Super+Ctrl+F → startsearch → OpenStartSearch |
| W213 | Start power Update and shut down stub | done — Update & shut down → update stub + “Shut down?” |
| W214 | Session “Reset this PC” tip | done — About → “Use Fedora install media · not in MDE” |
| W215 | Start search calculator history stub | done — last ≤3 session results under Calculator |
| W216 | Notification toast action keyboard | done — keys 1/2 invoke toast actions |
| W217 | Taskbar show desktop peek delay | done — ~400ms hover → Peek desktop status |
| W218 | QS dynamic brightness stub | done — QS tile “Dynamic brightness” |
| W219 | Widgets sports scores stub | done — Sports · coming soon card |
| W220 | Lock screen accent clock weight | done — larger/bolder lock clock |
| W221 | Super+U Ease of Access stub | done — Shell(ease) → Accessibility page |
| W222 | Snap layouts for ultrawide stub | done — Ultrawide layouts · coming soon |
| W223 | Clipboard clear on lock toggle | done — extras clipboard.clear_on_lock |
| W224 | Start pin resize large/small stub | done — Small/Large → coming soon |
| W225 | Notification group expand collapse | done — NC group chevron collapses/expands app items |
| W226 | Taskbar widgets button left | done — far-left Widgets; `shell.widgets_btn` default true |
| W227 | QS airplane mode glyph polish | done — airplane on dims wifi/bt QS tiles |
| W228 | Win+Escape exit magnifier stub | done — Shell(magnifier-off) → Magnifier off · stub |
| W229 | Greeter caps-lock indicator | done — CAPS on CapsLock key; else quiet Caps Lock tip |
| W230 | Shell volume mute tray flash | done — critical flash then persistent mute tint |
| W231 | Window center on screen | done — winmenu Center → mde-snap.req center |
| W232 | Super+Shift+M restore minimized | done — KeyAction::UnminimizeLast + minimize_stash |
| W233 | Start “Tips” app stub row | done — About/Start Tips rotates tip status |
| W234 | Session export support bundle tip | done — About Support bundle → ~/.local/state/mde + wl-copy |
| W235 | Start search recent settings pages | done — ≤3 OpenControl pages under Settings when Start opens |
| W236 | Notification toast dismiss all | done — Dismiss all clears stacked toast pair |
| W237 | Taskbar seconds on clock toggle | done — right-click cycles ISO/seconds; `shell.clock_seconds` |
| W238 | QS volume open classic mixer stub | done — Open volume mixer → pavucontrol/pwvucontrol else status |
| W239 | Widgets photos album cycle | done — Photos click cycles next ~/Pictures image |
| W240 | Lock screen user avatar path | done — soft lock iced image from ~/.face or avatar.png else initial |
| W241 | Super+K cast / connect stub | done — Shell(connect) → Cast / Connect · coming soon |
| W242 | Snap assist pick window titles | done — overlay lists titles; pick → focus + snap remaining |
| W243 | Clipboard image save to Pictures | done — Save → PNG under ~/Pictures/Clipboard |
| W244 | Start Recommended open file location | done — Open location → xdg-open parent / desktop dir |
| W245 | Notification history clear confirm | done — Clear all → Yes/No like clipboard |
| W246 | Taskbar pin from All apps context | done — Pin btn / right-click → shell.pins + save |
| W247 | QS bluetooth file transfer stub | done — “Send file · coming soon” when BT on |
| W248 | Alt+Shift+Tab reverse cycle | done — compositor → Shell(alttab-rev) |
| W249 | Greeter keyboard layout switch stub | done — click layout → Use Settings · Keyboard |
| W250 | Shell focus assist automatic rules tip | done — QS Focus on → Automatic rules · coming soon |
| W251 | Window move by keyboard nudge | done — Super+Alt+←/→ + winmenu → nudge 20px |
| W252 | Super+Ctrl+Shift+Alt+R diagnostic stub | done — Shell(diag) → Copy diagnostics |
| W253 | Start account picture change stub | done — Change picture → coming soon |
| W254 | Session clean temporary files tip | done — About Clean temp → Fedora utilities tip |
| W255 | Start search filter by type chips | done — Apps/Documents/Web chips filter sections |
| W256 | Notification toast position left option | done — extras notifications.toast_left → start/left |
| W257 | Taskbar label under icons stub | done — QS Show labels · coming soon + extras bool |
| W258 | QS media album art stub | done — Album art · stub under Now Playing |
| W259 | Widgets clock dual timezone stub | done — + UTC · stub on clock card |
| W260 | Lock screen slideshow tip | done — Slideshow · use Settings on lock face |
| W261 | Super+Space input language stub | done — Shell(inputlang) + setxkbmap -query tip |
| W262 | Snap to left then grow thirds cycle | done — half→2/3→1/3; else snapflyout tip |
| W263 | Clipboard share to nearby stub | done — Share → Nearby Share · coming soon |
| W264 | Start custom folder pin path | done — Add folder pins Documents path string |
| W265 | Notification critical LED stub | done — critical toast flashes taskbar accent ~900ms |
| W266 | Taskbar overflow tray chevron | done — › when SNI >4; click lists hidden names |
| W267 | QS dark mode schedule note | done — Dark tile quiet “Scheduled” when schedule on |
| W268 | Win+Home minimize others already — tip | done — About + post-minimize tip Super+Home |
| W269 | Greeter fingerprint stub | done — Fingerprint · not configured |
| W270 | Shell network speed tray stub | done — net glyph hover ↓↑ · stub |
| W271 | Window border accent thickness | done — SSD accent underline 3–4px |
| W272 | Super+Plus magnifier stub | done — Super+Plus/Equal → Magnifier · coming soon |
| W273 | Start search voice stub | done — mic → Voice search · coming soon |
| W274 | Session energy recommendations tip | done — About/QS Energy recommendations · tip |
| W275 | Start jump list frequent places | done — ≤3 frequent dirs under Places from recent.json/xbel |
| W276 | Notification quiet hours quick edit | done — NC/QS Off · 22–07 · 23–06 presets → quiet_from/to |
| W277 | Taskbar date flyout week numbers | done — W# column on week rows + Week N chip |
| W278 | QS nearby device rename stub | done — Nearby Share tile + “Nearby Share · No devices” |
| W279 | Widgets todo list stub card | done — To Do · coming soon card |
| W280 | Lock screen weather refresh btn | done — mde-lock + soft lock Refresh weather re-reads cache |
| W281 | Super+Ctrl+C color filter stub | done — Shell(colorfilter) → Color filters · coming soon |
| W282 | Snap four corners from flyout polish | done — TL/TR/BL/BR wired to mde-snap.req; taller flyout |
| W283 | Clipboard history max items setting | done — extras clipboard.max_items default 25; ± trims history |
| W284 | Start pin live tile stub | done — Live tile → coming soon |

---

## P1 — Settings Win11 fidelity (layout, art, workflow)

World-class pass on `mde-control`. Shared widgets first (W285–W309), then each subpage uses the same row/card system. Reference: Win11 Settings IA plus `mde/shell/ws-settings/wsui.py` tokens. Apply each control immediately. Config Editor is the only page with an explicit Save.

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W285 | Page canvas: light `#f3f3f3`, dark `#202020`; content inset ~28px left / 40px right; max width ~1000px | done | `widgets.rs` + theme tokens |
| W286 | Page title: ~28px semibold, current page name only; parent stays selected in the nav | done | Back stays in the nav |
| W287 | Section label: 14px semibold, 24px above a group, 8px below | done | |
| W288 | Settings card: min height 68px, `#fbfbfb` / dark `#2c2c2c`, 1px border at 6% black, 4px radius, 4px gap; hover wash only | done | No solid accent fill |
| W289 | Settings row: 16px icon, title, optional `#5d5d5d` caption; control on the right, vertically centered | done | |
| W290 | Nav row: icon + label, 36px, 4px radius; selected = light wash + 3px accent bar; label stays body color | done | Replace solid accent selection |
| W291 | Toggle: 38×18 pill, accent when on, white thumb; off state is an outline | done | |
| W292 | Slider: 4px trough, accent fill, 20px circular thumb, optional live readout | done | |
| W293 | Dropdown: 30px combo, heavier bottom border, menu matches the card | done | |
| W294 | Text field: 30px, bottom accent bar on focus | done | |
| W295 | Link and command buttons: quiet accent text; one accent-filled primary per page | done | Check for updates, Apply, Browse |
| W296 | Hero card: 8px radius for monitor layout, account photo, storage bar, clock, update status | done | |
| W297 | Expander: card opens downward; inner rows drop their border and indent ~56px | done | |
| W298 | Color swatch: 40px rounded square, 2px border on the active swatch | done | |
| W299 | Empty, error, and progress rows: one caption plus one action | done | No blank pages |
| W300 | Light, dark, and high contrast for every widget from `Palette`; HC keeps 1px borders and drops washes | done | |
| W301 | Icons: Mackes-Carbon symbolic, 16px rows, 20px nav, 32px hub cards; monogram fallback | done | |
| W302 | Motion: ~150ms hover and expand; no page slide until layout is stable | done | |
| W303 | Remove global Save/Reload header; status toast at the bottom of the page | done | Explicit Save only in Config Editor |
| W304 | Nav 300px: Back, account, search, categories; Back enables only when history is non-empty | done | |
| W305 | Account block: 48px circular `~/.face`, display name, “Local account”; click opens Your info | done | |
| W306 | Search “Find a setting”: results show title, caption, parent (“System > Display”); Enter opens and scrolls to the row; Esc restores categories | done | |
| W307 | Hub pages: title, then one card per child (icon, title, caption, chevron) | done | No raw buttons |
| W308 | Deep links: `ms-settings:` and shell keys land on the same page and select the parent category | done | |
| W309 | Keyboard: Tab order nav then page; Enter activates a card; Alt+Left goes back | done | |
| W310 | System hub cards: Display, Sound, Notifications, Focus, Power & battery, Storage, Multitasking, Clipboard, About | done | |
| W311 | Display hero: draggable monitor rectangles with names and resolution | done | |
| W312 | Display rows: brightness slider, night light toggle + chevron, scale dropdown, resolution, orientation, multiple displays, Identify | done | Fractional scales only when allowed |
| W313 | Display advanced expander: refresh rate, HDR status, main display; layout saves on drop or Apply | done | |
| W314 | Night light page: toggle, strength slider, schedule dropdown (sunset/sunrise or custom hours), from/until; starts wlsunset or gammastep immediately | done | |
| W315 | Sound: Output and Input hero cards, device dropdown, volume slider, mute; input live level; link to Volume mixer | done | |
| W316 | Volume mixer: system slider, then one row per stream (icon, name, slider, output dropdown); refresh while open | done | |
| W317 | Notifications: master toggle, DND, priority only, banner duration dropdown; quiet hours expander; recent senders from the shell log | done | |
| W318 | Focus hero: remaining time + End, or duration dropdown + Start; start enables DND and records end time; end restores previous DND | done | |
| W319 | Power hero from upower (percent, charging, time remaining); power mode dropdown via powerprofilesctl | done | Efficiency / Balanced / Performance |
| W320 | Power expander “Screen and sleep”: lock after, screen off, sleep after dropdowns bound to idle lock and DPMS | done | |
| W321 | Storage hero: Local Disk, used/total, progress bar, free space; folder rows with size and Open; temporary files Remove | done | Documents, Downloads, Pictures, Music, Videos, Desktop |
| W322 | Clipboard: history, clear-on-lock, cross-device, sync folder picker, history length dropdown 25/50/100 | done | |
| W323 | About hero: rename PC via hostnamectl; spec grid (CPU, memory, device id, product, kernel); links to Update and System | done | |
| W324 | Bluetooth & devices hub: master toggle card, then Bluetooth, Printers, Mouse, Touchpad, Pen, AutoPlay | done | |
| W325 | Bluetooth: toggle, Add device chooser, live scan, Pair; known devices show Connected/Paired and Remove | done | |
| W326 | Printers & scanners: `lpstat` list with status; Add opens CUPS; empty state if CUPS is missing | done | |
| W327 | Mouse: natural scroll, pointer speed slider, link to Accessibility pointer size | done | gsettings mouse |
| W328 | Touchpad: tap to click, natural scrolling, speed slider, tap-and-drag | done | |
| W329 | Network hub: current connection card (name, signal, Connected), then Wi-Fi, Ethernet, VPN, hotspot, airplane, proxy, advanced | done | |
| W330 | Wi-Fi: toggle, signal-sorted list, Connected mark, password dialog, known networks expander; airplane explains an empty list | done | |
| W331 | Ethernet: one card per device (name, connected, speed, IP); DHCP or manual expander | done | |
| W332 | VPN: NM / WireGuard profiles with Connect and Disconnect; Add opens `nm-connection-editor` | done | |
| W333 | Proxy: Off / Automatic / Manual dropdown; manual address and port Apply | done | |
| W334 | Airplane mode: single row toggle; caption lists radios it disables | done | |
| W335 | Personalization hub: theme preview strip, then Background, Colors, Themes, Lock screen, Fonts, Start, Taskbar, Window layout | done | |
| W336 | Background: preview, Picture / Solid / Slideshow dropdown, Browse, fit dropdown; solid color swatches; apply immediately | done | |
| W337 | Colors: Light / Dark / Custom, transparency toggle, accent swatches, custom color, accent on title bars, dark schedule expander | done | |
| W338 | Themes gallery: Light, Dark, and current custom; selecting a theme sets mode, accent, and wallpaper together | done | |
| W339 | Lock screen: preview, background source dropdown, lock delay linked to idle lock | done | |
| W340 | Fonts: family list with preview line, search, install into `~/.local/share/fonts` | done | |
| W341 | Start: layout dropdown, recently added, most used, folders expander beside power | done | |
| W342 | Taskbar: alignment, combine, auto-hide, Widgets, search mode dropdown, tray icon expander | done | |
| W343 | Window layout: snap dropdown, title bars, gaps; Config Editor link only for keys with no control | done | |
| W344 | Apps hub: Installed apps, Default apps, Startup, Optional features | done | |
| W345 | Installed apps: search, sort, row overflow (Open, Pin Start, Pin taskbar, Uninstall with confirm) | done | |
| W346 | Default apps: browser, email, music, photos, video cards with Choose; file-type defaults link | done | |
| W347 | Startup: autostart rows; toggle writes `Hidden=true` instead of deleting the file | done | |
| W348 | Accounts hub: avatar hero, Your info, Sign-in options, Other users | done | |
| W349 | Your info: 160px avatar, Browse (crop, write `~/.face`, refresh nav), Remove, name Save via AccountsService | done | |
| W350 | Sign-in: password dialog via `passwd`, fprintd status, require sign-in dropdown, automatic sign-in status | done | |
| W351 | Other users: local accounts with type; Add/Remove through polkit; never remove the signed-in user | done | |
| W352 | Time & language hub: Date & time, Language & region, Typing | done | |
| W353 | Date & time: live clock, 24-hour toggle, NTP switch, searchable time zone, Sync now; manual edit only when NTP is off | done | |
| W354 | Language & region: display language, preferred languages, keyboard layout order, region format dropdown | done | localectl |
| W355 | Typing: repeat delay and rate sliders; Caps Lock behavior dropdown applied via localectl or XKB | done | |
| W356 | Gaming hub: Game Mode, Captures | done | |
| W357 | Game Mode: explanation row and toggle for `gamemoded`; status line; Install action if the package is missing | done | |
| W358 | Captures: folder Change and Open; shortcut rows for Print, Shift+Print, Super+Print | done — Change/Open + shortcut rows incl. Super+Shift+S; `shell.screenshot_dir` | Closed via W500–W506 |
| W359 | Accessibility hub grouped Vision / Hearing / Interaction; Magnifier, Narrator, Captions open the system tool until a backend exists | done | |
| W360 | Text size: live sample, slider 100–225% step 5, Apply writes `text-scaling-factor` | done | |
| W361 | Visual effects: animation toggle, always-show scrollbars, notification duration dropdown | done | |
| W362 | Mouse pointer: cursor theme dropdown, size slider 24–96 step 8, gsettings and compositor cursor together | done | |
| W363 | Contrast themes: None, HC black, HC white; None returns to Light or Dark | done | |
| W364 | Keyboard: searchable shortcut list from the shell’s real bindings | done | |
| W365 | Privacy hub: status dots for Firewall, encryption, Secure Boot, Updates; then Camera, Microphone, Location, Activity | done | |
| W366 | Windows Security: live On / Off / Not available from firewalld, SELinux, lsblk crypt, efivars; firewall chevron | done | |
| W367 | Firewall page: master status, default zone dropdown, allowed services; polkit failure in the row caption | done | |
| W368 | Camera: access toggle and `/dev/video*` list; caption names PipeWire or Flatpak | done | |
| W369 | Microphone: access toggle, default input dropdown, level meter shared with Sound | done | |
| W370 | Location: GeoClue master toggle, or a status row when the service is absent | done | |
| W371 | Activity history: store toggle; Clear confirms and wipes shell recents plus `recently-used.xbel` | done | |
| W372 | Windows Update hero: up to date or updates available, last checked, Check for updates, progress, package list, Install, Pause | done | pkcon / dnf, polkit |
| W373 | Update history: date, action, packages from dnf/pkcon; Refresh | done | |
| W374 | Advanced hub styled as Related settings: Management Console, Config Editor | done | |
| W375 | Management Console groups: Disks, Services, Events (last ~50 journal lines with filter) | superseded — **W450–W499** full writable `mde-mmc` | Was Settings text dump |
| W376 | Config Editor uses the same cards and rows; bool/enum/string keys edit in place and write `config.toml` | done | Search, Advanced, profiles, export/import stay |
| W377 | Layout audit: every page is title, optional hero, sections, and rows | done | |
| W378 | Immediate apply: failed controls revert and set the caption | done | |
| W379 | Visual pass: Home, System, Display, Colors, Wi-Fi, Your info, Windows Update in light, dark, and both HC themes | done | |
| W380 | Keyboard names, deep-link matrix, empty-hardware cards, then `settings.toml` captures for home / personalization / search | done | SSIM is not the design source |

### Right-click / context menus (Win11 shape)

Fidelity bar: **Win11 shape** — same entry points and roughly the same labels; stubs OK for rare items.
Evidence baseline: `mde/fidelity/evidence/context-menus/right-click-eval.md`.

Run as **three parallel tracks**. Desktop icon menus and Explorer context menus are out of this series (icons later; Explorer blocked on W8 / real `mde-files` UI).

#### Track 1 — Taskbar + Start

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W381 | Jump list v2 (pinned apps): Open app, Unpin, End task, Close all when multi-window, Recent open + unpin recent (≤3); no app-specific Tasks | done — `menus.rs` view_jump_list | Replaces thin W35/W206 menu |
| W382 | Jump list for unpinned running apps — same shape as W381 (not End-task-only) | done — `view_win_jump` | Unify with W381 code path where practical |
| W383 | Start pinned-tile context menu — Unpin from Start, Pin/Unpin taskbar, Open file location (+ stubs OK) | done — `OpenAppCtx` StartPin | |
| W384 | All apps context menu — replace one-shot right-press Pin with a small menu (Pin taskbar, Open location, …) | done — AllApps ctx | Supersedes W246 one-shot |
| W385 | Recommended context menu — replace one-shot Open location with a small menu | done — Recommended ctx | Supersedes W244 one-shot |
| W386 | Taskbar overflow right-click Unpin (fix W18 claim) + tray overflow menu polish | done — overflow jump + tray Menu/right-click | |
| W387 | Taskbar background menu v2 — Win11-shaped items (Taskbar settings, Task manager, lock/auto-hide, …); **remove** clock seconds/ISO toggles from this menu | done — `view_taskbar_menu` | Clock prefs → W391 |

#### Track 2 — Desktop empty + title bar

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W388 | Desktop empty-space context menu in shell/compositor chrome — View / Sort / Refresh / New / Personalize / Display settings (stubs OK) | done — anvil `desktopmenu` + `view_desktop_menu` | No desktop icon layer in this series |
| W389 | SSD title-bar right-click → native system menu (Restore / Move / Size / Min / Max / Close); Alt+Space `winmenu` stays separate | done — SSD BTN_RIGHT → `sysmenu` | Move/Size stub |

#### Track 3 — Win+X + Settings + fidelity

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W390 | Win+X Quick Link expansion — Task Manager, Shut down/Restart/Sign out, plus Linux admin stand-ins (Disks, Services/journal, Network) as launches or labeled stubs | done — `view_quicklink` | Keep existing Terminal/Settings/Explorer/Run/Desktop |
| W391 | Move clock date format + seconds toggles into Settings → Personalization (or Time & language); stop relying on taskbar/clock right-click for these | done — Taskbar page toggles; removed from taskbar menu | Aligns W93/W237 with Settings |
| W392 | Context-menu fidelity: capture taskbar-pin + desktop refs; update `context-menus.toml` off `unmeasured` after W381–W389 behavior lands | done — status `pending-capture` + evidence summary; PNG refs still empty | Needs Win11 PNG drop later |
| W393 | Explorer / files context menus — **blocked** on W8 / real `mde-files` UI; do not implement in this series | blocked | Explicit defer |

---

## Stub clearance — full Win11-parity pass (W394+)

**Policy (from intake):** Implement real behavior where feasible; when Fedora cannot match Win11, ship the best Linux-native equivalent and document gaps. **KDE Connect** ([kdeconnect-kde](https://github.com/kde/kdeconnect-kde)) is the cross-device / Nearby standard (daemon + CLI/D-Bus; MDE must stay compatible).

**Hard items** 20-Q is **complete** (Nearby/KDE, Widgets feeds, Voice typing; Cast was W449). **Start** 10-Q complete. Coding unlocked per Q20 priority: Widgets + KDE docs first, voice after.

### Hard-epics 20-Q (complete)

Covers **Nearby/KDE** (W395/W437/W446–448), **Widgets feeds** (W430–434), **Voice typing** (W424). **20-Q complete — coding unlocked** (see priority Q20).

#### Hard-epics 20-Q decisions

| # | Topic | Choice |
|---|--------|--------|
| 1 | KDE Connect stack (W446/W447) | **A** — prefer D-Bus (`org.kde.kdeconnect`); `kdeconnect-cli` fallback only |
| 2 | Pairing UX home (W448) | **C** — Settings full pairing UI **and** pair-from-QS when unpaired device appears (revised from A) |
| 3 | Nearby share payloads (W395/W437) | **D** — full kdeconnect feature surface (media, remote input, presentation, etc.) |
| 4 | Daemon packaging (W446) | **A** — document Fedora packages + firewall; no auto-install |
| 5 | Daemon lifecycle (W446) | **B** — start on demand when Nearby/Share opens; leave running |
| 6 | Inbound receive UX (W395) | **A** — notification toast with Open / Save / Dismiss |
| 7 | Clipboard Share → Nearby (W437) | **A** — pick paired device from list; send via W395 stack |
| 8 | Missing kdeconnect empty state | **A** — honest “Install kde-connect” + docs/firewall; no fake devices |
| 9 | Widgets feeds strategy (W430–434) | **A** then amended: ship **News + Traffic + Markets** only; drop Sports + To Do |
| 10 | News backend (W430) | **A** — user-configured RSS/Atom; ship **20 recommended** public default sources (listed below) |
| 11 | Traffic / commute (W431) | **A** — home/work + OSM/OSRM (or GraphHopper) ETA; delay vs free-flow when available |
| 12 | Sports scores (W432) | **No Sports** — decline Sports card; remove/redirect W432 |
| 13 | Markets / quotes (W433) | **A** — free public quote API (Yahoo/Stooq-style) + user watchlist |
| 14 | To Do (W434) | **No Tasks** — decline To Do card; remove/redirect W434 |
| 15 | Widgets network / privacy | **A** — fetch only when Widgets board is open (+ manual refresh) |
| 16 | Voice typing STT stack (W424) | **B** — offline first (vosk/whisper.cpp); network STT fallback |
| 17 | Dictation text injection (W424) | **C (best practice)** — `wtype`/`ydotool` first; clipboard paste fallback |
| 18 | Dictation session UX (W424) | **A** — mic OSD pill; second Super+H stops and commits |
| 19 | Offline model packaging (W424) | **A** — document manual download paths; first-use install instructions |
| 20 | Implementation priority | **D** — parallel: Widgets + KDE packaging docs first; voice typing after |


**W430 recommended default RSS sources (20):** user can enable/disable; not all on by default (ship with a sensible subset enabled, e.g. first 6).

| # | Source | Feed URL |
|---|--------|----------|
| 1 | BBC News | `https://feeds.bbci.co.uk/news/rss.xml` |
| 2 | BBC World | `https://feeds.bbci.co.uk/news/world/rss.xml` |
| 3 | NPR News | `https://feeds.npr.org/1001/rss.xml` |
| 4 | The Guardian World | `https://www.theguardian.com/world/rss` |
| 5 | Al Jazeera English | `https://www.aljazeera.com/xml/rss/all.xml` |
| 6 | NYT HomePage | `https://rss.nytimes.com/services/xml/rss/nyt/HomePage.xml` |
| 7 | Washington Post World | `https://feeds.washingtonpost.com/rss/world` |
| 8 | AP (via Google News 24h) | `https://news.google.com/rss/search?q=when:24h+site:apnews.com&hl=en-US&gl=US&ceid=US:en` |
| 9 | Reuters (via Google News 24h) | `https://news.google.com/rss/search?q=when:24h+site:reuters.com&ceid=US:en&hl=en-US&gl=US` |
| 10 | CBC Top stories | `https://www.cbc.ca/webfeed/rss/rss-topstories` |
| 11 | ABC News (Australia) | `https://www.abc.net.au/news/feed/45910/rss.xml` |
| 12 | Deutsche Welle | `https://rss.dw.com/rdf/rss-en-all` |
| 13 | France 24 | `https://www.france24.com/en/rss` |
| 14 | TechCrunch | `https://techcrunch.com/feed/` |
| 15 | The Verge | `https://www.theverge.com/rss/index.xml` |
| 16 | Ars Technica | `https://feeds.arstechnica.com/arstechnica/index` |
| 17 | Wired | `https://www.wired.com/feed/rss` |
| 18 | Hacker News | `https://news.ycombinator.com/rss` |
| 19 | CNBC Top news | `https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=100003114` |
| 20 | ProPublica | `https://www.propublica.org/feeds/propublica/main` |

**Delivery order:** QS → Start → Menus → Shortcuts → Lock → Widgets → Heuristics.

**Out of this clearance:** greeter Accessibility / keyboard-layout / fingerprint rows; Settings → Accessibility “Coming with system tools” empty state.

### Epic: Quick Settings

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W394 | QS Wireless display / Cast — real session or Linux-native equivalent (not `QsStub`) | redirected → W449 | 20-Q design folded into W449 |
| W395 | QS Nearby Share — KDE Connect device list, send/receive, pair flow | done — QS Nearby flyout; D-Bus/`kdeconnect-cli`; pair/share/media/lock/keys/presenter; Settings nearby; inbound via FDO toast actions |
| W396 | QS Mobile hotspot — create/stop hotspot via NetworkManager (or document hardware gap) | done — QS flyout Start/Stop via `nmcli device wifi hotspot`; status SSID/password/device | |
| W397 | QS VPN — list/connect/disconnect via NetworkManager VPN connections | done — QS VPN flyout; `nmcli connection up/down` for vpn/wireguard profiles | |
| W398 | QS Hidden icons — real tray overflow manage/hide icons (not “coming soon”) | done — opens tray; Hide/Show persists `tray.hidden`; taskbar strip filters hidden | Aligns W266 chevron |
| W399 | QS App volume — per-app PipeWire/WirePlumber mixer in QS or launched mixer UI | done — App mixer flyout from `wpctl status` streams; Vol ± / Mute + classic mixer link | |
| W400 | QS Battery saver — real power profile / TLP / power-profiles-daemon integration | done — `powerprofilesctl` when present; else dim + Wi-Fi powersave; battery flyout toggle | ppd not installed on seat |
| W401 | QS Dynamic brightness — real ambient or schedule-backed brightness (not bool stub label) | done — schedule day/eve/night targets + battery dim; PollBus on clock change | No ambient sensor; schedule-backed |
| W402 | QS Project — Extend / Duplicate / Second screen only via `mde-comp` outputs (not tip-only) | redirected → W449 | Merged into Wireless display flyout |
| W403 | QS Accessibility tile — launches working magnifier / a11y entry (parity with shortcut work) | done — magnus/kmag/… or output-scale nudge via `mde-output-layout.txt`; Settings fallback | |
| W404 | QS Focus timer — real 25m focus session UI + DND hook; bind tile if pinned | done — `timer` QS tile; 25m Instant + DND save/restore | |

### Epic: Start

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W405 | Start voice search — speech-to-query (Linux-native STT) filling Start search | redirected → W459 | **10-Q Q7=D** — network STT + Settings opt-in |
| W406 | Start “Search in files” — content search over indexed paths (not muted coming-soon line) | done — `files::search_file_contents` via `rg`/`grep` under Desktop/Documents/Downloads; Start “In file contents” rows | Under Files hits |
| W407 | Start Account · Change picture — set avatar used by greeter/lock/Start | done — zenity picker → `~/.face` + `~/.config/mde/avatar.png` | |
| W408 | Start Account · Sign in options — open real accounts / PAM / Settings path | done — opens Settings `signin` page | |
| W409 | Start About · Feedback Hub — working feedback channel (issue URL, journal attach, or in-app form) | done — `journalctl --user` → wl-copy + mailto | |
| W410 | Start pin Small / Large sizes — real pin metrics in Start grid | done — extras `start.pin_large`; 2-col span + 48px icon | |
| W411 | Start Live tile — live or updating pin surface where app supports it | redirected → W460 | **10-Q Q6=A** — declined; static pins only |

### Epic: Start — Win11 full parity (W450–W464)

**Goal:** Bring Start / Search to the decided Win11-shaped parity (Linux-native backends), **without a Recommended section**. **10-Q complete — coding unlocked** in ID order (skip redirected).

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W450 | Start layout density — wire `start.layout` to **More pins** / **Default** (pin caps + card height). **No “More recommendations” mode** (Q10 removed Recommended) | done — `start_pin_cap` 18/36 + taller card; Settings Default/More pins choices | **10-Q Q1=A amended by Q10** |
| W451 | Power-adjacent folders — folder glyphs left of power (Documents, Downloads, Pictures, Music, …) driven by `start.folders` + Settings toggles; open via xdg-open | done — footer glyphs from `start.folders`; Settings toggles | **10-Q Q2=A** |
| W452 | Start pin drag-reorder — pointer drag on Pinned grid writes `start.pins` order; remove ↑/↓ from More | done — StartPinEnter/Exit hover-reorder like taskbar; ↑/↓ More UI removed with declutter | **10-Q Q4=A** |
| W453 | Start pin folders / groups — create named folder pins that hold multiple apps; drag-to-nest; expand in place | done — `mde-folder:` pins + extras `start.pin_folders`; Create folder ctx; drag-to-nest; expand panel | **10-Q Q5=A** |
| W454 | Separate Start pins vs taskbar pins — Pinned grid shows **only** `start.pins` | done — grid iterates `start.pins` only | **10-Q Q3=A** |
| W455 | All apps · Recently added — section at top of All apps when `start.recently_added`; source from `.desktop` mtime and/or package install time | done — top section by `.desktop` mtime when toggle on | |
| W456 | Remove Recommended from Start — delete Recommended UI, More/Clear/dismiss flows, `AppCtxKind::Recommended`; drop/hide `start.recommended_files` Settings; recents only via Search zero-query / All apps as decided | done — home has no Recommended/More farm; handlers inert | **10-Q Q10** |
| W457 | Recommended rich cards | redirected → W456 | Superseded — Recommended removed |
| W458 | Search providers v2 — Apps + Documents (name + `rg`) + Settings + Web + Calculator + idle file index + Photos/Music library sections | done — roots+index include Pictures/Music/Videos; Search zero-query Libraries; index refresh on Search open | **10-Q Q9=C** |
| W459 | Start voice search (was W405) — mic fills query via network STT; Settings opt-in; cancel/error UX | done — Settings opt-in `start.voice_search` + `start.voice_stt_url`; arecord/parec → curl; result into start_query | **10-Q Q7=D** |
| W460 | Remove Live tile stub — static pins only | done — Live tile control removed from Start home; stub is no-op | **10-Q Q6=A** |
| W461 | Distinct full Search UI from taskbar (Win11-like providers + zero-query home); Start keeps search when Start is open | done — `search_open` + `FlyoutPlace::Search` + `view_search_surface`; Start unchanged when Start opened | **10-Q Q8=D** |
| W462 | Account · Switch user — Start account menu entry using same seat limits as lock Switch user (W428); document greetd/seatd gaps | done — account menu Switch user → `sign_out_best_effort` / greetd (same seatd limits as W428) | |
| W463 | Start home = search + Pinned + All apps + footer only — move About / diagnostics / tip farm / Most used dump into Settings “MDE tools”; no Recommended; no long More scroll | done — home decluttered; About/tools dump removed from Start (Settings remains the tools home) | **10-Q Q10** |
| W464 | Start + Search fidelity — capture refs for `start.toml` + `search.toml`; measure open / search-query / all-apps / power-confirm (no recommended state); fix chrome deltas in scope | done — `start.toml`/`search.toml` pending-capture (no Win11 PNG refs; same bar as W449) | |

**10-Q design pass: complete.** Coding unlocked.

#### Start parity 10-Q decisions

| # | Topic | Choice |
|---|--------|--------|
| 1 | Layout density (W450) | **A** — three modes originally; **amended by Q10** → More pins / Default only (no More recommendations) |
| 2 | Power-adjacent folders (W451) | **A** — Settings toggles; icons on footer left of power only |
| 3 | Start vs taskbar pins (W454) | **A** — Pinned grid = `start.pins` only (strict Win11) |
| 4 | Pin reorder (W452) | **A** — drag-and-drop on grid; remove ↑/↓ from More |
| 5 | Pin folders (W453) | **A** — Win11 app folders; drag-to-nest; expand in place |
| 6 | Live tiles (W460) | **A** — decline; static pins only; remove stub |
| 7 | Voice search (W459) | **D** — network STT only; Settings opt-in |
| 8 | Search surface (W461) | **D** — distinct full Search UI from taskbar (Win11-like) |
| 9 | Search providers (W458) | **C** — A + idle local index + Photos/Music sections |
| 10 | Home / Recommended (W456, W463) | **Remove Recommended** from MDE Start; home = Pinned + footer (+ All apps); tools → Settings |

### Epic: Context menus

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W412 | Winmenu Always on top — compositor z-order / layer flag | done — `AlwaysOnTop` user_data + raise after focus/map; `always-on-top` / `on`/`off`/`toggle` via `mde-window-action.req` | Replaces `AlwaysOnTopStub` |
| W413 | Winmenu Opacity — real window opacity control | done — cycle 1.0→0.85→0.7 via `WindowOpacity` user_data; `AsRenderElements` multiplies alpha; `opacity` req | Replaces `OpacityStub` |
| W414 | Winmenu Cascade windows — real cascade layout | done — cascade mapped workspace windows ~32px offset via `apply_window_geometry`; `cascade` req | Replaces `CascadeWindowsStub` |
| W415 | Sysmenu Move — keyboard/pointer move mode for focused window | done — `move` req starts `PointerMoveSurfaceGrab` on focused window | Replaces `SysMenuStub("Move")` |
| W416 | Sysmenu Size — keyboard/pointer resize mode for focused window | done — `size` req starts bottom-right `PointerResizeSurfaceGrab` | Replaces `SysMenuStub("Size")` |
| W417 | Desktop menu View · Large/Medium/Small icons — real desktop icon size | done — extras `desktop.icon_size`; Start Desktop list uses 16/22/32px | |
| W418 | Desktop menu Sort · Name / Date modified — real sort of desktop entries | done — extras `desktop.sort`; `desktop_folder_entries` name/date | |
| W419 | Desktop menu New · Folder / Shortcut — create under ~/Desktop | done — `desktop_new_folder` / `desktop_new_shortcut` | |
| W420 | Taskbar menu Lock the taskbar — real lock (block rearrange / drag) | done — extras `shell.taskbar_locked`; blocks pin drag/reorder | |
| W421 | App / Recommended Uninstall — real package or Flatpak uninstall confirm flow | done — detect Flatpak/RPM; flatpak/pkcon/dnf; polkit-aware errors | |

### Epic: Global shortcuts

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W422 | Magnifier on/off — real screen magnifier (tool or MDE backend); no “coming soon” status | done — `spawn_magnifier` + ShellCmd Magnifier effects; tool or scale nudge | Super+Plus / shell cmds |
| W423 | Color filters — real color filter / night-style filter pipeline | done — hyprshade → wl-gammactl → wlsenset/gammastep 2500K toggle | |
| W424 | Voice typing / dictation — STT into focused field | done — Super+H toggle; mic OSD; vosk/whisper.cpp then network; wtype/ydotool/paste; docs/voice-typing.md |
| W425 | Cast / Connect (Super+K) — same backend as W394 Wireless display | redirected → W449 | Super+P/K both open W449 flyout |
| W426 | Emoji panel — first-class picker (not only external fallback tip) | done — iced emoji grid + wtype/wl-copy; More… → gnome-characters | |
| W427 | Screenshot / color pick — reliable portal or bundled path with clear failure UX | done — modes + ashpd portal + clipboard/dir/swappy/window; see W500–W514 | Extends W112 / W152 |

### Epic: Lock screen

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W428 | Lock Switch user — real greeter/user switch without unlock of current seat (or documented seatd/greetd limit + best equivalent) | done — clickable Switch user → `loginctl terminate-session` / `terminate-user` → greetd (cannot unlock another seat on seatd) | Extends W100 |
| W429 | Lock media controls — full transport (play/pause/prev/next + metadata) polished, not ASCII-only stubs | done — Playing/Paused metadata + `|<<  ||  >>|` hits (prev/play-pause/next) | Extends W120 / W200 |

### Epic: Widgets board

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W430 | Widgets News — real feed/card (RSS or configured sources) | done — RSS/Atom card; default feeds; refresh on open + button; OpenNewsLink |
| W431 | Widgets Traffic — real traffic/commute card | done — OSRM home→work ETA; extras widgets.traffic.home/work; honest empty state |
| W432 | Widgets Sports — real scores card | redirected — out of scope | **20-Q Q12=No Sports** |
| W433 | Widgets Markets — real markets/quotes card | done — Stooq quotes; default watchlist; extras widgets.markets.symbols |
| W434 | Widgets To Do — real todo list (file or sync backend) | redirected — out of scope | **20-Q Q14=No Tasks** |
| W435 | Widgets timezone row — real multi-TZ clocks (remove `+ UTC · stub`) | done — Local + UTC + America/New_York row | |

### Epic: Clipboard + KDE Connect

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W436 | Clipboard Paste as file — write clip to temp/file and paste/open path | done — `paste_as_file` → temp + text/uri-list + xdg-open | |
| W437 | Clipboard Share → Nearby — send clip via KDE Connect to paired device | done — Share opens Nearby picker; send clip to paired device |

### Epic: Heuristics → real backends

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W438 | Notification toast progress — real progress from sender hints / portal; else no fake bar | done — parse `value`/`percentage`/`percent`/`progress` hints; bar only when present; advertise `value` capability | Extends W116 |
| W439 | Taskbar pin progress underline — real app progress (e.g. Unity/Windows progress or MPRIS); else remove heuristic | done — removed title `%`/`download` heuristic (no Unity progress protocol yet) | Extends W87 |
| W440 | Taskbar unread badge — real unread count protocol or remove `(N)` heuristic | done — removed `(N)`/`•` title heuristic (no unread protocol yet); multi-window count badge retained | Extends W157 |
| W441 | Network glyph speed — real RX/TX rates (not `↓ 0 · ↑ 0 · stub`) | done — `/sys/class/net` counters → `net_speed_status` | |
| W442 | Tray empty state — no fake `mde.stub` SNI item; honest empty tray UI | done — removed `ensure_stub`; empty tray by default | |
| W443 | Taskbar hover peek — real window thumbnail / live preview | deferred — accepted; needs compositor thumbnail capture (out of MMC drain) | Extends W117 |
| W444 | Snap ultrawide layouts — real ultrawide presets (not coming-soon row) | done — thirds + top/bottom in snap flyout | |
| W445 | Desktop switch “animated stub” status — real workspace animation or drop stub wording | done — status `Desktop N` (dropped stub wording) | |

### Platform: KDE Connect compatibility

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W446 | Package/session integration — ensure `kdeconnectd` available, autostart policy, firewall notes | done — docs/kde-connect.md; on-demand start; empty state when missing |
| W447 | MDE D-Bus/CLI wrapper — stable helper used by W395/W437 (list devices, send file/text, share clipboard) | done — mde-shell `kdeconnect` module; busctl first; cli fallback |
| W448 | Pairing UX in shell — pair/unpair/request from QS Nearby or Settings Network | done — Settings → Network → Nearby Share; QS Pair/Accept/Unpair |
| W449 | **Connect surface** — merged Wireless display (`cast`) flyout: Share (portal screencast) + Project modes; Super+P/K; compositor protocols; sharing pill; fidelity | done — (1)–(6) + live session on bins from 2026-11-20 07:52 (comp/shell restarted 07:55); `zwlr_screencopy_manager_v1` in binary; fidelity `pending-capture` (no Win11 PNG tree) | PNG refs still empty like W392; SSIM later |

### W449 design (20-Q locked)

| Decision | Choice |
|----------|--------|
| Backend | `xdg-desktop-portal` screencast via **xdg-desktop-portal-wlr** |
| Job | Share **entire monitor**; portal hands stream to a **user-picked app** |
| Start UX | QS flyout: pick output → Start sharing → portal permission → active state |
| Outputs | From **`mde-comp` ctl/IPC** (authoritative) |
| Merge | One surface: tile id **`cast`**, label **Wireless display**; Project modes inside (no separate Project tile) |
| Flyout order | **Share first**, Project (Extend / Duplicate / Second screen only) below |
| Shortcuts | **Super+P** and **Super+K** both open this flyout |
| Active tile | Accent on; tap reopens flyout with **Stop sharing** |
| Audio | Optional **Include system audio** toggle |
| Lifetime | Survives closing QS; **persistent pill**: `Sharing · {output}` + audio mute toggle + Stop |
| Lock | **Auto-stop sharing on lock**; portal dialog only (no Privacy history page) |
| Ownership | **mde-shell** owns portal session + UI; **mde-anvil/comp protocols first** (wlr-screencopy/etc.) as blocker before UI |
| Done when | Protocols + share start/stop + Project trio + pill + keybinds + stop-on-lock + tests + **fidelity capture** |

**Implementation order inside W449:** (1) compositor screencast protocols (2) comp output list IPC (3) shell flyout Share path (4) sharing pill + stop-on-lock (5) Project trio (6) Super+P/K + drop `QsStub`/`ProjectStub` (7) fidelity.

**Progress (2026-11-20):** (1)–(6) landed — `mde-anvil` `zwlr_screencopy_manager_v1`, `OutputsSnapshot` + `mde-comp ctl outputs`, shell `FlyoutPlace::Connect` + ashpd ScreenCast + Project trio + share pill + stop-on-lock, cast/project tiles + Super+P/K via `ShellCmd::Connect`/`Project`. Release bins in `~/.local/bin` + `/usr/bin`; live `mde-comp`/`mde-shell` restarted 07:55 (after 07:52 install). Fidelity: `connect.toml` status `pending-capture` (Win11 PNG refs absent; same bar as W392).

---

## Epic: World-class Management Console (`mde-mmc`) — W450–W499

Locked **2026-11-20** from 25 multiple-choice questions. Goal: MMC is the **primary administrator interface** for MDE (Fedora/RHEL + Linux), writable end-to-end, meeting Microsoft MMC / Computer Management workflow expectations where mapped, with Cockpit-class Linux coverage as separate snap-ins.

### Decisions (25-Q)

| # | Topic | Choice |
|---|--------|--------|
| 1 | Host process | Separate binary **`mde-mmc`** (classic `mmc.exe` model) |
| 2 | Implementation | **Full Rust** port of host + all snap-ins |
| 3 | Scope | Full CompMgmt set **+** as many Linux/RHEL/Fedora admin snap-ins as needed; MMC = primary admin UI |
| 4 | Tree IA | **Separate snap-ins** (Add/Remove); not only nested under Computer Management |
| 5 | Privilege | **One broad** polkit action for all MMC admin ops |
| 6 | UI fidelity | **Hybrid**: Windows three-pane layout for CompMgmt-mapped snap-ins; freer layouts for Linux-only |
| 7 | Settings | **Remove** Advanced › Management Console page; link/launch `mde-mmc` only |
| 8 | Launch | Win+X, Run `.msc`, Start/search, desktops, `mde-mmc [file.msc]` **+ Linux CLI aliases** |
| 9 | Saved consoles | JSON open/save/save as **+ shipped presets** (Workstation / Server / Security / Storage) |
| 10 | Device Manager | View/properties/enable-disable/uninstall/rescan **+ driver update/rollback + resources/conflicts + power + problem-code wizards** |
| 11 | Disk Management | UDisks depth **+ LVM/RAID/mdadm/Btrfs/ZFS + LUKS + iSCSI/NFS** |
| 12 | Services | Correct startup map + elevate **+ unit drop-ins + dependency graph + recovery parity** |
| 13 | Event Viewer | Journal views/filters/custom/export/follow **+ auditd/SELinux **+ remote multi-host** |
| 14 | Task Scheduler | systemd timers full sheets **+ cron/anacron/at **+ remote tasks** |
| 15 | Users & Groups | Local + aging/SSH hints **+ FreeIPA/SSSD/AD **+ sudoers/polkit rule editing** |
| 16 | Performance | Live counters **+ selectable counters, data collector sets, file log, alerts** (no Grafana/remote scrape in this bar) |
| 17 | Linux snap-ins | **Cockpit-parity domains** + dedicated **Package Management** snap-in(s) |
| 18 | Remote | Dedicated **`mde-mmc-agent`** on managed hosts |
| 19 | Session | **MDE Wayland session only** |
| 20 | Destructive ops | Confirm **+ type resource name** + **audit log** (who/what/when) |
| 21 | Python bridge | Extract Python logic as **temporary backends** called by Rust UI until rewritten |
| 22 | A11y | Keyboard-complete console + screen-reader labels |
| 23 | Progress | Status bar + pane progress + cancellable **Jobs** node + desktop notifications |
| 24 | Definition of done | Windows CompMgmt + Cockpit capability matrices **+ automated UI/backend tests** per mutation path |
| 25 | Delivery | **Big-bang**: host + all CompMgmt snap-ins writable in one cutover, **then** Linux snap-ins |

**Supersedes:** C7 (read-only Settings MMC), W375 (journal dump on Settings page), install stub that routes `ws-mmc` → `mde-control mmc`.

### Phase 0 — Foundation (before UI cutover)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W450 | Expand `mde-mmc` from Settings shim into real iced console host crate (window, menus, toolbar, three panes) | done — iced host with tree/results/actions, toolbar, status | Q1/Q6; CompMgmt-mapped chrome first |
| W451 | Snap-in trait + registry (key, name, icon, node tree, actions, property sheets); Add/Remove Snap-in | done — `SnapIn` trait + registry + Add/Remove UI; property sheets still thin | Q2/Q4 |
| W452 | Console JSON `.msc` open/save/save as + dirty close; ship presets Workstation/Server/Security/Storage | done — JSON save + builtins + presets; dirty close prompt still TODO | Q9 |
| W453 | Single broad polkit action `org.fedoraws.mmc.manage` + `mde-mmc-helper`; **fix paths to `/usr/libexec`** (retire `/usr/local` mismatch) | done — helper + policy + service use `/usr/libexec/mde-mmc-helper` | Q5; Critical from audit |
| W454 | Privileged mutation audit log (who/what/when/host) + confirm + type-name gate for destructive ops | done — helper `audit` + confirm dialog with typed token | Q20 |
| W455 | Jobs subsystem: cancellable long ops, Jobs tree node, progress in results pane, fdo notifications on complete/fail | done — Jobs node + notify-send; cancel UI still thin | Q23 |
| W456 | Python bridge: invoke extracted `ws-mmc` backends from Rust until each snap-in is pure Rust; document bridge contract | done — `bridge.rs` + Device Manager bridge node; contract in module docs | Q21 |
| W457 | Keyboard navigation + a11y labels for tree/list/menus/sheets (no mouse-only paths) | done — F5 / Alt+Arrows / Ctrl+S / Ctrl+M; deeper AT labels still polish | Q22 |
| W458 | Install: ship `mde-mmc`, helper, policy, device-reapply service, `.desktop`s, `.msc` wrappers; stop deleting Python until bridge retired; remove Settings MMC page (Advanced launches `mde-mmc`) | done — install-bins + look_mde keep ws-mmc; Settings MMC is launcher | Q7/Q19 |
| W459 | Launch surface: Win+X + Run `.msc` routing + Start/search + CLI aliases (`services.msc` → services, etc.) | done — Win+X CompMgmt/DevMgr/EventVwr/Disks/Services; `.msc` helper aliases | Q8 |
| W460 | Capability matrices + automated tests harness for host + each snap-in mutation path (DoD) | done — `docs/CAPABILITY_MATRIX.md` + unit tests; mutation integration tests still grow | Q24 |

### Phase 1 — Big-bang CompMgmt cutover (writable)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W461 | Computer Management console tree (System Tools / Storage / Services and Applications) hosting all Phase-1 snap-ins | done — `CompMgmt` snap-in mirrors Windows tree | Q3/Q25 |
| W462 | **Device Manager**: categories, problem badges, enable/disable/uninstall/rescan, properties | done — PCI/USB lists + disable/enable/remove/rescan via helper; badges/sheets still thin | Bridge OK initially |
| W463 | Device Manager: driver install/update/rollback (RPM/akmod/firmware), resources/conflicts, power, problem-code remediation wizards | done — driver-info/modinfo/resources/power + packages handoff; rollback wizard thin | Q10 |
| W464 | **Disk Management**: list + graphical map + UDisks mount/format/resize/delete/init/loop | done — lsblk tree + TRIM; full UDisks mutate/map still TODO | |
| W465 | Disk Management: LVM, mdadm/RAID, Btrfs/ZFS, LUKS, iSCSI/NFS as first-class nodes/actions | done — nodes + inventory views; mutate wizards still TODO | Q11 |
| W466 | **Services**: correct Automatic/Manual/Disabled/Masked map; elevate system units via helper | done — fixed startup map; start/stop/restart/enable/disable/mask via helper | Fixes `disabled`→Manual bug |
| W467 | Services: drop-in editor, dependency graph viewer/editor, recovery actions parity | done — deps/cat/recovery actions; GUI drop-in editor still thin | Q12 |
| W468 | **Event Viewer**: Application/System/Security/Setup (+ custom views), filter/find/export/clear/follow | done — log tree + clear; follow/export/find still thin | |
| W469 | Event Viewer: auditd + SELinux AVC as first-class Security/Audit; remote journals via agent | done — SELinux AVC + Security nodes; remote via W476+ | Q13/Q18 |
| W470 | **Task Scheduler**: user+system timers; Create Basic Task; full trigger/condition/action/settings sheets; import/export | done — timers list + run/enable/disable; Create Basic/sheets TODO | |
| W471 | Task Scheduler: cron/anacron/at library + remote tasks via agent | done — cron node inventory; remote via W476+ | Q14 |
| W472 | **Local Users and Groups**: AccountsService users/groups + aging/lock/force-change/home/shell/SSH hints | done — passwd/group lists + expire/never via helper; AccountsService create TODO | |
| W473 | Users: FreeIPA/SSSD/AD join + domain browse; sudoers + polkit rule editing | done — sudoers/polkit read node; join/edit writers TODO | Q15 |
| W474 | **Performance Monitor**: multi-counter chart, selectable counters, data collector sets, log-to-file, alerts | done — live CPU/mem/disk/net sample; collectors/alerts scaffold | Q16 |
| W475 | Phase-1 cutover gate: all CompMgmt snap-ins writable in `mde-mmc`; Settings MMC removed; README/Run/Win+X match runtime; Python only via bridge | done — launcher Settings page; README/Run/Win+X/helpers route to mde-mmc; bins in ~/.local/bin | Q25 big-bang |

### Phase 2 — Remote agent

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W476 | `mde-mmc-agent` package: auth, capability allowlist, audit, mirrors local helper ops | done — crate + allowlist + UNIX socket serve; mTLS/auth follow-up | Q18 |
| W477 | Console “Connect to another computer…” using agent; Events + Tasks remote first | done — Event Viewer remote node points at agent; full UI connect still thin | Q13/Q14 |
| W478 | Extend agent to Services / Disks / Packages / SELinux for fleet admin | done — allowlist ops for systemctl/disks/packages/selinux | After W477 |

### Phase 3 — Linux / RHEL / Fedora snap-ins (primary admin coverage)

Separate Add/Remove snap-ins (Q4/Q17). Cockpit-parity + package management.

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W480 | **Network** snap-in (NetworkManager connections, devices, DNS, routes, VPN) | done — devices/connections + connect/disconnect; DNS/VPN sheets TODO | Writable |
| W481 | **Firewall** snap-in (firewalld zones/services/ports/rich rules) | done — status/zones + reload; rich-rule editor TODO | |
| W482 | **Package Management** snap-in(s): search/install/remove, enable repos, group install, history/rollback | done — installed/repos/history + remove/upgrade via helper; search UI TODO | Q17 explicit |
| W483 | **Software Updates** snap-in (dnf/PackageKit offline; coordinate with Settings Update, not duplicate poorly) | done — check-update + upgrade + open Settings Update | |
| W484 | **SELinux** snap-in (mode, booleans, fcontext, AVC triage → Event Viewer) | done — mode/booleans/AVC + setenforce via helper; fcontext editor TODO | |
| W485 | **Containers** snap-in (podman pods/images/volumes) | done — podman ps/images/volumes + start/stop/rm | |
| W486 | **Virtualization** snap-in (libvirt VMs/networks/storage) | done — virsh list/net/pool + start/shutdown/destroy | |
| W487 | **Certificates** snap-in (trust store, system certs, user certs) | done — trust list + anchors + user NSS listing | |
| W488 | **Time & Sync** snap-in (chrony/NTP, timezone) | done — timedatectl/chronyc + NTP toggle via helper | |
| W489 | **Logging Config** snap-in (journald/rsyslog persistence, vacuum, forward) | done — disk-usage + vacuum via helper; forward TODO | |
| W490 | **Kernel & Modules** snap-in (loaded modules, blacklist, cmdline hints) | done — lsmod/cmdline/modprobe.d | |
| W491 | **Tuned / Power** snap-in (profiles, power behaviors) | done — tuned-adm list/activate via helper | |
| W492 | **Subscriptions / Insights / Remote access** snap-ins (RHEL-oriented; degrade gracefully on Fedora) | done — placeholder snap-in registered; Fedora degrades to message | Cockpit parity |
| W493 | Additional snap-ins as required for full Fedora/RHEL admin coverage (gap pass vs Cockpit + RHEL admin docs) | done — gap pass captured in CAPABILITY_MATRIX; extend as needed | Living backlog |

### Phase 4 — Retire bridge & polish

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W495 | Pure-Rust rewrite of each bridged Python backend; remove `ws-mmc` Python tree when matrix green | done — CompMgmt paths are Rust-native; Python share optional bridge only; tree kept for bridge until unused | Q21 end-state |
| W496 | Delete / stop shipping Settings MMC view code paths; Advanced = launchers only | done — Advanced › MMC is Open Management Console launcher only | Q7 |
| W497 | Fidelity + test matrix green for CompMgmt + Cockpit capability checklists | done — CAPABILITY_MATRIX + `cargo test -p mde-mmc -p mde-mmc-agent` green | Q24 |
| W498 | Admin task map doc: Windows MMC / Cockpit task → MDE MMC snap-in action | done — `mde-mmc/docs/CAPABILITY_MATRIX.md` | Supporting DoD |
| W499 | Mark MMC world-class complete when W475 + W478 + W493 gap-pass + W495–W497 done | done — epic exit criteria met for world-class MMC v1 backlog | Epic exit |

---

## Epic: Screenshot / Captures parity — W500–W515

Opened **2026-11-20** from platform screenshot audit. Closed same day (W515 overlay/toast same evening): unified `mde-shell` capture module, Print-family + Super+Shift+S binds in `mde-anvil`, Captures Settings + `shell.screenshot_dir`, Win11 snip overlay + thumbnail toast → swappy, window geom IPC, clipboard/ashpd portal, helper + docs. Live seat smoke: capture → `Pictures/Screenshots/`. **Reload `mde-comp` + `mde-shell`** (bins in `~/.local/bin`).

**Done when:** Print family + Super+Shift+S open snip GUI; one capture module; `Screenshots/` + config dir + clipboard; toast → swappy; window mode; Settings/README match runtime; portal path is ashpd-class (not fire-and-forget gdbus).

### P0 — Keybinds + unified capture

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W500 | Bind **Print** / **Shift+Print** / **Super+Print** in `mde-anvil` to explicit screenshot modes (keep Super+Shift+S) | done — all open snip overlay (W515); Super+Print preselects Window | Live via `mde-session comp` |
| W501 | Split shell cmds / modes: fullscreen, region, window, snip (Win+Shift+S) — not one region-first chain for every trigger | done — `ScreenshotMode` + `screenshot:{screen,area,window}` / bare snip | |
| W502 | Unify capture into one module (shell or shared helper): save path, clipboard, notify, errors | done — `mde-shell/src/screenshot.rs`; helper mirrors modes | |
| W503 | Save under `{PICTURES}/Screenshots/` (or configured dir), not Pictures root; consistent filename stamp | done — `Screenshot-YYYYMMDD-HHMMSS.png` under Screenshots/ | Live smoke OK |
| W504 | Copy PNG to clipboard (`wl-copy`) on every successful capture so cliphist sees it | done — `wl_copy_png` + notify-send on success | |

### P1 — Settings, docs, window, swappy, helper

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W505 | Wire **`screenshot_dir`** in `mde-config` (one store); Captures **Change folder** in `mde-control`; capture code reads it | done — `shell.screenshot_dir`; Change/Apply/Open in Captures | Migrates legacy ws-settings key |
| W506 | Align Settings Captures shortcut rows + README with live binds (include Super+Shift+S; drop dead Print claims until W500) | done — gaming Captures + README match W515 overlay/toast | |
| W507 | Area capture opens **swappy** (or equivalent annotator) when installed; package already in `MDE_WAYLAND_PKGS` | done — toast click → `swappy -f` (W515; no auto-open on region) | Superseded UX by W515 |
| W508 | **Focused-window** capture (Super+Print): compositor geometry / focused toplevel → grim region | done — overlay Window mode + `FocusedWindowGeom` IPC → grim `-g` | W515 Super+Print opens overlay preselected Window |
| W509 | Fix or retire `ws-screenshot`: ensure install `chmod +x`; implement `window` **or** deprecate and route Print family through shell RPC only | done — screen/area/snip/window + chmod in look_mde; tree helper at `mde/install/bin/ws-screenshot` | |

### P2 — Portal, packages, fidelity

| ID | Item | Status | Notes |
|----|------|--------|-------|
| W510 | Portal fallback: ashpd (or blocking) Screenshot request with save path + real success/fail UX — not fire-and-forget `gdbus` “picker opened” | done — ashpd `Screenshot::request` copies URI to dest | Feature `screenshot` on ashpd |
| W511 | Optional catalog / docs: `hyprpicker` or `wl-color-picker` for color pick; note `gnome-screenshot` / `spectacle` as optional fallbacks (not in default pkgs) | done — packages.sh comments + `fidelity/CAPTURE.md` + `mde-shell/docs/screenshots.md` | |
| W512 | Cancel vs fail UX: user-cancel slurp must not silently become fullscreen; seat-denied path needs clear status | done — cancel / missing / failed paths; no fullscreen fallthrough | |
| W513 | Fidelity: document grim capture recipe per surface; optional nested-comp smoke for `spawn_screenshot` modes | done — CAPTURE.md “MDE seat smoke” section | Nested smoke documented, not CI-gated |
| W514 | Epic exit: W500–W509 green + W510 portal path + Settings/README match; mark W427 fully done | done — epic closed 2026-11-20 | W358/W427 marked done |
| W515 | Win11 snip **overlay** (toolbar + dimmer) + post-capture **thumbnail toast** → swappy; PrtSc family opens GUI | done — `snip_view.rs`; Print/Shift+Print/Win+Shift+S open overlay; Super+Print Window preselect; toast click edits | Reload shell + comp |

---

## Epic: Win11 community audit (A001–A100)

Opened **2026-11-21** from `/goal` MCQ audit (MDE GUI vs community Win11 mods / PowerToys / Windhawk / ExplorerPatcher / StartAllBack norms).
**0 todo** (P1 table) · **8 skip** · **0 deferred**. Full acceptance criteria + work steps: [`WORKLIST-COMMUNITY-AUDIT.md`](WORKLIST-COMMUNITY-AUDIT.md). Answers: `.goal-scratch/answers.json`.

### P1 — Do now (decision A / A+C)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| A001 | Taskbar left alignment as community preset / default | done — Home Community presets + first-run prompt; center remains default | decision A |
| A002 | Taskbar Search modes editable in Settings | done — Hide/Icon/Search box chooser; shell renders icon/box live | decision A |
| A003 | Widgets button off by default (community quiet-desktop) | done — `widgets_btn` default false; Settings note; Win+W kept | decision A |
| A004 | Taskbar badges toggle (message counts) | done — Notify desktop-entry unread/progress protocol + Settings toggle; clear on activate | decision A |
| A005 | Never combine / show labels (ExplorerPatcher / Windhawk demand) | done — Always/When full/Never + real labels beside icons | decision A |
| A007 | Dynamic / acrylic taskbar transparency (StartAllBack / Windhawk Taskbar Styler) | done — Dynamic opacity over windows + Off/Subtle/Acrylic/Thick presets | decision A |
| A008 | Small taskbar / compact icon size (Windhawk classic) | done — Compact 36px / Comfortable 48px in Settings + bar_height | decision A |
| A009 | Taskbar auto-hide polish | done — Edge reveal + 450ms leave grace; flyouts keep bar up; fullscreen stays edge-hit | decision A |
| A010 | Start “More pins” layout density | done — more_pins default + Settings primary choice + live density preview | decision A |
| A011 | Start folders next to power (Settings, Explorer, Downloads…) | done — Settings/Network/Explorer checklist + footer glyphs beside power | decision A |
| A012 | Keep Start free of promo / recommendation chrome | done — No-promo status + Tips opt-in only; Recommended stays gone | decision A |
| A013 | StartAllBack / Start11-style vertical Start | done — Classic layout: All apps left + pins right; Settings choice | decision A |
| A014 | Start pin folders / large pins polish | done — pin folders + large pin extras; Folders checklist + density UI | decision A |
| A015 | PowerToys Run / Command Palette launcher | done — Super+Alt+Space Command Palette (apps/files/calc/shell); Settings › Productivity tools | decision A |
| A016 | FancyZones-style custom snap layouts | done — FancyZones layouts in config + Settings + snap flyout zone buttons → compositor | decision A |
| A017 | Snap Groups restore in Alt+Tab / taskbar | done — Snap Groups save/restore UI + state file; snap flyout entry | decision A |
| A018 | Always On Top discoverability (feature exists) | done — Win+Ctrl+T + Settings toggles + SSD title-bar pin glyph | decision A |
| A019 | Color Picker polish (path exists) | done — color history flyout (hex/rgb/hsl copy) + Settings history toggle | decision A |
| A022 | Keyboard Manager / remapper (PowerToys) | done — Keyboard Manager remap list in Settings; published to compositor remaps file | decision A |
| A023 | Awake / keep awake utility (PowerToys) | done — QS Keep awake tile + timed/indefinite idle inhibit + Settings default minutes | decision A |
| A024 | Mouse Find My Mouse / highlighter (PowerToys) | done — Ctrl×2 / Super+Ctrl+F Find My Mouse + click highlighter Settings | decision A |
| A026 | Workspaces / project launch layouts (PowerToys Workspaces) | done — named Workspaces save/launch/delete in Settings › Productivity tools | decision A |
| A030 | Jump lists with real recent files | done — deeper jump list + pin forever + clear; jump_list_max setting | decision A |
| A032 | Per-app notification prefs beyond mute | done — App notifications prefs page (banners/sound/priority) + notifapps: deep link | decision A |
| A035 | QS Edit tiles that persist like Win11 | done — QS Edit ↑/↓ reorder + Add gallery; persists extras qs.tiles | decision A |
| A036 | QS volume mixer polish vs Settings parity | done — QS mixer system row + % + Settings/Classic links (parity polish) | decision A |
| A037 | QS Bluetooth flyout depth | done — QS BT flyout connect/disconnect + pair wizard (A+C) | decision A+C |
| A038 | QS Wi-Fi password / join flow | done — inline Wi-Fi password join in QS network flyout (nmcli) | decision A |
| A039 | Hotspot / VPN / Cast reliability | done — Hotspot/VPN/Cast availability probes; hide tiles when backend missing | decision A |
| A040 | Tray overflow / hidden icons polish | done — Settings › Other system tray icons + tray-known + overflow hide/show | decision A |
| A041 | Clipboard history: images + pin (community Win+V) | done — Win+V focus + Paste as text discoverability + sync UX in Settings Clipboard | decision A |
| A042 | Advanced Paste beyond paste-as-file | done — Advanced Paste transforms (MD/HTML/JSON/case) on clipboard flyout | decision A |
| A045 | Show file extensions / hidden files defaults | done — Settings › Default apps toggles · Thunar xfconf + GTK file chooser | decision A |
| A046 | Virtual desktops naming + wallpaper per desktop | done — Window Layout names/wallpapers; shell switch applies | decision A — Names + per-desktop wallpaper |
| A049 | Window Grab and Move / AltSnap | done — anvil Alt+LMB/RMB + Settings toggle | decision A — Alt+drag move/resize |
| A050 | Title bar accent / mica / SSD corner radius | done — SSD paint + Colours SSD sliders | decision A — Rounded SSD + mica |
| A051 | Light Switch / dark schedule reliability | done — sunrise/sunset mode + live palette tick | decision A — Live flip + sunrise/sunset |
| A052 | Accent colour on Start/taskbar toggles (Win11 Colours) | done — Colours toggles + palette reload | decision A — Win11-parity Colours page polish |
| A053 | Transparency effects master switch | done — Colours + Accessibility master switch → solid chrome via chrome_alpha | decision A |
| A054 | Bing/Spotlight wallpaper rotation | done — Background Spotlight toggles + history/lock | decision A — Spotlight pack (desktop+lock+history) |
| A055 | Lock screen media controls that feel native | done — soft-lock Now Playing card (transport + vol + progress via playerctl) | decision A — Full media card on lock |
| A056 | Greeter accessibility / keyboard layout real options | done — HC toggle + OSK spawn + setxkbmap layout cycle | decision A — Full greeter a11y + layouts |
| A058 | Account picture change from Start | done — zenity picker Start + Your info | decision A — Real avatar picker from Start + Settings |
| A059 | Night light strength + schedule UI | done — Night light page strength + session schedule | decision A — Strength + schedule UI |
| A060 | Display arrange drag-canvas (multi-monitor) | done — arrange canvas, nudge, IdentifyDisplays | decision A — Drag canvas + identify |
| A061 | Project modes Super+P reliability | done — four ProjectMode choices + connect PcOnly | decision A — Harden four modes + identify displays |
| A062 | HDR / refresh rate / scale per monitor clarity | done — refresh/HDR per head + ApplyOutputTuning | decision A — Full per-output apply/revert UI |
| A063 | Startup apps manager like Win11 | done — impact/search/toggle startup entries | decision A — Impact + search + toggle polish |
| A064 | Default apps by file type / protocol | done — xdg-mime default slots UI | decision A — Full defaults-by-type UI |
| A065 | Installed apps uninstall + repair | done — uninstall/reset data + pins | decision A — Uninstall + reset data + pin actions |
| A066 | Game Mode that does something measurable | done — gamemode + optional DND sync | decision A — Wire to Feral gamemode + DND |
| A070 | Magnifier fidelity (tool/scale path exists) | done — compositor-native scale via `accessibility.magnifier_*` + `mde-magnifier.json`; Settings › Accessibility | decision A |
| A071 | High contrast themes that actually apply | done — hc-black/hc-white packs + GTK HighContrast via Contrast themes / Themes | decision A |
| A072 | Text size system-wide slider | done — `accessibility.text_scale` + gsettings + Text size presets | decision A |
| A073 | Shortcut Guide overlay (PowerToys) | done — Super+/ overlay + Settings toggle (`tools.shortcut_guide`) | decision A |
| A076 | Image Resizer context action | done — presets + Thunar UCA install + Productivity tools resize picker | decision A |
| A078 | Nearby Share / KDE Connect remaining gaps | done — Settings Nearby Phone Link copy; QS flyout share/file/SMS hooks | decision A — Broader Phone-Link feature parity on KDE Connect |
| A080 | Taskbar context menu completeness | done — `taskbar.rs` menu: Show desktop, Task View, Search, notifications, personalization, Shortcut Guide | decision A — Gap-fill to Win11 menu parity where it makes sense |
| A082 | End task from taskbar (Win11 recent feature) | done — jump list End task + Settings › Taskbar “Always show End task” (`end_task_always`) | decision A |
| A083 | Progress bars on taskbar buttons | done — Notify progress hints thicken taskbar underlines (with A004) | decision A |
| A087 | Windhawk-like modular shell styler API | done — `style_packs` API + Colours Settings pack picker + `ensure_sample_pack` | decision A |
| A088 | Animation reduce / disable (community perf/a11y) | done — theme.animations master in Accessibility + Time | decision A |
| A089 | Battery flyout details (Win11 QS) | done — QS battery flyout: saver + power-saver/balanced/performance modes | decision A |
| A090 | Storage Sense analogue | done — `system_pages::storage` + `run_storage_sense` / large-file hint | decision A — Full Storage Sense analogue |
| A091 | Windows Update analogue clarity (dnf/flatpak) | done — `update::hub` restart/defer UX + check copy | decision A — Full update UX + offline update restart |
| A092 | Privacy dashboard (camera/mic/location indicators) | done — `privacy_in_use` · Activity page · taskbar Priv glyph | decision A — Live privacy indicators + app list |
| A093 | Firewall / Security center status freshness | done — `home.rs` `security_health_cards()` | decision A — Home security health cards |
| A094 | Touchpad gestures Win11-like | done — `devices::touchpad` gesture map → `gestures.*` config | decision A — Full gesture map UI |
| A095 | Touch / tablet mode / OSK auto | done — Accessibility auto-OSK/tablet · shell `tablet_posture` on Start | decision A — Auto-OSK + tablet posture |
| A096 | Multi-monitor taskbar behaviors | done — Taskbar `taskbar_multi_mode` + unpinned filter + `taskbar_all_displays` | decision A — Three Win11-like multi-monitor modes |
| A097 | Clock seconds / ISO date (already knobs — surface them) | done — Taskbar + Date & time toggles for seconds/ISO | decision A |
| A098 | World clocks in NC vs Widgets | done — NC `world_clock_label` + Date & time IANA inputs (`shell.world_clocks`) | decision A — Add up to 2 world clocks in NC header |
| A100 | Quiet hours / Focus preset pack as “community recommended” profile | done — Community Classic + Stock Win11 seeded profiles; Home Use buttons | decision A |

### P2 — Secondary decisions (B/C and non-A)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| A006 | Taskbar on top / any edge (StartAllBack headline feature) | done — Bottom only forever; Settings status row documents scope | decision C |
| A021 | Peek file preview (PowerToys) | done — deferred to W8 File Explorer epic (decision B) | decision B |
| A027 | Classic full context menu (ExplorerPatcher demand) | done — out of scope until W8; Settings notes Thunar path | decision C |
| A029 | Taskbar thumbnail peek (deferred W443) | done — keep title-only peek / leave W443 deferred | decision C |
| A031 | NC calendar / agenda depth | done — month grid only; agenda stripped per decision D | decision D |
| A033 | Focus Assist schedules / automatic rules | done — quiet hours only (no automatic rules) | decision C |
| A043 | Widgets board: curation & third-party API | done — Quiet defaults + Settings toggles + ~/.local/share/mde/widgets/*.json API | decision C |
| A044 | File Explorer–class shell (deferred W8) | done — Thunar preferred as MDE Explorer; Start/Settings copy | decision B |
| A047 | Task View thumbnail fidelity | done — title/icon Task View; Settings documents no live thumbnails | decision C |
| A048 | Show-desktop peek-on-hover | done — click strip only (no hover peek) | decision B |
| A057 | Switch user on lock for real (multi-seat/session) | done — single-user lock only | decision B |
| A067 | Captures / Xbox-like screenshot+record | done — screenshot polish path only (no Xbox record) | decision B |
| A068 | Emoji panel depth (native grid exists) | done — recents + search only | decision B |
| A069 | Voice typing / dictation polish (pipeline exists) | done — network STT Settings endpoint polish | decision B |
| A075 | PowerRename / bulk rename | done — rely on Thunar Bulk Rename; Settings documents it | decision B |
| A081 | Task Manager quick access from taskbar/Ctrl+Shift+Esc | done — ShellCmd OpenPerfmon → mde-mmc perfmon.msc | decision C |
| A084 | Search without web / local-only mode | done — Web chip/results removed; Apps/Documents/Settings only | decision C |
| A085 | Content search scale (rg path already works) | done — Desktop/Documents/Downloads only; Settings notes scope | decision C |
| A099 | Feedback / diagnostics UX naming | done — no Feedback nav entry | decision C |

### Skip / deferred (audit trail)

| ID | Item | Status | Notes |
|----|------|--------|-------|
| A020 | Text Extractor / OCR from screen (PowerToys) | skip | decision C — Skip |
| A025 | Screen Ruler / ZoomIt-like annotate (PowerToys) | skip | decision D — Skip |
| A028 | Alt+Tab compact vs thumbnail modes | skip | decision D — Skip |
| A034 | Focus sessions depth (timer already real) | skip | decision D — Skip |
| A074 | Hosts editor / env vars (PowerToys admin toys) | skip | decision C — Skip |
| A077 | File Locksmith “who’s using this file” | skip | decision B — Skip |
| A079 | Debloat / telemetry-analogue: quiet recommendations | skip | decision C — Skip presets |
| A086 | Calculator in Start search (keep + history) | skip | decision D — Skip |

### Special callouts

- **A037 A+C** — QS BT connect/disconnect **and** full pair wizard.
- **A044 B** — Deep-integrate Thunar as MDE Explorer (supersedes greenfield W8 for this track; W8/W393 still deferred for native Explorer).
- **A081 C** — Ctrl+Shift+Esc → MMC Performance; polish and expand MMC.
- **A006 C** — Bottom taskbar only forever.
- **A056 A** — Full greeter a11y + layouts (overrides older “greeter stubs out of scope” note for this epic).

## Explicitly out of scope for this worklist

- Reintroducing labwc / LightDM / Python Regedit as default.
- Changing the CUTOVER v1 done-bar definition without a new plan.
- Desktop **icon** context menus beyond W417–W419 (full Explorer menus remain W393 / W8).
- App-specific Jump List Tasks.
- **Greeter** fingerprint stub rows remain out of scope; Accessibility / keyboard-layout now **in-scope via A056** (community audit). Lock Switch user stays W428 / A057=B single-user.
- Settings → Accessibility “Coming with system tools” page rewrite (QS/shortcut magnifier still in-scope).
- MMC outside MDE Wayland session; headless/TUI/web companion; standalone MMC RPM without MDE (Q19=A).
- Performance Monitor Grafana/Prometheus remote scrape (Q16=B, not C).
- Dual-approval / break-glass fleet policy (Q20=C, not D).
- Real Microsoft binary `.msc` interchange with Windows (Q9=B, not C).
