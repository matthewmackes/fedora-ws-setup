# MDE — Mackes Desktop Environment 2026

Standalone desktop environment for Fedora: a **Rust** platform (`mde-comp` DRM/smithay + `mde-shell` + `mde-control`) with tabs-only windowing intent, iced shell chrome, and a single TOML config store. Login is **seatd + greetd** → `cage` + iced `mde-greeter` (or `agreety` without cage) → `mde-session` (LightDM / labwc / Python shell are not in the default install).

## Install

```bash
cd mde
sudo ./install.sh --profile=workstation
# or shell only:
sudo ./install.sh --profile=minimal
```

Dry-run:

```bash
./install.sh --dry-run --profile=workstation
```

Build uses `CARGO_TARGET_DIR=crates/target-user` and installs binaries under `/usr/bin`.

## Sessions

At greetd you get:

- **Mackes Desktop Environment** — `mde.desktop` → `mde-session` → `mde-comp --session` + `mde-shell`

## Layout

| Path | Purpose |
|------|---------|
| `/usr/bin/mde-session`, `mde-comp`, `mde-shell`, `mde-control`, `mde-greeter` | Session, DRM compositor (+ ctl socket), shell, Settings, graphical greeter |
| `/usr/bin/mde-privileged` | Polkit-gated helper |
| `~/.config/mde/config.toml` | Single config store (Config Editor in Settings) |
| `/usr/share/wayland-sessions/mde.desktop` | Login session |
| `/etc/greetd/config.toml` | Greeter stage (`cage -s -- mde-comp greeter` or `agreety` → `mde-session`; Fedora user `greetd`) |

## Uninstall

```bash
sudo ./install.sh --uninstall
sudo ./install.sh --uninstall --purge-config
```

Other desktop environments are **not** removed.

## Profiles

- **minimal** — seatd/greetd + Rust platform + plumbing
- **workstation** — minimal + baseline apps / firewalld / plymouth

## Cutover / smoke

```bash
cd mde/crates
./scripts/smoke.sh
```

See `crates/CUTOVER.md` for the one-laptop manual checklist.

## Neighbourhood Music

```bash
cd mde/crates && cargo build --release -p mde-music
sudo ./data/neighbourhood-music/install.sh
```
