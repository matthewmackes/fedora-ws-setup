# Screenshots (W500–W515)

## Shortcuts (mde-anvil → mde-shell)

| Shortcut | ShellCmd | Behavior |
|----------|----------|----------|
| Print | `screenshot` (snip) | Open snip overlay (toolbar) |
| Shift+Print | `screenshot:area` | Open overlay · Rectangular preselected |
| Super+Shift+S | `screenshot` (snip) | Open overlay (Win11 snip) |
| Super+Print | `screenshot:window` | Open overlay · Window preselected |

Overlay modes: **Rectangular** (slurp + grim), **Window** (focused geom IPC → grim `-g`), **Full screen** (grim). Escape / Cancel dismisses. No Freeform (slurp cannot do freeform cleanly).

## After capture

Successful shots save under Screenshots, `wl-copy` the PNG, and show a bottom-right **thumbnail toast** (~5s). Click the toast to open **swappy** (or `xdg-open` if swappy is missing). Swappy is not auto-launched on region capture.

## Save location

Default: `{xdg-user-dir PICTURES}/Screenshots/Screenshot-YYYYMMDD-HHMMSS.png`.

Override with `shell.screenshot_dir` in `~/.config/mde/config.toml` (Settings › Gaming › Captures).

## Fallbacks

1. grim (+ slurp for region)
2. ashpd Screenshot portal (blocking; copies `file://` URI into the save path)
3. `gnome-screenshot` / `spectacle` if installed

Region cancel (Escape) never falls through to fullscreen.

## Helper

`/usr/bin/ws-screenshot [screen|area|snip|window geom]` mirrors the same capture backends for labwc/manual use (no overlay).
