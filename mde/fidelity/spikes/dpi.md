# Spike S3 — fractional DPI

Command: `cargo test -p mde-fidelity --lib fractional_hit_targets`

Scales 1.0, 1.25, 1.5, 1.75, 2.0. A 48×48 logical button at (100, 80).

- Physical width equals `round(48 * scale)` within 1 px: pass
- Click 1 physical px inside the origin hits: pass
- Click 1 physical px outside misses: pass

`iced_layershell` 0.13.7 stores `wayland_scale_factor` from the compositor scale event (`application/state.rs`). It can report scale. No iced fork.

Result: PASS for approach A (fractional scale, logical hit testing). Integer-scale-2 downscale is not used.
