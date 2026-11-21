# Spike — task view / alt-tab thumbnails

`space_preview_elements` exists in `mde-anvil/src/render.rs`. This run did not host a client redraw, so no preview PNG was observed updating within 250 ms.

Result: FAIL the live cadence. Alt-tab and task view stay on icon and title until a nested session shows a non-empty `mde-preview-<id>.png` updating within 250 ms. That is a deviation, not a silent cut.
