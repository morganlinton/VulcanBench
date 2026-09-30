#!/usr/bin/env bash
# GPT-6.1 Sol on VulcanBench Frontier v4 (suite coding-intelligence-index-v4),
# all five effort levels (low/medium/high/extra-high/max; Codex offers no
# ultra for this model), through the Codex CLI on the ChatGPT subscription.
# Owner request in chat, 2026-09-29: run it now, alongside GPT-6 Sol's
# Code quality judging (v3.17), and ahead of the held Grok 4.7 chain.
#
# Codex CLI: pinned to 0.159.0 in its own prefix, the lowest release that
# serves gpt-6.1-sol on a ChatGPT account (0.157.0 and 0.158.0 are refused;
# docs/DECISIONS.md 2026-09-29). Other Codex columns keep their versions.
#
#   nohup caffeinate -i bash scripts/run_gpt61_sol_v4.sh > logs/gpt61-sol-v4.log 2>&1 &
#   echo $! > logs/gpt61-sol-v4.pid
#
# Re-running is safe: the sweep resumes with --only-missing.
set -u
cd /Users/morganlinton/dev/VulcanBench || exit 1
export CODEX_BIN_DIR=${CODEX_BIN_DIR:-/Users/morganlinton/.local/vulcanbench-codex-0.159.0/bin}
[ "$("$CODEX_BIN_DIR/codex" --version 2>&1)" = "codex-cli 0.159.0" ] || { echo "pinned Codex 0.159.0 missing at $CODEX_BIN_DIR" >&2; exit 2; }

echo "=== GPT-6.1 SOL CHAIN QUEUED $(date '+%F %H:%M:%S')"
while pgrep -f "vulcanbench run" >/dev/null 2>&1 || pgrep -f "vulcanconduct run" >/dev/null 2>&1; do sleep 60; done
echo "=== gpt-6.1-sol SWEEP START $(date '+%F %H:%M:%S')"
while :; do
  MODEL=gpt-6.1-sol OUTROOT=runs-effort-gpt61-sol bash scripts/cii-v4-board/run_codex_effort_sweep.sh
  status=$?
  # Exit 1 is only the concurrency guard; 3 is auth expiry (needs 'codex login'); 4 is a model the CLI refuses; 5 is a blocked level.
  [ "$status" -ne 1 ] && break
  echo "=== sweep refused (a run is still active), retrying in 300s $(date '+%F %H:%M:%S')"
  sleep 300
done
echo "=== gpt-6.1-sol sweep exited $status $(date '+%F %H:%M:%S')"
if [ "$status" -ne 0 ]; then
  echo "=== CHAIN STOPPED on gpt-6.1-sol (exit $status) $(date '+%F %H:%M:%S')"
  exit "$status"
fi
echo "=== GPT-6.1 SOL CHAIN DONE $(date '+%F %H:%M:%S')"
