# Context menus — behavior landed (W392)

Shell right-click surfaces for W381–W391 are implemented. Visual reference PNGs for Win11 comparison are still required before SSIM gates.

## Ready for capture

| State | How to open in MDE |
|-------|--------------------|
| `taskbar-pin` | Right-click a pinned taskbar app → jump list (Open / Unpin / End task or Close all / Recent) |
| `desktop` | Right-click empty desktop (compositor → `desktopmenu`) → View / Sort / Refresh / New / Personalize / Display |

## Also shippable (not in `context-menus.toml` states yet)

- Unpinned running jump list (same shape as pin)
- Start pin / All apps / Recommended app context menus
- Taskbar background menu (no clock format toggles)
- Win+X Quick Link expanded
- SSD title-bar system menu (`sysmenu`)

## Spec

`mde/fidelity/specs/context-menus.toml` — status moved to `pending-capture` (behavior done; refs empty).
