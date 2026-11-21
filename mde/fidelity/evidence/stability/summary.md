# Stability

- `mde-config::supervisor` decides compositor restart (3, then stop) and lock respawn (10, crash never unlocks). Unit tests pass.
- `mde-session` restarts `mde-comp` with that policy and logs `mde-budget` timestamps. Shell respawn is unchanged (8, then 60 s cooldown).
- `mde-shell` respawns `mde-lock` on a non-zero exit and only unlocks on exit 0. A lock flag at `$XDG_RUNTIME_DIR/mde-locked` re-engages the lock if the shell itself restarts.
- Resize grabs in `mde-anvil` log and cancel on an unexpected state. They no longer panic.
- Panic hooks that append to `~/.local/state/mde/session.log` and abort are installed in session, comp, shell, control, greeter, lock, and files.
- `scripts/bench.sh` prints every budget as pass, fail, or not-run. Without `MDE_BENCH_RUNNER=1` the lines are not-run and the script exits 2. That is not a pass.
- `scripts/soak.sh --self-test` checks the 1 MB/hour slope math. A 30 minute live soak was not started.
- `scripts/fuzz.sh` runs the garbage-input parser test for `MDE_FUZZ_SECONDS` (default 60). It was not left running for a full minute in this pass; `cargo test -p mde-fidelity --lib fuzz_parsers_accept_garbage` covers a 2000-iteration burst.
