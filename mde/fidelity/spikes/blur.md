# Spike S0 — backdrop blur

Command: `cargo test -p mde-fidelity --lib blur_moves_the_step_and_is_not_a_flat_tint`

Geometry on a 320×180 stand-in (left black, right white, red window crossing the bar), Kawase offsets 4, 8, 16, 24:

- Bar pixels are not pure red: pass
- Bar is not one flat tint: pass
- Step edge spreads by at least 8 px: pass

GPU timestamp queries were not available. No GLES device was opened, so the 1.5 ms / llvmpipe 8 ms clause was not measured.

Result: FAIL the shipping gate. Approach A (framebuffer Kawase behind layer-shell) stays the chosen approach. Do not paint tinted rectangles as acrylic. Surface cards that require S0 stay blocked until a GLES run records GPU time within budget.
