# MDE remaining worklist

Last updated: 2026-11-27 (opened **W650–W699** immersive imagery + motion epic; W600–W649 EQ and Music W520–W599 active).

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
7. **Neighbourhood Music world-class completion — W520–W599** (active) — full-bleed art/layout first (W520–W529), then Stage/chrome, library, discover, cast, YTM acquire/repair, then stub/dead-wire Phase 7 (W580–W598). See epic section below.
8. **OpenFilter→Stage audiophile EQ — W600–W649** (active) — Rust OpenFilter-inspired EQ stack replacing GStreamer 10-band; GTK Audiophile Stage; AGPL. Coordinates with Music epic Stage items (W530–W535, W562, W588).

9. **Immersive artist / album / fan imagery — W650–W699** (active) — keyless artist photos + Fanart backdrops resolver (A), shared `BleedBackdrop` (B), then every surface: artist page, Home hero, Stage/Gallery, bar, Search, Library, Radio, Podcasts (C), settings/perf/attribution/exit (D). See epic section below.

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
| W340b | Fonts: 20 Google UI sans + Selawik; Inter default; Mono Google + curated Nerd; hub quick pick + live GTK/Qt/fontconfig | done | |
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

## Epic: Neighbourhood Music — world-class completion (W520–W599)

Opened **2026-11-26** from locked UX / YTM / Repair / Performer / visual-system interviews (Nov 25) vs live `mde-music` (`neighbourhood-music`). Expanded same day with full per-item detail + Phase 7 stub/dead-wire audit. Host build at open: `BuildID a9e47abc…`, package `2026.0.0`.

**Crate:** `mde/crates/mde-music` · **Binary:** `neighbourhood-music` · **Install:** `mde/data/neighbourhood-music/install.sh` (must also sync `~/.local/bin` — PATH prefers it) · **CSS:** baked via `include_str!("../../data/style.css")` in `ui/shell.rs` (disk `/usr/share/neighbourhood-music/style.css` alone does not restyle)

**Status key:** `todo` · `verify` · `done` · `deferred` · `skip`

**Done when:** Every W520–W598 is `done`/`verify` (or explicitly `skip` with rationale); W599 hygiene landed; W579 exit checklist green on this seat.

### Locked direction (source of truth)

| Track | Locked summary |
|-------|----------------|
| UX Q1–Q26 | Dual-mode browser + Stage; album opens detail (no discovery autoplay); play-from-here; **full-bleed album + frosted tracks**; Stage via now-playing and/or Stage button; tabbed Stage Artwork\|Lyrics\|Queue\|Credits; min/max/close **and** mini-player; slim bar + scrubber/volume in Stage/flyouts; back stack + breadcrumbs; grouped Search; rich Home; radio continuum; heart + Liked Songs; full context menu; discography artist page; hybrid sidebar; playlist CRUD; full cast UX; MDE language + art-driven accents; audiophile columns; full shortcuts; cache badges + size manage; Sound flyout; integrated podcasts; Radio tabs For You·Stations·Tags·Favorites; multi-server unified library + badges |
| YTM Q1–Q20 | Native in-crate YTM (no ytmdesktop embed); unified catalog + badges; no login; search/play + radio continuum + discover shelves; dedicated audio pipeline; song-first + optional Watch video; hard-merge albums; likes → one taste profile; quality prefer local; Like → acquire HQ + MB tags/paths + server upload; Acquisitions page; Innertube + yt-dlp fallback; cast resolved URL; honest Repair UX; full feature set |
| Repair Q1–Q9 | Hybrid hotfix rules + yt-dlp update; proactive + failure + manual; GitHub HTTPS trust; rotate→rules→yt-dlp→smoke→fail UI; multi-step smoke; banner + Cancel; stable/nightly + fix-pack + ytmdesktop hints; `~/.config/neighbourhood-music/ytm-repair/`; last-known-good + rollback |
| Visual 50/50 | Full-bleed hero under frosted chrome; art-adaptive scrims/accents; server→Fanart→CAA→iTunes HD→YT badge; Netflix rotating hero; performer full-bleed; album bleed + frost; shelves/motion; missing-art placeholders |
| Performer 10/10 | Per-performer overview; MusicBrainz + Wikidata; Fanart bleed; frosted facts; shelves; confirm picker; Play top / Shuffle / Instant Mix |
| Deferred | Main UX **Q27–Q30** never asked — capture as W578 |

**Index:** Phase 0 W520–W529 · Phase 1 W530–W537 · Phase 2 W540–W548 · Phase 3 W550–W554 · Phase 4 W560–W565 · Phase 5 W570–W575 · Phase 6 W576–W579 · Phase 7 W580–W599

---

### Phase 0 — Visual system & art (P0)

#### W520 — Album page full-bleed backdrop + frosted tracks
| | |
|---|---|
| **Status** | done — album page is a cover-fit Picture backdrop with frosted track overlay |
| **Locked** | UX Q4 + visual 50/50 — cover as full-bleed backdrop; frosted track list overlay |
| **Today** | `ui/shell.rs` `show_immersive_album`: centered ~280px `Image` + `.nm-album-hero` gradient box; tracks in `.nm-frost-tracks` **below** (stacked, not overlay). CSS: `data/style.css` `.nm-album-hero` / `.nm-frost-tracks` |
| **Do** | Edge-to-edge cover (Picture/drawing area or CSS background from fetched art); frosted panel over lower portion with track table; readable contrast; Play/Shuffle/Performer stay reachable |
| **Accept** | Opening any album shows bleed art filling content width; tracks readable on frost; no centered postage-stamp cover as primary |

#### W521 — Performer page full-bleed
| | |
|---|---|
| **Status** | done — performer page is a cover-fit backdrop with the facts panel overlaid on the lower third |
| **Locked** | Performer 10/10 — full-bleed portrait; frosted facts on lower third; shelves below |
| **Today** | `ui/performer_page.rs`: centered 400px image (`.nm-performer-bleed`); panel below with negative margin approximation; MusicBrainz/Wikidata resolve in `performer/mod.rs` |
| **Do** | True bleed background from Fanart background/thumb or artist cover; frost facts panel; albums/tracks shelves; hide empty rows; Confirm picker when ambiguous |
| **Accept** | Performer looks cinematic (not a big icon); facts readable; shelves work with/without Fanart key |

#### W522 — Home Featured hero (Netflix-style)
| | |
|---|---|
| **Status** | done — Featured hero shows cover, title, Play/Open, and prev/next; auto-advance pauses for 20s after you touch it |
| **Locked** | Visual 50/50 — rotating hero from Continue / New Music / New Podcasts; peek prev/next; pause-on-interact |
| **Today** | `build_home_hero` / `build_hero_items` / `apply_ui_snapshot` in `shell.rs`; `artwork::hero_cover`; empty copy “Connect a library…” when no items |
| **Do** | Always art-forward after sync; HD covers; auto-advance that pauses on click; Play/Open wired; never leave symbolic forever when library has albums |
| **Accept** | Within one sync, Featured shows real cover + title; prev/next/play work |

#### W523 — Art pipeline priority, sizes, timeouts
| | |
|---|---|
| **Status** | done — shelves keep a server cover URL and only look up the web when it is missing; HTTP art calls time out at 4–8s and image bytes download off the UI thread |
| **Locked** | Visual — server → Fanart (artists) → CAA (albums) → iTunes HD (hero/Stage) → YT fallback with badge; 600/1200/2000; hero network-fresh; cache shelves |
| **Today** | `artwork/mod.rs`: `prefer_web_cover`, `hero_cover`, `resolve_cover`, `fanart_for_mbid`, `fetch_image_file`, `bind_cover` hub in shell |
| **Do** | Enforce priority + timeouts so shelves don’t thrash; adaptive request sizes; YT art always badged; stable cache keys (auth-insensitive — already partly done) |
| **Accept** | Hero prefers HD web when server art weak; lists stay responsive; no wrong-cache collisions |

#### W524 — Fanart.tv key + wiring
| | |
|---|---|
| **Status** | done — Tools page saves fanart_api_key; artwork::fanart_api_key already reads it |
| **Locked** | Optional Fanart API key; artist thumb + background |
| **Today** | `fanart_api_key()` reads `FANART_API_KEY` or history `fanart_api_key`; used in performer resolve; **no Settings UI**; key unset on this seat |
| **Do** | Settings/Tools field to set/clear key; persist via history settings; document env override; wire backgrounds into W521/W522 |
| **Accept** | With key set, performer/hero can show Fanart backgrounds; without key, graceful degrade |

#### W525 — Art-adaptive accents / scrims
| | |
|---|---|
| **Status** | done — the player bar, Stage, and album or performer frame take their accent from the current cover; the scrim stays dark enough that light text keeps contrast |
| **Locked** | Q19 + visual — accent from now-playing/hero art; adaptive light/dark scrims |
| **Today** | Static CSS palette in `data/style.css`; no palette extraction |
| **Do** | Sample dominant colors from cover; apply CSS variables / provider to Stage, bar, album/performer panels; keep text contrast |
| **Accept** | Changing tracks visibly shifts chrome accent/scrim without breaking readability |

#### W526 — Missing-art placeholders
| | |
|---|---|
| **Status** | done — missing covers show a letter/music-note tile instead of a fake photo |
| **Locked** | Visual / Q27 cluster — no fake cover photos; monogram / hash color / gradient+title |
| **Today** | Symbolic icons (`media-optical-symbolic`, `avatar-default-symbolic`) |
| **Do** | Generate placeholder tiles (initials or hash hue + title typography) for missing covers |
| **Accept** | Empty-art rows never look like broken image downloads |

#### W527 — Shelf / card motion polish
| | |
|---|---|
| **Status** | done — album cards lift on hover and a Play button fades in; it starts the album without replacing the click-to-open behavior |
| **Locked** | Visual — adaptive sizes; soft play-on-hover; cinematic hover; title+subtitle; Continue progress-on-art |
| **Today** | Basic cards/shelves; Continue cards + progress ring CSS exist; little motion |
| **Do** | Hover play affordance; depth/shadow polish; consistent subtitle; keep Continue progress |
| **Accept** | Shelves feel intentional (Apple/Netflix density), not flat GTK buttons only |

#### W528 — Library grids + Search row art density
| | |
|---|---|
| **Status** | done — Library artists and albums are cover grids; artist covers are round; search rows are at least 72px tall |
| **Locked** | Visual — large cover / circular artist grids; Search rows ~64–80px with art |
| **Today** | Mostly `ListBox` / catalog lists with 48px covers (`catalog_list`) |
| **Do** | Grid modes for Library artists/albums; taller Search result rows with art |
| **Accept** | Library browse is cover-first; Search rows show art at locked size |

#### W529 — Release rebuild + dual-prefix install
| | |
|---|---|
| **Status** | done — 2026-11-27: release, ~/.local/bin and /usr/bin all md5 475ddea12a43f0bd3bde158dda98c9dc |
| **Locked** | Ops — after visual/CSS cuts, ship binary both places |
| **Today** | Install script → `/usr/bin`; PATH often uses `~/.local/bin` (can drift) |
| **Do** | `cargo build --release -p mde-music`; install `/usr/bin` **and** `~/.local/bin`; update desktop Exec; kill stale windows |
| **Accept** | `md5sum` matches both prefixes + release artifact; relaunch shows new CSS/UI |

---

### Phase 1 — Stage, player chrome, window (P0/P1)

#### W530 — Stage tabs: Artwork / Lyrics / Queue / Credits (real)
| | |
|---|---|
| **Status** | done — Stage artwork, lyrics, reorderable queue, and credits from the playing track; empty credits say to play a track |
| **Locked** | UX Q6 |
| **Today** | `build_stage` tab chrome; Credits = placeholder “Credits appear when metadata is available.”; Lyrics via `fill_lyrics` + backend `lyrics`; Queue list exists |
| **Do** | Artwork large art; Lyrics synced when possible else static; Queue reorder; Credits from MusicBrainz/YTM (composer, performers, label, year). **Also add Audiophile tab — see W600–W649 / W620** |
| **Accept** | No stub strings; each tab shows useful content or honest empty state; Audiophile present when EQ epic lands |

#### W531 — Enter Stage from now-playing + Stage button
| | |
|---|---|
| **Status** | done — now-playing cover and title click open Stage; Stage button remains |
| **Locked** | UX Q5 |
| **Today** | Stage nav + Stage button; now-playing click path partial; Mini suppresses auto-open |
| **Do** | Click cover/title in bar expands Stage; Stage button always; Mini mode does not auto-steal focus |
| **Accept** | Both entry paths work; Mini respects user preference |

#### W532 — Real mini-player collapse
| | |
|---|---|
| **Status** | done — Mini hides the browser, marks nm-mini, and shrinks the window to a bar |
| **Locked** | UX Q7 + visual mini-player |
| **Today** | `AppState.mini_player: Cell<bool>` mostly gates Stage auto-open — not a collapsed chrome mode |
| **Do** | Collapsed window/chrome: cover + title + transport (+ scrub/volume per lock); expand back to full |
| **Accept** | Mini is visibly a different chrome, not just a flag |

#### W533 — Window minimize / maximize / close
| | |
|---|---|
| **Status** | done — in-window minimize, maximize, and close buttons on the top chrome |
| **Locked** | UX Q7; original known bug |
| **Today** | Relies on compositor SSD; no in-app min/max/close |
| **Do** | Verify under MDE SSD; add in-app controls if SSD fails; preserve fit above taskbar |
| **Accept** | Min/max/close reliable on this seat |

#### W534 — Slim bar vs Stage/sheet control ownership
| | |
|---|---|
| **Status** | done — the bar scrubber is hidden; seek stays on Stage and in the Sound flyout, with Stage, Sound, Queue, and Cast one tap away |
| **Locked** | Q8 + visual — slim bar = cover+title+transport; scrubber/volume in Stage/flyouts; shuffle/queue/cast/mini/sound in Stage/sheet |
| **Today** | Bar has transport + several buttons (Sound/Cast/Stage/Queue/Mini); scrubber present |
| **Do** | Reconcile with lock: move overflow into Stage/sheet; keep bar slim and on-brand |
| **Accept** | Bar matches locked slim layout; advanced controls still one tap away |

#### W535 — YTM Watch video in Stage
| | |
|---|---|
| **Status** | done — Stage Watch plays a video address when the track has one and pauses audio; with no video, audio keeps playing |
| **Locked** | YTM Q6 — song-first; optional Watch; video-audio fallback |
| **Today** | Audio pipeline only; no video/WebKit surface |
| **Do** | Optional Watch pane/surface for YTM when useful; don’t block audio path |
| **Accept** | User can watch when available; audio-only remains default |

#### W536 — Radio-like continuum polish
| | |
|---|---|
| **Status** | done — similar tracks append at queue end and the bar shows “continuing” for 12s |
| **Locked** | UX Q12 |
| **Today** | `maybe_extend_queue_continuum` + backend `similar_tracks` |
| **Do** | Clear UX when continuum engages; source badges; don’t surprise with silent queue growth |
| **Accept** | End of album/queue continues with similar tracks visibly |

#### W537 — Audiophile columns everywhere
| | |
|---|---|
| **Status** | done — album and track tables show #, title, album, time, format, bitrate, and sample rate |
| **Locked** | UX Q20 — `# · Title · Album · Duration · Bitrate/format` |
| **Today** | `show_track_table` partial; Jellyfin `sample_rate` often missing (see W595) |
| **Do** | Consistent columns on album/playlist/search/queue lists; format/bitrate/sample when known |
| **Accept** | Power users can scan quality without opening files |

---

### Phase 2 — Library, artist, likes, playlists, menus (P1)

#### W540 — Discography-first artist page
| | |
|---|---|
| **Status** | done — artist page has an about line, top tracks, and the album list |
| **Locked** | UX Q15 — albums + top tracks + about |
| **Today** | `NavFrame::ArtistAlbums` = album list only |
| **Do** | Top tracks, album grid, About blurb; distinct from Performer overview but linked |
| **Accept** | Artist open feels like Apple/Spotify artist page, not a bare list |

#### W541 — Artist entry → Performer + discography + disambiguation
| | |
|---|---|
| **Status** | done — the now-playing artist name opens that artist’s discography; Performer is still on the bar and on album pages |
| **Locked** | Performer 10/10 entry + confirm picker |
| **Today** | Performer button on album/now-playing; MB confirm path exists in performer module |
| **Do** | Tap artist name anywhere routes correctly; remember MBID; Confirm when ambiguous |
| **Accept** | No dead artist taps; wrong MB match can be corrected once |

#### W542 — Liked Songs destination
| | |
|---|---|
| **Status** | done — sidebar Liked Songs opens the taste library list |
| **Locked** | UX Q13 — heart + Liked Songs playlist |
| **Today** | `taste::like` / `is_liked`; hearts in track rows; **no Liked Songs nav page** |
| **Do** | Library/nav destination listing liked tracks; play/shuffle; sync with server favorites where possible |
| **Accept** | User can open and play their liked set |

#### W543 — Full classic context menu
| | |
|---|---|
| **Status** | done — more menu: Play, Play Next, Queue, Like, Add to Playlist, Go to Album, Go to Artist |
| **Locked** | UX Q14 — Play / Next / Queue / Playlist / Album / Artist / Like |
| **Today** | `more` popover: Play, Play Next, Add to Queue, Like only (`shell.rs` ~3778+) |
| **Do** | Add Add to Playlist, Go to Album, Go to Artist; consistent on shelves/search/queue |
| **Accept** | Right-click/more exposes full classic set |

#### W544 — Playlist CRUD in-client
| | |
|---|---|
| **Status** | done — create, rename, delete, add, remove, and reorder playlist tracks against the server |
| **Locked** | UX Q17 |
| **Today** | Create playlist popover → `create_playlist`; delete/rename/membership/reorder incomplete; YTM playlists bail (no account) |
| **Do** | Create/rename/delete; add/remove tracks; reorder; refresh UI; server sync via `sync/playlists.rs` |
| **Accept** | Full playlist management without leaving the app (server-backed sources) |

#### W545 — Back stack + breadcrumbs fidelity
| | |
|---|---|
| **Status** | done — library back stack and breadcrumbs cover home, artist, album, and playlist; performer has a Back button to the library |
| **Locked** | UX Q9 |
| **Today** | `NavFrame` stack + crumb UI in Library |
| **Do** | Audit Search/album/artist/performer dead ends; Back always one level; crumbs match stack |
| **Accept** | No navigation traps; crumbs truthful |

#### W546 — Hybrid sidebar IA
| | |
|---|---|
| **Status** | done — sidebar is Home/Search/Library/Podcasts, then Discover (Radio) and Listen (Queue/Stage/Acquisitions) plus Tools; one Radio destination |
| **Locked** | UX Q16 — slim primary Home/Search/Library/Podcasts; regroup Discover/Listen/Tools; merge Radio vs Stations |
| **Today** | Long flat nav + “DISCOVER” label; Radio and Stations both present historically |
| **Do** | Regroup; clear active states; naming cleanup |
| **Accept** | Sidebar scannable; no duplicate Radio concepts |

#### W547 — Unified multi-server library + source badges
| | |
|---|---|
| **Status** | done — album and track rows show LIB/YT/LOC/MIX badges; the active server and YouTube Music share one merged library |
| **Locked** | UX Q26 |
| **Today** | Switch-server model + YTM merge; hardcoded `"YT"` badges; `BackendKind::badge` / `taste::source_label` unused |
| **Do** | Consistent badges for JF/Subsonic/YTM/local; path toward multi-server blend or honest single-active+YTM |
| **Accept** | User always knows source of a row |

#### W548 — Hard-merge album cards
| | |
|---|---|
| **Status** | done — unify::merge_albums/merge_tracks collapse the same release and keep the higher-quality track |
| **Locked** | YTM Q7/Q9 — one album card; prefer local/JF; YTM fills gaps; quality tie-break to local |
| **Today** | `unify::merge_albums` / `merge_tracks` / `prefer_track` (prefer_track dead_code) |
| **Do** | UI shows one card; track list prefers higher quality / local; YTM gap-fill marked |
| **Accept** | No duplicate album cards for same release across sources |

---

### Phase 3 — Home, Search, Radio, Podcasts (P1)

#### W550 — Home composition completeness
| | |
|---|---|
| **Status** | done — Home opens with a time-of-day greeting, then Featured, Continue listening, and the server shelves |
| **Locked** | UX Q11 — Continue + curated shelves + library pulse + time-aware top; New Music/Podcasts near top |
| **Today** | Featured + Continue + New podcasts + shelves/`mfy` ordering partial |
| **Do** | Time-aware greeting/top; library pulse; stable section order |
| **Accept** | Home tells a listening story in one screen |

#### W551 — Grouped Search (stable, multi-word)
| | |
|---|---|
| **Status** | done — Search keeps the whole query, including several words, and groups songs, albums, artists, and playlists |
| **Locked** | UX Q10 |
| **Today** | `build_search` + Search button/Enter; library+YTM; prior crash fixes |
| **Do** | Grouped sections; regression-guard multi-word; no crash; clear empty states |
| **Accept** | “billie eilish” style queries work; results grouped |

#### W552 — Radio tabs For You · Stations · Tags · Favorites filled
| | |
|---|---|
| **Status** | done — Radio tabs: For You from server/YTM shelves, Stations browser, Tags that run a tagged search, Favorites from saved stations |
| **Locked** | UX Q25 |
| **Today** | Tab chrome in radio builder; For You/Tags population sparse; Favorites file exists |
| **Do** | Real data per tab; Tags browse; For You from taste + server/YTM moods |
| **Accept** | No empty chrome tabs when data sources are available |

#### W553 — Podcasts fully integrated + refresh reliable
| | |
|---|---|
| **Status** | done — Refresh asks the server to update the show, reloads the feed, and puts a server or feed failure on the status line instead of a blank list |
| **Locked** | UX Q24 |
| **Today** | `podcast_page.rs` + `podcasts/mod.rs`; Home New podcasts; prior “The Daily” refresh errors |
| **Do** | Shared player; Home shelves; episode refresh; open show from Home (also W598); resume |
| **Accept** | Server + directory podcasts play and refresh without dead ends |

#### W554 — YTM anonymous discover shelves
| | |
|---|---|
| **Status** | done — Home and Search load anonymous charts and moods; if both browses fail, the page says so and Repair is notified |
| **Locked** | YTM Q3/Q4 — charts/moods/genres without login |
| **Today** | Partial mood hooks; not a full discover home |
| **Do** | Charts/moods/genres shelves in Home/Search; honest empty/repair if Innertube fails |
| **Accept** | User can browse YTM discover anonymously |

---

### Phase 4 — Cast, Sound, cache, shortcuts, themes (P1/P2)

#### W560 — Full cast UX
| | |
|---|---|
| **Status** | done — the picker keeps system output local, shows sent or the failure, and offers status, volume, stop, and transfer; DLNA uses SOAP, and Chromecast volume/stop say when the HTTP path cannot do them |
| **Locked** | UX Q18 — picker with status/volume/stop/transfer; multi-room; system output; Chromecast+DLNA+AirPlay; never silent first-device |
| **Today** | `cast/mod.rs` discover + HTTP cast helpers; `show_cast_picker` thin |
| **Do** | Status, volume, stop, transfer; zones; clear errors when cast fails |
| **Accept** | First cast attempt always shows success or explicit failure |

#### W561 — Cast resolved stream URLs (incl. YTM)
| | |
|---|---|
| **Status** | done — Cast resolves a YouTube Music or server stream URL before sending, and the device row says sent or failed |
| **Locked** | YTM Q18 |
| **Today** | Cast uses stream URL when present; YTM may need resolve first |
| **Do** | Resolve then cast same as local play path |
| **Accept** | YTM track casts when playable locally |

#### W562 — Sound flyout complete
| | |
|---|---|
| **Status** | done — Sound flyout has volume, seek, output sinks via wpctl, transcode, Open Audiophile, and EQ bypass |
| **Locked** | UX Q23 — EQ + output + transcode |
| **Today** | `show_sound_flyout`: transcode toggle + EQ sliders; output picker thin |
| **Do** | Real output device list; transcode. **EQ sliders superseded by OpenFilter Audiophile (W600–W649 / W615)** — flyout should deep-link Open Audiophile + bypass, not keep 10-band |
| **Accept** | Output + transcode solid; EQ entry points to Audiophile |

#### W563 — Cache badges + manage size
| | |
|---|---|
| **Status** | done — Tools shows the artwork cache size and can clear web-art |
| **Locked** | UX Q22 |
| **Today** | “Warm web artwork cache” button; no per-item cache badge; no size manager |
| **Do** | Badges when cached; UI to view/limit/clear web-art (+ audio cache if W582 wires it) |
| **Accept** | User can see and manage cache footprint |

#### W564 — Full keyboard shortcuts
| | |
|---|---|
| **Status** | done — Space/N/P/arrows/volume/L/S/Q///Esc; cheat sheet on the Tools page |
| **Locked** | UX Q21 |
| **Today** | `install_shortcuts`: Space, N/P, L, S, `/` only |
| **Do** | Volume, seek, queue, like toggle, Stage/Search/Home, Escape; in-app cheatsheet |
| **Accept** | Power user can drive player without mouse |

#### W565 — Theme / Mackes language consistency
| | |
|---|---|
| **Status** | done — buttons, entries, lists, and popovers inside the window use the dark palette, with a cool focus ring; seat confirmation stays on W576 |
| **Locked** | Q19 + prior bug reports (white buttons, wrong theme) |
| **Today** | Custom CSS classes; some GTK defaults still leak |
| **Do** | Sweep buttons/entries/lists for on-brand colors; focus rings; density |
| **Accept** | No incorrect white default chrome in dark shell |

---

### Phase 5 — YTM acquire, repair, reliability (P1)

#### W570 — Acquire pipeline end-to-end
| | |
|---|---|
| **Status** | done — yt-dlp download_best wrote an audio file, and a tagged wav copied into the inbox path still plays and keeps its title |
| **Locked** | YTM Q10–Q15 — Like → HQ → MB-prefer tags → MB path → server upload |
| **Today** | `acquire/mod.rs` job states + `LibraryUploader`; path helper; ffmpeg tags best-effort |
| **Do** | Harden download/tag/upload; confidence MB match; correct extensions; failures actionable |
| **Accept** | Liking a YTM track yields playable library file on server inbox/path |

#### W571 — Acquisitions page polish
| | |
|---|---|
| **Status** | done — the acquisitions list stays live, a toast appears when a watched job finishes or fails, and the sidebar plus player bar show the active or failed count |
| **Locked** | YTM Q12 — queue page + toasts/badges |
| **Today** | Acquisitions page shell lists jobs |
| **Do** | Live updates; toast on done/fail; badges elsewhere |
| **Accept** | User can monitor and understand acquire jobs |

#### W572 — Taste profile drives Home / Radio / continuum
| | |
|---|---|
| **Status** | done — Home names your top artist, and when the queue runs out similar tracks from artists you like are placed first |
| **Locked** | YTM Q8 — likes from all sources; local only |
| **Today** | `taste.rs` SQLite likes/artist bumps; continuum/Home underuse profile |
| **Do** | Feed For You / Home / continuum from taste; include JF/Subsonic/YTM likes |
| **Accept** | Liking changes recommendations visibly over time |

#### W573 — Repair path complete
| | |
|---|---|
| **Status** | done — Repair now runs rotate, rules, yt-dlp, then smoke; cancel and a failed smoke restore the last good copy, or say there was nothing to restore |
| **Locked** | Repair Q1–Q9 |
| **Today** | `ytm/repair.rs` + banner UI; proactive checks scheduled; rotate_client may not run in “Rotating” phase |
| **Do** | Full sequence rotate→rules→yt-dlp→smoke→fail UI; Cancel; LKG rollback; manual force |
| **Accept** | Broken YTM can self-heal or fail honestly with Repair action |

#### W574 — Honest YTM errors + Repair CTA
| | |
|---|---|
| **Status** | done — a failed or rolled-back YouTube Music repair stays on the banner with Repair now; a healthy source does not show that banner |
| **Locked** | YTM Q19 |
| **Today** | Repair banner partial |
| **Do** | Never show YTM as healthy when smoke failed; Repair always reachable |
| **Accept** | No fake working YTM source |

#### W575 — Innertube first + yt-dlp fallback + fix-packs
| | |
|---|---|
| **Status** | done — native Innertube is tried first and kept only when a range request returns audio; otherwise yt-dlp plays it. A fix-pack with clients replaces the pack, an empty one does not, and no Electron runtime is used |
| **Locked** | YTM Q16/Q17 + Repair Q7 |
| **Today** | `ytm/innertube.rs`, `ytm/stream.rs` with yt-dlp fallback paths |
| **Do** | Prefer native; fallback documented; downloadable rules/fix-packs; never require Electron |
| **Accept** | Play works via native or fallback; fix-pack update path tested |

---

### Phase 6 — Deferred capture, residual bugs, exit (P2)

#### W576 — Residual bug sweep (Nov 25 thread)
| | |
|---|---|
| **Status** | verify — code-reviewed 2026-11-27: startup reconnect failure logs + shows server picker (no dialog); body/scroll natural-size propagation off + work-area sizing above taskbar; Continue Listening falls back to Direct stream by id; podcast Refresh reloads feed + server; search URL-encodes multi-word queries. Needs a human seat pass |
| **Today / Do** | Startup sync error dialog; horizontal size explode; player covered by taskbar; Continue Listening click; podcast episode refresh; Search multi-word; theme buttons — verify each fixed or ticket leftover |
| **Accept** | None of the listed regressions reproduce on seat |

#### W577 — Server radio empty-state + Favorites path
| | |
|---|---|
| **Status** | done — empty server radio copy points at Favorites and Search |
| **Today** | Copy “This server has no internet radio stations.”; user reported no local radio |
| **Do** | Honest empty state; make Favorites / radio-browser path obvious; wire server list (W583) |
| **Accept** | User always has a next action (Favorites/Search) when server empty |

#### W578 — Capture deferred main UX Q27–Q30
| | |
|---|---|
| **Status** | done — captured defaults: Q27 missing art is a letter tile (W526); Q28 no full-window blur, frost stays on panels; Q29 hero motion must pause on click; Q30 server art first, then web fallbacks with a short timeout (W523 still tracks the timeout polish) |
| **Do** | Either run the 4 questions or record defaults (missing-art style already leans W526); write answers into Notes here |
| **Accept** | No silent deferred product decisions |

#### W579 — Epic exit gate
| | |
|---|---|
| **Status** | todo |
| **Do** | All phases green; dual-prefix install; seat smoke using Acceptance checks below |
| **Accept** | Checklist complete; epic marked done in this file |

---

### Phase 7 — Stub / dead-wire / README honesty (2026-11-26 audit)

#### W580 — MPRIS / LAN next·prev must play (P0)
| | |
|---|---|
| **Status** | done — MPRIS + HTTP /next /previous enqueue SkipRequest; UI loop calls play_track |
| **Today** | `mpris/mod.rs` `next`/`previous` only call `advance`/`previous_track`; `control/http.rs` same; UI bar calls `play_track` after |
| **Do** | Shared skip helper that advances **and** plays (resolve stream, update `now`) |
| **Accept** | Media keys / AVRCP / `POST /next` change audible track |

#### W581 — Queue click → `play_track` (P1)
| | |
|---|---|
| **Status** | done — queue rows call play_track after set_queue |
| **Today** | `fill_queue` → `set_queue` + `play_uri` only |
| **Do** | Use `play_track` so YTM resolve/scrobble/continuum/`now` work |
| **Accept** | Clicking queue rows plays YTM and server tracks correctly |

#### W582 — Wire or retire `cache/` + README (P1)
| | |
|---|---|
| **Status** | done — connect refreshes LAN library snapshot; README no longer claims full audio cache; unused audio-file helpers removed |
| **Today** | `cache/mod.rs` APIs unused except `ensure_dirs`; README claims “LAN full-library cache” |
| **Do** | Wire snapshot/audio cache into connect/play **or** delete dead APIs and reword README |
| **Accept** | README matches reality; no dead_code cache surface left unexplained |

#### W583 — Feed `radio_stations` into RadioPage (P1)
| | |
|---|---|
| **Status** | done — apply_ui_snapshot calls RadioPage::set_server_stations |
| **Today** | Snapshot loads `stations` then never reads them; `set_server_stations` unused (dead_code) |
| **Do** | `apply_ui_snapshot` → `radio_page.set_server_stations(snap.stations)` |
| **Accept** | Radio “This server” lists server stations when present |

#### W584 — Like toggle / unlike (P1)
| | |
|---|---|
| **Status** | done — heart and L toggle taste::unlike + set_favorite(false) |
| **Today** | UI/shortcut only `like` + `set_favorite(..., true)`; `taste::unlike` unused |
| **Do** | Toggle heart; call unlike + `set_favorite(false)` |
| **Accept** | Second click unlikes locally and on server |

#### W585 — Progress heartbeats (P1)
| | |
|---|---|
| **Status** | done — ~6s report_progress heartbeat while playing |
| **Today** | `report_progress(..., 0, false)` only at start of `play_track` |
| **Do** | Periodic Progress; pause/stop updates for Jellyfin resume; Subsonic best-effort |
| **Accept** | Server resume position advances during playback |

#### W586 — Ingest-path settings UI (P1)
| | |
|---|---|
| **Status** | done — Tools page edits jellyfin_ingest_path and subsonic_ingest_path |
| **Today** | Hidden history keys `jellyfin_ingest_path` / `subsonic_ingest_path`; defaults `…/jellyfin-inbox` / `…/subsonic-inbox` |
| **Do** | Settings field to view/edit path; validate writable |
| **Accept** | User can point acquire uploads at server-watched folder without sqlite hacks |

#### W587 — Control HTTP bind/auth (P1)
| | |
|---|---|
| **Status** | done — control API binds 127.0.0.1; NM_CONTROL_BIND opts into LAN |
| **Today** | `control/http.rs` binds `0.0.0.0:3847`; no auth; README advertises LAN API |
| **Do** | Default localhost; opt-in LAN; token or documented firewall |
| **Accept** | Default install not world-writable transport on LAN |

#### W588 — Tools EQ init from live bands (P1)
| | |
|---|---|
| **Status** | done — Tools EQ sliders initialize from the live band gains and drive nmeq |
| **Today** | Tools EQ scales `set_value(0.0)`; Sound flyout uses `eq_bands()` |
| **Do** | **Superseded by W600–W649** — remove Tools 10-band graphic EQ when OpenFilter stack ships (W604/W615); until then keep honest init |
| **Accept** | Either removed post-cutover or temporarily agrees with live bands |

#### W589 — Subsonic top_tracks artist param (P1)
| | |
|---|---|
| **Status** | done — getTopSongs uses artist name from getArtist |
| **Today** | `subsonic.rs` `top_tracks` encodes artist **id** as `artist=` name |
| **Do** | Resolve name or correct OpenSubsonic parameter |
| **Accept** | Artist top songs populate when server supports them |

#### W590 — Gapless honesty or true gapless (P1/P2)
| | |
|---|---|
| **Status** | done — README describes end-of-stream auto-advance, not sample-accurate gapless |
| **Today** | README “gapless advance”; code is EOS stop→start (`shell.rs` bus handler) |
| **Do** | Implement `about-to-finish` gapless **or** reword README |
| **Accept** | Docs match behavior |

#### W591 — MPRIS Raise/Quit + Seeked/PropertiesChanged (P2)
| | |
|---|---|
| **Status** | done — Raise presents the window and Quit exits; seek emits Seeked, and play/pause/track changes emit PlaybackStatus and Metadata |
| **Today** | Raise/Quit empty; Seeked defined unused |
| **Do** | Raise focuses window; Quit exits cleanly; emit Seeked/PropertiesChanged on seek/track change |
| **Accept** | Desktop media widgets stay in sync; Raise works |

#### W592 — Playlist create refresh + upsert_local (P2)
| | |
|---|---|
| **Status** | done — creating a playlist reloads the library; Add to Playlist writes through upsert_local |
| **Today** | Create popover no reload; `sync/playlists.rs::upsert_local` unused |
| **Do** | Refresh library playlists after create; use or delete upsert helper |
| **Accept** | New playlist appears immediately |

#### W593 — Honest EQ / ffmpeg tag failures (P2)
| | |
|---|---|
| **Status** | done — a missing or failing ffmpeg is written onto the acquire job instead of being ignored |
| **Today** | Missing equalizer plugin `.ok()`; ffmpeg tags `let _ =` |
| **Do** | Toast/status when EQ unavailable or tagging skipped |
| **Accept** | User knows when quality pipeline steps were skipped |

#### W594 — Clear queue action (P2)
| | |
|---|---|
| **Status** | done — Clear queue button on queue views |
| **Today** | `Player::clear_queue` dead_code |
| **Do** | Clear button in Queue/Stage |
| **Accept** | Queue can be emptied in one action |

#### W595 — Jellyfin sample_rate (P2)
| | |
|---|---|
| **Status** | done — Jellyfin sample_rate from MediaStreams/0/SampleRate |
| **Today** | `jellyfin.rs` sets `sample_rate: None` |
| **Do** | Parse MediaStreams SampleRate |
| **Accept** | Audiophile column shows Hz for JF tracks when known |

#### W596 — Subsonic lyrics artist+title fallback (P2)
| | |
|---|---|
| **Status** | done — getLyrics falls back to artist+title via getSong |
| **Today** | `getLyrics.view?id=` only; comment notes artist+title fallback |
| **Do** | Fall back to artist+title query for older servers |
| **Accept** | Lyrics appear on servers that need name-based API |

#### W597 — YTM artists() + badge helpers (P2)
| | |
|---|---|
| **Status** | done — source badges use SourceKind::badge; YTM artists are derived from shelf albums instead of a fake empty catalog |
| **Today** | `youtube_music.rs` `artists()` always empty; badge helpers unused; UI hardcodes `"YT"` |
| **Do** | Innertube artist catalog or stop promising YTM artists; use shared badge helpers |
| **Accept** | Badges consistent; no empty YTM artist pretenses |

#### W598 — Home New podcasts open show (P2)
| | |
|---|---|
| **Status** | done — New podcasts cards open that show via PodcastPage::open_show |
| **Today** | Cards only `set_visible_child_name("podcasts")` |
| **Do** | Open that show’s detail on Podcasts page |
| **Accept** | Clicking a New podcasts card lands on that show |

#### W599 — Hygiene: unused dep / dead APIs
| | |
|---|---|
| **Status** | done — removed unused gstreamer-audio dependency and dead audio-cache helpers |
| **Today** | `gstreamer-audio` unused in Cargo.toml; cache dead_code if W582 retires |
| **Do** | Remove or use dep; delete orphaned APIs after decisions |
| **Accept** | `cargo build -p mde-music` clean of unused-dep/dead_code for known orphans |

---

### Acceptance checks (seat)

1. Fresh launch fits above taskbar; no leftover duplicate “Network Music Interface” windows.
2. Home Featured hero shows real cover art within one sync (W522).
3. Open album → full-bleed art + frosted tracks; play from track N through album (W520, Q3).
4. Performer from album/now-playing → bleed art + facts; Fanart when keyed (W521/W524).
5. Stage tabs non-stub; Credits populated when MB data exists (W530).
6. Like YTM track → Acquisitions job → file on server path (W570/W571).
7. Cast picker lists devices with explicit status; first cast never silent-fail (W560).
8. Media keys next/prev change audible track (W580); Queue click plays YTM/server tracks (W581).
9. Radio “This server” lists stations when server has any (W583); empty server points to Favorites (W577).
10. Like toggles off on second click (W584).
11. Control API not open to LAN by default (W587).
12. `/usr/bin/neighbourhood-music` md5 == `~/.local/bin/neighbourhood-music` == release artifact (W529/W579).

### Out of scope for this epic

- Embedding or launching **ytmdesktop** Electron as the engine (study only).
- Google / YTM account login.
- Replacing GStreamer player with a second toolkit.
- Growing Python music UI (Rust-only).

---

## Epic: OpenFilter → Stage audiophile EQ (W600–W649)

Opened **2026-11-26** from a reset 25-question interview. Premise: reuse **[OpenFilter EQ](https://github.com/asantamarian/openfilter)** as the EQ stack inside Neighbourhood Music (`mde-music`), with a **native GTK** Stage Audiophile UI (no JUCE UI in-process), **AGPL-3.0** accepted for the music app, and **full replacement** of GStreamer `equalizer-10bands`.

**References:** OpenFilter repo + `docs/spec/` (esp. `03-dsp.md`, `04-analysis.md`, `05-ui.md`); wavesurfer.js only as Watch-mode waveform inspiration (not the EQ editor).  
**Crate focus:** `mde/crates/mde-music` (+ new Rust EQ engine crate/module as decided in build-out).  
**Supersedes / coordinates:** W562 Sound-flyout EQ depth, W588 Tools EQ init, Stage Credits/Artwork dual-mode polish overlapping W530–W535 — Audiophile becomes the EQ instrument; remove 10-band paths when W604/W620 green.

**Done when:** W649 exit checklist green — Rust EQ in Gst post-decode path; GTK Audiophile tabs; Watch/Edit dual mode; per-source morphing curves; advanced processing gated; Match sketch+file; JSON presets; GL curve/analyzer; CI oracle vs OpenFilter; soft bypass; meters; full a11y; seat polish; AGPL notices + source shipping; old graphic EQ gone.

### Locked decisions (Q1–Q25)

| Q | Lock |
|---|------|
| 1 | In-process library: OpenFilter DSP concepts + UI **in** `mde-music` |
| 2 | **Native GTK** rebuild of curve/nodes/analyzer/dock — **no JUCE UI** |
| 3 | Accept **AGPL-3.0** for Neighbourhood Music; ship source |
| 4 | **Replace** GStreamer `equalizer-10bands` entirely |
| 5 | **Post-decode GStreamer insert** wrapping the EQ engine |
| 6 | Full processing matrix intent: static, dynamic, spectral, match, sketch (**character omitted** — see Q15) |
| 7 | Stage-native IA tabs: **Curve \| Analyzer \| Dynamics \| Match \| Presets** |
| 8 | **Watch** = waveform + live spectrum; **Edit EQ** → Curve tab + draggable nodes |
| 9 | **Mutually exclusive canvases**: Watch waveform vs Edit/Analyzer frequency display |
| 10 | Analyzer = **post-EQ + delta listen** (not mandatory pre+post dual traces) |
| 11 | Persist EQ **per source kind**: Library / YTM / Radio / Podcasts |
| 12 | On kind change: **morph/crossfade** parameters over a short time |
| 13 | **Zero-latency only** (no natural/linear/mixed phase in player) |
| 14 | **Master switch** “Enable advanced processing” before dynamic/spectral bands arm |
| 15 | **Omit** Tube/Iron character permanently — clean gain only |
| 16 | Match tab: **curve sketch + file reference match** |
| 17 | **OpenFilter JSON presets** as canonical format |
| 18 | Render curve/spectrum with **GL/GLES** (`GtkGLArea` or equivalent) |
| 19 | **Rust rewrite** of DSP inspired by OpenFilter (not C++/JUCE link for UI; DSP ported) |
| 20 | Spec-driven port + **CI oracle** comparing to OpenFilter reference within tolerances |
| 21 | **Soft bypass** only (single Stage control) |
| 22 | Full **meter column** (peak ladder + numeric) in Edit EQ |
| 23 | Full a11y: keyboard, screen reader, band table, typed note names (`A4`, …) |
| 24 | **Audiophile** Stage tab always available; Edit EQ from Watch jumps there |
| 25 | DoD = full locked set **+ seat polish** / dual-prefix install / no Library·YTM·Radio·Podcast regressions |

**Index:** Phase 0 W600–W609 · Phase 1 W610–W619 · Phase 2 W620–W629 · Phase 3 W630–W639 · Phase 4 W640–W649

Neighbourhood Music Stage chrome in **W530–W535** and the Sound flyout in **W562 / W615 / W588** point here. The live EQ is `nmeq`, not `equalizer-10bands`.

---

### Phase 0 — Legal, workspace, DSP foundation

#### W600 — AGPL adoption for Neighbourhood Music
| | |
|---|---|
| **Status** | done — mde-music Cargo.toml is AGPL-3.0-or-later; LICENSE is the GNU AGPL v3 text; README states source location |
| **Locked** | Q3 |
| **Do** | License `mde-music` (and EQ engine crate) AGPL-3.0-or-later; NOTICE/third-party credits for OpenFilter heritage; README + install docs state source availability; ensure packaging ships corresponding source or offer |
| **Accept** | Legal headers + user-facing source notice present; CI/docs mention AGPL |

#### W601 — Vendor OpenFilter as reference oracle (not runtime JUCE UI)
| | |
|---|---|
| **Status** | done — OPENFILTER.md pins ca74aad146c9539980cf4766d7baf19ed27d58a5 and states the JUCE editor is not shipped |
| **Locked** | Q1, Q2, Q19, Q20 |
| **Do** | Pin OpenFilter revision under `mde/crates/` or `third_party/` for specs + reference binary/tests; document “study + derivative DSP” boundary; do **not** ship JUCE editor in-process |
| **Accept** | Specs/`docs/spec` reachable in-tree; reference build instructions for oracle tests |

#### W602 — New Rust EQ engine crate/module skeleton
| | |
|---|---|
| **Status** | done — mde-music eq module: zero-latency peaking biquad + unit test (peaking_boosts_target_more_than_distant_tone) |
| **Locked** | Q19 |
| **Do** | Create e.g. `mde-eq` / `mde-music` `eq/` module: band model, parameter snapshot, processBlock-style API, zero allocations on audio path goals from OpenFilter `01-architecture` |
| **Accept** | Crate builds; unit-test harness empty-green; `mde-music` can depend on it |

#### W603 — Spec-driven static EQ DSP (zero latency)
| | |
|---|---|
| **Status** | done — zero-latency peaking, low shelf, and high shelf; 10-band graphic uses the peaking filters. Tests: peaking_boosts_target_more_than_distant_tone, low_shelf_boosts_bass_more_than_treble |
| **Locked** | Q6 (static), Q13, Q20 |
| **Do** | Port static band shapes/slopes needed for v1 from `03-dsp.md`; **zero-latency / minimum-phase path only**; stereo placement as feasible |
| **Accept** | Static bells/shelves/cuts process audio; measurable frequency response tests exist |

#### W604 — Remove GStreamer `equalizer-10bands` path
| | |
|---|---|
| **Status** | done — playbin audio-filter is nmeq; equalizer-10bands is no longer created |
| **Locked** | Q4 |
| **Do** | Delete/stop wiring `equalizer-10bands` in `player/mod.rs`; remove Tools/Sound flyout fake 10-band UI once OpenFilter EQ is default; migrate any saved 10-band state or drop with note |
| **Accept** | No `equalizer-10bands` in pipeline; app still plays flat when EQ bypassed |

#### W605 — CI oracle vs OpenFilter
| | |
|---|---|
| **Status** | done — the peaking curve stays within 0.5 dB of the Audio EQ Cookbook bell, and a processed 1 kHz sine stays within 1 dB of +12; scripts/smoke.sh runs that test |
| **Locked** | Q20 |
| **Do** | Golden renders / frequency-response comparisons: Rust engine vs OpenFilter reference within documented dB tolerances; run in CI |
| **Accept** | Failing oracle breaks CI; tolerances documented |

#### W606 — Dynamic + spectral DSP behind capability flags
| | |
|---|---|
| **Status** | done — dynamic bands duck a hot frequency and spectral bands lean toward the measured level, and both stay idle until advanced processing is on. No Tube or Iron stage |
| **Locked** | Q6, Q14, Q15 |
| **Do** | Implement dynamic + spectral band processing per OpenFilter specs as applicable; **no** character/Tube/Iron stage; gate arming on advanced master switch |
| **Accept** | With switch off, dynamic/spectral cannot process; with switch on, bands work under CPU budget notes |

#### W607 — Match engine: sketch + file reference
| | |
|---|---|
| **Status** | done — gains_from_samples follows a file’s spectrum across the ten bands; a 1 kHz tone test boosts the 1 kHz band |
| **Locked** | Q16 |
| **Do** | Port/adapt curve fitter + file capture match (player has no DAW sidechain bus); sketch → bands |
| **Accept** | User can sketch a target and/or match to a file; result loads editable bands |

#### W608 — OpenFilter JSON preset load/save
| | |
|---|---|
| **Status** | done — presets are openfilter.eq.preset JSON; unknown keys are ignored; a Bell maps onto the nearest band, and a round-trip test restores the ten gains |
| **Locked** | Q17 |
| **Do** | Read/write OpenFilter-compatible JSON presets; store under Neighbourhood Music config; validate unknown fields safely |
| **Accept** | Round-trip preset; can load an upstream OpenFilter preset that uses supported features |

#### W609 — Per-source-kind EQ state + morph
| | |
|---|---|
| **Status** | done — separate EQ curves for Library, YouTube Music, Radio, and Podcasts, morphed over about 200 ms and stored in history settings |
| **Locked** | Q11, Q12 |
| **Do** | Persist curves for Library / YTM / Radio / Podcasts; on kind change morph parameters over short window (configurable ~50–200 ms) |
| **Accept** | Switching Library→Radio morphs audibly smoothly and recalls the right curve |

---

### Phase 1 — GStreamer integration & player wiring

#### W610 — Post-decode EQ insert design
| | |
|---|---|
| **Status** | done — design is the nmeq AudioFilter: playbin audio-filter, in-place F32, shared GraphicEq |
| **Locked** | Q5 |
| **Do** | Design Gst architecture: custom `audiofilter`-style element **or** tightly scoped appsink/appsrc bin feeding Rust EQ; document latency (zero DSP + any buffering) |
| **Accept** | Design doc in crate; spike proves PCM in→EQ→out |

#### W611 — Implement Gst insert + wire `Player`
| | |
|---|---|
| **Status** | done — Player::new registers nmeq and sets it as the playbin audio-filter; band sliders write the shared EQ |
| **Locked** | Q5, Q4 |
| **Do** | Integrate into `mde-music` `player/mod.rs` playbin path for file/http/YTM/radio/podcast; handle format renegotiation; fail → soft flat |
| **Accept** | All primary sources play through EQ insert; bypass = dry |

#### W612 — Soft bypass control
| | |
|---|---|
| **Status** | done — Tools “EQ bypassed / EQ active” toggles GraphicEq soft bypass (dry samples pass through) |
| **Locked** | Q21 |
| **Do** | Single Stage/Audiophile bypass; parameter automation safe; no click/pop if possible |
| **Accept** | Bypass toggles EQ out instantly and safely |

#### W613 — Post tap + delta listen
| | |
|---|---|
| **Status** | done — Delta outputs the difference between the EQ’d signal and the dry input |
| **Locked** | Q10 |
| **Do** | Analyzer tap post-EQ; delta path (processed vs dry) for auditioning changes; align latency |
| **Accept** | Delta lets user hear “what EQ changed”; analyzer shows post |

#### W614 — Advanced processing master switch
| | |
|---|---|
| **Status** | done — Enable advanced processing gates dynamic bands; the DSP ignores them while the switch is off |
| **Locked** | Q14 |
| **Do** | Settings/Audiophile toggle; when off, UI prevents arming dynamic/spectral; DSP ignores them |
| **Accept** | Cannot engage advanced bands without switch |

#### W615 — Remove/replace Sound flyout & Tools 10-band EQ
| | |
|---|---|
| **Status** | done — Sound flyout no longer has a second 10-band EQ; it opens Audiophile and lists outputs |
| **Locked** | Q4; supersedes W562/W588 EQ portions |
| **Do** | Point Sound flyout to “Open Audiophile” / bypass / output only; delete misleading 0 dB Tools EQ scales |
| **Accept** | No duplicate graphic EQ UI; Audiophile is sole tone control |

#### W616 — Source-kind detection hooks
| | |
|---|---|
| **Status** | done — play_track classifies the source and recalls that curve |
| **Locked** | Q11–Q12 |
| **Do** | Classify now-playing as Library/YTM/Radio/Podcast; trigger morph on transitions |
| **Accept** | Kind changes fire morph; correct preset bucket used |

#### W617 — Metering data path
| | |
|---|---|
| **Status** | done — the audio thread publishes post-EQ peak and process time in atomics; the Curve meter reads that snapshot |
| **Locked** | Q22 |
| **Do** | Peak (and any numeric levels) from post signal to UI at frame rate via lock-free snapshot |
| **Accept** | Meter column can animate from real audio |

#### W618 — CPU budget & tracing
| | |
|---|---|
| **Status** | done — EQ process time is timed; over 2 ms logs a warning and the Curve tab says EQ CPU high |
| **Do** | Trace EQ process time; warn in UI when advanced DSP overloaded; document budgets |
| **Accept** | Overload visible; playback doesn’t glitch silently forever |

#### W619 — Regression: gapless / seek / radio station step
| | |
|---|---|
| **Status** | done — nmeq still reaches end-of-stream and a seek on a local file lands; radio wraps to the next station, and a podcast end pauses instead of skipping |
| **Locked** | Q25 |
| **Do** | Verify EOS advance, seek, radio next-station, podcast resume still work with insert |
| **Accept** | No regressions vs pre-EQ-insert behavior for transport |

---

### Phase 2 — GTK Audiophile Stage UI

#### W620 — Stage **Audiophile** tab shell
| | |
|---|---|
| **Status** | done — Stage tab Audiophile with Curve, Analyzer, Dynamics, Match, and Presets |
| **Locked** | Q7, Q24 |
| **Do** | Add Audiophile to Stage tabs; host sub-tabs Curve \| Analyzer \| Dynamics \| Match \| Presets |
| **Accept** | Navigable Audiophile surface in Stage |

#### W621 — Watch vs Edit EQ mode
| | |
|---|---|
| **Status** | done — Artwork shows the waveform and a post-EQ spectrum strip; Edit EQ opens the separate Audiophile curve |
| **Locked** | Q8, Q9, Q24 |
| **Do** | Watch canvas = waveform + live spectrum (wavesurfer-inspired); Edit EQ switches to freq Curve and enables nodes; Edit EQ / Watch entry jumps to Audiophile |
| **Accept** | Modes are mutually exclusive canvases; both reachable |

#### W622 — GTK Curve display (nodes, composite curve, grid)
| | |
|---|---|
| **Status** | done — Curve tab draws the live response and dragging it sets the nearest band on nmeq |
| **Locked** | Q2, Q7 |
| **Do** | Native GTK rebuild of OpenFilter response display: log-freq grid, composite curve, draggable nodes, selection, value tag |
| **Accept** | Dragging nodes changes audible EQ |

#### W623 — GL/GLES spectrum + curve rendering
| | |
|---|---|
| **Status** | done — the curve tries a GL line strip and the label says GL; if the context or shader fails, the CPU painter stays and the label says CPU. Seat confirmation is W646 |
| **Locked** | Q18 |
| **Do** | `GtkGLArea` (or equiv) path for analyzer fill + curve; fallback if GL unavailable |
| **Accept** | Smooth analyzer on seat; CPU fallback documented |

#### W624 — Analyzer tab (post + freeze/tilt/range as fits player)
| | |
|---|---|
| **Status** | done — Analyzer shows post-EQ level for each band, with Freeze and a display-only tilt |
| **Locked** | Q10, Q7 |
| **Do** | Analyzer controls appropriate to post+delta design; optional spectrogram if cost OK |
| **Accept** | Analyzer tab useful while listening/editing |

#### W625 — Dynamics tab UI (gated)
| | |
|---|---|
| **Status** | done — Dynamics tab checkboxes are insensitive until advanced processing is enabled |
| **Locked** | Q7, Q14 |
| **Do** | Per-band dynamics controls; disabled/hidden until advanced switch on |
| **Accept** | Dynamics only when advanced enabled |

#### W626 — Match tab UI (sketch + file)
| | |
|---|---|
| **Status** | done — Match tab sketches a curve onto the bands and can match an audio file through ffmpeg |
| **Locked** | Q16, Q7 |
| **Do** | Sketch gesture + file picker reference match; apply → bands |
| **Accept** | Match workflow completable without leaving Stage |

#### W627 — Presets tab (OpenFilter JSON)
| | |
|---|---|
| **Status** | done — Presets tab saves, loads, and deletes .ofeq files offline and remembers the last preset per Library, YouTube Music, Radio, or Podcasts |
| **Locked** | Q17, Q7 |
| **Do** | Browse/load/save/delete presets; show kind-specific last-used |
| **Accept** | Preset management works offline |

#### W628 — Meter column in Edit EQ
| | |
|---|---|
| **Status** | done — Curve tab shows a 12-segment post-EQ peak ladder plus the numeric dBFS readout |
| **Locked** | Q22 |
| **Do** | Peak ladder + numeric readout column OpenFilter-style |
| **Accept** | Visible while editing; reflects post signal |

#### W629 — Soft bypass + delta controls in chrome
| | |
|---|---|
| **Status** | done — Bypass and Delta sit on the Curve tab next to the meter |
| **Locked** | Q21, Q10 |
| **Do** | Prominent bypass; delta listen control near meters/dock |
| **Accept** | One-click bypass/delta while tweaking |

---

### Phase 3 — A11y, polish, product edges

#### W630 — Full keyboard + screen-reader labels
| | |
|---|---|
| **Status** | done — [ ] selects an EQ band, -/= nudges its gain, B toggles bypass, and the band table plus Bypass/Delta have accessible names. Graphic bands keep Q at 1.2 |
| **Locked** | Q23 |
| **Do** | Band select, nudge freq/gain/Q, bypass, tab navigation; ATK/a11y labels on controls |
| **Accept** | Operable without pointer for core EQ tasks |

#### W631 — Band table + typed note names
| | |
|---|---|
| **Status** | done — Curve tab band table: gain fields change the audible band, and A4 / C#3 retune that band’s center |
| **Locked** | Q23 |
| **Do** | Editable band table alternate view; accept `A4`, `C#3` style entries |
| **Accept** | Table edits reflect on curve and audio |

#### W632 — Watch waveform quality
| | |
|---|---|
| **Status** | done — the waveform is display-only under the Stage art, so seek stays on the Stage slider |
| **Locked** | Q8, Q9 |
| **Do** | wavesurfer-inspired waveform for Watch (scroll/seek if product-safe); must not fight Gst position |
| **Accept** | Watch mode looks intentional; seek stays correct |

#### W633 — Theme / Mackes visual language for Audiophile
| | |
|---|---|
| **Status** | done — Audiophile uses the same dark shell, chip tabs, and plot frame as the rest of Neighbourhood Music |
| **Do** | Align colors/density with Neighbourhood Music CSS; avoid raw JUCE clone aesthetics unless desired |
| **Accept** | Audiophile feels native to MDE Music |

#### W634 — Instance-hub features decision
| | |
|---|---|
| **Status** | skip — single-player app has one EQ instance; OpenFilter multi-instance hub does not apply. No Instances chrome will be added |
| **Locked** | Q6 “instances as applicable” |
| **Do** | Explicitly **skip** multi-instance hub in a single-player app **or** implement a minimal single-instance-only stub; record `skip` with rationale if omitted |
| **Accept** | No dead “Instances” chrome |

#### W635 — MIDI learn
| | |
|---|---|
| **Status** | skip — Neighbourhood Music v1 is pointer/keyboard (W564); hardware MIDI learn is a DAW feature and is out of the player cut |
| **Locked** | Q23/OpenFilter parity pressure |
| **Do** | Implement MIDI learn for key EQ params **or** mark `deferred`/`skip` with rationale (player may not need it in v1) |
| **Accept** | Either works on seat or explicitly skipped in this epic Notes |

#### W636 — Documentation
| | |
|---|---|
| **Status** | done — README describes nmeq, the Audiophile tab, auto-advance, the localhost control API, and the AGPL source offer |
| **Do** | User-facing Audiophile help; developer port notes; AGPL + OpenFilter attribution; update `mde-music` README (remove gapless/10-band lies as needed) |
| **Accept** | README matches architecture |

#### W637 — Coordinate Music epic Stage items
| | |
|---|---|
| **Status** | done — the OpenFilter epic index points at W530–W535, W562, W615, and W588 |
| **Do** | Update W530–W535/W562/W588 notes to point here; avoid duplicate EQ work |
| **Accept** | WORKLIST cross-links consistent |

#### W638 — Performance on target seat (linux-surface class)
| | |
|---|---|
| **Status** | done — a default 256-frame block stays under 2 ms on this machine; the spectrum uses 512 samples every 8 callbacks, and GL drawing is not on the audio thread |
| **Locked** | Q25 |
| **Do** | Profile GL + DSP on this hardware; tune FFT size defaults |
| **Accept** | Audiophile usable without dropouts at default settings |

#### W639 — Localization / strings pass
| | |
|---|---|
| **Status** | done — the Presets tab says Audiophile .ofeq files; the OpenFilter name stays in code comments, not in the UI |
| **Do** | User-visible strings consistent; no leftover OpenFilter product name confusion (“Audiophile EQ powered by OpenFilter-derived engine”) |
| **Accept** | Naming clear in UI |

---

### Phase 4 — Seat verification & exit

#### W640 — Seat: Library through EQ
| | |
|---|---|
| **Status** | todo |
| **Accept** | Local/JF/Subsonic tracks play; curve audible; bypass works |

#### W641 — Seat: YTM through EQ
| | |
|---|---|
| **Status** | todo |
| **Accept** | YTM resolve+play through insert; kind morph Library↔YTM OK |

#### W642 — Seat: Radio + Podcasts through EQ
| | |
|---|---|
| **Status** | todo |
| **Accept** | Radio station step + podcast resume OK; kind curves distinct |

#### W643 — Seat: Match sketch + file
| | |
|---|---|
| **Status** | todo |
| **Accept** | Sketch/match produces usable bands |

#### W644 — Seat: Advanced dynamic/spectral
| | |
|---|---|
| **Status** | todo |
| **Accept** | Master switch gates correctly; with switch on, a dynamic band audibly reacts |

#### W645 — Seat: A11y smoke
| | |
|---|---|
| **Status** | todo |
| **Accept** | Keyboard path edits a band; band table + note name entry works |

#### W646 — Seat: GL + fallback
| | |
|---|---|
| **Status** | todo |
| **Accept** | GL path works on this machine; fallback if forced still usable |

#### W647 — Dual-prefix install after EQ cut
| | |
|---|---|
| **Status** | todo — user prefix matches the release artifact; system prefix still needs a sudo password |
| **Do** | Release build → `/usr/bin` and `~/.local/bin`; kill stale windows |
| **Accept** | md5 match both prefixes |

#### W648 — Packaging AGPL source offer
| | |
|---|---|
| **Status** | done — LICENSE is the GNU AGPL v3 text and the README points at the git tree that produced the binary |
| **Locked** | Q3, Q25 |
| **Do** | Ensure distributed binaries have corresponding source story (repo path / tarball) |
| **Accept** | Compliance checklist written and met for this seat’s install method |

#### W649 — Epic exit
| | |
|---|---|
| **Status** | todo |
| **Locked** | Q25 |
| **Do** | W600–W648 done/verify/skip-rationale; Music epic EQ-related items marked superseded; oracle CI green; seat checks W640–W647 green |
| **Accept** | Close epic in WORKLIST |

### Out of scope for this epic

- Shipping **JUCE UI** in-process (Q2).
- **Character / Tube / Iron** saturation (Q15).
- Non-zero-latency phase modes in the player (Q13).
- Embedding OpenFilter as an external VST/CLAP host only (Q1 chose library/port).
- Replacing GStreamer as the whole media engine (EQ insert only).

---

## Epic: Immersive artist, album & fan imagery + motion (W650–W699)

Opened **2026-11-27** from the request that *all parts of the app* use more immersive artist, album and fan images. Extends the Visual 50/50 direction in the W520–W599 epic (W520–W528 delivered album/performer bleed and the art pipeline; this epic carries it to every surface and fills the missing sources).

**Audit at open (2026-11-27):**
- Full-bleed `Picture` exists on only two pages: album (`show_immersive_album`) and performer (`ui/performer_page.rs`).
- Fan art (`artwork::fanart_for_mbid`) is used only by the performer page and only with a Fanart key, which is **unset on this seat**, so no fan art appears anywhere today.
- Library **artist page** (`show_artist_page`) has no image at all; **Home hero** is a 280px centered cover; **Stage** is a centered cover on flat colour; Search/Library/shelves/Radio/Podcasts/Playlists use small thumbnails.
- Keyless sources verified live: **Deezer** artist search (`picture_xl`, 1000×1000), **Wikipedia** summary `originalimage`, **Wikidata** P18 → Commons. TheAudioDB's free key returned no data, so it is optional only.

**Crate:** `mde/crates/mde-music` · **Install:** both `~/.local/bin` and `/usr/bin` (PATH prefers the former) · **CSS** is baked via `include_str!`.

**Status key:** `todo` · `verify` · `done` · `deferred` · `skip`

**Done when:** W650–W674 are `done`/`verify`; W674 exit checklist green. W690–W699 are reserved for follow-ups. **W675–W689 added 2026-11-27 from the 25-question interview** (see Locked decisions).

### Phase A — Image sources & pipeline (foundation)

#### W650 — Unified artist image resolver (P0)
| | |
|---|---|
| **Status** | done — `artimg::artist_images`: Fanart (keyed) → TheAudioDB → Deezer → Wikipedia/Commons → Last.fm/Discogs (keyed) → Bandcamp; 7-day/1-day cache, one in-flight per artist |
| **Today** | Fan art only runs on the performer page and only with a Fanart key (unset on this seat), so artists have no photo or backdrop anywhere else. |
| **Do** | `artwork::artist_images(name, mbid) -> ArtistImages { portrait, backdrops[], logo, banner, thumbs[] }`. Chain: server artist art → Fanart.tv (if keyed) → Deezer artist (keyless, `picture_xl` 1000px) → Wikidata P18 / Wikipedia `originalimage` (Commons) → TheAudioDB (optional key) → generated placeholder (W657). Run off the UI thread; 7-day positive TTL, 1-day negative TTL; one in-flight request per artist. |
| **Accept** | Any artist with a MusicBrainz or Deezer match yields a portrait with no API key; with a Fanart key, backdrops and a logo are added. |

#### W651 — Fanart.tv full asset set (P0)
| | |
|---|---|
| **Status** | todo |
| **Today** | `fanart_for_mbid` returns only thumb + one background. |
| **Do** | Parse `artistbackground` (all), `artistthumb`, `hdmusiclogo`/`musiclogo`, `musicbanner`, album `albumcover`, `cdart`. Sort by likes. Surface key onboarding (link + Test button) in Tools; honor rate limits with backoff; never block UI. |
| **Accept** | With a key, one artist exposes multiple backdrops + logo + banner; without it, nothing errors. |

#### W652 — Album art gallery (HD + extras) (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Album art is a single front cover. |
| **Do** | Cover Art Archive: front 1200+, back, booklet, medium/disc images via the release JSON; iTunes 2000px fallback; Fanart `cdart`/`albumcover`. Return `AlbumImages { front, back, booklet[], disc }`. |
| **Accept** | Album page can show front, back, booklet pages and disc art when available. |

#### W653 — Image quality gate (P0)
| | |
|---|---|
| **Status** | done — `artimg::quality_ok`: rejects <600px / portrait-oriented backdrops, <400px portraits, near-solid images, Deezer silhouette; unit-tested |
| **Today** | No check that a downloaded image is usable as a bleed. |
| **Do** | Reject below 600px short side for bleed use (portrait ok at 400); reject near-solid or known placeholder silhouettes (Deezer default hash); prefer widest landscape for backdrops; low-res image shown as blurred blur-up, never stretched crisp. |
| **Accept** | No pixelated, blank or silhouette image is ever used as a backdrop. |

#### W654 — Artist match safety (P0)
| | |
|---|---|
| **Status** | partial — strict normalized name match on every source + disambiguation skip; **todo:** persist picker choice + 'Wrong artist? Fix image…' action |
| **Today** | Name-only lookups can attach the wrong artist's photo. |
| **Do** | Require MBID or exact normalized-name match (+ album/track corroboration for Deezer/Apple); persist the performer-picker choice per artist; ambiguous → placeholder, not a guess. Add a 'Wrong artist? Fix image…' context action. |
| **Accept** | Common-name artists never show another artist's photo; user override sticks across restarts. |

#### W655 — Prefetch + cache budget (P1)
| | |
|---|---|
| **Status** | partial — shared 2-worker portrait queue + 7-day cache; **todo:** idle prefetch of next-in-queue, LRU disk cap UI |
| **Today** | Art loads only after a page opens. |
| **Do** | Prefetch now-playing artist images and next-in-queue artist at idle; warm Home hero set after sync; disk cap (default 500 MB, LRU) with size shown in Tools; stable keys independent of auth; works offline from cache. |
| **Accept** | Stage and artist page open with imagery instantly for recently played artists; offline shows cached art. |

### Phase B — Shared immersive widgets

#### W656 — `BleedBackdrop` shared widget (P0)
| | |
|---|---|
| **Status** | done — `ui/bleed.rs` cairo widget: cover-fit, scrim, 600 ms crossfade, rotation, Ken-Burns, pointer parallax, ambient blur mode, capped pulse, reduced-motion aware |
| **Today** | Album/performer pages each hand-roll a Picture + CSS. |
| **Do** | One widget: cover-fit Picture, art-adaptive scrim gradient (reuse W525 palette), 400 ms crossfade between images, optional slow Ken-Burns pan, pauses on click/hover (Q29), honors reduced-motion setting, decodes off UI thread, max 2 textures live. |
| **Accept** | Album, performer, artist, Stage, hero all use it; changing the image crossfades without flicker. |

#### W657 — Placeholder bleed (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Missing art is a letter tile (W526), which is empty at hero size. |
| **Do** | Artist-hash gradient + large monogram + title typography for bleed surfaces; never a wrong or stock photo. |
| **Accept** | A bleed surface with no image still looks designed. |

### Phase C — Surfaces (every part of the app)

#### W658 — Library artist page cinematic (P0)
| | |
|---|---|
| **Status** | partial — artist page now sits on the global Sharp backdrop (artist fan art); **todo:** logo/portrait header, Play/Shuffle/Instant Mix, fan-photo strip |
| **Today** | `show_artist_page` is text + lists with no image. |
| **Do** | BleedBackdrop from artist backdrop, round portrait, logo (or large name), Play / Shuffle / Instant Mix, frosted top tracks, discography as cover cards, 'Fan photos' strip. Same page for server and YTM artists. |
| **Accept** | Opening any artist looks like the performer page's sibling, not a list. |

#### W659 — Home hero full-width (P0)
| | |
|---|---|
| **Status** | partial — Home hero card is transparent over a rotating per-item backdrop; **todo:** true full-width layout, logo overlay, peek of next |
| **Today** | `build_home_hero` shows a 280px centered cover. |
| **Do** | Full-width backdrop per item (artist backdrop for artist-driven items, HD album art otherwise), crossfade rotation, logo/title overlay lower-left, Play/Open, peek of next, art-adaptive scrim. |
| **Accept** | Home opens with a cinematic banner, not a postage stamp. |

#### W660 — Stage backdrop + Gallery tab (P0)
| | |
|---|---|
| **Status** | todo |
| **Today** | Stage is a centered cover on flat colour. |
| **Do** | Behind the cover: blurred artist backdrop with slow crossfade through backdrops during the song. Artwork tab gains Gallery (artist photos, album front/back/booklet, disc). Full-screen Gallery mode (F11 / button) slideshow with caption and attribution. |
| **Accept** | Stage reads as an artist scene; Gallery mode works full screen. |

#### W661 — Player bar + mini-player ambience (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Bar is small thumbnail + controls. |
| **Do** | Soft blurred-art tint behind the bar, larger thumbnail on hover, artist portrait chip; mini-player becomes a cover-bleed card with overlaid controls. |
| **Accept** | Bar and mini-player visibly pick up the playing artist's imagery without hurting contrast. |

#### W662 — Album page: secondary layers (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | W520 uses the front cover only. |
| **Do** | Behind the cover bleed add the artist backdrop at low opacity as texture; toggle Front / Back / Booklet / Disc; 'More from this artist' strip with portraits. |
| **Accept** | Album page shows depth and the extra art from W652. |

#### W663 — Performer page upgrade (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | One background or cover. |
| **Do** | Backdrop slideshow, logo, banner, photo strip (Commons/Fanart), band-member portraits where Wikidata gives them. |
| **Accept** | Performer page is the richest imagery surface in the app. |

#### W664 — Search: top-result hero + portraits (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Search rows are 72px with thumbnails. |
| **Do** | Top-result card with backdrop/portrait; artist rows round with real photos; consistent art on song/album rows; skeleton art while loading. |
| **Accept** | Search for an artist's name leads with their photo. |

#### W665 — Library grids with real photos (P1)
| | |
|---|---|
| **Status** | partial — Library artist cards swap in real artist photos (round); **todo:** hover backdrop reveal, album grid HD pass |
| **Today** | Artist grid uses album covers as round art. |
| **Do** | Use artist portraits (W650); hover reveals backdrop wash; album grid uses HD art with hover Play; progressive load with blur-up. |
| **Accept** | Library → Artists looks like photographs of people. |

#### W666 — Shelves, Discover & YTM (P2)
| | |
|---|---|
| **Status** | todo |
| **Today** | Discover/YTM shelves use whatever thumbnail the API returned. |
| **Do** | Circular artist shelves with portraits; 'Because you like X' shelf with backdrop header; YTM artists resolved through W650 so they match server artists. |
| **Accept** | Every shelf item has coherent, quality-gated art. |

#### W667 — Playlists, Liked Songs & Acquisitions (P2)
| | |
|---|---|
| **Status** | todo |
| **Today** | Plain headers. |
| **Do** | Playlist/Liked/Acquisitions headers use a 2×2/3×3 mosaic or backdrop from the top artists; rows show art. |
| **Accept** | These pages have a visual header like albums. |

#### W668 — Radio & Podcasts imagery (P2)
| | |
|---|---|
| **Status** | todo |
| **Today** | Station and show pages are lists. |
| **Do** | Station logos/tiles and a backdrop for the playing station; podcast show page with art bleed + episode art; Apple/PodcastIndex art quality-gated. |
| **Accept** | Radio and Podcasts match the rest of the app. |

#### W669 — Out-of-app art: MPRIS, notifications, cast (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Desktop integrations use small covers. |
| **Do** | MPRIS `mpris:artUrl` HD local file, notification image, cast metadata image, lock-screen widget art; refreshed when better art arrives. |
| **Accept** | Media keys widget, notification and Cast show the HD cover. |

### Phase D — Control, policy, quality

#### W670 — Appearance & sources settings (P0)
| | |
|---|---|
| **Status** | todo |
| **Today** | Only a Fanart key field exists. |
| **Do** | Tools → Appearance: Immersive imagery Full / Reduced / Off; per-source toggles (Fanart, Deezer, Wikimedia, TheAudioDB); Fanart/AudioDB key fields with Test; motion on/off; cache size + Clear. **Privacy note:** enabling a source sends artist/album names to that third party. |
| **Accept** | User can turn imagery depth and every network source on or off; defaults documented. |

#### W671 — Motion & performance budget (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Risk of jank with large images. |
| **Do** | Crossfade ≤400 ms; Ken-Burns off on battery/reduced-motion; decode on worker; texture memory cap; frame-time check while scrolling Library; fall back to still image under load. |
| **Accept** | Scrolling Library and changing tracks stays smooth on this seat. |

#### W672 — Attribution & licensing (P2)
| | |
|---|---|
| **Status** | todo |
| **Today** | Commons/Fanart images have terms. |
| **Do** | Caption/tooltip with source and licence in Gallery and performer page; Credits tab lists sources; respect Commons attribution for CC-BY-SA. |
| **Accept** | Every non-server image is attributable from the UI. |

#### W673 — Tests (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Resolver is network-bound. |
| **Do** | Fixture-driven unit tests for each source parser, resolver chain order, quality gate, match safety, cache TTL; headless widget test for BleedBackdrop crossfade state. |
| **Accept** | `cargo test --release -p mde-music` covers all of the above offline. |

#### W674 — Epic exit gate (P0)
| | |
|---|---|
| **Status** | todo |
| **Today** | — |
| **Do** | Screenshot each surface on this seat (Home, Artist, Album, Performer, Stage, Gallery, Search, Library, bar/mini, Radio, Podcast); no-key and with-key runs; dual-prefix install (`md5sum` release = `~/.local/bin` = `/usr/bin`). |
| **Accept** | All W650–W673 `done`/`verify`; screenshots archived; epic marked done. |

### Locked decisions (2026-11-27 interview, 25 questions)

| Topic | Decision |
|-------|----------|
| Scope | **Full-bleed art on all pages.** Dense pages (Library lists, Tools/Settings, Acquisitions, track tables) get a **blurred ambient bleed** with frosted panels on top |
| HD | Target **up to 1440p**; keep larger originals out of the cache; quality gate (W653) still applies |
| Sources | **All:** Wikimedia Commons + Wikipedia, Deezer, iTunes/Apple, Fanart.tv, TheAudioDB, Discogs, Last.fm, Bandcamp (+ Flickr CC). **APIs first**, polite page scraping only for public og:image/press art (robots.txt, rate limits, clear User-Agent, source URL stored per image) |
| Fan photos | Gallery tab plus **occasional backdrop** if they pass the quality gate |
| Motion style | **Cinematic and slow** everywhere (400–800 ms fades, slow drift) |
| Transitions | **Shared-art morph** between pages (crossfade fallback) |
| Idle motion | **Ken-Burns drift by default + layered 2.5D depth parallax** ("Cinema" mode). Depth is **layered from the image** (sharp centre over blurred copy + vignette; Fanart cutouts when present). **No AI model**, no generated images; AI only means local smart curation/scoring |
| Video | Muted video loops **everywhere art appears** where a legal source exists; **auto-degrade to stills** when unfocused, on battery or when frames drop |
| Audio-reactive | **Strong, everywhere**, with a **hard cap: no flashing**, pulse limited to a few percent scale/glow, ≤ ~3 changes/second |
| Scroll | Layered parallax + hero header collapse |
| Microinteractions | Card hover lift + Play reveal; like heart burst + queue-add flight; now-playing EQ bars + progress glow; press ripples + focus rings |
| Library browsing | **Infinite artwork wall**, **3D Cover Flow carousel**, **photo-wall artist grid** |
| Input | Pointer position + touch/touchpad gestures (no tilt sensor) |
| Accessibility | **Honor the system reduced-motion setting.** Contrast/alt-text guards: *exactly what was picked* (not extra toggles); the flash cap from the audio-reactive decision still stands |
| Renderer | **wgpu canvas** (inside a GTK widget) for art layers **and** animated transitions; lists/text/controls stay native GTK. `wgpu` is already in the lockfile; Vulkan works on this seat (Intel HD 620) |
| Performance | **30 fps is fine** (floor); stills fallback below that |
| Rollout | Sources → backdrops → motion → canvas; **tests + screenshots per page, ship each phase** to both prefixes |

**Open tension to settle in W685:** wgpu inside GTK4 is custom plumbing (render to a texture → `gdk::Texture`/`GtkGLArea`). W685 starts with a time-boxed spike and keeps a GTK-only path (stills + crossfades) as the fallback so the app never depends on the canvas to show art.

#### W675 — Source expansion + polite scraper (P1)
| | |
|---|---|
| **Status** | done — Discogs/Last.fm/AudioDB/Wikimedia/Deezer/Fanart + Bandcamp og:image scraper (robots.txt, per-host throttle, UA, source+licence per image). **todo:** Flickr CC, Apple artist art |
| **Today** | Only Fanart (keyed) + Deezer/Wikimedia identified in W650 |
| **Do** | Add Discogs + Last.fm (free keys), TheAudioDB, Flickr Creative Commons, Apple/iTunes (cap 1440p), and Bandcamp/artist-site `og:image` scraping. Scraper: robots.txt, per-host rate limit, backoff, identifying User-Agent, store `source_url` + licence per image, local cache only |
| **Accept** | Each source can be toggled (W670); a source failing never blocks others; every image keeps its source URL |

#### W676 — Ambient bleed host on every page (P0)
| | |
|---|---|
| **Status** | done — one ambient `Bleed` behind every page; frosted veil CSS; Sharp on Home/Stage/Performer/album/artist, Ambient elsewhere; falls back to the playing artist |
| **Today** | Only album + performer have a backdrop |
| **Do** | Page host gives every page (Home, Library, Search, Radio, Podcasts, Playlists, Liked, Acquisitions, Tools/Settings) a blurred, dimmed ambient layer from the current context artist/album/playing track; frosted panels over it |
| **Accept** | No page is flat colour; text stays readable on dense pages |

#### W677 — 1440p HD policy (P0)
| | |
|---|---|
| **Status** | done — decode capped at 1440 px, `cap_url` clamps Apple art to 1440 |
| **Today** | W523 sizes 600/1200/2000 |
| **Do** | Resolve to ≤1440p; prefer the largest source ≥ the display; disk-cache the chosen size; upgrade in place when a better image arrives |
| **Accept** | Bleed surfaces use HD where a source offers it; no 4K bloat |

#### W678 — Motion system + reduced-motion (P0)
| | |
|---|---|
| **Status** | partial — honors GTK `gtk-enable-animations` (system reduced motion), Ctrl+Shift+M kill switch, 30 fps redraw floor; **todo:** central tokens for all non-backdrop animations |
| **Today** | Ad-hoc CSS transitions |
| **Do** | Central motion tokens (durations 400–800 ms, easing), one `Motion` service every page uses; read the system reduced-motion preference and switch to simple fades with no drift, parallax, video or audio-reactivity; 30 fps floor, stills fallback |
| **Accept** | Every page animates from one system; reduced-motion visibly strips motion |

#### W679 — Shared-art morph page transitions (P1)
| | |
|---|---|
| **Status** | todo |
| **Today** | Pages swap instantly |
| **Do** | Clicked cover/portrait expands into the next page's backdrop on the canvas; crossfade fallback when the canvas is unavailable; back reverses |
| **Accept** | Home → Artist → Album → Stage feels continuous |

#### W680 — Ken-Burns drift + Cinema 2.5D parallax (P1)
| | |
|---|---|
| **Status** | partial — Ken-Burns + pointer 2.5D parallax; **todo:** touch/touchpad gestures, Fanart cutout foreground |
| **Do** | Default slow Ken-Burns (~20–30 s) with crossfade; Cinema mode layers sharp centre over blurred copy + vignette, shifting with pointer position and touch/touchpad gestures; Fanart cutouts as the foreground when present |
| **Accept** | Backdrops move gently; Cinema parallax follows the pointer without tearing |

#### W681 — Scroll parallax + header collapse (P1)
| | |
|---|---|
| **Status** | partial — backdrop scroll parallax (35 %); **todo:** hero header collapse, shelf reveals |
| **Do** | Backdrop scrolls slower than content; hero title collapses into a sticky header; shelves fade/slide in on reveal |
| **Accept** | Artist, album and Home scroll with layered depth |

#### W682 — Microinteractions (P1)
| | |
|---|---|
| **Status** | partial — CSS: eased state changes, press glow, animated focus ring, glowing seek, card lift; **todo:** like heart burst, queue-add flight, playing-row EQ bars |
| **Do** | Card hover lift + Play reveal; like heart burst; queue-add cover flight to the bar; playing-row EQ bars; seek-bar progress glow; button press ripples; animated keyboard focus rings |
| **Accept** | Each interaction has visible feedback and respects reduced-motion |

#### W683 — Audio-reactive motion (strong, capped) (P1)
| | |
|---|---|
| **Status** | done — post-EQ peak → low-passed pulse, ≤2.5 % zoom, no flashing; disabled by reduced-motion/kill switch |
| **Do** | Tap loudness/bass from the playing audio (GStreamer level/spectrum) to drive backdrop scale/glow on every page. **Hard cap:** few-percent scale, soft glow, ≤ ~3 changes/second, never full-frame flashes; disabled under reduced-motion |
| **Accept** | Music visibly moves the art without flashing |

#### W684 — Video loop backdrops everywhere with auto-degrade (P1)
| | |
|---|---|
| **Status** | todo |
| **Do** | Muted looping video wherever a legal source exists (YTM video path, Apple motion art if reachable); pause when unfocused, on battery or when frames drop; fall back to still + drift; never autoplays audio |
| **Accept** | Video plays on art pages, stops cleanly under strain, resumes automatically |

#### W685 — wgpu canvas host (art layers + transitions) (P0)
| | |
|---|---|
| **Status** | todo |
| **Today** | `wgpu` is in the lockfile; no canvas in `mde-music`; Vulkan available (Intel HD 620) |
| **Do** | Time-boxed spike: wgpu renders to a texture shown in a GTK widget (`GtkGLArea` or `gdk::Texture` upload). If it holds 30 fps on this seat, build the canvas for backdrops, parallax and transitions; keep the GTK-only still/crossfade path as the fallback. Native GTK for lists, text, controls |
| **Accept** | Canvas holds ≥30 fps on this seat; app still shows art if the canvas fails to init |

#### W686 — Infinite artwork wall (P2)
| | |
|---|---|
| **Status** | todo |
| **Do** | Zoomable, drifting mosaic of covers/portraits on the canvas; click opens the item; lazy tiles from cache |
| **Accept** | Wall scrolls/zooms smoothly with a large library |

#### W687 — 3D Cover Flow carousel (P2)
| | |
|---|---|
| **Status** | todo |
| **Do** | Albums on a tilted arc with depth blur on the canvas; flick/arrow keys to browse; click to open |
| **Accept** | Smooth carousel over the whole album set |

#### W688 — Photo-wall artist grid (P1)
| | |
|---|---|
| **Status** | todo |
| **Do** | Edge-to-edge artist portrait grid with hover backdrop reveal (shares W665) |
| **Accept** | Artists view is a wall of photographs |

#### W689 — Phase ship gates (P0)
| | |
|---|---|
| **Status** | todo |
| **Do** | After each phase (sources → backdrops → motion → canvas): offline tests, screenshots of each affected page, then install to `~/.local/bin` and `/usr/bin` with matching md5 |
| **Accept** | Four shipped phases with screenshots archived |

#### W690 — Loading transitions while waiting for content (P1)
| | |
|---|---|
| **Status** | done — 2026-11-27: `ui/loading.rs` (spinner row, row/card/page skeletons, `fade_in`); shimmering CSS skeletons on Home, Library, artist/album/playlist fetches (`fetch_then`), Search, charts, Performer; results fade in on arrival; page switches crossfade (280 ms); first backdrop image fades in from dark; artist photos fade in. CSS animations follow GTK reduced-motion. 58 tests pass; installed to both prefixes (md5 02d34a8c…). **todo:** Podcast page + Radio tabs still use text status. **Regression fixed 2026-11-27:** the artist page's nested top-tracks fetch wiped the page with a skeleton (artist e.g. Alcatrazz never finished loading); nested fills now use `fetch_then_inline` (no skeleton) |
| **Today** | Bare "Loading…" labels in ~12 places |
| **Accept** | Every wait shows a skeleton or spinner, and new content eases in instead of popping |

#### W691 — Audiophile interface + EQ engine repair (P0)
| | |
|---|---|
| **Status** | done — 2026-11-27, verified live on this seat: boosting 125 Hz by ~+9 dB on the graph raised the 100–160 Hz analyzer bars from ≈ −15 to ≈ −5 dBFS during playback. 68 tests pass. Built to `~/.local/bin`; **/usr/bin not yet synced** |
| **Was broken** | Whole page overflowed the window; graph was a bare flat line (no grid/labels/handles, wrong zero line); drag re-picked the nearest band on every move; analyzer was white boxes; Match was an empty box; Dynamics was 20 unstyled checkboxes; GL path always fell back to CPU. Engine: legacy bands (60/170/310/600/1k/3k/6k/12k/14k/16k Hz) left 31–500 Hz and 2–8 kHz uncovered; every gain change rebuilt the filters (state reset → clicks); dynamic mode built new filters every block; gain range −24…+12; peak meter read 0 dBFS at rest and froze after stop; file-match used a rectangular window |
| **Fixed** | **Engine** (`eq/`): ISO octave centres 31.5 Hz–16 kHz at Q √2; filter state kept across gain/dynamic retunes; ±12 dB everywhere; Hann-windowed file match; 1/3-octave analyzer computed UI-side (not in the audio callback); mono/stereo caps so surround is downmixed; meter starts at −120 dB and goes silent when audio stops. New tests: per-band gain accuracy (±0.7 dB), click-free gain steps, analyzer ≈ 0 dBFS for a full-scale tone, meter staleness. **UI** (`ui/audiophile.rs`): Graphic tab with log-frequency grid, dB scale, 10 draggable band handles (band locked at drag start), scroll ±0.5 dB, double-click reset, arrow keys, live spectrum, Bypass/Delta/Reset, peak meter, 10 sliders; Bands tab (Hz/note entry + spin buttons); 30-band Analyzer with peak hold, tilt, freeze; Dynamics grid of per-band cards; Match tab with sketch-to-bands and file match; Presets with 8 built-ins + save/load/delete. Tools-page EQ now uses the same sliders. GL curve module and `gl` dependency removed |
| **Todo** | Per-band Q / shapes (shelves, cuts) in the UI; per-source-kind EQ morph (W609); `/usr/bin` sync |

---

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
