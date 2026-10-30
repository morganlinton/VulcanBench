#!/usr/bin/env bash
# GPT-6.1 Terra on VulcanBench Frontier v4 (suite coding-intelligence-index-v4),
# all five effort levels (low/medium/high/extra-high/max; ultra stays blocked by
# vulcanbench.toml), through the Codex CLI on the ChatGPT subscription.
# Owner request in chat, 2026-10-26: benchmark the model on its release day.
#
# Codex CLI: pinned to 0.159.0 in its own prefix, the release OpenAI names as
# the minimum for gpt-6.1-terra and the one GPT-6.1 Sol ran on
# (docs/DECISIONS.md 2026-10-26). Other Codex columns keep their versions.
#
# Run it from a run worktree pinned to a bench tag (docs/HOW_WE_WORK.md); it
# runs the code of the checkout it lives in and writes to OUTROOT, which
# defaults to the main checkout's gitignored runs folder:
#
#   cd ../VulcanBench-runs/bench-2026-10-27-gpt61-terra
#   nohup caffeinate -i bash scripts/run_gpt61_terra_v4.sh > $HOME/dev/VulcanBench/logs/gpt61-terra-v4.log 2>&1 &
#   echo $! > $HOME/dev/VulcanBench/logs/gpt61-terra-v4.pid
#
# Re-running is safe: the sweep resumes with --only-missing.
set -u
cd "$(dirname "$0")/.." || exit 1
# Detached runs start with the bare macOS PATH; codex.js needs node.
export PATH="$HOME/.local/node/bin:$PATH"
export CODEX_BIN_DIR=${CODEX_BIN_DIR:-$HOME/.local/share/vulcanbench/codex-0.159.0/node_modules/.bin}
export OUTROOT=${OUTROOT:-$HOME/dev/VulcanBench/runs-effort-gpt61-terra}
[ "$("$CODEX_BIN_DIR/codex" --version 2>&1)" = "codex-cli 0.159.0" ] || { echo "pinned Codex 0.159.0 missing at $CODEX_BIN_DIR" >&2; exit 2; }

echo "=== GPT-6.1 TERRA CHAIN QUEUED $(date '+%F %H:%M:%S') from $PWD ($(git describe --tags --always --dirty 2>/dev/null))"
while pgrep -f "vulcanbench run" >/dev/null 2>&1 || pgrep -f "vulcanconduct run" >/dev/null 2>&1; do sleep 60; done
echo "=== gpt-6.1-terra SWEEP START $(date '+%F %H:%M:%S')"
while :; do
  MODEL=gpt-6.1-terra bash scripts/cii-v4-board/run_codex_effort_sweep.sh
  status=$?
  # Exit 1 is only the concurrency guard; 3 is auth expiry (needs 'codex login'); 4 is a model the CLI refuses; 5 is a blocked level.
  [ "$status" -ne 1 ] && break
  echo "=== sweep refused (a run is still active), retrying in 300s $(date '+%F %H:%M:%S')"
  sleep 300
done
echo "=== gpt-6.1-terra sweep exited $status $(date '+%F %H:%M:%S')"
if [ "$status" -ne 0 ]; then
  echo "=== CHAIN STOPPED on gpt-6.1-terra (exit $status) $(date '+%F %H:%M:%S')"
  exit "$status"
fi
echo "=== GPT-6.1 TERRA CHAIN DONE $(date '+%F %H:%M:%S')"
