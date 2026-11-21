# Spike S2 — rounded shadow

Command: `cargo test -p mde-fidelity --lib shadow_corner_is_round`

200×120 rect, radius 8 px (titlebar spec is unmeasured, so the spike uses the fallback), shadow box blur 8, over the alpha mask.

- Bilinear alpha on eight quarter-circle samples is within 0.5 of the 0.5 edge: pass
- Outer corner alpha of the padded box is under 0.05: pass
- Side sample just outside the top edge is over 0.2: pass

Result: PASS for approach A (SDF rounded rect plus a separable shadow). Use A for SSD when the titlebar spec is measured. Nine-patch shadows are not used.
