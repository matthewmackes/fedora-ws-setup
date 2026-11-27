#!/usr/bin/env bash
# Headless cutover smoke tests for the Rust MDE platform.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target-user}"
export CARGO_TARGET_DIR="$TARGET"

cd "$ROOT"
echo "== cargo test (foundation) =="
cargo test -p mde-config -p mde-theme -p mde-comp
cargo test -p mde-shell --bins

echo "== EQ oracle (Audio EQ Cookbook ±0.5 dB) =="
cargo test -p mde-music --bin neighbourhood-music -- oracle_peaking_matches_cookbook

echo "== release build =="
cargo build --release \
  -p mde-comp -p mde-shell -p mde-control -p mde-session \
  -p mde-privileged -p mde-neighbourhood -p mde-greeter

BIN="$TARGET/release"
echo "== selftests =="
out="$("$BIN/mde-comp" selftest)"
echo "$out"
echo "$out" | grep -q '"ok":true'
out="$("$BIN/mde-privileged" selftest)"
echo "$out"
echo "$out" | grep -q '"ok":true'
out="$("$BIN/mde-comp" greeter)"
echo "$out"
echo "$out" | grep -q '"ready":true'
out="$("$BIN/mde-lock" --selftest)"
echo "$out"
echo "$out" | grep -q '"ok":true'
echo "$out" | grep -q 'pam-login'

echo "== config round-trip =="
TMP="$(mktemp -d)"
export XDG_CONFIG_HOME="$TMP"
# Always isolate runtime so smoke never steals a live session socket/DRM seat.
export XDG_RUNTIME_DIR="$TMP/run"
mkdir -p "$XDG_RUNTIME_DIR" "$TMP/mde"
cat >"$TMP/mde/config.toml" <<'EOF'
schema = 1
active_profile = "default"

[profiles.default]
display_name = "Smoke"

[profiles.default.session]
xwayland = false

[profiles.default.compositor]
layout = "tabs"
server_tab_strip = true

[[profiles.default.displays.profiles]]
name = "laptop"

[[profiles.default.displays.profiles.outputs]]
name = "eDP-1"
enabled = true
mode = "1920x1080"
scale = 1.25
EOF
"$BIN/mde-comp" outputs >/dev/null

echo "== ctl against control-plane =="
"$BIN/mde-comp" --ctl-only >/tmp/mde-comp-session.log 2>&1 &
comp_pid=$!
trap 'kill "$comp_pid" 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do
  [[ -S "$XDG_RUNTIME_DIR/mde-comp.sock" ]] && break
  sleep 0.05
done
"$BIN/mde-comp" ctl tab-add 1 Firefox firefox >/dev/null
out="$("$BIN/mde-comp" ctl apply-profile laptop)"
echo "$out"
echo "$out" | grep -q '"ok": true\|"ok":true'
echo "$out" | grep -q '"applied"'
"$BIN/mde-comp" ctl tabs | grep -q Firefox
kill "$comp_pid" 2>/dev/null || true
wait "$comp_pid" 2>/dev/null || true
trap - EXIT
echo ok

echo "== token gate (theme crate) =="
./scripts/check-tokens.sh --theme

echo "SMOKE OK"
