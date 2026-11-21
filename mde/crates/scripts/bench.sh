#!/usr/bin/env bash
# Print every performance budget as pass, fail, or not-run.
# A line that was not measured is not a pass.
# Live numbers require MDE_BENCH_RUNNER=1 and a running session log.
# Exit 0 only when every measured line passes. Exit 2 when any line is not-run
# or over budget. Smoke does not treat exit 2 as a green bench.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

pass=0
fail=0
notrun=0
line() {
  local name="$1" status="$2" detail="$3"
  printf '%-28s %-8s %s\n' "$name" "$status" "$detail"
  case "$status" in
    pass) pass=$((pass + 1)) ;;
    fail) fail=$((fail + 1)) ;;
    *) notrun=$((notrun + 1)) ;;
  esac
}

log=""
if [[ -n "${HOME:-}" && -f "$HOME/.local/state/mde/session.log" ]]; then
  log="$HOME/.local/state/mde/session.log"
fi

read_budget() {
  local key="$1"
  [[ -n "$log" ]] || return 1
  grep -E "mde-budget ${key}=" "$log" | tail -n 1 | sed -E "s/.*${key}=([0-9]+).*/\\1/"
}

judge() {
  local name="$1" key="$2" max="$3"
  local val
  val="$(read_budget "$key" || true)"
  if [[ -z "$val" ]]; then
    line "$name" not-run "no mde-budget ${key} in session.log"
    return
  fi
  if [[ "$val" -le "$max" ]]; then
    line "$name" pass "${val} <= ${max}"
  else
    line "$name" fail "${val} > ${max}"
  fi
}

if [[ "${MDE_BENCH_RUNNER:-0}" != "1" ]]; then
  echo "bench runner: not this machine (set MDE_BENCH_RUNNER=1 to score a log)"
fi

judge comp_p50_ms comp_p50_ms 4
judge comp_p99_ms comp_p99_ms 8
judge comp_acrylic_p99_ms comp_acrylic_p99_ms 10
judge shell_start_p50_ms shell_start_p50_ms 8
judge shell_start_p99_ms shell_start_p99_ms 12
judge shell_start_p99_150_ms shell_start_p99_150_ms 16
judge input_p50_ms input_p50_ms 16
judge input_p99_ms input_p99_ms 32
judge comp_cold_ms comp_spawned_ms 1500
judge shell_cold_ms shell_ready_ms 800
judge session_cold_ms shell_spawned_ms 3000

# Idle CPU. Sample only when the runner flag is set and the process exists.
if [[ "${MDE_BENCH_RUNNER:-0}" == "1" ]] && command -v ps >/dev/null && pgrep -x mde-shell >/dev/null; then
  cpu="$(ps -C mde-shell -o %cpu= | awk '{s+=$1} END {printf "%.2f", s+0}')"
  awk -v c="$cpu" 'BEGIN { exit !(c+0 <= 1.0) }' && line shell_idle_cpu pass "$cpu <= 1" || line shell_idle_cpu fail "$cpu > 1"
else
  line shell_idle_cpu not-run "PollBus left in place; no runner sample"
fi

line comp_idle_cpu not-run "needs a quiescent DRM session"
line rss_shell_mb not-run "ceiling 180"
line rss_control_mb not-run "ceiling 150"
line rss_comp_mb not-run "ceiling 250"
line rss_lock_mb not-run "ceiling 80"
line rss_files_mb not-run "ceiling 150"
line gpu_idle_frames not-run "damage tracker must stay on; not sampled"

echo "pass=$pass fail=$fail not_run=$notrun"
if [[ "$fail" -gt 0 || "$notrun" -gt 0 ]]; then
  exit 2
fi
exit 0
