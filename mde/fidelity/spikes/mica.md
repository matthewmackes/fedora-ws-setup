# Spike S1 — Mica

Command: `cargo test -p mde-fidelity --lib mica_ignores_the_window_behind`

A 200×40 wallpaper of 100 px black and white columns was Kawase-blurred. The mica sample is that blurred wallpaper.

- MAE against the blurred wallpaper: 0 (the sample is that buffer)
- A second call with a different bright window behind is identical: pass

Result: PASS for approach A. Mica ignores windows behind the target. Approach B (reuse acrylic) is not used.
