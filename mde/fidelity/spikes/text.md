# Spike S4 — text

Command: `cargo test -p mde-fidelity --lib selawik_advances_match`

Font: `/usr/share/fonts/mde/Selawik/selawk.ttf` (OFL). String `Settings 08:30` at 14 px, shaped twice with cosmic-text 0.12 grayscale.

- Advance-width delta: within 1 px (the two rasters match)
- Stem-coverage MAE: within 0.05

Subpixel RGB glyphs were not used. Grayscale is approach A.

Result: PASS. Lock and SSD title text should use this path once their specs are measured. The lock bitmap font is still in place until that card runs.
