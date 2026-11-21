# Right-click / context-menu evaluation (MDE shell vs Windows 11)

**Date:** 2026-11-19  
**Scope:** All interactive shell chrome surfaces (taskbar, Start, tray, desktop, window chrome, files).  
**Method:** Code inventory of every `on_right_press` / context-menu path under `mde-shell`, `mde-anvil` SSD, and `mde-files`, compared to Windows 11 expected behavior.  
**Visual fidelity:** `mde/fidelity/specs/context-menus.toml` remains `status = "unmeasured"` (no reference captures).

## Verdict

Right-click is **partially implemented on the taskbar cluster** (Start → Quick Link, pin jump lists, tray ContextMenu, taskbar/clock settings menu) and **mostly absent elsewhere**. Versus Windows 11, MDE covers the *shape* of a few high-traffic menus with thinner item sets, and misses entire surfaces (desktop, Explorer, title-bar system menu, Start pin tiles, status glyphs).

Overall fidelity for right-click behavior: **low–medium on taskbar; missing on desktop/files/window chrome.**

---

## Inventory: surfaces with right-click wired

| # | Surface | Trigger (code) | Opens | Menu / action contents |
|---|---------|----------------|-------|------------------------|
| 1 | Start button | `taskbar.rs` → `ToggleQuickLink` | Quick Link flyout | Terminal, Settings, Explorer, Run, Desktop folder (`menus.rs` `view_quicklink`) |
| 2 | Taskbar pin | `OpenJumpList(pin)` | Jump list | Unpin from taskbar; End task (if running); ≤3 Recent files (`view_jump_list`) |
| 3 | Unpinned running app | `OpenWinJump(index)` | Win jump | End task only (`view_win_jump`) |
| 4 | SNI tray icon | `TrayMenu(id)` | Tray menu | dbusmenu items or StatusNotifier `ContextMenu` (`sni.rs`) |
| 5 | Clock | `OpenTaskbarMenu` | Taskbar menu | Taskbar settings, Task manager, Auto-hide, Show/Hide seconds, ISO/month-day (`view_taskbar_menu`) |
| 6 | Taskbar background | `OpenTaskbarMenu` | Same taskbar menu | Same as clock |
| 7 | Start → All apps row | `PinToTaskbar(id)` | **No menu** | Immediate pin (W246) |
| 8 | Start → Recommended card | `OpenRecommendedLocation(id)` | **No menu** | Immediate open parent/location (W244) |

Evidence anchors: `mde/crates/mde-shell/src/taskbar.rs`, `menus.rs`, `start.rs`, `sni.rs`; worklist W20/W35/W93/W161/W206/W237/W246.

---

## Inventory: shell surfaces with **no** right-click

| Surface | Win11 expects | MDE today |
|---------|---------------|-----------|
| Empty desktop / wallpaper | Desktop context menu (View, Sort, Refresh, New, Personalize, Display settings, …) | No desktop context layer; fidelity refs for `context-menus/desktop` are missing |
| Desktop icons | Open / shortcut menus | W45 lists `~/Desktop` from Start only; no icon context menu |
| File Explorer items | Streamlined context menu (Cut/Copy/Paste/Rename/Share/Delete + Show more options) | `mde-files` is `xdg-open` only (`mde-files/src/main.rs`); no Explorer UI |
| Window title bar (SSD) | Right-click → system menu (Restore/Move/Size/Min/Max/Close) | SSD handles caption buttons only (`mde-anvil/.../ssd.rs`); no `Button::Right` path. Alt+Space `winmenu` exists separately |
| Start pinned tiles | Unpin from Start, Pin to taskbar, Move, folders, … | Left-click launch / reorder arrows only; **no** `on_right_press` on Start pins |
| Task View button | (limited / none in stock Win11; often no rich menu) | Left-click only |
| Widgets button | Widgets / news settings-ish | Left-click only |
| Net / Vol / Battery glyphs | Left → Quick Settings; right often unused or OS-specific | Left → Quick Settings only |
| Show-desktop strip | Peek / show desktop (left) | Left / hover only (`ShowDesktop`) |
| Taskbar overflow `+N` | Jump / unpin (W18 claims right-click Unpin) | Overflow list is left-click activate only (`view_pin_overflow`) — **W18 claim not matched by current wiring** |
| Notifications / toasts | App / notification actions | No right-click handlers in `notify_view.rs` |
| Widgets board cards | Card-specific menus | No right-click in `widgets_view.rs` |
| Search / Task View tiles | Limited Win11 menus | No right-click |

---

## Per-surface comparison to Windows 11

### 1. Start button → Quick Link (Win+X)

| Windows 11 Win+X (typical) | MDE Quick Link |
|----------------------------|----------------|
| Terminal / Windows Terminal | Terminal ✓ |
| Settings | Settings ✓ |
| File Explorer | Explorer ✓ |
| Run | Run ✓ |
| Desktop (rarely as top-level; more via folders) | Desktop folder ✓ |
| Task Manager | **Missing** (available from taskbar menu instead) |
| Device Manager, Disk Management, Computer Management | **Missing** |
| Network Connections, Power Options | **Missing** |
| Apps and Features, System, Event Viewer | **Missing** |
| Shut down / Sign out / Restart submenu | **Missing** (Start power menu is separate, left-click) |
| Search, Windows PowerShell (legacy builds) | **Missing** |

**Gap:** Correct entry point (right-click Start + Super+X), ~5 of ~15+ Win+X destinations.

### 2. Taskbar pin → Jump List

| Windows 11 Jump List | MDE |
|----------------------|-----|
| App tasks / custom destinations | **Missing** |
| Pinned destinations (user-pinned) | **Missing** |
| Recent destinations | ≤3 recent files ✓ (thin) |
| Open / launch new instance | **Missing** as explicit item |
| Pin / Unpin from taskbar | Unpin ✓; Pin only from elsewhere |
| Close window / Close all windows | End task when matched running ✓ (single close path, not “Close all”) |
| Shift+right-click → classic window menu | **Missing** |

**Gap:** Core Unpin + Recent + End task present; missing launch instance, pin recent, app tasks, close-all, Shift+right-click.

### 3. Unpinned running window on taskbar

| Windows 11 | MDE |
|------------|-----|
| Full jump list + Close | End task only |

**Gap:** Severely reduced vs grouped jump list.

### 4. Tray (notification area) icons

| Windows 11 | MDE |
|------------|-----|
| App-owned context menu | SNI `ContextMenu` + dbusmenu ✓ (T1) |
| Overflow chevron menus | Overflow stub (`TrayOverflowStub` / W266) |

**Gap:** Best parity area when apps expose StatusNotifier menus; chrome overflow still stubby.

### 5. Clock / date

| Windows 11 | MDE |
|------------|-----|
| Left-click → Notification Center / calendar | Left → notifications ✓ |
| Right-click → Adjust date/time, Notification settings, … | Right → **taskbar settings menu** (settings, Task manager, auto-hide, seconds, date format) |

**Gap:** Different menu model. MDE folds clock preferences into the taskbar menu (usable, not Win11-shaped). Comment on `ToggleClockDateFormat` still describes cycling via right-click, but clock wiring uses `OpenTaskbarMenu`.

### 6. Empty taskbar background

| Windows 11 | MDE |
|------------|-----|
| Taskbar settings, Task manager, toolbars (legacy), lock the taskbar, cascade/show windows, … | Taskbar settings, Task manager, Auto-hide, clock seconds, date format |

**Gap:** Partial. Missing lock taskbar, show windows side-by-side, toolbars, search/widgets toggles from menu, etc.

### 7. Start → All apps / Recommended

| Windows 11 | MDE |
|------------|-----|
| Right-click → rich menu (Pin to Start/taskbar, Uninstall, Run as admin, Open file location, …) | All apps: **immediate Pin to taskbar** (no menu). Recommended: **immediate Open location**. |

**Gap:** Actions exist as one-shot shortcuts, not Win11 context menus. No Unpin from Start, Uninstall, Run as admin, Pin to Start, etc.

### 8. Desktop

| Windows 11 | MDE |
|------------|-----|
| Full desktop + icon context menus; visual refs expected under `context-menus/desktop` | **Not implemented**. Spec `context-menus.toml` state `desktop` has empty `refs`. |

**Gap:** Complete miss. Highest-visibility Win11 right-click surface after Explorer.

### 9. Files / Explorer

| Windows 11 | MDE |
|------------|-----|
| Streamlined + classic context menus | `mde-files` opens system FM via `xdg-open`; no MDE context UI (W8 deferred) |

**Gap:** Out of shell scope today; fidelity `files/context-menu` refs missing.

### 10. Window system menu

| Windows 11 | MDE |
|------------|-----|
| Right-click title bar / Alt+Space | Alt+Space `winmenu` with Minimize/Maximize/Center/Close/Undo snap + stubs; **no title-bar right-click** |

**Gap:** Keyboard path only; SSD right-click absent. Several winmenu items are stubs (Always on top, Opacity, Cascade).

---

## Claims vs code mismatches

| Claim / note | Status |
|--------------|--------|
| W18 “+N opens More pins; right-click Unpin” | Overflow UI has **no** `on_right_press`; Unpin lives on pin jump list only |
| W93/W237 “right-click clock cycles ISO/seconds” | Clock right-click opens taskbar menu; cycling is `ToggleClockDateFormat` (not wired on the clock face) |
| `context-menus` fidelity surface | Unmeasured; no PNGs for taskbar-pin or desktop |
| C4 claim lists `context-menus` | Blocked with other visual cards (`surfaces/summary.md`) |

---

## Priority gaps (Win11 parity order)

1. **Desktop empty-space context menu** — zero coverage; expected by users and by fidelity `desktop` state.  
2. **Jump list completeness** — Open app, Close all, pin recent, app tasks; unify pin vs unpinned running.  
3. **Start tile / All apps real context menus** — replace one-shot right-press with menu (Pin/Unpin Start & taskbar, Open location, Uninstall where applicable).  
4. **Win+X completeness** — Task Manager, power, admin tools.  
5. **SSD title-bar right-click → system menu** — mirror Alt+Space.  
6. **Taskbar overflow right-click** — align code with W18 or fix the worklist claim.  
7. **Explorer context menus** — blocked on W8 / measured `mde-files` UI.  
8. **Visual measurement** — capture refs for `context-menus.taskbar-pin` and `context-menus.desktop`.

---

## Evidence index

| Artifact | Role |
|----------|------|
| `mde/crates/mde-shell/src/taskbar.rs` | All taskbar `on_right_press` bindings + `view_taskbar_menu` |
| `mde/crates/mde-shell/src/menus.rs` | Jump list, win jump, quick link, winmenu views |
| `mde/crates/mde-shell/src/start.rs` | All-apps pin + Recommended location right-press |
| `mde/crates/mde-shell/src/sni.rs` | Tray ContextMenu / dbusmenu |
| `mde/crates/mde-anvil/src/shell/ssd.rs` | Caption hit-testing (no right-click menu) |
| `mde/crates/mde-files/src/main.rs` | No Explorer chrome / no context menu |
| `mde/fidelity/specs/context-menus.toml` | Unmeasured visual surface |
| `mde/crates/WORKLIST.md` | W18, W20, W35, W93, W161, W206, W237, W246 |
| Microsoft: Jump Lists, Win+X / shortcuts, File Explorer context menu | Win11 expected behavior baseline |

---

## Summary scorecard

| Area | Win11 parity |
|------|--------------|
| Start → Quick Link | Partial (~30%) |
| Taskbar pin jump list | Partial (~40%) |
| Unpinned running jump | Low (~15%) |
| Tray icon menus | Good when SNI/dbusmenu present |
| Taskbar / clock menu | Partial (~40%), different shape |
| Start All apps / Recommended | Actions only, not menus |
| Desktop | None |
| Files / Explorer | None (deferred) |
| Title-bar system menu | Keyboard only |
| Widgets / NC / QS glyphs | None |

**Bottom line:** MDE’s right-click story is a **taskbar-centric subset** of Windows 11. The most important missing pieces for “feels like Win11” are the **desktop context menu**, **fuller jump lists**, and **real Start context menus** instead of one-shot right-press actions.
