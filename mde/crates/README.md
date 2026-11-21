# MDE Rust platform

Unified Mackes Desktop Environment built in Rust.

| Crate | Role |
|-------|------|
| `mde-config` | Single TOML store (`~/.config/mde/config.toml`) + legacy migration |
| `mde-theme` | Iced/compositor palette tokens (no GTK/Qt bridges) |
| `mde-comp` | Smithay compositor scaffold — tabs-only, outputs, greeter stage |
| `mde-greeter` | Iced Win11-style sign-in for greetd; also builds `mde-lock` (ext-session-lock client) |
| `mde-shell` | One iced process: taskbar, Start, QS, tray, notifications, clipboard |
| `mde-control` | Settings + MMC + **Config Editor** (Regedit replacement) |
| `mde-privileged` | Polkit-gated helper |
| `mde-session` | Session entry after greetd handoff |
| `mde-neighbourhood` | Network Neighbourhood launcher |
| `mde-music` | Neighbourhood Music (optional; `cargo build -p mde-music`) |

## Build

```bash
cd mde/crates
export CARGO_TARGET_DIR=$PWD/target-user
cargo test -p mde-config -p mde-theme -p mde-comp
cargo build --release
# Nested winit window (no DRM/VT): 
# cargo run -p mde-comp --features nested -- --nested
```

Legacy Python-delegating crates (`mde-taskbar`, `mde-start`, `mde-settings`, `mde-mmc`) are thin shims to `mde-shell` / `mde-control` and are not workspace members.

## Cutover gate

**CUTOVER v1** (daily driver): Rust greetd → `mde-session` → `mde-comp` + `mde-shell`, with soft lock / `mde-lock` (ext-session-lock + PAM unlock), clipboard paste-back, Settings import + volume, tray. Graphical greeter is **L1 done**; session lock is **L2 done**. See [CUTOVER.md](CUTOVER.md).

```bash
./scripts/smoke.sh
```

Install release bins to `~/.local/bin` (session PATH) and/or `/usr/bin` with sudo.
