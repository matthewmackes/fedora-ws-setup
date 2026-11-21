# Reference capture checklist

No Windows 11 captures are in this tree. Do not measure against `mde/assets` (Plymouth, a wallpaper, and a padOS GTK2 theme). Do not copy Segoe UI, Segoe Fluent Icons, Windows wallpapers, or SVGs out of a Windows install.

## MDE seat smoke (grim)

On a live MDE Wayland seat (`WAYLAND_DISPLAY` set), capture chrome for fidelity evidence:

```bash
mkdir -p mde/fidelity/evidence/manual
grim "mde/fidelity/evidence/manual/seat-$(date +%Y%m%d-%H%M%S).png"
# Region: grim -g "$(slurp)" …
# Shell modes (write ShellCmd into the running session):
#   screenshot:screen | screenshot:area | screenshot:window | screenshot (snip)
```

Optional nested compositor smoke (when `mde-comp --features nested` is available): start nested, run `grim` against that display, confirm PNGs land under `Pictures/Screenshots`.

### Optional capture tools

Default install ships `grim`, `slurp`, `swappy`, `wl-clipboard`. Optional fallbacks (not in `MDE_WAYLAND_PKGS`): `gnome-screenshot`, `spectacle`. Color pick prefers `hyprpicker` or `wl-color-picker` when installed; otherwise grim+slurp PPM sample.

Record the Windows build (`winver`) into every spec's `reference_build` when you promote it. Hide the cursor. PNG, lossless, 8-bit, no scaling after capture.

## Displays

- 1920×1080 at 100% (96 DPI), scale tag `1.0`
- 2560×1600 at 150% (144 DPI), scale tag `1.5`

## Themes

`light`, `dark`, `hc-black`, `hc-white`, accent `#0078D4`.

Extra dark 1920×1080 capture with a non-blue accent, named `dark_1.0_<state>_accent.png`, for taskbar, Start, and title bar only.

## Files

`mde/fidelity/refs/<surface>/<theme>_<scale>_<state>.png`

The full list is `mde/fidelity/missing-captures.txt`.

| Surface | States |
|---|---|
| taskbar | rest, hover-icon, pressed-icon, running, overflow, autohide |
| start | open, search-empty, search-query, all-apps, power-confirm |
| search | search-focus |
| widgets | open, hover-card |
| notification-center | empty, grouped, calendar-open |
| quick-settings | open, slider-hover, dnd-on |
| calendar | month, today, agenda |
| tray-overflow | open, menu |
| task-view | two-desktops, hover-window |
| snap-layouts | flyout, hover-zone, assist |
| context-menus | taskbar-pin, desktop |
| files | home, selected, context-menu, address-focus |
| titlebar | inactive, hover, max, snap-hover |
| settings | home, personalization, search |
| lock | clock, password, caps |
| login | clock, password, error |
| osd | volume, brightness |
| toasts | one, stacked, action |
| alt-tab | three-windows, desktop-selected |

## Motion

Dark, 1920×1080, 100% only, 60 fps frames:

Start open, Start close, quick settings open, notification center open, snap flyout open, toast in.

Store `mde/fidelity/refs/<surface>/motion_<action>/frame_####.png` plus `timestamps.txt` (frame index and milliseconds from the triggering input).

## After capture

From `mde/crates`:

```bash
export CARGO_TARGET_DIR=$PWD/target-user
./scripts/measure.sh bar --image ../fidelity/refs/taskbar/dark_1.0_rest.png
./scripts/measure.sh color --image ../fidelity/refs/taskbar/dark_1.0_rest.png --x 20 --y 1050
./scripts/measure.sh scale --logical 0,1032,1920,48 --physical 0,1548,2880,72 --factor 1.5
```

Write the numbers into the spec with `source` set to the file and pixel. Set `status = "measured"` only when every state file exists and the 1.0 and 1.5 logical rectangles agree within 1 px. The tool checks that agreement; it does not invent a missing number.
