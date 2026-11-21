# MDE × Win11 community audit — 100 suggestions

Sources: PowerToys, Windhawk (Start/Taskbar/NC stylers), ExplorerPatcher, StartAllBack/Start11, Windows Central essential tweaks, r/Windows11 community norms.
MDE evidence: live tree audit 2026-11-20 (`mde-shell` surfaces, `mde-control`/`nav.rs`, `mde-config`, `WORKLIST.md`). Intentional remainders: **W8** Explorer, **W443** hover thumbnails, **W393** (blocked on W8), no real Mica blur, greeter a11y stubs, fidelity captures mostly unmeasured.

Status: presenting one MCQ at a time. Answers recorded below as `ANSWER:`.
Reconciled: stub/gap claims updated after full GUI inventory (Start has no Recommended; Widgets/QS/Task View/Snap/NC calendar largely real).

---

## 001 — Taskbar left alignment as community preset / default
**Community:** Top Win11 tweak — Personalization → Taskbar → Left (Windows Central, Reddit).
**MDE today:** Settings › Taskbar already has Left/Center choices (`SetAlign`); `taskbar_left`/`taskbar_align` in config; **default remains center**.
**Suggestion:** Add a Home/first-run **“Classic left taskbar”** preset (and/or change the install default) so community preference is one click, not buried discovery.
**Choices:**
- A) Home/first-run “Classic left” preset (keep center default)
- B) No preset; only improve Settings copy/docs
- C) Change default install to left-aligned
- D) Skip — center default is the MDE brand
- E) Other (specify)

ANSWER: A

## 002 — Taskbar Search modes editable in Settings
**Community:** Hide Search from taskbar; use Start / Win+S instead.
**MDE today:** `taskbar_search` defaults to `"hide"`; Settings shows Search mode as a **status row only** (not hide/icon/box chooser).
**Suggestion:** Add Win11-like Search mode choices (Hide / Search icon / Search box) with live shell apply.
**Choices:**
- A) Full hide/icon/box chooser with live apply (high)
- B) Keep hide default; status row + docs only
- C) Default to icon Search like stock Win11
- D) Skip
- E) Other (specify)

ANSWER: A

## 003 — Widgets button off by default (community quiet-desktop)
**Community:** Disable Widgets / news feed; many remove the button entirely.
**MDE today:** `widgets_btn` defaults true; board has real weather, RSS news, OSRM traffic, Stooq markets, photos, system, agenda/clock (Sports/To Do out of scope).
**Suggestion:** Match community quiet installs: default Widgets button OFF (Win+W still opens board) and/or a “hide news/markets feeds” master switch.
**Choices:**
- A) Default Widgets button OFF; keep board for Win+W
- B) Keep button on; add “hide news/markets” feed kill switch
- C) Remove Widgets entirely from MDE v1
- D) Leave defaults as-is
- E) Other (specify)

ANSWER: A

## 004 — Taskbar badges toggle (message counts)
**Community:** Often disabled to reduce distraction; when on, expect real counts.
**MDE today:** `taskbar_badges` exists; Unity/unread/progress heuristics removed (W439/W440) — no app progress/unread protocol.
**Suggestion:** Either implement a real badge/urgency path (SNI/Unity-like) or default badges OFF and clarify Settings copy that counts are not available yet.
**Choices:**
- A) Implement real badge protocol + keep Settings toggle
- B) Default badges OFF; honest Settings copy
- C) Remove badges UI entirely
- D) Leave knobs unused for now
- E) Other (specify)

ANSWER: A

## 005 — Never combine / show labels (ExplorerPatcher / Windhawk demand)
**Community:** Ungroup taskbar buttons + labels is a top ExplorerPatcher/Windhawk ask.
**MDE today:** `taskbar_combine` default `"always"`; “Show labels” still stub-ish (W257).
**Suggestion:** Support Never combine + labels next to icons like Win10/ExplorerPatcher.
**Choices:**
- A) Full Never/When full/Always + labels (high priority)
- B) Labels only while keeping Always combine
- C) Defer to v1.1
- D) Skip — icon-only clustered bar is intentional
- E) Other (specify)

ANSWER: A

## 006 — Taskbar on top / any edge (StartAllBack headline feature)
**Community:** StartAllBack sells taskbar-on-top / any edge.
**MDE today:** Bottom bar via layer-shell; no top/left/right placement.
**Suggestion:** Allow taskbar edge placement (at least top + bottom).
**Choices:**
- A) Top + bottom placement (high)
- B) All four edges
- C) Bottom only forever
- D) Defer indefinitely
- E) Other (specify)

ANSWER: C

## 007 — Dynamic / acrylic taskbar transparency (StartAllBack / Windhawk Taskbar Styler)
**Community:** Dynamic transparency / glassmorphism heavily recommended.
**MDE today:** `theme.transparency` + `opacity` exist for chrome.
**Suggestion:** Add “dynamic transparency” (more opaque over content / clearer over wallpaper) and optional blur strength presets.
**Choices:**
- A) Dynamic opacity + blur presets
- B) Keep static opacity slider only
- C) Opaque taskbar option as community “OLED/burn-in” preset
- D) Skip visual glass work
- E) Other (specify)

ANSWER: A

## 008 — Small taskbar / compact icon size (Windhawk classic)
**Community:** Smaller taskbar height + icon size is a top Windhawk mod.
**MDE today:** Fixed ~48px Win11 density.
**Suggestion:** Offer Compact (32–40px) and Comfortable (48px) taskbar sizes.
**Choices:**
- A) Compact + Comfortable sizes
- B) Continuous height slider
- C) Keep 48px only
- D) Larger accessibility size only
- E) Other (specify)

ANSWER: A

## 009 — Taskbar auto-hide polish
**Community:** Auto-hide remains a common power-user setting.
**MDE today:** `taskbar_autohide` exists; polish/toggle history in shell.
**Suggestion:** Match Win11: reveal on edge hover, hide after leave, don’t steal focus from fullscreen.
**Choices:**
- A) Full Win11-parity auto-hide behavior
- B) Keep current; fix bugs only
- C) Remove auto-hide
- D) Defer
- E) Other (specify)

ANSWER: A

## 010 — Start “More pins” layout density
**Community:** More pins / fewer recommendations is the #1 Start tweak on Win11.
**MDE today:** Recommended section already removed (W456/W463); `start.layout` supports default/more_pins; pin folders + large pins via extras.
**Suggestion:** Make More pins the clear community preset in Settings › Start (density + collapsed pinned), matching Win11 “More pins” without bringing Recommended back.
**Choices:**
- A) Promote More pins as primary Start layout + live preview
- B) Keep layout toggle; polish Settings copy only
- C) Always more_pins; drop layout enum
- D) Skip — current Start density is fine
- E) Other (specify)

ANSWER: A

## 011 — Start folders next to power (Settings, Explorer, Downloads…)
**Community:** Enable folders beside power button.
**MDE today:** `start.folders` + footer glyphs already implemented (W451).
**Suggestion:** Settings › Start › Folders should mirror Win11 checklist UX with live preview.
**Choices:**
- A) Polish Folders settings to Win11 checklist parity
- B) Add more folder kinds (Network, Recycle)
- C) Already good — skip
- D) Remove folder shortcuts
- E) Other (specify)

ANSWER: A

## 012 — Keep Start free of promo / recommendation chrome
**Community:** Turn off app recommendations / account notifications / Store promos.
**MDE today:** Recommended section already removed; Tips widget and account stubs remain elsewhere.
**Suggestion:** Lock in a “no promo surfaces” policy: no Recommended return, Tips opt-in only, no account marketing rows on Start.
**Choices:**
- A) Formal no-promo policy + Tips opt-in only (high)
- B) Already good enough — skip
- C) Also strip Tips / discovery widgets by default
- D) Reintroduce optional Recommended for recent files only
- E) Other (specify)

ANSWER: A

## 013 — StartAllBack / Start11-style vertical Start
**Community:** Paid Start replacements offer vertical / Win10 Start.
**MDE today:** Modern centered Start card.
**Suggestion:** Optional classic vertical Start (all apps left, pins right) à la Windhawk side-by-side / Start11.
**Choices:**
- A) Add Classic vertical Start layout option
- B) Win10-style Start as alternate shell mode
- C) Modern Start only
- D) Defer
- E) Other (specify)

ANSWER: A

## 014 — Start pin folders / large pins polish
**Community:** Grouping + tile sizes still requested (even post-Live Tiles).
**MDE today:** `extras.start.pin_folders` + `start.pin_large` already support folders and large pins; drag reorder exists.
**Suggestion:** Settings › Start UX to manage folders/sizes (not only shell gestures) + Medium size if community wants three sizes.
**Choices:**
- A) Settings UI for folders + sizes + optional Medium
- B) Shell gestures only — polish discoverability
- C) Drop large pins; flat grid only
- D) Skip — current is enough
- E) Other (specify)

ANSWER: A

## 015 — PowerToys Run / Command Palette launcher
**Community:** PowerToys Run / Command Palette is the #1 productivity addon.
**MDE today:** Start search covers apps/docs/web hint; no dedicated Run palette with plugins.
**Suggestion:** Add Super+R / Super+Space Command Palette (apps, files, calc, shell commands, plugins).
**Choices:**
- A) Full Command Palette with plugins
- B) Lightweight app launcher only
- C) Enhance Start search instead (no separate palette)
- D) Skip
- E) Other (specify)

ANSWER: A

## 016 — FancyZones-style custom snap layouts
**Community:** FancyZones is the flagship PowerToys window tool.
**MDE today:** Snap flyout has halves/quarters/thirds/ultrawide presets + assist overlay + edge drag; picker shows titles only (no live thumbs); no user-drawn zone editor.
**Suggestion:** Add user-editable zone layouts (FancyZones analogue) on top of existing presets.
**Choices:**
- A) Full FancyZones analogue (editor + layouts)
- B) Keep built-in presets; polish Win+Z picker only
- C) Add 2–3 community preset packs only (no editor)
- D) Skip
- E) Other (specify)

ANSWER: A

## 017 — Snap Groups restore in Alt+Tab / taskbar
**Community:** Snap Groups are a praised Win11 multitasking feature.
**MDE today:** Snap assist exists; group memory unclear / incomplete.
**Suggestion:** Persist snap groups and restore via Alt+Tab or taskbar hover.
**Choices:**
- A) Persist + restore Snap Groups
- B) Session-only groups
- C) Skip Snap Groups
- D) Defer
- E) Other (specify)

ANSWER: A

## 018 — Always On Top discoverability (feature exists)
**Community:** PowerToys Always On Top is widely used (often Win+Ctrl+T).
**MDE today:** Compositor AlwaysOnTop via winmenu / `mde-window-action` (W412) — real z-order, not a stub.
**Suggestion:** Add a dedicated shortcut + Settings toggle and optional title-bar pin glyph for discoverability.
**Choices:**
- A) Shortcut + Settings + title-bar indicator
- B) Shortcut only
- C) Keep winmenu-only — skip
- D) Defer polish
- E) Other (specify)

ANSWER: A

## 019 — Color Picker polish (path exists)
**Community:** System color picker is a staple (PowerToys / Super+Shift+C).
**MDE today:** Screenshot/color-pick path via portal + tools (W427); not a dedicated history-rich PowerToys picker UI.
**Suggestion:** Native eyedropper with hex/rgb/hsl copy + recent colors history.
**Choices:**
- A) Native picker UI + history
- B) Keep portal/tool path; polish failure UX only
- C) Bundle one Wayland picker as hard dep
- D) Skip
- E) Other (specify)

ANSWER: A

## 020 — Text Extractor / OCR from screen (PowerToys)
**Community:** OCR grab is highly recommended for power users.
**MDE today:** No equivalent.
**Suggestion:** Region select → OCR → clipboard (tesseract/gnome OCR).
**Choices:**
- A) Implement Text Extractor
- B) Document external tool only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: C

## 021 — Peek file preview (PowerToys)
**Community:** Spacebar Peek is beloved.
**MDE today:** File Explorer deferred (W8); no Peek.
**Suggestion:** Quick preview for selected file from Start search / future Explorer.
**Choices:**
- A) Peek from Start search results
- B) Wait for File Explorer epic (W8)
- C) Skip Peek
- D) Integrate with Thunar preview if possible
- E) Other (specify)

ANSWER: B

## 022 — Keyboard Manager / remapper (PowerToys)
**Community:** Remap keys/shortcuts without Autohotkey.
**MDE today:** Accessibility shortcuts page; limited remaps.
**Suggestion:** GUI remapper for keys and shortcuts stored in config.toml.
**Choices:**
- A) Full Keyboard Manager
- B) Common presets only (Caps→Esc, etc.)
- C) Skip — use system tools
- D) Defer
- E) Other (specify)

ANSWER: A

## 023 — Awake / keep awake utility (PowerToys)
**Community:** Awake is a common “long job” helper.
**MDE today:** Power page / idle lock; no temporary inhibit UI.
**Suggestion:** Tray/QS “Keep awake” with timed or indefinite inhibit.
**Choices:**
- A) Add Awake to QS + tray
- B) Settings-only inhibit
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 024 — Mouse Find My Mouse / highlighter (PowerToys)
**Community:** Find My Mouse + click highlighter for presentations/a11y.
**MDE today:** Limited pointer a11y.
**Suggestion:** Ctrl×2 spotlight + optional click highlighter.
**Choices:**
- A) Both Find My Mouse + Highlighter
- B) Find My Mouse only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 025 — Screen Ruler / ZoomIt-like annotate (PowerToys)
**Community:** Measure/annotate tools for tech users.
**MDE today:** Magnifier stub; screenshot path exists.
**Suggestion:** Pixel ruler and/or ZoomIt-style zoom+annotate.
**Choices:**
- A) Screen Ruler + zoom annotate
- B) Ruler only
- C) Improve magnifier only
- D) Skip
- E) Other (specify)

ANSWER: D

## 026 — Workspaces / project launch layouts (PowerToys Workspaces)
**Community:** Launch app sets to positions.
**MDE today:** Display profiles; no app workspace presets.
**Suggestion:** Save/restore named window layouts + apps.
**Choices:**
- A) Full Workspaces feature
- B) Script hooks only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 027 — Classic full context menu (ExplorerPatcher demand)
**Community:** “Show more options” hate; restore classic menu.
**MDE today:** File Explorer deferred; shell menus are custom.
**Suggestion:** When Explorer lands (or via Thunar), ship dense classic-style context menus by default.
**Choices:**
- A) Classic menus as MDE default for file manager
- B) Modern + “more options”
- C) Out of scope until W8
- D) Skip
- E) Other (specify)

ANSWER: C

## 028 — Alt+Tab compact vs thumbnail modes
**Community:** Many prefer classic Alt+Tab icons vs large thumbnails (ExplorerPatcher).
**MDE today:** Alt+Tab overlay with faces; `alttab.show_desktop` toggle + synthetic Desktop entry (W208); not live thumbnails.
**Suggestion:** Compact (icons) vs Thumbnail modes; keep Desktop include toggle.
**Choices:**
- A) Compact + Thumbnail modes (high)
- B) Keep current faces; polish Desktop toggle only
- C) Icons-only forever
- D) Skip
- E) Other (specify)

ANSWER: D

## 029 — Taskbar thumbnail peek (deferred W443)
**Community:** Live thumbnails on hover are expected Win11 chrome.
**MDE today:** Explicitly **deferred W443** — hover shows truncated title only (`spikes/thumbnails.md`); no live thumbnail.
**Suggestion:** Prioritize compositor-backed hover peek thumbnails vs keep deferred.
**Choices:**
- A) Real live thumbnails now (high — und defer W443)
- B) Static last-frame cache
- C) Keep title-only peek / leave deferred
- D) Disable peek hover entirely
- E) Other (specify)

ANSWER: C

## 030 — Jump lists with real recent files
**Community:** Jump lists are core Win productivity.
**MDE today:** Jump lists exist with recent files (menus.rs); depth/quality varies.
**Suggestion:** Deepen recent tracking (GTK/Qt/xdg), pin forever, clear list.
**Choices:**
- A) Deeper recent + pin forever
- B) Keep current recent depth
- C) Remove jump lists
- D) Defer
- E) Other (specify)

ANSWER: A

## 031 — NC calendar / agenda depth
**Community:** Win11 NC clock→calendar + agenda expectations.
**MDE today:** Month grid + local agenda (`agenda.rs`, `~/.local/share/mde/agenda.json`) already in NC.
**Suggestion:** Next step is sync (CalDAV/Nextcloud) and/or richer agenda CRUD — not greenfield calendar chrome.
**Choices:**
- A) CalDAV/cloud sync + richer agenda CRUD
- B) Local agenda polish only
- C) Keep current calendar/agenda
- D) Strip agenda; month grid only
- E) Other (specify)

ANSWER: D

## 032 — Per-app notification prefs beyond mute
**Community:** Per-app banners/sounds/priority in Win11 Settings.
**MDE today:** NC can mute/unmute per app; group Settings opens System › Notifications (W185 path); no full per-app banner/sound/priority page.
**Suggestion:** Win11-like per-app prefs (banners, sound, priority allowlist) with deep link preselecting the app.
**Choices:**
- A) Full per-app notification prefs page + deep link
- B) Keep mute + global DND/priority only
- C) Priority allowlist editor only
- D) Skip
- E) Other (specify)

ANSWER: A

## 033 — Focus Assist schedules / automatic rules
**Community:** Priority only / alarms / automatic when duplicating display.
**MDE today:** DND + priority + quiet hours fields.
**Suggestion:** Automatic Focus rules (schedule, when casting/fullscreen, game mode).
**Choices:**
- A) Automatic Focus rules engine
- B) Manual DND/priority only
- C) Quiet hours only polish
- D) Skip
- E) Other (specify)

ANSWER: C

## 034 — Focus sessions depth (timer already real)
**Community:** Focus sessions in Win11 Clock / Settings.
**MDE today:** QS 25m focus timer toggles DND and ends on expiry; Settings Focus can start/end sessions.
**Suggestion:** Add session history, custom durations, and optional break timer like Win11 Focus.
**Choices:**
- A) Custom durations + breaks + history
- B) Custom duration only
- C) Keep 25m QS + Settings as-is
- D) Skip
- E) Other (specify)

ANSWER: D

## 035 — QS Edit tiles that persist like Win11
**Community:** Rearrange Quick Settings tiles.
**MDE today:** Edit mode persists `extras qs.tiles` (W98 done).
**Suggestion:** Drag-reorder polish + Win11-like “Add or remove quick settings” gallery.
**Choices:**
- A) Drag-reorder + gallery UI polish
- B) Keep current add/remove
- C) Fixed tile set only
- D) Skip
- E) Other (specify)

ANSWER: A

## 036 — QS volume mixer polish vs Settings parity
**Community:** Per-app volume from QS is expected.
**MDE today:** QS nested app mixer via `wpctl` already exists alongside Settings › Volume mixer.
**Suggestion:** Ensure QS mixer matches Settings (device picker, mute, % labels) and is discoverable from the volume row.
**Choices:**
- A) Parity polish + discoverability pass
- B) Keep both; no work
- C) Collapse to Settings-only mixer
- D) QS-only; remove Settings duplicate
- E) Other (specify)

ANSWER: A

## 037 — QS Bluetooth flyout depth
**Community:** Action Center BT device list is table stakes.
**MDE today:** QS BT tile + Settings device list/connect exist; confirm QS expandable list parity with Settings (pair vs connect).
**Suggestion:** Ensure QS BT flyout can connect/disconnect known devices without opening Settings; pair stays in Settings if needed.
**Choices:**
- A) QS connect/disconnect for known devices (high)
- B) Power toggle + “Open Bluetooth settings” only
- C) Full pair wizard in QS
- D) Skip — current is enough
- E) Other (specify)

ANSWER: A+C — QS connect/disconnect + full pair wizard

## 038 — QS Wi-Fi password / join flow
**Community:** Join networks from QS without opening full Settings.
**MDE today:** Unknown-SSID password often still defers to NM/nmtui (W147 note); known networks connect from Settings/QS paths.
**Suggestion:** Inline password prompt for unknown SSIDs inside QS.
**Choices:**
- A) Inline password join in QS (high)
- B) Always open Network Settings for new SSIDs
- C) Leave nmtui deferral
- D) Skip
- E) Other (specify)

ANSWER: A

## 039 — Hotspot / VPN / Cast reliability
**Community:** Hotspot, VPN, Cast in Action Center.
**MDE today:** QS tiles + nested VPN/hotspot/nearby exist; Cast/share uses xdg-desktop-portal screencast + project modes — quality depends on portal/receiver.
**Suggestion:** Harden reliability and hide/disable tiles when backend missing instead of dead ends.
**Choices:**
- A) Harden all three + hide when unavailable
- B) VPN + hotspot only; Cast stays best-effort
- C) Keep current behavior
- D) Remove flaky tiles from default QS set
- E) Other (specify)

ANSWER: A

## 040 — Tray overflow / hidden icons polish
**Community:** Select which tray icons show / overflow.
**MDE today:** ≤4 visible SNI + `›` overflow; QS hidden-icon mgmt persists `extras.tray.hidden` (honest empty state W442).
**Suggestion:** Settings › Taskbar “Other system tray icons” page mirroring Win11, plus overflow UX polish.
**Choices:**
- A) Dedicated Settings tray-icons page + overflow polish
- B) Keep QS-only hidden management
- C) Show all icons; drop hide list
- D) Skip
- E) Other (specify)

ANSWER: A

## 041 — Clipboard history: images + pin (community Win+V)
**Community:** Win+V clipboard history is essential; pin favorites.
**MDE today:** History with images/pin/clear largely done (T3).
**Suggestion:** Keyboard Win+V focus, paste-as-plain, sync polish.
**Choices:**
- A) Win+V focus + paste-as-plain + sync UX polish
- B) Already enough — skip
- C) Add OCR on clipped images
- D) Remove cross-device sync
- E) Other (specify)

ANSWER: A

## 042 — Advanced Paste beyond paste-as-file
**Community:** PowerToys Advanced Paste; Win paste-as-file asks.
**MDE today:** Clipboard already supports paste-back, paste plain, paste as file, save image, Nearby share.
**Suggestion:** Add PowerToys-like transforms (markdown/HTML/JSON, optional AI later) on top of existing paste-as-file.
**Choices:**
- A) Add text/format transforms (Advanced Paste lite)
- B) Keep current clipboard actions — skip
- C) Add OCR-on-image paste only
- D) Defer transforms
- E) Other (specify)

ANSWER: A

## 043 — Widgets board: curation & third-party API
**Community:** Widgets useful only with real content; many users strip news.
**MDE today:** Weather, RSS news, traffic, markets, photos, system, agenda/clock are real; Sports/To Do out of scope.
**Suggestion:** Curate default card set (community quiet vs stock) and/or pluggable widget API.
**Choices:**
- A) Quiet default card set + per-card toggles
- B) Pluggable third-party widget API
- C) Both A and B
- D) Keep current board defaults
- E) Other (specify)

ANSWER: C — quiet defaults + toggles + pluggable API

## 044 — File Explorer–class shell (deferred W8)
**Community:** Explorer customization is half of Win11 modding culture.
**MDE today:** W8 deferred; use system file manager.
**Suggestion:** Prioritize MDE File Explorer (tabs, home, Quick Access).
**Choices:**
- A) Start W8 Explorer epic now (high)
- B) Deep-integrate Thunar as “MDE Explorer”
- C) Keep deferred
- D) Skip forever
- E) Other (specify)

ANSWER: B

## 045 — Show file extensions / hidden files defaults
**Community:** Every power-user install enables extensions.
**MDE today:** Depends on file manager.
**Suggestion:** MDE first-run sets file manager to show extensions + hidden toggle in Settings.
**Choices:**
- A) First-run + Settings toggles for FM defaults
- B) Document only
- C) Skip
- D) Defer with W8
- E) Other (specify)

ANSWER: A

## 046 — Virtual desktops naming + wallpaper per desktop
**Community:** Win11 desktops with names/wallpapers are popular.
**MDE today:** Workspaces exist; animation stub polish (W168).
**Suggestion:** Named desktops + optional per-desktop wallpaper.
**Choices:**
- A) Names + per-desktop wallpaper
- B) Names only
- C) Keep numbered desktops
- D) Skip
- E) Other (specify)

ANSWER: A

## 047 — Task View thumbnail fidelity
**Community:** Win+Tab Task View with desktop/window previews.
**MDE today:** Task View UI exists (`taskview.rs`) — per-desktop columns, new/close/move, Super+Tab; window faces are not live compositor thumbnails.
**Suggestion:** Add live (or cached) window/desktop thumbnails in Task View.
**Choices:**
- A) Live thumbnails in Task View (high)
- B) Cached last-frame thumbs
- C) Keep title/icon Task View
- D) Skip
- E) Other (specify)

ANSWER: C

## 048 — Show-desktop peek-on-hover
**Community:** Far-right show-desktop and Aero Peek nostalgia.
**MDE today:** 2px far-right show-desktop strip + Super+D already exist.
**Suggestion:** Optional peek-on-hover (temporarily show desktop while hovering the strip).
**Choices:**
- A) Add peek-on-hover (high community nostalgia)
- B) Keep click strip only
- C) Widen strip; no peek
- D) Skip
- E) Other (specify)

ANSWER: B

## 049 — Window Grab and Move / AltSnap
**Community:** AltSnap / PowerToys Grab and Move widely recommended.
**MDE today:** Standard Wayland move from titlebar.
**Suggestion:** Alt+drag move/resize from anywhere (AltSnap parity).
**Choices:**
- A) Alt+drag move/resize
- B) Super+drag only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 050 — Title bar accent / mica / SSD corner radius
**Community:** Visual polish (mica, rounded) is Windhawk/StartAllBack territory.
**MDE today:** Accent on titlebars; 2px underline; true corner radius deferred (W44).
**Suggestion:** Real rounded SSD corners + mica titlebars.
**Choices:**
- A) Rounded SSD + mica
- B) Keep underline accent only
- C) Client-side decorations only
- D) Skip
- E) Other (specify)

ANSWER: A

## 051 — Light Switch / dark schedule reliability
**Community:** Auto light/dark by time (PowerToys Light Switch / Win11).
**MDE today:** `theme.dark_schedule` exists.
**Suggestion:** Ensure live shell/compositor flip without relogin + sunrise/sunset option.
**Choices:**
- A) Live flip + sunrise/sunset
- B) Keep clock-window schedule; ensure live apply
- C) Manual only
- D) Skip
- E) Other (specify)

ANSWER: A

## 052 — Accent colour on Start/taskbar toggles (Win11 Colours)
**Community:** “Show accent on Start and taskbar” is a standard toggle.
**MDE today:** `accent_on_start_taskbar` exists.
**Suggestion:** Settings › Colours should mirror Win11 wording with live preview.
**Choices:**
- A) Win11-parity Colours page polish
- B) Already fine — skip
- C) Force accent always
- D) Never accent chrome
- E) Other (specify)

ANSWER: A

## 053 — Transparency effects master switch
**Community:** Disable transparency for battery/perf/OLED.
**MDE today:** `theme.transparency` exists.
**Suggestion:** Master switch disables mica/acrylic everywhere including flyouts.
**Choices:**
- A) Verify master switch kills all translucent chrome
- B) Separate taskbar vs flyout transparency
- C) Skip
- D) Default transparency off
- E) Other (specify)

ANSWER: A

## 054 — Bing/Spotlight wallpaper rotation
**Community:** Spotlight-like wallpapers popular.
**MDE today:** `bing_daily` exists.
**Suggestion:** Spotlight-like lock+desktop pack with likes/history.
**Choices:**
- A) Spotlight pack (desktop+lock+history)
- B) Keep Bing daily only
- C) Local slideshow only
- D) Skip online wallpapers
- E) Other (specify)

ANSWER: A

## 055 — Lock screen media controls that feel native
**Community:** Media on lock screen is expected.
**MDE today:** playerctl line + skip stubs on lock.
**Suggestion:** Full media card (art, scrub, volume) on lock/greeter.
**Choices:**
- A) Full media card on lock
- B) Keep transport buttons only
- C) No media on lock (privacy)
- D) Skip
- E) Other (specify)

ANSWER: A

## 056 — Greeter accessibility / keyboard layout real options
**Community:** Login a11y + layout switch matter.
**MDE today:** Greeter a11y/layout stubs (W149/W249).
**Suggestion:** Real high-contrast, on-screen keyboard, layout switch on greeter.
**Choices:**
- A) Full greeter a11y + layouts
- B) OSK + layout only
- C) Leave stubs
- D) Skip
- E) Other (specify)

ANSWER: A

## 057 — Switch user on lock for real (multi-seat/session)
**Community:** Fast user switching is classic Windows.
**MDE today:** Switch user limited / “not available” paths (W428/W462).
**Suggestion:** Real switch-user via greetd multi-session if feasible.
**Choices:**
- A) Real multi-user switch
- B) Keep single-user lock only
- C) Document limitation only
- D) Defer
- E) Other (specify)

ANSWER: B

## 058 — Account picture change from Start
**Community:** Change picture from Start account flyout.
**MDE today:** Start still uses `AccountPictureStub` / coming soon (W253); Settings › Your info may have partial avatar path.
**Suggestion:** Real avatar picker wired to greeter/Start/Settings.
**Choices:**
- A) Real avatar picker from Start + Settings
- B) Settings-only avatar; Start opens that page
- C) Leave stub
- D) Skip
- E) Other (specify)

ANSWER: A

## 059 — Night light strength + schedule UI
**Community:** Blue light filter with schedule is standard.
**MDE today:** Night light via wlsunset/gammastep; Settings toggle.
**Suggestion:** Temperature slider + sunset schedule like Win11.
**Choices:**
- A) Strength + schedule UI
- B) On/off only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 060 — Display arrange drag-canvas (multi-monitor)
**Community:** Drag monitors in Settings is expected.
**MDE today:** Layout file + profiles; UI thin (C4/D4 notes).
**Suggestion:** Visual drag-to-arrange displays + identity flash.
**Choices:**
- A) Drag canvas + identify
- B) Keep text/profile apply
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 061 — Project modes Super+P reliability
**Community:** PC screen only / Duplicate / Extend / Second only.
**MDE today:** Super+P / Connect flyout already drives Extend/Duplicate/Second-only via `mde-output-layout.txt`; PC-only and multi-DPI edge cases may still be thin.
**Suggestion:** Harden all four Win11 project modes + identity flash on apply.
**Choices:**
- A) Harden four modes + identify displays
- B) Keep Extend/Duplicate/Second; document limits
- C) Skip — current is enough
- D) Defer
- E) Other (specify)

ANSWER: A

## 062 — HDR / refresh rate / scale per monitor clarity
**Community:** Per-monitor scale/refresh is daily driver essentials.
**MDE today:** Modes/scale via profiles; fractional scale supported.
**Suggestion:** Per-output refresh/scale/resolution pickers with Apply/Revert 15s.
**Choices:**
- A) Full per-output apply/revert UI
- B) Profiles only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 063 — Startup apps manager like Win11
**Community:** Disable startup apps early.
**MDE today:** Settings Startup page exists.
**Suggestion:** Impact estimates + enable/disable XDG autostart with search.
**Choices:**
- A) Impact + search + toggle polish
- B) Keep simple list
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 064 — Default apps by file type / protocol
**Community:** Default browser/email/etc. is critical.
**MDE today:** Default apps page exists.
**Suggestion:** Win11-like per-extension and per-protocol defaults UI.
**Choices:**
- A) Full defaults-by-type UI
- B) Browser/email/photo/music shortcuts only
- C) Keep current
- D) Skip
- E) Other (specify)

ANSWER: A

## 065 — Installed apps uninstall + repair
**Community:** Apps list with uninstall is Settings staple.
**MDE today:** Installed apps page.
**Suggestion:** dnf/flatpak uninstall, open install location, pin actions already partly there — add repair/reset app data.
**Choices:**
- A) Uninstall + reset data + pin actions
- B) Uninstall only polish
- C) Keep current
- D) Skip
- E) Other (specify)

ANSWER: A

## 066 — Game Mode that does something measurable
**Community:** Game Mode / Xbox Game Bar debates; still expected entry.
**MDE today:** Gaming › Game Mode page.
**Suggestion:** Real Game Mode: focus priority, inhibit notifications, optional MangoHud/gamemode integration.
**Choices:**
- A) Wire to Feral gamemode + DND
- B) DND-only Game Mode
- C) Keep cosmetic page
- D) Remove Gaming section
- E) Other (specify)

ANSWER: A

## 067 — Captures / Xbox-like screenshot+record
**Community:** Win+G / captures folder culture.
**MDE today:** Screenshots path; Super+Shift+S; recording unclear.
**Suggestion:** Screenshot + optional region video record + gallery.
**Choices:**
- A) Screenshot + recording + gallery
- B) Screenshot polish only
- C) Keep current grim path
- D) Skip
- E) Other (specify)

ANSWER: B

## 068 — Emoji panel depth (native grid exists)
**Community:** Win+. emoji panel is daily use.
**MDE today:** Native iced emoji grid + wtype/wl-copy; More… → gnome-characters (W426).
**Suggestion:** Add search, skin tones, kaomoji/GIF tabs, and recents like Win11.
**Choices:**
- A) Search + skin tones + recents (+ optional kaomoji)
- B) Recents + search only
- C) Keep current grid
- D) Skip
- E) Other (specify)

ANSWER: B

## 069 — Voice typing / dictation polish (pipeline exists)
**Community:** Voice typing popular on Win11 (Win+H).
**MDE today:** Super+H dictation with mic OSD + vosk/whisper/network + wtype (W424); Start voice search opt-in (W459).
**Suggestion:** Reliability polish, offline model packaging, and Settings discoverability.
**Choices:**
- A) Package offline models + Settings UX polish
- B) Network STT polish only
- C) Keep current pipeline as-is
- D) Remove voice features
- E) Other (specify)

ANSWER: B

## 070 — Magnifier fidelity (tool/scale path exists)
**Community:** Magnifier is core a11y (Win++/-).
**MDE today:** `spawn_magnifier` / Super+Plus uses magnus/kmag or output-scale nudge (W422/W403) — not a stub tip.
**Suggestion:** Compositor-native lens/full-screen zoom with follow-focus, beyond external tools.
**Choices:**
- A) Compositor-native magnifier
- B) Keep external tool + scale nudge; polish UX
- C) External tools only; drop scale nudge
- D) Skip
- E) Other (specify)

ANSWER: A

## 071 — High contrast themes that actually apply
**Community:** Contrast themes for low vision.
**MDE today:** Contrast themes page exists.
**Suggestion:** Force high-contrast palette across shell + GTK + titlebars.
**Choices:**
- A) System-wide contrast theme pack
- B) Shell-only contrast
- C) Keep page cosmetic
- D) Skip
- E) Other (specify)

ANSWER: A

## 072 — Text size system-wide slider
**Community:** Accessibility text size.
**MDE today:** Text size page exists.
**Suggestion:** Scale UI fonts across shell/settings/apps via config + gsettings.
**Choices:**
- A) System-wide text scale
- B) Shell chrome only
- C) Keep current
- D) Skip
- E) Other (specify)

ANSWER: A

## 073 — Shortcut Guide overlay (PowerToys)
**Community:** Hold Win → shortcut cheat sheet.
**MDE today:** Shortcuts settings page; no overlay guide.
**Suggestion:** Hold Super → Shortcut Guide overlay.
**Choices:**
- A) Implement Shortcut Guide
- B) Settings list only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 074 — Hosts editor / env vars (PowerToys admin toys)
**Community:** Dev-oriented PowerToys modules.
**MDE today:** MMC + Config Editor.
**Suggestion:** MMC pages for hosts + environment variable profiles.
**Choices:**
- A) Add Hosts + Env profiles to MMC
- B) Config Editor only is enough
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: C

## 075 — PowerRename / bulk rename
**Community:** PowerRename is a favorite Explorer addon.
**MDE today:** No bulk rename utility.
**Suggestion:** Bulk rename tool (regex) integrable with FM context menu.
**Choices:**
- A) Ship PowerRename analogue
- B) Rely on existing FM tools
- C) Skip
- D) Defer with W8
- E) Other (specify)

ANSWER: B — Thunar already has bulk rename; rely on FM tools

## 076 — Image Resizer context action
**Community:** PowerToys Image Resizer.
**MDE today:** None.
**Suggestion:** Right-click resize presets for images.
**Choices:**
- A) Implement Image Resizer
- B) Skip
- C) Defer with W8
- D) Script only
- E) Other (specify)

ANSWER: A

## 077 — File Locksmith “who’s using this file”
**Community:** PowerToys File Locksmith.
**MDE today:** None.
**Suggestion:** “Open files” inspector for path → PID.
**Choices:**
- A) Implement Locksmith
- B) Skip
- C) MMC-only advanced tool
- D) Defer
- E) Other (specify)

ANSWER: B

## 078 — Nearby Share / KDE Connect remaining gaps
**Community:** Phone Link / Nearby Sharing expectations.
**MDE today:** QS Nearby + Settings pairing via D-Bus/CLI (W395/W446–448); clipboard share to device (W437); intake wanted broad feature surface.
**Suggestion:** Close remaining Phone-Link-class gaps (SMS/notifications mirror, battery, reliable inbound toasts) and first-run install of `kde-connect`.
**Choices:**
- A) Broader Phone-Link feature parity on KDE Connect
- B) Harden current share/pair/clipboard only
- C) Keep as-is
- D) Hide Nearby from default QS
- E) Other (specify)

ANSWER: A

## 079 — Debloat / telemetry-analogue: quiet recommendations
**Community:** Win11 debloat scripts are controversial but popular; for MDE = no promo noise.
**MDE today:** No Store ads; Tips stub exists.
**Suggestion:** “Quiet desktop” preset: no Tips, no Widgets news, no Recommended, DND optional.
**Choices:**
- A) Ship Quiet desktop preset
- B) Individual toggles only
- C) Skip presets
- D) Other (specify)
- E) Aggressive remove all discovery UI

ANSWER: C

## 080 — Taskbar context menu completeness
**Community:** Discoverability of taskbar settings / Task Manager / show desktop.
**MDE today:** Right-click already has Taskbar settings, Task Manager, lock/auto-hide/widgets, calendar link (`menus.rs` W387).
**Suggestion:** Audit vs Win11 menu (Search, widgets, toolbars leftovers) and add any missing high-value entries only.
**Choices:**
- A) Gap-fill to Win11 menu parity where it makes sense
- B) Keep current menu — skip
- C) Slim menu further
- D) Add customizable menu entries
- E) Other (specify)

ANSWER: A

## 081 — Task Manager quick access from taskbar/Ctrl+Shift+Esc
**Community:** Muscle memory.
**MDE today:** MMC/services; system monitor launch?
**Suggestion:** Bind Ctrl+Shift+Esc to system monitor / mde-mmc performance.
**Choices:**
- A) Dedicated Task Manager surface
- B) Launch existing monitor (gnome-system-monitor/btop GUI)
- C) MMC Performance page only
- D) Skip
- E) Other (specify)

ANSWER: C — MMC Performance; polish and expand MMC

## 082 — End task from taskbar (Win11 recent feature)
**Community:** “End task” on taskbar is a praised recent Win11 addition.
**MDE today:** Jump list End task exists.
**Suggestion:** Ensure discoverable End task + Settings › For developers equivalent toggle.
**Choices:**
- A) Always show End task + optional “developer” mode
- B) Keep jump list only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 083 — Progress bars on taskbar buttons
**Community:** Downloads show progress on icons.
**MDE today:** Title-heuristic / Unity progress removed (W439/W440); running underline only.
**Suggestion:** Reintroduce progress via a real protocol (SNI/Unity-like or compositor hint), not title scraping.
**Choices:**
- A) Real progress protocol (high)
- B) Leave without progress cues
- C) Restore title heuristic temporarily
- D) Defer
- E) Other (specify)

ANSWER: A

## 084 — Search without web / local-only mode
**Community:** Disable Bing/web in Start search is a top privacy tweak.
**MDE today:** Web chip + DuckDuckGo hint (W95).
**Suggestion:** Local-only search mode; disable web chip by policy/setting.
**Choices:**
- A) Local-only default + optional web
- B) Keep web hint
- C) Remove web entirely
- D) Skip
- E) Other (specify)

ANSWER: C

## 085 — Content search scale (rg path already works)
**Community:** Indexer/content search expected on Win11-class Start.
**MDE today:** `files::search_file_contents` via `rg`/`grep` under Desktop/Documents/Downloads (W406) — not a stub.
**Suggestion:** Expand roots, optional Tracker/Recoll index for speed, and Settings to choose indexed folders.
**Choices:**
- A) Indexer-backed search + folder picker
- B) Expand rg roots only (no daemon index)
- C) Keep Desktop/Documents/Downloads only
- D) Skip
- E) Other (specify)

ANSWER: C

## 086 — Calculator in Start search (keep + history)
**Community:** Inline calc in search is loved (W175 exists).
**MDE today:** Tiny safe parser + history stub done.
**Suggestion:** Expand functions (hex, units) + pin Calculator app.
**Choices:**
- A) Expand calc engine + history
- B) Keep simple arithmetic
- C) Remove inline calc
- D) Skip
- E) Other (specify)

ANSWER: D

## 087 — Windhawk-like modular shell styler API
**Community:** Windhawk’s modular mods are the customization platform of choice.
**MDE today:** Single theme tokens in mde-theme.
**Suggestion:** Safe user CSS/QSS-like or TOML style packs for Start/Taskbar/NC without forking.
**Choices:**
- A) Official style-pack API
- B) Document theme tokens only
- C) Skip third-party styling
- D) Defer
- E) Other (specify)

ANSWER: A

## 088 — Animation reduce / disable (community perf/a11y)
**Community:** Disable animations common tweak.
**MDE today:** `theme.animations` exists.
**Suggestion:** Ensure toggle disables shell + compositor animations thoroughly.
**Choices:**
- A) Audit + harden animations off
- B) Keep as-is
- C) Default animations off
- D) Skip
- E) Other (specify)

ANSWER: A

## 089 — Battery flyout details (Win11 QS)
**Community:** Battery % + estimate + power mode.
**MDE today:** Battery flyout exists (W88).
**Suggestion:** Add time remaining, power mode (Best power efficiency / Balanced / Best performance), energy saver.
**Choices:**
- A) Full battery + power mode flyout
- B) Keep % + charging
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 090 — Storage Sense analogue
**Community:** Storage cleanup recommendations.
**MDE today:** Storage page exists.
**Suggestion:** Temp cleanup, trash, large files, optional scheduled cleanup.
**Choices:**
- A) Full Storage Sense analogue
- B) Simple disk usage view
- C) Keep current
- D) Skip
- E) Other (specify)

ANSWER: A

## 091 — Windows Update analogue clarity (dnf/flatpak)
**Community:** Users expect an Update UI even on Linux DE clones.
**MDE today:** Update + history pages.
**Suggestion:** One-click check/apply with reboot deferral matching Start power update actions.
**Choices:**
- A) Full update UX + offline update restart
- B) Keep check-update stubs polished
- C) Link to GNOME Software / Discover only
- D) Skip
- E) Other (specify)

ANSWER: A

## 092 — Privacy dashboard (camera/mic/location indicators)
**Community:** Privacy indicators + per-app toggles.
**MDE today:** Privacy pages exist.
**Suggestion:** Taskbar privacy indicator when mic/camera in use + click to see apps.
**Choices:**
- A) Live privacy indicators + app list
- B) Settings toggles only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 093 — Firewall / Security center status freshness
**Community:** Security center glance.
**MDE today:** Security/firewall pages.
**Suggestion:** Home card health (firewalld, SELinux, updates, disk) like Win11 Home.
**Choices:**
- A) Home security health cards
- B) Keep separate pages
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 094 — Touchpad gestures Win11-like
**Community:** Three/four finger gestures.
**MDE today:** Touchpad settings page.
**Suggestion:** Map 3-finger swipe desktops, 4-finger Start/overview if hardware allows.
**Choices:**
- A) Full gesture map UI
- B) Basic scroll/tap only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 095 — Touch / tablet mode / OSK auto
**Community:** Tablet posture expectations.
**MDE today:** On-screen keyboard exists.
**Suggestion:** Auto-OSK on text focus for touch; optional tablet layout.
**Choices:**
- A) Auto-OSK + tablet posture
- B) Manual OSK only
- C) Skip
- D) Defer
- E) Other (specify)

ANSWER: A

## 096 — Multi-monitor taskbar behaviors
**Community:** Show taskbar on all displays / apps where open.
**MDE today:** `taskbar_all_displays` default true.
**Suggestion:** Win11 behaviors: buttons on all taskbars vs main only vs where window is open.
**Choices:**
- A) Three Win11-like multi-monitor modes
- B) All displays on/off only
- C) Main display taskbar only forever
- D) Skip
- E) Other (specify)

ANSWER: A

## 097 — Clock seconds / ISO date (already knobs — surface them)
**Community:** Clock with seconds; ISO date for power users.
**MDE today:** `clock_seconds`, `clock_date_iso` exist.
**Suggestion:** Expose clearly in Taskbar/Date & time settings (not only Advanced/TOML).
**Choices:**
- A) Surface in Taskbar + Date & time UI
- B) Advanced/TOML only is fine
- C) Skip
- D) Add timezone flyout calendar world clocks
- E) Other (specify)

ANSWER: A

## 098 — World clocks in NC vs Widgets
**Community:** Additional clocks in Win11 Date & time / NC.
**MDE today:** Multi-TZ clocks live on Widgets board (W435); NC header is primarily local + calendar.
**Suggestion:** Surface 1–2 world clocks in NC header (or link out), matching Win11 Date & time extras.
**Choices:**
- A) Add up to 2 world clocks in NC header
- B) Keep world clocks in Widgets only
- C) Settings › Date & time list only
- D) Skip
- E) Other (specify)

ANSWER: A

## 099 — Feedback / diagnostics UX naming
**Community:** Feedback Hub is mocked on Win11 clones; power users want diagnostics export.
**MDE today:** About Feedback already copies `journalctl --user` + mailto (W409); older “coming soon” label may linger in copy.
**Suggestion:** Rename to “Export diagnostics / Send feedback” and attach optional issue template + smoke log bundle.
**Choices:**
- A) Rename + richer diagnostic bundle + issue template
- B) Keep current journal+mailto
- C) Remove Feedback entry
- D) Skip
- E) Other (specify)

ANSWER: C

## 100 — Quiet hours / Focus preset pack as “community recommended” profile
**Community:** Bundles of tweaks applied on every install.
**MDE today:** Profiles in config.toml.
**Suggestion:** Ship named profiles: `Community Classic` (left taskbar, no widgets/search, more pins, local search) and `Stock Win11` (center, widgets on).
**Choices:**
- A) Ship Community Classic + Stock Win11 profiles
- B) One Quiet profile only
- C) User-made profiles only
- D) Skip
- E) Other (specify)

ANSWER: A
