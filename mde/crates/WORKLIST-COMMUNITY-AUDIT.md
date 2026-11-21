# Worklist from MDE × Win11 community audit

Generated from 100 MCQ decisions (goal: audit MDE GUI vs community Win11 recommendations).

**Totals:** 100 items — P1=73, P2=18, P3=1, skip=8

## Priority summary

| Priority | Count | Meaning |
|----------|------:|---------|
| P1 | 73 | Primary build / high community parity |
| P2 | 18 | Secondary / polish / alternate path |
| P3 | 1 | Lower urgency |
| deferred | 0 | Explicitly deferred |
| skip | 8 | Won't do (recorded for audit trail) |

## Index (doable items first)

| ID | Pri | Decision | Item |
|----|-----|----------|------|
| A001 | P1 | A | Taskbar left alignment as community preset / default |
| A002 | P1 | A | Taskbar Search modes editable in Settings |
| A003 | P1 | A | Widgets button off by default (community quiet-desktop) |
| A004 | P1 | A | Taskbar badges toggle (message counts) |
| A005 | P1 | A | Never combine / show labels (ExplorerPatcher / Windhawk demand) |
| A007 | P1 | A | Dynamic / acrylic taskbar transparency (StartAllBack / Windhawk Taskbar Styler) |
| A008 | P1 | A | Small taskbar / compact icon size (Windhawk classic) |
| A009 | P1 | A | Taskbar auto-hide polish |
| A010 | P1 | A | Start “More pins” layout density |
| A011 | P1 | A | Start folders next to power (Settings, Explorer, Downloads…) |
| A012 | P1 | A | Keep Start free of promo / recommendation chrome |
| A013 | P1 | A | StartAllBack / Start11-style vertical Start |
| A014 | P1 | A | Start pin folders / large pins polish |
| A015 | P1 | A | PowerToys Run / Command Palette launcher |
| A016 | P1 | A | FancyZones-style custom snap layouts |
| A017 | P1 | A | Snap Groups restore in Alt+Tab / taskbar |
| A018 | P1 | A | Always On Top discoverability (feature exists) |
| A019 | P1 | A | Color Picker polish (path exists) |
| A022 | P1 | A | Keyboard Manager / remapper (PowerToys) |
| A023 | P1 | A | Awake / keep awake utility (PowerToys) |
| A024 | P1 | A | Mouse Find My Mouse / highlighter (PowerToys) |
| A026 | P1 | A | Workspaces / project launch layouts (PowerToys Workspaces) |
| A030 | P1 | A | Jump lists with real recent files |
| A032 | P1 | A | Per-app notification prefs beyond mute |
| A035 | P1 | A | QS Edit tiles that persist like Win11 |
| A036 | P1 | A | QS volume mixer polish vs Settings parity |
| A037 | P1 | A+C | QS Bluetooth flyout depth |
| A038 | P1 | A | QS Wi-Fi password / join flow |
| A039 | P1 | A | Hotspot / VPN / Cast reliability |
| A040 | P1 | A | Tray overflow / hidden icons polish |
| A041 | P1 | A | Clipboard history: images + pin (community Win+V) |
| A042 | P1 | A | Advanced Paste beyond paste-as-file |
| A045 | P1 | A | Show file extensions / hidden files defaults |
| A046 | P1 | A | Virtual desktops naming + wallpaper per desktop |
| A049 | P1 | A | Window Grab and Move / AltSnap |
| A050 | P1 | A | Title bar accent / mica / SSD corner radius |
| A051 | P1 | A | Light Switch / dark schedule reliability |
| A052 | P1 | A | Accent colour on Start/taskbar toggles (Win11 Colours) |
| A053 | P1 | A | Transparency effects master switch |
| A054 | P1 | A | Bing/Spotlight wallpaper rotation |
| A055 | P1 | A | Lock screen media controls that feel native |
| A056 | P1 | A | Greeter accessibility / keyboard layout real options |
| A058 | P1 | A | Account picture change from Start |
| A059 | P1 | A | Night light strength + schedule UI |
| A060 | P1 | A | Display arrange drag-canvas (multi-monitor) |
| A061 | P1 | A | Project modes Super+P reliability |
| A062 | P1 | A | HDR / refresh rate / scale per monitor clarity |
| A063 | P1 | A | Startup apps manager like Win11 |
| A064 | P1 | A | Default apps by file type / protocol |
| A065 | P1 | A | Installed apps uninstall + repair |
| A066 | P1 | A | Game Mode that does something measurable |
| A070 | P1 | A | Magnifier fidelity (tool/scale path exists) |
| A071 | P1 | A | High contrast themes that actually apply |
| A072 | P1 | A | Text size system-wide slider |
| A073 | P1 | A | Shortcut Guide overlay (PowerToys) |
| A076 | P1 | A | Image Resizer context action |
| A078 | P1 | A | Nearby Share / KDE Connect remaining gaps |
| A080 | P1 | A | Taskbar context menu completeness |
| A082 | P1 | A | End task from taskbar (Win11 recent feature) |
| A083 | P1 | A | Progress bars on taskbar buttons |
| A087 | P1 | A | Windhawk-like modular shell styler API |
| A088 | P1 | A | Animation reduce / disable (community perf/a11y) |
| A089 | P1 | A | Battery flyout details (Win11 QS) |
| A090 | P1 | A | Storage Sense analogue |
| A091 | P1 | A | Windows Update analogue clarity (dnf/flatpak) |
| A092 | P1 | A | Privacy dashboard (camera/mic/location indicators) |
| A093 | P1 | A | Firewall / Security center status freshness |
| A094 | P1 | A | Touchpad gestures Win11-like |
| A095 | P1 | A | Touch / tablet mode / OSK auto |
| A096 | P1 | A | Multi-monitor taskbar behaviors |
| A097 | P1 | A | Clock seconds / ISO date (already knobs — surface them) |
| A098 | P1 | A | World clocks in NC vs Widgets |
| A100 | P1 | A | Quiet hours / Focus preset pack as “community recommended” profile |
| A006 | P2 | C | Taskbar on top / any edge (StartAllBack headline feature) |
| A021 | P2 | B | Peek file preview (PowerToys) |
| A027 | P2 | C | Classic full context menu (ExplorerPatcher demand) |
| A029 | P2 | C | Taskbar thumbnail peek (deferred W443) |
| A033 | P2 | C | Focus Assist schedules / automatic rules |
| A043 | P2 | C | Widgets board: curation & third-party API |
| A044 | P2 | B | File Explorer–class shell (deferred W8) |
| A047 | P2 | C | Task View thumbnail fidelity |
| A048 | P2 | B | Show-desktop peek-on-hover |
| A057 | P2 | B | Switch user on lock for real (multi-seat/session) |
| A067 | P2 | B | Captures / Xbox-like screenshot+record |
| A068 | P2 | B | Emoji panel depth (native grid exists) |
| A069 | P2 | B | Voice typing / dictation polish (pipeline exists) |
| A075 | P2 | B | PowerRename / bulk rename |
| A081 | P2 | C | Task Manager quick access from taskbar/Ctrl+Shift+Esc |
| A084 | P2 | C | Search without web / local-only mode |
| A085 | P2 | C | Content search scale (rg path already works) |
| A099 | P2 | C | Feedback / diagnostics UX naming |
| A031 | P3 | D | NC calendar / agenda depth |

### Skipped (audit trail only)

| ID | Decision | Item |
|----|----------|------|
| A020 | C | Text Extractor / OCR from screen (PowerToys) |
| A025 | D | Screen Ruler / ZoomIt-like annotate (PowerToys) |
| A028 | D | Alt+Tab compact vs thumbnail modes |
| A034 | D | Focus sessions depth (timer already real) |
| A074 | C | Hosts editor / env vars (PowerToys admin toys) |
| A077 | B | File Locksmith “who’s using this file” |
| A079 | C | Debloat / telemetry-analogue: quiet recommendations |
| A086 | D | Calculator in Start search (keep + history) |

---

## Detailed work items

### A001 — Taskbar left alignment as community preset / default

- **Priority:** `P1`
- **Decision:** `A` — Home/first-run “Classic left” preset (keep center default)
- **Community driver:** Top Win11 tweak — Personalization → Taskbar → Left (Windows Central, Reddit).
- **MDE today:** Settings › Taskbar already has Left/Center choices (`SetAlign`); `taskbar_left`/`taskbar_align` in config; **default remains center**.
- **Intent:** Add a Home/first-run **“Classic left taskbar”** preset (and/or change the install default) so community preference is one click, not buried discovery.

**Acceptance criteria:**
1. Behavior matches decision `A` (Home/first-run “Classic left” preset (keep center default)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Settings › Taskbar already has Left/Center choices (`SetAlign`); `taskbar_left`/`taskbar_align` in config; **default remains center**.
2. Implement decision **A**: Home/first-run “Classic left” preset (keep center default)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A001 (Taskbar left alignment as community preset / default); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A002 — Taskbar Search modes editable in Settings

- **Priority:** `P1`
- **Decision:** `A` — Full hide/icon/box chooser with live apply (high)
- **Community driver:** Hide Search from taskbar; use Start / Win+S instead.
- **MDE today:** `taskbar_search` defaults to `"hide"`; Settings shows Search mode as a **status row only** (not hide/icon/box chooser).
- **Intent:** Add Win11-like Search mode choices (Hide / Search icon / Search box) with live shell apply.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full hide/icon/box chooser with live apply (high)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `taskbar_search` defaults to `"hide"`; Settings shows Search mode as a **status row only** (not hide/icon/box chooser).
2. Implement decision **A**: Full hide/icon/box chooser with live apply (high)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A002 (Taskbar Search modes editable in Settings); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A003 — Widgets button off by default (community quiet-desktop)

- **Priority:** `P1`
- **Decision:** `A` — Default Widgets button OFF; keep board for Win+W
- **Community driver:** Disable Widgets / news feed; many remove the button entirely.
- **MDE today:** `widgets_btn` defaults true; board has real weather, RSS news, OSRM traffic, Stooq markets, photos, system, agenda/clock (Sports/To Do out of scope).
- **Intent:** Match community quiet installs: default Widgets button OFF (Win+W still opens board) and/or a “hide news/markets feeds” master switch.

**Acceptance criteria:**
1. Behavior matches decision `A` (Default Widgets button OFF; keep board for Win+W) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `widgets_btn` defaults true; board has real weather, RSS news, OSRM traffic, Stooq markets, photos, system, agenda/clock (Sports/To Do out of scope).
2. Implement decision **A**: Default Widgets button OFF; keep board for Win+W
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A003 (Widgets button off by default (community quiet-desktop)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A004 — Taskbar badges toggle (message counts)

- **Priority:** `P1`
- **Decision:** `A` — Implement real badge protocol + keep Settings toggle
- **Community driver:** Often disabled to reduce distraction; when on, expect real counts.
- **MDE today:** `taskbar_badges` exists; Unity/unread/progress heuristics removed (W439/W440) — no app progress/unread protocol.
- **Intent:** Either implement a real badge/urgency path (SNI/Unity-like) or default badges OFF and clarify Settings copy that counts are not available yet.

**Acceptance criteria:**
1. Behavior matches decision `A` (Implement real badge protocol + keep Settings toggle) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `taskbar_badges` exists; Unity/unread/progress heuristics removed (W439/W440) — no app progress/unread protocol.
2. Implement decision **A**: Implement real badge protocol + keep Settings toggle
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A004 (Taskbar badges toggle (message counts)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A005 — Never combine / show labels (ExplorerPatcher / Windhawk demand)

- **Priority:** `P1`
- **Decision:** `A` — Full Never/When full/Always + labels (high priority)
- **Community driver:** Ungroup taskbar buttons + labels is a top ExplorerPatcher/Windhawk ask.
- **MDE today:** `taskbar_combine` default `"always"`; “Show labels” still stub-ish (W257).
- **Intent:** Support Never combine + labels next to icons like Win10/ExplorerPatcher.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full Never/When full/Always + labels (high priority)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `taskbar_combine` default `"always"`; “Show labels” still stub-ish (W257).
2. Implement decision **A**: Full Never/When full/Always + labels (high priority)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A005 (Never combine / show labels (ExplorerPatcher / Windhawk demand)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A006 — Taskbar on top / any edge (StartAllBack headline feature)

- **Priority:** `P2`
- **Decision:** `C` — Bottom only forever
- **Community driver:** StartAllBack sells taskbar-on-top / any edge.
- **MDE today:** Bottom bar via layer-shell; no top/left/right placement.
- **Intent:** Allow taskbar edge placement (at least top + bottom).

**Acceptance criteria:**
1. Behavior matches decision `C` (Bottom only forever) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Bottom bar via layer-shell; no top/left/right placement.
2. Implement decision **C**: Bottom only forever
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A006 (Taskbar on top / any edge (StartAllBack headline feature)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A007 — Dynamic / acrylic taskbar transparency (StartAllBack / Windhawk Taskbar Styler)

- **Priority:** `P1`
- **Decision:** `A` — Dynamic opacity + blur presets
- **Community driver:** Dynamic transparency / glassmorphism heavily recommended.
- **MDE today:** `theme.transparency` + `opacity` exist for chrome.
- **Intent:** Add “dynamic transparency” (more opaque over content / clearer over wallpaper) and optional blur strength presets.

**Acceptance criteria:**
1. Behavior matches decision `A` (Dynamic opacity + blur presets) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `theme.transparency` + `opacity` exist for chrome.
2. Implement decision **A**: Dynamic opacity + blur presets
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A007 (Dynamic / acrylic taskbar transparency (StartAllBack / Windhawk Taskbar Styler)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A008 — Small taskbar / compact icon size (Windhawk classic)

- **Priority:** `P1`
- **Decision:** `A` — Compact + Comfortable sizes
- **Community driver:** Smaller taskbar height + icon size is a top Windhawk mod.
- **MDE today:** Fixed ~48px Win11 density.
- **Intent:** Offer Compact (32–40px) and Comfortable (48px) taskbar sizes.

**Acceptance criteria:**
1. Behavior matches decision `A` (Compact + Comfortable sizes) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Fixed ~48px Win11 density.
2. Implement decision **A**: Compact + Comfortable sizes
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A008 (Small taskbar / compact icon size (Windhawk classic)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A009 — Taskbar auto-hide polish

- **Priority:** `P1`
- **Decision:** `A` — Full Win11-parity auto-hide behavior
- **Community driver:** Auto-hide remains a common power-user setting.
- **MDE today:** `taskbar_autohide` exists; polish/toggle history in shell.
- **Intent:** Match Win11: reveal on edge hover, hide after leave, don’t steal focus from fullscreen.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full Win11-parity auto-hide behavior) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `taskbar_autohide` exists; polish/toggle history in shell.
2. Implement decision **A**: Full Win11-parity auto-hide behavior
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A009 (Taskbar auto-hide polish); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A010 — Start “More pins” layout density

- **Priority:** `P1`
- **Decision:** `A` — Promote More pins as primary Start layout + live preview
- **Community driver:** More pins / fewer recommendations is the #1 Start tweak on Win11.
- **MDE today:** Recommended section already removed (W456/W463); `start.layout` supports default/more_pins; pin folders + large pins via extras.
- **Intent:** Make More pins the clear community preset in Settings › Start (density + collapsed pinned), matching Win11 “More pins” without bringing Recommended back.

**Acceptance criteria:**
1. Behavior matches decision `A` (Promote More pins as primary Start layout + live preview) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Recommended section already removed (W456/W463); `start.layout` supports default/more_pins; pin folders + large pins via extras.
2. Implement decision **A**: Promote More pins as primary Start layout + live preview
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A010 (Start “More pins” layout density); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A011 — Start folders next to power (Settings, Explorer, Downloads…)

- **Priority:** `P1`
- **Decision:** `A` — Polish Folders settings to Win11 checklist parity
- **Community driver:** Enable folders beside power button.
- **MDE today:** `start.folders` + footer glyphs already implemented (W451).
- **Intent:** Settings › Start › Folders should mirror Win11 checklist UX with live preview.

**Acceptance criteria:**
1. Behavior matches decision `A` (Polish Folders settings to Win11 checklist parity) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `start.folders` + footer glyphs already implemented (W451).
2. Implement decision **A**: Polish Folders settings to Win11 checklist parity
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A011 (Start folders next to power (Settings, Explorer, Downloads…)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A012 — Keep Start free of promo / recommendation chrome

- **Priority:** `P1`
- **Decision:** `A` — Formal no-promo policy + Tips opt-in only (high)
- **Community driver:** Turn off app recommendations / account notifications / Store promos.
- **MDE today:** Recommended section already removed; Tips widget and account stubs remain elsewhere.
- **Intent:** Lock in a “no promo surfaces” policy: no Recommended return, Tips opt-in only, no account marketing rows on Start.

**Acceptance criteria:**
1. Behavior matches decision `A` (Formal no-promo policy + Tips opt-in only (high)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Recommended section already removed; Tips widget and account stubs remain elsewhere.
2. Implement decision **A**: Formal no-promo policy + Tips opt-in only (high)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A012 (Keep Start free of promo / recommendation chrome); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A013 — StartAllBack / Start11-style vertical Start

- **Priority:** `P1`
- **Decision:** `A` — Add Classic vertical Start layout option
- **Community driver:** Paid Start replacements offer vertical / Win10 Start.
- **MDE today:** Modern centered Start card.
- **Intent:** Optional classic vertical Start (all apps left, pins right) à la Windhawk side-by-side / Start11.

**Acceptance criteria:**
1. Behavior matches decision `A` (Add Classic vertical Start layout option) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Modern centered Start card.
2. Implement decision **A**: Add Classic vertical Start layout option
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A013 (StartAllBack / Start11-style vertical Start); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A014 — Start pin folders / large pins polish

- **Priority:** `P1`
- **Decision:** `A` — Settings UI for folders + sizes + optional Medium
- **Community driver:** Grouping + tile sizes still requested (even post-Live Tiles).
- **MDE today:** `extras.start.pin_folders` + `start.pin_large` already support folders and large pins; drag reorder exists.
- **Intent:** Settings › Start UX to manage folders/sizes (not only shell gestures) + Medium size if community wants three sizes.

**Acceptance criteria:**
1. Behavior matches decision `A` (Settings UI for folders + sizes + optional Medium) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `extras.start.pin_folders` + `start.pin_large` already support folders and large pins; drag reorder exists.
2. Implement decision **A**: Settings UI for folders + sizes + optional Medium
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A014 (Start pin folders / large pins polish); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A015 — PowerToys Run / Command Palette launcher

- **Priority:** `P1`
- **Decision:** `A` — Full Command Palette with plugins
- **Community driver:** PowerToys Run / Command Palette is the #1 productivity addon.
- **MDE today:** Start search covers apps/docs/web hint; no dedicated Run palette with plugins.
- **Intent:** Add Super+R / Super+Space Command Palette (apps, files, calc, shell commands, plugins).

**Acceptance criteria:**
1. Behavior matches decision `A` (Full Command Palette with plugins) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Start search covers apps/docs/web hint; no dedicated Run palette with plugins.
2. Implement decision **A**: Full Command Palette with plugins
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A015 (PowerToys Run / Command Palette launcher); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A016 — FancyZones-style custom snap layouts

- **Priority:** `P1`
- **Decision:** `A` — Full FancyZones analogue (editor + layouts)
- **Community driver:** FancyZones is the flagship PowerToys window tool.
- **MDE today:** Snap flyout has halves/quarters/thirds/ultrawide presets + assist overlay + edge drag; picker shows titles only (no live thumbs); no user-drawn zone editor.
- **Intent:** Add user-editable zone layouts (FancyZones analogue) on top of existing presets.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full FancyZones analogue (editor + layouts)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Snap flyout has halves/quarters/thirds/ultrawide presets + assist overlay + edge drag; picker shows titles only (no live thumbs); no user-drawn zone editor.
2. Implement decision **A**: Full FancyZones analogue (editor + layouts)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A016 (FancyZones-style custom snap layouts); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A017 — Snap Groups restore in Alt+Tab / taskbar

- **Priority:** `P1`
- **Decision:** `A` — Persist + restore Snap Groups
- **Community driver:** Snap Groups are a praised Win11 multitasking feature.
- **MDE today:** Snap assist exists; group memory unclear / incomplete.
- **Intent:** Persist snap groups and restore via Alt+Tab or taskbar hover.

**Acceptance criteria:**
1. Behavior matches decision `A` (Persist + restore Snap Groups) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Snap assist exists; group memory unclear / incomplete.
2. Implement decision **A**: Persist + restore Snap Groups
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A017 (Snap Groups restore in Alt+Tab / taskbar); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A018 — Always On Top discoverability (feature exists)

- **Priority:** `P1`
- **Decision:** `A` — Shortcut + Settings + title-bar indicator
- **Community driver:** PowerToys Always On Top is widely used (often Win+Ctrl+T).
- **MDE today:** Compositor AlwaysOnTop via winmenu / `mde-window-action` (W412) — real z-order, not a stub.
- **Intent:** Add a dedicated shortcut + Settings toggle and optional title-bar pin glyph for discoverability.

**Acceptance criteria:**
1. Behavior matches decision `A` (Shortcut + Settings + title-bar indicator) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Compositor AlwaysOnTop via winmenu / `mde-window-action` (W412) — real z-order, not a stub.
2. Implement decision **A**: Shortcut + Settings + title-bar indicator
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A018 (Always On Top discoverability (feature exists)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A019 — Color Picker polish (path exists)

- **Priority:** `P1`
- **Decision:** `A` — Native picker UI + history
- **Community driver:** System color picker is a staple (PowerToys / Super+Shift+C).
- **MDE today:** Screenshot/color-pick path via portal + tools (W427); not a dedicated history-rich PowerToys picker UI.
- **Intent:** Native eyedropper with hex/rgb/hsl copy + recent colors history.

**Acceptance criteria:**
1. Behavior matches decision `A` (Native picker UI + history) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Screenshot/color-pick path via portal + tools (W427); not a dedicated history-rich PowerToys picker UI.
2. Implement decision **A**: Native picker UI + history
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A019 (Color Picker polish (path exists)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A020 — Text Extractor / OCR from screen (PowerToys)

- **Priority:** `skip`
- **Decision:** `C` — Skip
- **Community driver:** OCR grab is highly recommended for power users.
- **MDE today:** No equivalent.
- **Intent:** Region select → OCR → clipboard (tesseract/gnome OCR).

**Status:** skip — no implementation work. Kept for audit completeness.

### A021 — Peek file preview (PowerToys)

- **Priority:** `P2`
- **Decision:** `B` — Wait for File Explorer epic (W8)
- **Community driver:** Spacebar Peek is beloved.
- **MDE today:** File Explorer deferred (W8); no Peek.
- **Intent:** Quick preview for selected file from Start search / future Explorer.

**Acceptance criteria:**
1. Behavior matches decision `B` (Wait for File Explorer epic (W8)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: File Explorer deferred (W8); no Peek.
2. Implement decision **B**: Wait for File Explorer epic (W8)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A021 (Peek file preview (PowerToys)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A022 — Keyboard Manager / remapper (PowerToys)

- **Priority:** `P1`
- **Decision:** `A` — Full Keyboard Manager
- **Community driver:** Remap keys/shortcuts without Autohotkey.
- **MDE today:** Accessibility shortcuts page; limited remaps.
- **Intent:** GUI remapper for keys and shortcuts stored in config.toml.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full Keyboard Manager) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Accessibility shortcuts page; limited remaps.
2. Implement decision **A**: Full Keyboard Manager
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A022 (Keyboard Manager / remapper (PowerToys)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A023 — Awake / keep awake utility (PowerToys)

- **Priority:** `P1`
- **Decision:** `A` — Add Awake to QS + tray
- **Community driver:** Awake is a common “long job” helper.
- **MDE today:** Power page / idle lock; no temporary inhibit UI.
- **Intent:** Tray/QS “Keep awake” with timed or indefinite inhibit.

**Acceptance criteria:**
1. Behavior matches decision `A` (Add Awake to QS + tray) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Power page / idle lock; no temporary inhibit UI.
2. Implement decision **A**: Add Awake to QS + tray
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A023 (Awake / keep awake utility (PowerToys)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A024 — Mouse Find My Mouse / highlighter (PowerToys)

- **Priority:** `P1`
- **Decision:** `A` — Both Find My Mouse + Highlighter
- **Community driver:** Find My Mouse + click highlighter for presentations/a11y.
- **MDE today:** Limited pointer a11y.
- **Intent:** Ctrl×2 spotlight + optional click highlighter.

**Acceptance criteria:**
1. Behavior matches decision `A` (Both Find My Mouse + Highlighter) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Limited pointer a11y.
2. Implement decision **A**: Both Find My Mouse + Highlighter
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A024 (Mouse Find My Mouse / highlighter (PowerToys)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A025 — Screen Ruler / ZoomIt-like annotate (PowerToys)

- **Priority:** `skip`
- **Decision:** `D` — Skip
- **Community driver:** Measure/annotate tools for tech users.
- **MDE today:** Magnifier stub; screenshot path exists.
- **Intent:** Pixel ruler and/or ZoomIt-style zoom+annotate.

**Status:** skip — no implementation work. Kept for audit completeness.

### A026 — Workspaces / project launch layouts (PowerToys Workspaces)

- **Priority:** `P1`
- **Decision:** `A` — Full Workspaces feature
- **Community driver:** Launch app sets to positions.
- **MDE today:** Display profiles; no app workspace presets.
- **Intent:** Save/restore named window layouts + apps.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full Workspaces feature) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Display profiles; no app workspace presets.
2. Implement decision **A**: Full Workspaces feature
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A026 (Workspaces / project launch layouts (PowerToys Workspaces)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A027 — Classic full context menu (ExplorerPatcher demand)

- **Priority:** `P2`
- **Decision:** `C` — Out of scope until W8
- **Community driver:** “Show more options” hate; restore classic menu.
- **MDE today:** File Explorer deferred; shell menus are custom.
- **Intent:** When Explorer lands (or via Thunar), ship dense classic-style context menus by default.

**Acceptance criteria:**
1. Behavior matches decision `C` (Out of scope until W8) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: File Explorer deferred; shell menus are custom.
2. Implement decision **C**: Out of scope until W8
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A027 (Classic full context menu (ExplorerPatcher demand)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A028 — Alt+Tab compact vs thumbnail modes

- **Priority:** `skip`
- **Decision:** `D` — Skip
- **Community driver:** Many prefer classic Alt+Tab icons vs large thumbnails (ExplorerPatcher).
- **MDE today:** Alt+Tab overlay with faces; `alttab.show_desktop` toggle + synthetic Desktop entry (W208); not live thumbnails.
- **Intent:** Compact (icons) vs Thumbnail modes; keep Desktop include toggle.

**Status:** skip — no implementation work. Kept for audit completeness.

### A029 — Taskbar thumbnail peek (deferred W443)

- **Priority:** `P2`
- **Decision:** `C` — Keep title-only peek / leave deferred
- **Community driver:** Live thumbnails on hover are expected Win11 chrome.
- **MDE today:** Explicitly **deferred W443** — hover shows truncated title only (`spikes/thumbnails.md`); no live thumbnail.
- **Intent:** Prioritize compositor-backed hover peek thumbnails vs keep deferred.

**Acceptance criteria:**
1. Behavior matches decision `C` (Keep title-only peek / leave deferred) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Explicitly **deferred W443** — hover shows truncated title only (`spikes/thumbnails.md`); no live thumbnail.
2. Implement decision **C**: Keep title-only peek / leave deferred
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A029 (Taskbar thumbnail peek (deferred W443)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A030 — Jump lists with real recent files

- **Priority:** `P1`
- **Decision:** `A` — Deeper recent + pin forever
- **Community driver:** Jump lists are core Win productivity.
- **MDE today:** Jump lists exist with recent files (menus.rs); depth/quality varies.
- **Intent:** Deepen recent tracking (GTK/Qt/xdg), pin forever, clear list.

**Acceptance criteria:**
1. Behavior matches decision `A` (Deeper recent + pin forever) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Jump lists exist with recent files (menus.rs); depth/quality varies.
2. Implement decision **A**: Deeper recent + pin forever
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A030 (Jump lists with real recent files); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A031 — NC calendar / agenda depth

- **Priority:** `P3`
- **Decision:** `D` — Strip agenda; month grid only
- **Community driver:** Win11 NC clock→calendar + agenda expectations.
- **MDE today:** Month grid + local agenda (`agenda.rs`, `~/.local/share/mde/agenda.json`) already in NC.
- **Intent:** Next step is sync (CalDAV/Nextcloud) and/or richer agenda CRUD — not greenfield calendar chrome.

**Acceptance criteria:**
1. Behavior matches decision `D` (Strip agenda; month grid only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Month grid + local agenda (`agenda.rs`, `~/.local/share/mde/agenda.json`) already in NC.
2. Implement decision **D**: Strip agenda; month grid only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A031 (NC calendar / agenda depth); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A032 — Per-app notification prefs beyond mute

- **Priority:** `P1`
- **Decision:** `A` — Full per-app notification prefs page + deep link
- **Community driver:** Per-app banners/sounds/priority in Win11 Settings.
- **MDE today:** NC can mute/unmute per app; group Settings opens System › Notifications (W185 path); no full per-app banner/sound/priority page.
- **Intent:** Win11-like per-app prefs (banners, sound, priority allowlist) with deep link preselecting the app.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full per-app notification prefs page + deep link) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: NC can mute/unmute per app; group Settings opens System › Notifications (W185 path); no full per-app banner/sound/priority page.
2. Implement decision **A**: Full per-app notification prefs page + deep link
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A032 (Per-app notification prefs beyond mute); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A033 — Focus Assist schedules / automatic rules

- **Priority:** `P2`
- **Decision:** `C` — Quiet hours only polish
- **Community driver:** Priority only / alarms / automatic when duplicating display.
- **MDE today:** DND + priority + quiet hours fields.
- **Intent:** Automatic Focus rules (schedule, when casting/fullscreen, game mode).

**Acceptance criteria:**
1. Behavior matches decision `C` (Quiet hours only polish) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: DND + priority + quiet hours fields.
2. Implement decision **C**: Quiet hours only polish
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A033 (Focus Assist schedules / automatic rules); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A034 — Focus sessions depth (timer already real)

- **Priority:** `skip`
- **Decision:** `D` — Skip
- **Community driver:** Focus sessions in Win11 Clock / Settings.
- **MDE today:** QS 25m focus timer toggles DND and ends on expiry; Settings Focus can start/end sessions.
- **Intent:** Add session history, custom durations, and optional break timer like Win11 Focus.

**Status:** skip — no implementation work. Kept for audit completeness.

### A035 — QS Edit tiles that persist like Win11

- **Priority:** `P1`
- **Decision:** `A` — Drag-reorder + gallery UI polish
- **Community driver:** Rearrange Quick Settings tiles.
- **MDE today:** Edit mode persists `extras qs.tiles` (W98 done).
- **Intent:** Drag-reorder polish + Win11-like “Add or remove quick settings” gallery.

**Acceptance criteria:**
1. Behavior matches decision `A` (Drag-reorder + gallery UI polish) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Edit mode persists `extras qs.tiles` (W98 done).
2. Implement decision **A**: Drag-reorder + gallery UI polish
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A035 (QS Edit tiles that persist like Win11); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A036 — QS volume mixer polish vs Settings parity

- **Priority:** `P1`
- **Decision:** `A` — Parity polish + discoverability pass
- **Community driver:** Per-app volume from QS is expected.
- **MDE today:** QS nested app mixer via `wpctl` already exists alongside Settings › Volume mixer.
- **Intent:** Ensure QS mixer matches Settings (device picker, mute, % labels) and is discoverable from the volume row.

**Acceptance criteria:**
1. Behavior matches decision `A` (Parity polish + discoverability pass) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: QS nested app mixer via `wpctl` already exists alongside Settings › Volume mixer.
2. Implement decision **A**: Parity polish + discoverability pass
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A036 (QS volume mixer polish vs Settings parity); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A037 — QS Bluetooth flyout depth

- **Priority:** `P1`
- **Decision:** `A+C` — QS connect/disconnect for known devices (high) + Full pair wizard in QS
- **User note:** QS connect/disconnect for known devices AND full pair wizard in QS
- **Community driver:** Action Center BT device list is table stakes.
- **MDE today:** QS BT tile + Settings device list/connect exist; confirm QS expandable list parity with Settings (pair vs connect).
- **Intent:** Ensure QS BT flyout can connect/disconnect known devices without opening Settings; pair stays in Settings if needed.

**Acceptance criteria:**
1. Behavior matches decision `A+C` (QS connect/disconnect for known devices (high) + Full pair wizard in QS) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: QS BT tile + Settings device list/connect exist; confirm QS expandable list parity with Settings (pair vs connect).
2. Implement decision **A+C**: QS connect/disconnect for known devices (high) + Full pair wizard in QS
3. Honor user note: QS connect/disconnect for known devices AND full pair wizard in QS
4. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
5. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
6. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
7. VT1 verify acceptance for A037 (QS Bluetooth flyout depth); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A038 — QS Wi-Fi password / join flow

- **Priority:** `P1`
- **Decision:** `A` — Inline password join in QS (high)
- **Community driver:** Join networks from QS without opening full Settings.
- **MDE today:** Unknown-SSID password often still defers to NM/nmtui (W147 note); known networks connect from Settings/QS paths.
- **Intent:** Inline password prompt for unknown SSIDs inside QS.

**Acceptance criteria:**
1. Behavior matches decision `A` (Inline password join in QS (high)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Unknown-SSID password often still defers to NM/nmtui (W147 note); known networks connect from Settings/QS paths.
2. Implement decision **A**: Inline password join in QS (high)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A038 (QS Wi-Fi password / join flow); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A039 — Hotspot / VPN / Cast reliability

- **Priority:** `P1`
- **Decision:** `A` — Harden all three + hide when unavailable
- **Community driver:** Hotspot, VPN, Cast in Action Center.
- **MDE today:** QS tiles + nested VPN/hotspot/nearby exist; Cast/share uses xdg-desktop-portal screencast + project modes — quality depends on portal/receiver.
- **Intent:** Harden reliability and hide/disable tiles when backend missing instead of dead ends.

**Acceptance criteria:**
1. Behavior matches decision `A` (Harden all three + hide when unavailable) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: QS tiles + nested VPN/hotspot/nearby exist; Cast/share uses xdg-desktop-portal screencast + project modes — quality depends on portal/receiver.
2. Implement decision **A**: Harden all three + hide when unavailable
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A039 (Hotspot / VPN / Cast reliability); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A040 — Tray overflow / hidden icons polish

- **Priority:** `P1`
- **Decision:** `A` — Dedicated Settings tray-icons page + overflow polish
- **Community driver:** Select which tray icons show / overflow.
- **MDE today:** ≤4 visible SNI + `›` overflow; QS hidden-icon mgmt persists `extras.tray.hidden` (honest empty state W442).
- **Intent:** Settings › Taskbar “Other system tray icons” page mirroring Win11, plus overflow UX polish.

**Acceptance criteria:**
1. Behavior matches decision `A` (Dedicated Settings tray-icons page + overflow polish) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: ≤4 visible SNI + `›` overflow; QS hidden-icon mgmt persists `extras.tray.hidden` (honest empty state W442).
2. Implement decision **A**: Dedicated Settings tray-icons page + overflow polish
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A040 (Tray overflow / hidden icons polish); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A041 — Clipboard history: images + pin (community Win+V)

- **Priority:** `P1`
- **Decision:** `A` — Win+V focus + paste-as-plain + sync UX polish
- **Community driver:** Win+V clipboard history is essential; pin favorites.
- **MDE today:** History with images/pin/clear largely done (T3).
- **Intent:** Keyboard Win+V focus, paste-as-plain, sync polish.

**Acceptance criteria:**
1. Behavior matches decision `A` (Win+V focus + paste-as-plain + sync UX polish) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: History with images/pin/clear largely done (T3).
2. Implement decision **A**: Win+V focus + paste-as-plain + sync UX polish
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A041 (Clipboard history: images + pin (community Win+V)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A042 — Advanced Paste beyond paste-as-file

- **Priority:** `P1`
- **Decision:** `A` — Add text/format transforms (Advanced Paste lite)
- **Community driver:** PowerToys Advanced Paste; Win paste-as-file asks.
- **MDE today:** Clipboard already supports paste-back, paste plain, paste as file, save image, Nearby share.
- **Intent:** Add PowerToys-like transforms (markdown/HTML/JSON, optional AI later) on top of existing paste-as-file.

**Acceptance criteria:**
1. Behavior matches decision `A` (Add text/format transforms (Advanced Paste lite)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Clipboard already supports paste-back, paste plain, paste as file, save image, Nearby share.
2. Implement decision **A**: Add text/format transforms (Advanced Paste lite)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A042 (Advanced Paste beyond paste-as-file); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A043 — Widgets board: curation & third-party API

- **Priority:** `P2`
- **Decision:** `C` — Both A and B
- **User note:** Quiet default card set + per-card toggles AND pluggable third-party widget API
- **Community driver:** Widgets useful only with real content; many users strip news.
- **MDE today:** Weather, RSS news, traffic, markets, photos, system, agenda/clock are real; Sports/To Do out of scope.
- **Intent:** Curate default card set (community quiet vs stock) and/or pluggable widget API.

**Acceptance criteria:**
1. Behavior matches decision `C` (Both A and B) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Weather, RSS news, traffic, markets, photos, system, agenda/clock are real; Sports/To Do out of scope.
2. Implement decision **C**: Both A and B
3. Honor user note: Quiet default card set + per-card toggles AND pluggable third-party widget API
4. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
5. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
6. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
7. VT1 verify acceptance for A043 (Widgets board: curation & third-party API); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A044 — File Explorer–class shell (deferred W8)

- **Priority:** `P2`
- **Decision:** `B` — Deep-integrate Thunar as “MDE Explorer”
- **Community driver:** Explorer customization is half of Win11 modding culture.
- **MDE today:** W8 deferred; use system file manager.
- **Intent:** Prioritize MDE File Explorer (tabs, home, Quick Access).

**Acceptance criteria:**
1. Behavior matches decision `B` (Deep-integrate Thunar as “MDE Explorer”) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: W8 deferred; use system file manager.
2. Implement decision **B**: Deep-integrate Thunar as “MDE Explorer”
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A044 (File Explorer–class shell (deferred W8)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A045 — Show file extensions / hidden files defaults

- **Priority:** `P1`
- **Decision:** `A` — First-run + Settings toggles for FM defaults
- **Community driver:** Every power-user install enables extensions.
- **MDE today:** Depends on file manager.
- **Intent:** MDE first-run sets file manager to show extensions + hidden toggle in Settings.

**Acceptance criteria:**
1. Behavior matches decision `A` (First-run + Settings toggles for FM defaults) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Depends on file manager.
2. Implement decision **A**: First-run + Settings toggles for FM defaults
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A045 (Show file extensions / hidden files defaults); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A046 — Virtual desktops naming + wallpaper per desktop

- **Priority:** `P1`
- **Decision:** `A` — Names + per-desktop wallpaper
- **Community driver:** Win11 desktops with names/wallpapers are popular.
- **MDE today:** Workspaces exist; animation stub polish (W168).
- **Intent:** Named desktops + optional per-desktop wallpaper.

**Acceptance criteria:**
1. Behavior matches decision `A` (Names + per-desktop wallpaper) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Workspaces exist; animation stub polish (W168).
2. Implement decision **A**: Names + per-desktop wallpaper
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A046 (Virtual desktops naming + wallpaper per desktop); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A047 — Task View thumbnail fidelity

- **Priority:** `P2`
- **Decision:** `C` — Keep title/icon Task View
- **Community driver:** Win+Tab Task View with desktop/window previews.
- **MDE today:** Task View UI exists (`taskview.rs`) — per-desktop columns, new/close/move, Super+Tab; window faces are not live compositor thumbnails.
- **Intent:** Add live (or cached) window/desktop thumbnails in Task View.

**Acceptance criteria:**
1. Behavior matches decision `C` (Keep title/icon Task View) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Task View UI exists (`taskview.rs`) — per-desktop columns, new/close/move, Super+Tab; window faces are not live compositor thumbnails.
2. Implement decision **C**: Keep title/icon Task View
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A047 (Task View thumbnail fidelity); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A048 — Show-desktop peek-on-hover

- **Priority:** `P2`
- **Decision:** `B` — Keep click strip only
- **Community driver:** Far-right show-desktop and Aero Peek nostalgia.
- **MDE today:** 2px far-right show-desktop strip + Super+D already exist.
- **Intent:** Optional peek-on-hover (temporarily show desktop while hovering the strip).

**Acceptance criteria:**
1. Behavior matches decision `B` (Keep click strip only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: 2px far-right show-desktop strip + Super+D already exist.
2. Implement decision **B**: Keep click strip only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A048 (Show-desktop peek-on-hover); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A049 — Window Grab and Move / AltSnap

- **Priority:** `P1`
- **Decision:** `A` — Alt+drag move/resize
- **Community driver:** AltSnap / PowerToys Grab and Move widely recommended.
- **MDE today:** Standard Wayland move from titlebar.
- **Intent:** Alt+drag move/resize from anywhere (AltSnap parity).

**Acceptance criteria:**
1. Behavior matches decision `A` (Alt+drag move/resize) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Standard Wayland move from titlebar.
2. Implement decision **A**: Alt+drag move/resize
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A049 (Window Grab and Move / AltSnap); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A050 — Title bar accent / mica / SSD corner radius

- **Priority:** `P1`
- **Decision:** `A` — Rounded SSD + mica
- **Community driver:** Visual polish (mica, rounded) is Windhawk/StartAllBack territory.
- **MDE today:** Accent on titlebars; 2px underline; true corner radius deferred (W44).
- **Intent:** Real rounded SSD corners + mica titlebars.

**Acceptance criteria:**
1. Behavior matches decision `A` (Rounded SSD + mica) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Accent on titlebars; 2px underline; true corner radius deferred (W44).
2. Implement decision **A**: Rounded SSD + mica
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A050 (Title bar accent / mica / SSD corner radius); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A051 — Light Switch / dark schedule reliability

- **Priority:** `P1`
- **Decision:** `A` — Live flip + sunrise/sunset
- **Community driver:** Auto light/dark by time (PowerToys Light Switch / Win11).
- **MDE today:** `theme.dark_schedule` exists.
- **Intent:** Ensure live shell/compositor flip without relogin + sunrise/sunset option.

**Acceptance criteria:**
1. Behavior matches decision `A` (Live flip + sunrise/sunset) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `theme.dark_schedule` exists.
2. Implement decision **A**: Live flip + sunrise/sunset
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A051 (Light Switch / dark schedule reliability); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A052 — Accent colour on Start/taskbar toggles (Win11 Colours)

- **Priority:** `P1`
- **Decision:** `A` — Win11-parity Colours page polish
- **Community driver:** “Show accent on Start and taskbar” is a standard toggle.
- **MDE today:** `accent_on_start_taskbar` exists.
- **Intent:** Settings › Colours should mirror Win11 wording with live preview.

**Acceptance criteria:**
1. Behavior matches decision `A` (Win11-parity Colours page polish) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `accent_on_start_taskbar` exists.
2. Implement decision **A**: Win11-parity Colours page polish
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A052 (Accent colour on Start/taskbar toggles (Win11 Colours)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A053 — Transparency effects master switch

- **Priority:** `P1`
- **Decision:** `A` — Verify master switch kills all translucent chrome
- **Community driver:** Disable transparency for battery/perf/OLED.
- **MDE today:** `theme.transparency` exists.
- **Intent:** Master switch disables mica/acrylic everywhere including flyouts.

**Acceptance criteria:**
1. Behavior matches decision `A` (Verify master switch kills all translucent chrome) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `theme.transparency` exists.
2. Implement decision **A**: Verify master switch kills all translucent chrome
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A053 (Transparency effects master switch); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A054 — Bing/Spotlight wallpaper rotation

- **Priority:** `P1`
- **Decision:** `A` — Spotlight pack (desktop+lock+history)
- **Community driver:** Spotlight-like wallpapers popular.
- **MDE today:** `bing_daily` exists.
- **Intent:** Spotlight-like lock+desktop pack with likes/history.

**Acceptance criteria:**
1. Behavior matches decision `A` (Spotlight pack (desktop+lock+history)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `bing_daily` exists.
2. Implement decision **A**: Spotlight pack (desktop+lock+history)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A054 (Bing/Spotlight wallpaper rotation); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A055 — Lock screen media controls that feel native

- **Priority:** `P1`
- **Decision:** `A` — Full media card on lock
- **Community driver:** Media on lock screen is expected.
- **MDE today:** playerctl line + skip stubs on lock.
- **Intent:** Full media card (art, scrub, volume) on lock/greeter.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full media card on lock) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: playerctl line + skip stubs on lock.
2. Implement decision **A**: Full media card on lock
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A055 (Lock screen media controls that feel native); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A056 — Greeter accessibility / keyboard layout real options

- **Priority:** `P1`
- **Decision:** `A` — Full greeter a11y + layouts
- **Community driver:** Login a11y + layout switch matter.
- **MDE today:** Greeter a11y/layout stubs (W149/W249).
- **Intent:** Real high-contrast, on-screen keyboard, layout switch on greeter.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full greeter a11y + layouts) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Greeter a11y/layout stubs (W149/W249).
2. Implement decision **A**: Full greeter a11y + layouts
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A056 (Greeter accessibility / keyboard layout real options); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A057 — Switch user on lock for real (multi-seat/session)

- **Priority:** `P2`
- **Decision:** `B` — Keep single-user lock only
- **Community driver:** Fast user switching is classic Windows.
- **MDE today:** Switch user limited / “not available” paths (W428/W462).
- **Intent:** Real switch-user via greetd multi-session if feasible.

**Acceptance criteria:**
1. Behavior matches decision `B` (Keep single-user lock only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Switch user limited / “not available” paths (W428/W462).
2. Implement decision **B**: Keep single-user lock only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A057 (Switch user on lock for real (multi-seat/session)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A058 — Account picture change from Start

- **Priority:** `P1`
- **Decision:** `A` — Real avatar picker from Start + Settings
- **Community driver:** Change picture from Start account flyout.
- **MDE today:** Start still uses `AccountPictureStub` / coming soon (W253); Settings › Your info may have partial avatar path.
- **Intent:** Real avatar picker wired to greeter/Start/Settings.

**Acceptance criteria:**
1. Behavior matches decision `A` (Real avatar picker from Start + Settings) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Start still uses `AccountPictureStub` / coming soon (W253); Settings › Your info may have partial avatar path.
2. Implement decision **A**: Real avatar picker from Start + Settings
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A058 (Account picture change from Start); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A059 — Night light strength + schedule UI

- **Priority:** `P1`
- **Decision:** `A` — Strength + schedule UI
- **Community driver:** Blue light filter with schedule is standard.
- **MDE today:** Night light via wlsunset/gammastep; Settings toggle.
- **Intent:** Temperature slider + sunset schedule like Win11.

**Acceptance criteria:**
1. Behavior matches decision `A` (Strength + schedule UI) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Night light via wlsunset/gammastep; Settings toggle.
2. Implement decision **A**: Strength + schedule UI
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A059 (Night light strength + schedule UI); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A060 — Display arrange drag-canvas (multi-monitor)

- **Priority:** `P1`
- **Decision:** `A` — Drag canvas + identify
- **Community driver:** Drag monitors in Settings is expected.
- **MDE today:** Layout file + profiles; UI thin (C4/D4 notes).
- **Intent:** Visual drag-to-arrange displays + identity flash.

**Acceptance criteria:**
1. Behavior matches decision `A` (Drag canvas + identify) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Layout file + profiles; UI thin (C4/D4 notes).
2. Implement decision **A**: Drag canvas + identify
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A060 (Display arrange drag-canvas (multi-monitor)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A061 — Project modes Super+P reliability

- **Priority:** `P1`
- **Decision:** `A` — Harden four modes + identify displays
- **Community driver:** PC screen only / Duplicate / Extend / Second only.
- **MDE today:** Super+P / Connect flyout already drives Extend/Duplicate/Second-only via `mde-output-layout.txt`; PC-only and multi-DPI edge cases may still be thin.
- **Intent:** Harden all four Win11 project modes + identity flash on apply.

**Acceptance criteria:**
1. Behavior matches decision `A` (Harden four modes + identify displays) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Super+P / Connect flyout already drives Extend/Duplicate/Second-only via `mde-output-layout.txt`; PC-only and multi-DPI edge cases may still be thin.
2. Implement decision **A**: Harden four modes + identify displays
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A061 (Project modes Super+P reliability); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A062 — HDR / refresh rate / scale per monitor clarity

- **Priority:** `P1`
- **Decision:** `A` — Full per-output apply/revert UI
- **Community driver:** Per-monitor scale/refresh is daily driver essentials.
- **MDE today:** Modes/scale via profiles; fractional scale supported.
- **Intent:** Per-output refresh/scale/resolution pickers with Apply/Revert 15s.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full per-output apply/revert UI) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Modes/scale via profiles; fractional scale supported.
2. Implement decision **A**: Full per-output apply/revert UI
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A062 (HDR / refresh rate / scale per monitor clarity); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A063 — Startup apps manager like Win11

- **Priority:** `P1`
- **Decision:** `A` — Impact + search + toggle polish
- **Community driver:** Disable startup apps early.
- **MDE today:** Settings Startup page exists.
- **Intent:** Impact estimates + enable/disable XDG autostart with search.

**Acceptance criteria:**
1. Behavior matches decision `A` (Impact + search + toggle polish) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Settings Startup page exists.
2. Implement decision **A**: Impact + search + toggle polish
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A063 (Startup apps manager like Win11); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A064 — Default apps by file type / protocol

- **Priority:** `P1`
- **Decision:** `A` — Full defaults-by-type UI
- **Community driver:** Default browser/email/etc. is critical.
- **MDE today:** Default apps page exists.
- **Intent:** Win11-like per-extension and per-protocol defaults UI.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full defaults-by-type UI) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Default apps page exists.
2. Implement decision **A**: Full defaults-by-type UI
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A064 (Default apps by file type / protocol); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A065 — Installed apps uninstall + repair

- **Priority:** `P1`
- **Decision:** `A` — Uninstall + reset data + pin actions
- **Community driver:** Apps list with uninstall is Settings staple.
- **MDE today:** Installed apps page.
- **Intent:** dnf/flatpak uninstall, open install location, pin actions already partly there — add repair/reset app data.

**Acceptance criteria:**
1. Behavior matches decision `A` (Uninstall + reset data + pin actions) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Installed apps page.
2. Implement decision **A**: Uninstall + reset data + pin actions
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A065 (Installed apps uninstall + repair); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A066 — Game Mode that does something measurable

- **Priority:** `P1`
- **Decision:** `A` — Wire to Feral gamemode + DND
- **Community driver:** Game Mode / Xbox Game Bar debates; still expected entry.
- **MDE today:** Gaming › Game Mode page.
- **Intent:** Real Game Mode: focus priority, inhibit notifications, optional MangoHud/gamemode integration.

**Acceptance criteria:**
1. Behavior matches decision `A` (Wire to Feral gamemode + DND) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Gaming › Game Mode page.
2. Implement decision **A**: Wire to Feral gamemode + DND
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A066 (Game Mode that does something measurable); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A067 — Captures / Xbox-like screenshot+record

- **Priority:** `P2`
- **Decision:** `B` — Screenshot polish only
- **Community driver:** Win+G / captures folder culture.
- **MDE today:** Screenshots path; Super+Shift+S; recording unclear.
- **Intent:** Screenshot + optional region video record + gallery.

**Acceptance criteria:**
1. Behavior matches decision `B` (Screenshot polish only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Screenshots path; Super+Shift+S; recording unclear.
2. Implement decision **B**: Screenshot polish only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A067 (Captures / Xbox-like screenshot+record); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A068 — Emoji panel depth (native grid exists)

- **Priority:** `P2`
- **Decision:** `B` — Recents + search only
- **Community driver:** Win+. emoji panel is daily use.
- **MDE today:** Native iced emoji grid + wtype/wl-copy; More… → gnome-characters (W426).
- **Intent:** Add search, skin tones, kaomoji/GIF tabs, and recents like Win11.

**Acceptance criteria:**
1. Behavior matches decision `B` (Recents + search only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Native iced emoji grid + wtype/wl-copy; More… → gnome-characters (W426).
2. Implement decision **B**: Recents + search only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A068 (Emoji panel depth (native grid exists)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A069 — Voice typing / dictation polish (pipeline exists)

- **Priority:** `P2`
- **Decision:** `B` — Network STT polish only
- **Community driver:** Voice typing popular on Win11 (Win+H).
- **MDE today:** Super+H dictation with mic OSD + vosk/whisper/network + wtype (W424); Start voice search opt-in (W459).
- **Intent:** Reliability polish, offline model packaging, and Settings discoverability.

**Acceptance criteria:**
1. Behavior matches decision `B` (Network STT polish only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Super+H dictation with mic OSD + vosk/whisper/network + wtype (W424); Start voice search opt-in (W459).
2. Implement decision **B**: Network STT polish only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A069 (Voice typing / dictation polish (pipeline exists)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A070 — Magnifier fidelity (tool/scale path exists)

- **Priority:** `P1`
- **Decision:** `A` — Compositor-native magnifier
- **Community driver:** Magnifier is core a11y (Win++/-).
- **MDE today:** `spawn_magnifier` / Super+Plus uses magnus/kmag or output-scale nudge (W422/W403) — not a stub tip.
- **Intent:** Compositor-native lens/full-screen zoom with follow-focus, beyond external tools.

**Acceptance criteria:**
1. Behavior matches decision `A` (Compositor-native magnifier) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `spawn_magnifier` / Super+Plus uses magnus/kmag or output-scale nudge (W422/W403) — not a stub tip.
2. Implement decision **A**: Compositor-native magnifier
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A070 (Magnifier fidelity (tool/scale path exists)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A071 — High contrast themes that actually apply

- **Priority:** `P1`
- **Decision:** `A` — System-wide contrast theme pack
- **Community driver:** Contrast themes for low vision.
- **MDE today:** Contrast themes page exists.
- **Intent:** Force high-contrast palette across shell + GTK + titlebars.

**Acceptance criteria:**
1. Behavior matches decision `A` (System-wide contrast theme pack) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Contrast themes page exists.
2. Implement decision **A**: System-wide contrast theme pack
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A071 (High contrast themes that actually apply); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A072 — Text size system-wide slider

- **Priority:** `P1`
- **Decision:** `A` — System-wide text scale
- **Community driver:** Accessibility text size.
- **MDE today:** Text size page exists.
- **Intent:** Scale UI fonts across shell/settings/apps via config + gsettings.

**Acceptance criteria:**
1. Behavior matches decision `A` (System-wide text scale) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Text size page exists.
2. Implement decision **A**: System-wide text scale
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A072 (Text size system-wide slider); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A073 — Shortcut Guide overlay (PowerToys)

- **Priority:** `P1`
- **Decision:** `A` — Implement Shortcut Guide
- **Community driver:** Hold Win → shortcut cheat sheet.
- **MDE today:** Shortcuts settings page; no overlay guide.
- **Intent:** Hold Super → Shortcut Guide overlay.

**Acceptance criteria:**
1. Behavior matches decision `A` (Implement Shortcut Guide) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Shortcuts settings page; no overlay guide.
2. Implement decision **A**: Implement Shortcut Guide
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A073 (Shortcut Guide overlay (PowerToys)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A074 — Hosts editor / env vars (PowerToys admin toys)

- **Priority:** `skip`
- **Decision:** `C` — Skip
- **Community driver:** Dev-oriented PowerToys modules.
- **MDE today:** MMC + Config Editor.
- **Intent:** MMC pages for hosts + environment variable profiles.

**Status:** skip — no implementation work. Kept for audit completeness.

### A075 — PowerRename / bulk rename

- **Priority:** `P2`
- **Decision:** `B` — Rely on existing FM tools
- **User note:** Thunar already has bulk rename — rely on existing FM tools
- **Community driver:** PowerRename is a favorite Explorer addon.
- **MDE today:** No bulk rename utility.
- **Intent:** Bulk rename tool (regex) integrable with FM context menu.

**Acceptance criteria:**
1. Behavior matches decision `B` (Rely on existing FM tools) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: No bulk rename utility.
2. Implement decision **B**: Rely on existing FM tools
3. Honor user note: Thunar already has bulk rename — rely on existing FM tools
4. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
5. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
6. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
7. VT1 verify acceptance for A075 (PowerRename / bulk rename); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A076 — Image Resizer context action

- **Priority:** `P1`
- **Decision:** `A` — Implement Image Resizer
- **Community driver:** PowerToys Image Resizer.
- **MDE today:** None.
- **Intent:** Right-click resize presets for images.

**Acceptance criteria:**
1. Behavior matches decision `A` (Implement Image Resizer) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: None.
2. Implement decision **A**: Implement Image Resizer
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A076 (Image Resizer context action); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A077 — File Locksmith “who’s using this file”

- **Priority:** `skip`
- **Decision:** `B` — Skip
- **Community driver:** PowerToys File Locksmith.
- **MDE today:** None.
- **Intent:** “Open files” inspector for path → PID.

**Status:** skip — no implementation work. Kept for audit completeness.

### A078 — Nearby Share / KDE Connect remaining gaps

- **Priority:** `P1`
- **Decision:** `A` — Broader Phone-Link feature parity on KDE Connect
- **Community driver:** Phone Link / Nearby Sharing expectations.
- **MDE today:** QS Nearby + Settings pairing via D-Bus/CLI (W395/W446–448); clipboard share to device (W437); intake wanted broad feature surface.
- **Intent:** Close remaining Phone-Link-class gaps (SMS/notifications mirror, battery, reliable inbound toasts) and first-run install of `kde-connect`.

**Acceptance criteria:**
1. Behavior matches decision `A` (Broader Phone-Link feature parity on KDE Connect) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: QS Nearby + Settings pairing via D-Bus/CLI (W395/W446–448); clipboard share to device (W437); intake wanted broad feature surface.
2. Implement decision **A**: Broader Phone-Link feature parity on KDE Connect
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A078 (Nearby Share / KDE Connect remaining gaps); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A079 — Debloat / telemetry-analogue: quiet recommendations

- **Priority:** `skip`
- **Decision:** `C` — Skip presets
- **Community driver:** Win11 debloat scripts are controversial but popular; for MDE = no promo noise.
- **MDE today:** No Store ads; Tips stub exists.
- **Intent:** “Quiet desktop” preset: no Tips, no Widgets news, no Recommended, DND optional.

**Status:** skip — no implementation work. Kept for audit completeness.

### A080 — Taskbar context menu completeness

- **Priority:** `P1`
- **Decision:** `A` — Gap-fill to Win11 menu parity where it makes sense
- **Community driver:** Discoverability of taskbar settings / Task Manager / show desktop.
- **MDE today:** Right-click already has Taskbar settings, Task Manager, lock/auto-hide/widgets, calendar link (`menus.rs` W387).
- **Intent:** Audit vs Win11 menu (Search, widgets, toolbars leftovers) and add any missing high-value entries only.

**Acceptance criteria:**
1. Behavior matches decision `A` (Gap-fill to Win11 menu parity where it makes sense) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Right-click already has Taskbar settings, Task Manager, lock/auto-hide/widgets, calendar link (`menus.rs` W387).
2. Implement decision **A**: Gap-fill to Win11 menu parity where it makes sense
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A080 (Taskbar context menu completeness); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A081 — Task Manager quick access from taskbar/Ctrl+Shift+Esc

- **Priority:** `P2`
- **Decision:** `C` — MMC Performance page only
- **User note:** MMC Performance page only; polish and expand the MMC
- **Community driver:** Muscle memory.
- **MDE today:** MMC/services; system monitor launch?
- **Intent:** Bind Ctrl+Shift+Esc to system monitor / mde-mmc performance.

**Acceptance criteria:**
1. Behavior matches decision `C` (MMC Performance page only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: MMC/services; system monitor launch?
2. Implement decision **C**: MMC Performance page only
3. Honor user note: MMC Performance page only; polish and expand the MMC
4. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
5. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
6. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
7. VT1 verify acceptance for A081 (Task Manager quick access from taskbar/Ctrl+Shift+Esc); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A082 — End task from taskbar (Win11 recent feature)

- **Priority:** `P1`
- **Decision:** `A` — Always show End task + optional “developer” mode
- **Community driver:** “End task” on taskbar is a praised recent Win11 addition.
- **MDE today:** Jump list End task exists.
- **Intent:** Ensure discoverable End task + Settings › For developers equivalent toggle.

**Acceptance criteria:**
1. Behavior matches decision `A` (Always show End task + optional “developer” mode) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Jump list End task exists.
2. Implement decision **A**: Always show End task + optional “developer” mode
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A082 (End task from taskbar (Win11 recent feature)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A083 — Progress bars on taskbar buttons

- **Priority:** `P1`
- **Decision:** `A` — Real progress protocol (high)
- **Community driver:** Downloads show progress on icons.
- **MDE today:** Title-heuristic / Unity progress removed (W439/W440); running underline only.
- **Intent:** Reintroduce progress via a real protocol (SNI/Unity-like or compositor hint), not title scraping.

**Acceptance criteria:**
1. Behavior matches decision `A` (Real progress protocol (high)) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Title-heuristic / Unity progress removed (W439/W440); running underline only.
2. Implement decision **A**: Real progress protocol (high)
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A083 (Progress bars on taskbar buttons); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A084 — Search without web / local-only mode

- **Priority:** `P2`
- **Decision:** `C` — Remove web entirely
- **Community driver:** Disable Bing/web in Start search is a top privacy tweak.
- **MDE today:** Web chip + DuckDuckGo hint (W95).
- **Intent:** Local-only search mode; disable web chip by policy/setting.

**Acceptance criteria:**
1. Behavior matches decision `C` (Remove web entirely) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Web chip + DuckDuckGo hint (W95).
2. Implement decision **C**: Remove web entirely
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A084 (Search without web / local-only mode); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A085 — Content search scale (rg path already works)

- **Priority:** `P2`
- **Decision:** `C` — Keep Desktop/Documents/Downloads only
- **Community driver:** Indexer/content search expected on Win11-class Start.
- **MDE today:** `files::search_file_contents` via `rg`/`grep` under Desktop/Documents/Downloads (W406) — not a stub.
- **Intent:** Expand roots, optional Tracker/Recoll index for speed, and Settings to choose indexed folders.

**Acceptance criteria:**
1. Behavior matches decision `C` (Keep Desktop/Documents/Downloads only) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `files::search_file_contents` via `rg`/`grep` under Desktop/Documents/Downloads (W406) — not a stub.
2. Implement decision **C**: Keep Desktop/Documents/Downloads only
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A085 (Content search scale (rg path already works)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A086 — Calculator in Start search (keep + history)

- **Priority:** `skip`
- **Decision:** `D` — Skip
- **Community driver:** Inline calc in search is loved (W175 exists).
- **MDE today:** Tiny safe parser + history stub done.
- **Intent:** Expand functions (hex, units) + pin Calculator app.

**Status:** skip — no implementation work. Kept for audit completeness.

### A087 — Windhawk-like modular shell styler API

- **Priority:** `P1`
- **Decision:** `A` — Official style-pack API
- **Community driver:** Windhawk’s modular mods are the customization platform of choice.
- **MDE today:** Single theme tokens in mde-theme.
- **Intent:** Safe user CSS/QSS-like or TOML style packs for Start/Taskbar/NC without forking.

**Acceptance criteria:**
1. Behavior matches decision `A` (Official style-pack API) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Single theme tokens in mde-theme.
2. Implement decision **A**: Official style-pack API
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A087 (Windhawk-like modular shell styler API); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A088 — Animation reduce / disable (community perf/a11y)

- **Priority:** `P1`
- **Decision:** `A` — Audit + harden animations off
- **Community driver:** Disable animations common tweak.
- **MDE today:** `theme.animations` exists.
- **Intent:** Ensure toggle disables shell + compositor animations thoroughly.

**Acceptance criteria:**
1. Behavior matches decision `A` (Audit + harden animations off) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `theme.animations` exists.
2. Implement decision **A**: Audit + harden animations off
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A088 (Animation reduce / disable (community perf/a11y)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A089 — Battery flyout details (Win11 QS)

- **Priority:** `P1`
- **Decision:** `A` — Full battery + power mode flyout
- **Community driver:** Battery % + estimate + power mode.
- **MDE today:** Battery flyout exists (W88).
- **Intent:** Add time remaining, power mode (Best power efficiency / Balanced / Best performance), energy saver.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full battery + power mode flyout) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Battery flyout exists (W88).
2. Implement decision **A**: Full battery + power mode flyout
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A089 (Battery flyout details (Win11 QS)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A090 — Storage Sense analogue

- **Priority:** `P1`
- **Decision:** `A` — Full Storage Sense analogue
- **Community driver:** Storage cleanup recommendations.
- **MDE today:** Storage page exists.
- **Intent:** Temp cleanup, trash, large files, optional scheduled cleanup.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full Storage Sense analogue) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Storage page exists.
2. Implement decision **A**: Full Storage Sense analogue
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A090 (Storage Sense analogue); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A091 — Windows Update analogue clarity (dnf/flatpak)

- **Priority:** `P1`
- **Decision:** `A` — Full update UX + offline update restart
- **Community driver:** Users expect an Update UI even on Linux DE clones.
- **MDE today:** Update + history pages.
- **Intent:** One-click check/apply with reboot deferral matching Start power update actions.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full update UX + offline update restart) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Update + history pages.
2. Implement decision **A**: Full update UX + offline update restart
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A091 (Windows Update analogue clarity (dnf/flatpak)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A092 — Privacy dashboard (camera/mic/location indicators)

- **Priority:** `P1`
- **Decision:** `A` — Live privacy indicators + app list
- **Community driver:** Privacy indicators + per-app toggles.
- **MDE today:** Privacy pages exist.
- **Intent:** Taskbar privacy indicator when mic/camera in use + click to see apps.

**Acceptance criteria:**
1. Behavior matches decision `A` (Live privacy indicators + app list) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Privacy pages exist.
2. Implement decision **A**: Live privacy indicators + app list
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A092 (Privacy dashboard (camera/mic/location indicators)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A093 — Firewall / Security center status freshness

- **Priority:** `P1`
- **Decision:** `A` — Home security health cards
- **Community driver:** Security center glance.
- **MDE today:** Security/firewall pages.
- **Intent:** Home card health (firewalld, SELinux, updates, disk) like Win11 Home.

**Acceptance criteria:**
1. Behavior matches decision `A` (Home security health cards) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Security/firewall pages.
2. Implement decision **A**: Home security health cards
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A093 (Firewall / Security center status freshness); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A094 — Touchpad gestures Win11-like

- **Priority:** `P1`
- **Decision:** `A` — Full gesture map UI
- **Community driver:** Three/four finger gestures.
- **MDE today:** Touchpad settings page.
- **Intent:** Map 3-finger swipe desktops, 4-finger Start/overview if hardware allows.

**Acceptance criteria:**
1. Behavior matches decision `A` (Full gesture map UI) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Touchpad settings page.
2. Implement decision **A**: Full gesture map UI
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A094 (Touchpad gestures Win11-like); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A095 — Touch / tablet mode / OSK auto

- **Priority:** `P1`
- **Decision:** `A` — Auto-OSK + tablet posture
- **Community driver:** Tablet posture expectations.
- **MDE today:** On-screen keyboard exists.
- **Intent:** Auto-OSK on text focus for touch; optional tablet layout.

**Acceptance criteria:**
1. Behavior matches decision `A` (Auto-OSK + tablet posture) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: On-screen keyboard exists.
2. Implement decision **A**: Auto-OSK + tablet posture
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A095 (Touch / tablet mode / OSK auto); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A096 — Multi-monitor taskbar behaviors

- **Priority:** `P1`
- **Decision:** `A` — Three Win11-like multi-monitor modes
- **Community driver:** Show taskbar on all displays / apps where open.
- **MDE today:** `taskbar_all_displays` default true.
- **Intent:** Win11 behaviors: buttons on all taskbars vs main only vs where window is open.

**Acceptance criteria:**
1. Behavior matches decision `A` (Three Win11-like multi-monitor modes) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `taskbar_all_displays` default true.
2. Implement decision **A**: Three Win11-like multi-monitor modes
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A096 (Multi-monitor taskbar behaviors); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A097 — Clock seconds / ISO date (already knobs — surface them)

- **Priority:** `P1`
- **Decision:** `A` — Surface in Taskbar + Date & time UI
- **Community driver:** Clock with seconds; ISO date for power users.
- **MDE today:** `clock_seconds`, `clock_date_iso` exist.
- **Intent:** Expose clearly in Taskbar/Date & time settings (not only Advanced/TOML).

**Acceptance criteria:**
1. Behavior matches decision `A` (Surface in Taskbar + Date & time UI) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: `clock_seconds`, `clock_date_iso` exist.
2. Implement decision **A**: Surface in Taskbar + Date & time UI
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A097 (Clock seconds / ISO date (already knobs — surface them)); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A098 — World clocks in NC vs Widgets

- **Priority:** `P1`
- **Decision:** `A` — Add up to 2 world clocks in NC header
- **Community driver:** Additional clocks in Win11 Date & time / NC.
- **MDE today:** Multi-TZ clocks live on Widgets board (W435); NC header is primarily local + calendar.
- **Intent:** Surface 1–2 world clocks in NC header (or link out), matching Win11 Date & time extras.

**Acceptance criteria:**
1. Behavior matches decision `A` (Add up to 2 world clocks in NC header) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Multi-TZ clocks live on Widgets board (W435); NC header is primarily local + calendar.
2. Implement decision **A**: Add up to 2 world clocks in NC header
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A098 (World clocks in NC vs Widgets); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A099 — Feedback / diagnostics UX naming

- **Priority:** `P2`
- **Decision:** `C` — Remove Feedback entry
- **Community driver:** Feedback Hub is mocked on Win11 clones; power users want diagnostics export.
- **MDE today:** About Feedback already copies `journalctl --user` + mailto (W409); older “coming soon” label may linger in copy.
- **Intent:** Rename to “Export diagnostics / Send feedback” and attach optional issue template + smoke log bundle.

**Acceptance criteria:**
1. Behavior matches decision `C` (Remove Feedback entry) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: About Feedback already copies `journalctl --user` + mailto (W409); older “coming soon” label may linger in copy.
2. Implement decision **C**: Remove Feedback entry
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A099 (Feedback / diagnostics UX naming); capture evidence under `mde/fidelity/evidence/` if chrome changed.

### A100 — Quiet hours / Focus preset pack as “community recommended” profile

- **Priority:** `P1`
- **Decision:** `A` — Ship Community Classic + Stock Win11 profiles
- **Community driver:** Bundles of tweaks applied on every install.
- **MDE today:** Profiles in config.toml.
- **Intent:** Ship named profiles: `Community Classic` (left taskbar, no widgets/search, more pins, local search) and `Stock Win11` (center, widgets on).

**Acceptance criteria:**
1. Behavior matches decision `A` (Ship Community Classic + Stock Win11 profiles) on a live MDE seat.
2. Setting/config is discoverable where the decision implies user control (Settings page and/or profile preset), not only a hidden TOML key.
3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional.

**Work steps:**
1. Confirm current tree still matches: Profiles in config.toml.
2. Implement decision **A**: Ship Community Classic + Stock Win11 profiles
3. Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.
4. Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.
5. Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.
6. VT1 verify acceptance for A100 (Quiet hours / Focus preset pack as “community recommended” profile); capture evidence under `mde/fidelity/evidence/` if chrome changed.

---

## Notable special decisions

- **A037 = A+C:** QS Bluetooth connect/disconnect for known devices **and** full pair wizard in QS.
- **A044 = B:** Deep-integrate Thunar as “MDE Explorer” (not a greenfield W8 Explorer epic).
- **A075 = B:** Thunar already has bulk rename — rely on FM tools.
- **A081 = C:** Ctrl+Shift+Esc → MMC Performance; **polish and expand MMC**.
- **A006 = C:** Bottom taskbar only forever (no top/side edges).
- **A029 = C / A047 = C:** Leave taskbar peek deferred; keep title/icon Task View (no live thumbs push).
