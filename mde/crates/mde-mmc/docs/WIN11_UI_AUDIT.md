# MMC Windows 11 look / feel / layout / icons audit

**Date:** 2026-11-20  
**Scope:** Primary host `mde-mmc` (iced) and legacy bridge UI `ws-mmc` (GTK).  
**Reference:** Classic MMC / Computer Management chrome on Windows 11, plus MDE shell tokens (`mde_theme`, `mde/fidelity/tokens`, Mackes-Carbon icons).  
**Method:** Static inspection of current tree + live icon-theme resolution under `/usr/share/icons`. No interactive screenshot pass in this audit.

---

## Surfaces inventoried

| Surface | Role today | UI toolkit |
|---------|------------|------------|
| `mde/crates/mde-mmc` | **Primary** admin console (W450–W499 cutover) | iced |
| `mde/shell/ws-mmc` (+ `lib/ws-mmc`) | Legacy GTK console; kept for bridge / install | GTK3 + `wsui` CSS |
| Settings Advanced › MMC | Launcher only (opens `mde-mmc`) | iced Settings |

There is **no** `mde/fidelity/specs/*` entry for MMC. Capability fidelity (`CAPABILITY_MATRIX.md`) is covered; visual fidelity is not.

---

## Executive verdict

| Area | `mde-mmc` (primary) | `ws-mmc` (legacy) |
|------|---------------------|-------------------|
| Look (color, type, materials) | Partial — palette + Selawik only | Closer — Settings-aligned CSS |
| Feel (menus, selection, motion) | Weak — button chrome, no menus/context | Stronger — real menubar, hover 83 ms, soft selection |
| Layout (three panes, CompMgmt) | Structural match, chrome incomplete | Structural + chrome match |
| Icons | **Named but never rendered** | Rendered in tree, lists, toolbar, actions |

**Bottom line:** After the Rust cutover, MMC **capability** is primary in `mde-mmc`, but **Windows 11 / classic MMC visual fidelity regressed** relative to the GTK `ws-mmc` host. Icons and several chrome affordances exist in the model and dependency graph but are unused in the iced view.

---

## 1. Look (theme, type, materials)

### What works in `mde-mmc`

- Uses `Palette::load()` and `mde_theme::iced_theme` (accent / light-dark from MDE config).
- Default UI font is **Selawik** (`mde_theme::FONT`) with shipped font bytes loaded.
- Pane backgrounds use `bg_elevated` + `border` (elevated chrome token path).

### Gaps vs Win11 / MDE shell

| Finding | Evidence | Severity |
|---------|----------|----------|
| No use of fidelity space/radius/type_ramp modules | `main.rs` hardcodes spacing `6`/`8`/`12` and pane `radius: 4.0`; no `mde_theme::space` / `radius` / `type_ramp` | Medium |
| Pane radius 4 px ≠ shell Win11 flyout/control radii (12 / 8) | `pane_style` vs `FLYOUT_RADIUS` / `CONTROL_RADIUS` in `mde-theme` | Low–Medium |
| Results rows use **system monospace**, not Cascadia / Selawik body | `view_results` uses `iced::Font::MONOSPACE` for list cells | Medium |
| No Mica / acrylic / transparency path for console chrome | Theme docs: alpha tints are placeholders; MMC only solid fills | Low (MMC on Win11 is also largely opaque) |
| Window title is static `"Management Console"` | `iced::application("Management Console", …)` — never `"Computer Management - [node]"` | Medium |
| Confirm / Add-Remove are full-window replacements, not Win11-style dialogs/sheets | `view_confirm` / `view_add_remove` | Medium |
| Dark/HC palette loads, but selection styling is iced `primary` (solid accent fill), not soft accent wash | Tree/results selected buttons use `button::primary` | Medium |

### Legacy `ws-mmc` look (baseline)

`mmcui.py` CSS targets `#settings.mmc`: `#f3f3f3` chrome, white panes, 6 px pane radius, caption greys, accent-tinted tree selection (`alpha(@ACCENT@, 0.16/0.24)`), toolbar hover 83 ms — aligned with Settings/`wsui` Win11 app chrome. That polish is **not** ported to iced.

---

## 2. Feel (interaction chrome)

### Classic MMC / Win11 CompMgmt expectations

- Real **File / Action / View / Help** menus
- Icon toolbar (Back, Forward, Up, Show/Hide tree, Refresh, Export, Help, Properties, …)
- Context menus on tree and results
- Status bar with context
- Dynamic window title with console + selected node

### `mde-mmc` today

| Expectation | Status |
|-------------|--------|
| Menubar | **Fake** — text buttons `"File › Add/Remove…"`, `"File › Save"`, `"Help › About"` |
| Toolbar | Text buttons only (`Back`, `Forward`, `Up`, `Tree`, `Refresh`, `Actions`) — no icons, no separators |
| Context menus | Absent |
| Keyboard | Partial — F5, Alt+←/→/↑, Ctrl+S, Ctrl+M |
| Status bar | Present (single line + dirty `•`) |
| Actions pane | Present as full-width secondary buttons (`secondary` / `danger`) — not MMC-style link rows with icons |
| Property sheets | Trait/epic noted “thin”; no sheet host UI in `main.rs` |
| Motion | None (shell has Win11 curves; MMC does not use `mde_theme::motion`) |

### `ws-mmc` today

Real `Gtk.MenuBar`, icon toolbar via `ui.icon_image`, tree icon column, Actions rows with icons, context menus (`popup`), dynamic title `"{name} - [{node}]"`, window icon from snap-in. This is the closer CompMgmt feel.

---

## 3. Layout

### Structural (both hosts)

Three-pane model matches classic MMC:

1. Console tree (left)
2. Results (center)
3. Actions (right)

Toggle tree / actions exists in both. Default window ~1180×760 in iced — reasonable for CompMgmt density.

### Layout gaps in `mde-mmc`

| Finding | Evidence | Severity |
|---------|----------|----------|
| Tree is flat indented text — no expand/collapse glyphs, no icon column | `view_tree` formats `"  ".repeat(depth) + title` only | High (feel/layout) |
| Results is not a columned list view | Headers joined with ` · `; cells joined with ` \| `; monospace buttons | High |
| No resizable paned splitters | Fixed widths 260 / Fill / 240; spacing padding only | Medium |
| No results banner / summary / preview split used by GTK folder & detail views | GTK has `.banner`, `.preview`, property notebooks | Medium |
| Add/Remove is a checklist of text buttons, not dual-list MMC dialog | `view_add_remove` | Medium |
| `devmgmt` / `diskmgmt` builtins disable tree+actions (`console.rs`) — closer to Win single-snapin mode | OK structurally | — (positive) |

---

## 4. Icons

### Dependency vs rendering

- `mde-mmc` **enables** `mde-theme` feature `icons` (`Cargo.toml`).
- **Zero** call sites: no `mde_theme::icons::{icon_file,chrome_icon,locate}`, no `iced` `Image` / handle load under `mde-mmc/src`.
- `TreeNode.icon` and `ResultRow.icon` are populated throughout snap-ins and **discarded** in `view_tree` / `view_results`.

### Name resolution (Mackes-Carbon-Light)

Audited FreeDesktop-style names used as `icon:` / `TreeNode::new(..., icon, ...)` / `fn icon()`:

- **27 / 28 resolve** under Mackes-Carbon-Light (or fallback themes).
- **Missing:** `preferences-system-time` (Time & Sync snap-in). Closest installed names include `preferences-date-and-time`, `clocks`.

Icon vocabulary is Linux/FreeDesktop + Mackes (not Fluent SVG assets). That matches the rest of MDE chrome and is acceptable **if rendered**; it is not Win11 Segoe Fluent glyphs.

### Reuse / ambiguity

Several snap-ins share the same icon (`computer` for CompMgmt, Device Manager, Virtualization; `security-high` for Firewall and Certificates; `text-x-generic` for Event Viewer and Logging). Fine as placeholders; weak as Win11 distinct snap-in identity once icons are drawn.

### Legacy `ws-mmc`

Icons are first-class: tree `icon-name` column, `ListView` pixbuf column, toolbar/`Action.icon`, `pick_icon` fallbacks, Device Manager category icons + problem badges.

---

## 5. Priority gap list

Ordered by visual impact on “does this feel like MMC on Win11 / MDE”:

1. **Render icons** in tree + results (+ toolbar) via `mde_theme::icons` — **done** (polish set 1)
2. **Replace monospace pipe-tables** with a real column list — **done** (set 2)
3. **Real menubar + icon toolbar** — **done** (set 3; iced dropdown menus, not native OS menubar)
4. **Dynamic window title** — **done** (set 4; app icon still default)
5. **Soft accent selection** and spacing/radius tokens — **done** (set 5)
6. Fix **`preferences-system-time`** → `preferences-date-and-time` — **done** (set 6)
7. Context menus + property-sheet host — **done** (right-click + Properties sheet; continue pass)
8. Optional: fidelity spec under `mde/fidelity/specs/` — **done** (`mmc.toml`, unmeasured)

---

## 6. Ten polish sets (2026-11-20)

Implemented against this audit in `mde-mmc` (`chrome.rs` + `main.rs` rewrite). Gate: `cargo test -p mde-mmc` green.

| Set | Focus | Evidence |
|-----|--------|----------|
| 1 | Render Mackes icons (tree, results, toolbar, actions) | `chrome::icon_face` → `mde_theme::icons::{chrome_icon,icon_file}`; iced `image` feature |
| 2 | Columned results (headers + cells, Selawik body) | `view_results` header row + per-column cells; no pipe/`MONOSPACE` list rows |
| 3 | File / Action / View / Help + icon toolbar | `MenuId` dropdowns; `ToolKind` icon buttons |
| 4 | Dynamic title `{console} - [{node}]` | `iced::application(Mmc::window_title, …)` |
| 5 | Soft accent wash selection + tokens | `select_row_style` 18% accent; `space::grid`, `CONTROL_RADIUS`, 6px panes, `#f3f3f3` canvas |
| 6 | Time & Sync icon name | `preferences-date-and-time` in `time_sync.rs` + `all_meta` |
| 7 | Action link-rows + status bar | Icon+label action rows; status shows node \| selection \| message |
| 8 | Modal confirm / Add-Remove cards | `stack!` dim overlay + `FLYOUT_RADIUS` cards (Esc dismisses) |
| 9 | Tree icons + expand/collapse | Chevron + `collapsed` set; `flatten_visible` |
| 10 | Results banner + this evidence table | Node icon/title/description banner; preview block under rows |

### Residual follow-up (2026-11-20 continue)

| Item | Status | Evidence |
|------|--------|----------|
| Right-click context menus | **done** | `mouse_area(...).on_right_press(OpenContext)` on tree + results; Action menu also lists actions |
| Property-sheet host | **done** | `view_properties_card` (Alt+Enter / toolbar / context); key-value + selection cells |
| Resizable paned splitters | **done** | 6px drag hit-targets; `tree_w` / `actions_w` clamped |
| Window/app icon from snap-in | **done** | `resolve_window_icon` → `window::Settings.icon` at launch |
| `mde/fidelity/specs/mmc.toml` | **done** | unmeasured scaffold with default metrics |
| Live screenshot / dark-mode pixel review | open | needs human capture + measure |

### Continue pass 2 (2026-11-20)

| Item | Status | Evidence |
|------|--------|----------|
| Dark / HC chrome | **done** | HC solid selection + 2px border; dark accent wash 28%; dark dim 0.55; `#2c2c2c` panes; dark Mackes icons via `icon_for` |
| Add/Remove dual-list | **done** | Available \| console lists; click to move |
| Distinct snap-in icons | **done** | Device=`video-display`, Events=`document-open-recent`, Firewall=`preferences-system-network`, Virt=`virtual-machine`, Certs=`application-certificate`, Logging=`accessories-text-editor` |
| Context dismiss outside click | **done** | `overlay_corner` full-window `CloseMenus` hit target under the menu card |

### Continue pass 3 (2026-11-20)

| Item | Status | Evidence |
|------|--------|----------|
| Menubar outside-click dismiss | **done** | Dropdown stacked above full-window `CloseMenus` hit target |
| File › New / Open… | **done** | `NewConsole`, `view_open_card` + `console::list_saved`; Ctrl+N / Ctrl+O |
| Column header sort | **done** | `SortColumn` toggles ▲/▼; sorts `results.rows` by cell |
| Empty results state | **done** | Banner + muted copy when no rows/detail |

### Continue pass 4 (2026-11-20) — icon contrast + chrome

Re-audit found pass 2’s “dark Mackes icons via `icon_for`” **inverted** vs Mackes theme semantics:

| Theme | Glyphs | Correct canvas |
|-------|--------|----------------|
| `Mackes-Carbon-Light` | `#f4f4f4` fills | **Dark / HC black UI** |
| `Mackes-Carbon` | `currentColor` → dark pixels | **Light / HC white UI** |

Prior `icon_face_themed` passed `icon_file(..., true)` on dark UI → preferred **Mackes-Carbon** (dark glyphs on dark canvas). Light UI used `chrome_icon` → **Light** glyphs on light canvas. Both were low-contrast.

| Item | Status | Evidence |
|------|--------|----------|
| Light icons in dark mode | **done** | `mde_theme::icons::for_ui(..., dark_ui=true)` → Mackes-Carbon-Light; `chrome::icon_for` uses `palette.dark` (incl. HC) |
| Dark icons in light mode | **done** | `for_ui(..., false)` → Mackes-Carbon first |
| Theme-order unit tests | **done** | `mde-theme` `--features icons`: `dark_ui_uses_light_glyphs`, `light_ui_prefers_dark_glyphs`, `for_ui_theme_order_matches_canvas` |
| Menubar / toolbar / status hairlines | **done** | `chrome::chrome_rule` between chrome bands |
| Visible paned grips | **done** | Splitter alpha 0.14 (HC solid) |

---

## Evidence index

| Claim | Where verified |
|-------|----------------|
| Icons **rendered** in iced view | `chrome.rs` `icon_face` / `icon_for`; call sites in `main.rs` tree/results/toolbar/actions |
| Dark UI uses **light** glyphs | `icons::for_ui` + `locate(..., false)` → `Mackes-Carbon-Light`; SVG `#f4f4f4`; evidence luminance ≈244 (`fidelity/evidence/mmc/icon-contrast.md`) |
| Light UI uses **dark** glyphs | `icons::for_ui` + `locate(..., true)` → `Mackes-Carbon`; SVG `currentColor`; evidence luminance ≈0 |
| iced `image` feature | `mde-mmc/Cargo.toml` |
| Soft selection (not solid primary) | `select_row_style` |
| Menubar + icon toolbar | `view_menubar` / `view_toolbar` |
| Dynamic title | `Mmc::window_title` |
| Time icon fixed | no `preferences-system-time` under `mde-mmc/src` |
| Tests | `cargo test -p mde-mmc` — 5 passed; `cargo test -p mde-theme --features icons` — 15 passed |

---

## Audit status

**Initial audit:** complete (look / feel / layout / icons).  
**Ten polish sets:** complete.  
**Residual continue pass:** context menus, property sheets, splitters, window icon, fidelity spec scaffold — complete.  
**Continue pass 2:** dark/HC chrome, dual-list Add/Remove, distinct icons, context dismiss — complete (icon theme order corrected in pass 4).  
**Continue pass 3:** menubar outside dismiss; File › New / Open… (path + saved list); column header sort; empty results state; Ctrl+N/O — complete.  
**Continue pass 4:** light icons in dark mode + dark icons in light mode; chrome hairlines; visible splitters; theme unit tests — complete.  
**Continue pass 5 (info architecture):** summary chips, property blocks, gauge charts, weighted tables, status-colored cells — complete (see below).  
**Still open:** interactive screenshot / SSIM measure against `mde/fidelity/specs/mmc.toml` (human capture; automated grim attempted when seat allows).

---

## Continue pass 5 (2026-11-20) — information division / tables / charts

Goal: clearer Win11-friendly hierarchy without dumping raw text; tables where denser; gauges for live metrics.

| Item | Status | Evidence |
|------|--------|----------|
| Results model: `summaries`, `gauges`, `properties`, `column_weights` | **done** | `model.rs` |
| Summary KPI chips under banner | **done** | `chrome::summary_chip`; rendered in `view_results` |
| Property sheet blocks (key/value) | **done** | `chrome::property_block`; Firewall / Time / SELinux mode |
| Horizontal gauge bars (charts) | **done** | `chrome::gauge_row`; PerfMon + Disk root FS |
| Weighted column tables + header hairline | **done** | `chrome::col_length`; Services/Events/Disks/… |
| Status / Level cell tinting | **done** | `chrome::cell_color` / `tone_from_status` |
| PerfMon: chips + gauges + counter table | **done** | `snapin/perfmon.rs` |
| Services: Running/Stopped/Failed chips | **done** | `snapin/services.rs` |
| Events: Level \| Time \| Source \| Message | **done** | `snapin/events.rs` |
| Disks: root gauge; LVM/remote/LUKS tables | **done** | `snapin/disks.rs` |
| Firewall / Time / SELinux / Network structured | **done** | respective snap-ins |
| Tests | **done** | `cargo test -p mde-mmc` — 5 passed |
