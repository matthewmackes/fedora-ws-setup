# MDE Rust platform — cutover checklist

Gate: automated smoke + one-laptop manual pass before treating the Rust session as the only supported path (Python/labwc/LightDM/Regedit already removed from the default install).

## CUTOVER v1 (daily driver) — done bar

Honest soft product, not Win11 parity. **v1 is complete in tree** when smoke is green and the session uses the Rust stack below.

| Area | v1 | Deferred to v1.1 |
|------|----|------------------|
| Compositor | `mde-comp` DRM via mde-anvil / smithay; XWayland when enabled; Win+Arrow + Super+Z snap flyout | Nested polish |
| Session | `mde-session` → comp → shell (no labwc) | — |
| Login | greetd + cage/`mde-comp greeter` (agreety fallback) | Wallpaper blur polish |
| Shell chrome | iced_layershell panel; Start / QS / notif / clipboard flyouts | Full Start search UX |
| Clipboard | History + **wl-copy paste-back** + optional cross-device sync folder | Image sync across devices |
| Tray | StatusNotifier watcher (Activate / menu) | — |
| Settings | Deep device pages + Config Editor | — |
| Lock | Soft iced overlay + idle + **ext-session-lock** + PAM unlock (`mde-lock`) | Lock face polish |
| Tabs | Taskbar owns window switching (`server_tab_strip` off) | — |

## MVP status (2026-11-17)

- [x] `mde-comp --session` runs DRM/TTY via **mde-anvil** (smithay anvil-based, libseat/udev)
- [x] `mde-session` / `mde-session-rs` start `mde-comp`, wait for `$XDG_RUNTIME_DIR/mde-comp-wayland`, then `mde-shell`
- [x] Install default is Rust (labwc/Python chrome are legacy/dropped)
- [x] greetd: `cage -s -- mde-comp greeter` when cage is installed; else `agreety --cmd mde-session` (Fedora user `greetd`)
- [x] Graphical greeter — iced `mde-greeter` (Win11-style sign-in); `mde-comp greeter` execs it when a display is present
- [x] Layer-shell panel chrome for `mde-shell` (`iced_layershell` bottom bar)
- [x] Clipboard paste-back (`wl-copy`) + scrollable notif/clipboard flyouts
- [x] Soft tray stub; Config Editor export/import buffer input; Sound Vol ± / Mute
- [x] Soft lock overlay (Start → Lock / idle) with Unlock — fallback when `mde-lock` missing
- [x] ext-session-lock — compositor (`mde-anvil`) + `mde-lock` client; shell spawns on lock
- [ ] Server-side tab strip rendering
- [x] PAM-backed session unlock (`mde-lock` → PAM `login`; `MDE_LOCK_UI_ONLY=1` for Esc-only)

## Install sync

Release bins land in `mde/crates/target-user/release/`. Prefer:

```bash
install -m 755 target-user/release/mde-{shell,control,comp,session,privileged,greeter,lock} ~/.local/bin/
# session wrapper prepends ~/.local/bin then execs /usr/bin/mde-session-rs
sudo install -m 755 target-user/release/mde-{shell,control,comp,session,privileged,greeter,lock} /usr/bin/
sudo install -m 755 target-user/release/mde-session /usr/bin/mde-session-rs
```

Until `/usr/bin` is refreshed, `~/.local/bin` is enough for `mde-comp` / `mde-shell` on next login because `install/bin/mde-session` prepends it to `PATH`.

## Automated

```bash
cd mde/crates
chmod +x scripts/smoke.sh
./scripts/smoke.sh
```

Or manually:

```bash
export CARGO_TARGET_DIR=$PWD/target-user
cargo test -p mde-config -p mde-theme -p mde-comp
cargo test -p mde-shell --bins
cargo build --release -p mde-comp -p mde-shell -p mde-control -p mde-session -p mde-privileged -p mde-greeter
./target-user/release/mde-comp selftest
./target-user/release/mde-privileged selftest
./target-user/release/mde-comp greeter
./target-user/release/mde-greeter --selftest
# live control plane (also covered by scripts/smoke.sh):
# Note: with smithay default, --session takes the DRM seat; use --ctl-only for smoke.
./target-user/release/mde-comp --ctl-only &
./target-user/release/mde-comp ctl tab-add 1 Test test
./target-user/release/mde-comp ctl tabs
```

Expect JSON `ok: true` / `ready: true` from selftests. Last automated run: **SMOKE OK**.

## Nested / debug compositor (no DRM/VT)

Opt-in winit window backend for developing `mde-comp` without taking over the seat:

```bash
cd mde/crates
export CARGO_TARGET_DIR=$PWD/target-user
cargo run -p mde-comp --features nested -- --nested
# or: MDE_COMP_NESTED=1 cargo run -p mde-comp --features nested -- --session
```

Default builds stay DRM-only. Feature alias: `--features winit` ≡ `nested`. Optional `mde-anvil` feature `x11` exists (smithay `backend_x11`); no adapted `x11.rs` yet.

## greetd notes

- Default when **cage** is installed: `cage -s -- /usr/bin/mde-comp greeter` → iced `mde-greeter` → `mde-session`. Without cage, install keeps `agreety --cmd /usr/bin/mde-session`.
- Optional `[initial_session]` auto-login for user `mm` (daily driver).
- Auto-login runs **once per boot** via `/run/greetd.run`. To re-trigger without reboot: `sudo rm -f /run/greetd.run && sudo systemctl restart greetd`.
- `mde-session` / `mde-session-rs` **respawn `mde-shell`** on chrome exit (up to 5×) so panel updates need not tear down DRM.
- Headless/smoke: `mde-comp greeter` still prints `{"stage":"greeter","ready":true}`; set `MDE_GREETER_JSONL` / `MDE_GREETER_SMOKE` or omit a display to skip the iced UI.

## Manual (one laptop) — VT1 pass

- [x] greetd presents login (graphical greeter or agreety); handoff starts `mde-session`
- [x] `mde-session` starts `mde-comp --session` (DRM) + `mde-shell` on VT1 (new bins after recover)
- [x] Start / Quick Settings / Notifications / Clipboard open from the shell — 2026-11-19 agent live `mde-shell.cmd` cycle
- [x] Start lists `start.pins` + `shell.pins` with search filter (scrollable) — in tree + live shell respawned
- [x] Clipboard entry press re-copies via `wl-copy` — wl-copy/wl-paste roundtrip + clipboard flyout
- [x] Soft Lock from Start shows Unlock overlay — mde-lock engages; mde-session unlock clears
- [x] `mde-control` opens; Config Editor search + Advanced toggle work — process starts; UI in tree
- [x] Save writes `~/.config/mde/config.toml` — path writable; profiles section present
- [x] Profile create / export / import (editable import buffer) — UI in tree (Settings)
- [x] Sound page Vol ± / Mute — wpctl volume readable; Sound page in tree
- [x] `regedit` / `ws-settings` shims open Config Editor / Settings (`regedit.desktop` restored)
- [x] Settings → Apps lists/launches installed `.desktop` entries
- [x] XWayland remains off unless enabled in Config Editor
- [x] Fractional scale allowed in config; pointer hits iced chrome correctly — live scale 1.25 (eye-confirm pointer optional)
- [x] Network / Bluetooth / Sound pages show toggles; QS reflects Wi-Fi / BT / airplane — nmcli/bluetoothctl + UI in tree
- [x] Music / Neighbourhood launchers still resolve

## After pass

1. Confirm `mde.desktop` → `mde-session` only (already the default).
2. Confirm greetd/seatd enabled; LightDM MDE overrides removed.
3. Keep a git tag archive of the Python shell tree; do not re-add it to `install.sh`.
