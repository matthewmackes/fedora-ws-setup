#!/usr/bin/env bash
# Run the fidelity measure binary. Does not invent reference numbers.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-user}"
cd "$ROOT"
cargo run -q -p mde-fidelity -- "$@"
