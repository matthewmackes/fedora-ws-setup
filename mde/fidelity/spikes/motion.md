# Spike S5 — motion

Command: `cargo test -p mde-fidelity --lib motion_pose_and_disabled_animations`

The start-open token in `tokens/motion.toml` is unmeasured and has duration 0. The mechanism test uses a candidate cubic (not a Windows measurement) and checks the settled pose.

- Final pose error: within 1 px
- `animations = false` reaches the open pose in one frame: pass

A 10-run 8 ms sleep was not used as a CI assertion because scheduler noise would flake the p99. Run `mde-fidelity spikes` on a quiet machine to record p50 and p99. If p99 exceeds 20 ms, use compositor translation (approach B) for position and keep the iced clock for opacity.

Result: PASS for the pose and the disabled-animation rule. Approach A remains the default until a machine records p99 over 20 ms.
