#!/usr/bin/env bash
# GPT-6 Luna then GPT-6 Sol on VulcanBench Frontier v4 (suite
# coding-intelligence-index-v4), all five effort levels (low/medium/high/
# extra-high/max; ultra is blocked by vulcanbench.toml), through the Codex CLI
# on the ChatGPT subscription. Owner request in chat, 2026-09-25.
#
# These are the GPT-6 models (Codex slugs gpt-6-luna, gpt-6-sol), not the
# GPT-5.6 Luna and Sol already on the board; outputs go to their own roots so
# the two generations never mix.
#
# Serial (docs/DECISIONS.md 2026-09-13: speed cards rank by wall clock). If
# WAIT_PID is set, the chain waits for that process to exit first; either way
# it waits until no vulcanbench or vulcanconduct run is active.
#
#   nohup caffeinate -i bash scripts/run_gpt6_luna_sol_v4.sh > logs/gpt6-luna-sol-v4.log 2>&1 &
#   echo $! > logs/gpt6-luna-sol-v4.pid
#
# Codex CLI: pinned to 0.155.0 in its own prefix. The global 0.153.4 gets a
# 400 ("not supported when using Codex with a ChatGPT account") for both
# models; 0.155.0 is the lowest release that runs them. Other Codex columns
# keep 0.153.4 (docs/DECISIONS.md 2026-09-25).
#
# Re-running is safe: each sweep resumes with --only-missing.
set -u
cd /Users/morganlinton/dev/VulcanBench || exit 1
export CODEX_BIN_DIR=${CODEX_BIN_DIR:-/Users/morganlinton/.local/vulcanbench-codex-0.155.0/bin}
[ "$("$CODEX_BIN_DIR/codex" --version 2>&1)" = "codex-cli 0.155.0" ] || { echo "pinned Codex 0.155.0 missing at $CODEX_BIN_DIR" >&2; exit 2; }

active() {
  pgrep -f "vulcanbench run" >/dev/null 2>&1 || pgrep -f "vulcanconduct run" >/dev/null 2>&1
}

echo "=== GPT-6 LUNA+SOL CHAIN QUEUED $(date '+%F %H:%M:%S') (WAIT_PID=${WAIT_PID:-none})"
while [ -n "${WAIT_PID:-}" ] && kill -0 "$WAIT_PID" 2>/dev/null; do sleep 300; done
while active; do sleep 60; done

for pair in "gpt-6-luna runs-effort-gpt6-luna" "gpt-6-sol runs-effort-gpt6-sol"; do
  set -- $pair
  echo "=== $1 SWEEP START $(date '+%F %H:%M:%S')"
  while :; do
    MODEL=$1 OUTROOT=$2 bash scripts/cii-v4-board/run_codex_effort_sweep.sh
    status=$?
    # Exit 1 is only the concurrency guard; 3 is auth expiry (needs 'codex login'); 4 is a model the CLI refuses; 5 is a blocked level.
    [ "$status" -ne 1 ] && break
    echo "=== $1 sweep refused (a run is still active), retrying in 300s $(date '+%F %H:%M:%S')"
    sleep 300
  done
  echo "=== $1 sweep exited $status $(date '+%F %H:%M:%S')"
  if [ "$status" -ne 0 ]; then
    echo "=== CHAIN STOPPED on $1 (exit $status) $(date '+%F %H:%M:%S')"
    exit "$status"
  fi
done
echo "=== GPT-6 LUNA+SOL CHAIN DONE $(date '+%F %H:%M:%S')"
