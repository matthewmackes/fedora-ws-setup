# MMC fidelity evidence notes

**Date:** 2026-11-20

## Automated grim attempt

- Built `target-user/release/mde-mmc`.
- Launched `mde-mmc services.msc` on `WAYLAND_DISPLAY=wayland-1` with a 3s timeout.
- Captured full seat via `grim` → `services-live.png` (≈1.5 MB).
- Result: seat capture succeeded, but the frame shows the active editor/desktop rather than a clear `mde-mmc` chrome close-up (window may not have focused / may have been covered). Treat as **seat smoke only**, not an SSIM reference for `specs/mmc.toml`.

## Icon contrast (pass 4)

- See `icon-contrast.md` + `icon-*-{light,dark}-glyphs.png`.
- Mackes-Carbon-Light opaque luminance ≈ **244** (light glyphs for dark UI).
- Mackes-Carbon opaque luminance ≈ **0** (dark glyphs for light UI).
- Code path: `mde_theme::icons::for_ui` → `chrome::icon_for(palette, …)` with `palette.dark`.

## Next human measure steps

1. Run `mde-mmc compmgmt.msc` and `mde-mmc services.msc` on the MDE seat (dark mode on — config `mode = "dark"`).
2. Capture states listed in `mde/fidelity/specs/mmc.toml` (`compmgmt-root`, `services-list`, `add-remove-snapins`).
3. Fill `[[states]].refs` and region rects; flip `status` to `measured` when metrics are real.
