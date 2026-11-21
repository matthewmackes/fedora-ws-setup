#!/usr/bin/env bash
# Fuzz snap tokens, shell command classes, and config TOML.
# Default cargo test is a short burst. Set MDE_FUZZ_SECONDS=60 for the CI minute.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-user}"
export MDE_FUZZ_SECONDS="${MDE_FUZZ_SECONDS:-60}"
cd "$ROOT"
echo "== fuzz ${MDE_FUZZ_SECONDS}s =="
cargo test -p mde-fidelity --lib fuzz_parsers_accept_garbage -- --exact --nocapture
echo "FUZZ OK"
