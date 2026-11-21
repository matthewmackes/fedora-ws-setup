# Surfaces

Visual cards are blocked. Every spec under `mde/fidelity/specs/` is `status = "unmeasured"`. The missing PNG list is `mde/fidelity/missing-captures.txt` (463 files). No pixel values were invented and no surface was restyled to a guessed Windows size.

Claims: `mde/fidelity/claims/C0.toml` through `E1.toml`, all `blocked`.

What did land without a reference capture:

- Shell views live in per-surface modules (`taskbar.rs`, `start.rs`, `quick.rs`, `notify_view.rs`, `calendar_view.rs`, `widgets_view.rs`, `tray_view.rs`, `osd_view.rs`, `alttab.rs`, `taskview.rs`, `snap_view.rs`, `menus.rs`). `cargo test -p mde-shell --bins` passed, 34 tests.
- `mde-files` exists and opens the path with `xdg-open`. It does not draw Explorer chrome.
- Mackes-Carbon stays. Its license is Apache-2.0 (`mde/fidelity/LICENSES/mackes-carbon.txt`).

Acrylic is not shipping. Spike S0 failed the GPU time clause (`mde/fidelity/spikes/blur.md`).
