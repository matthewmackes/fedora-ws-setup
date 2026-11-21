# Spike S6 — snap layouts

Command: `cargo test -p mde-fidelity --lib snap_rects_match_integer_fractions`

Work area 1920×1032 after a 48 px bar (the current product constant, not a measured Windows size).

- Left half: (0, 0, 960, 1032)
- Right half: x = 960, width 960
- Top-left quarter: 960×516
- Left two-thirds: width 1280
- Opposite of left is right, and that rect starts where left ends

These fractions match the compositor's integer snap math. `specs/snap-layouts.toml` is still unmeasured, so this does not claim Windows pixel parity.

Result: PASS for approach A (keep `mde-snap.req`, rects from fractions). The picker stays in the shell.
