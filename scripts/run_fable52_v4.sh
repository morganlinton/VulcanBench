#!/usr/bin/env bash
# Claude Fable 5.2 on VulcanBench Frontier v4 (suite coding-intelligence-index-v4)
# through Claude Code on the claude.ai subscription. Owner request in chat,
# 2026-11-09: benchmark the model on its release day, keep full traces
# (tool calls included) and analyze how it thinks differently from Fable 5.1.
#
# Run conditions (docs/DECISIONS.md 2026-11-09):
# - Claude Code pinned to 2.1.321, the first release that recognizes
#   claude-fable-5-2 (2.1.307 serves it but logs it as an unrecognized model
#   with no price basis). The pinned binary is checked by sha256 and never
#   self-updates (the adapter sets DISABLE_AUTOUPDATER=1).
# - Thinking summaries recorded: --thinking-display summarized, through
#   VULCANBENCH_CLAUDE_THINKING_DISPLAY. Visibility only per Anthropic's docs.
# - Refusal fallback on (Claude Code's default), counted and disclosed, as for
#   every Claude column since 2026-09-24.
# - LEVELS defaults to "low": the owner reviews Low's quota and credit use
#   before Medium to Max run (LEVELS="medium high extra-high max").
#
# Run it from a run worktree pinned to a bench tag (docs/HOW_WE_WORK.md):
#
#   cd ../VulcanBench-runs/bench-2026-11-09-fable52-frontier-v4
#   nohup caffeinate -i bash scripts/run_fable52_v4.sh > $HOME/dev/VulcanBench/logs/fable52-v4.log 2>&1 &
#   echo $! > $HOME/dev/VulcanBench/logs/fable52-v4.pid
#
# Re-running is safe: the sweep resumes with --only-missing.
set -u
cd "$(dirname "$0")/.." || exit 1

PIN_DIR=${CLAUDE_PIN_DIR:-$HOME/.local/share/vulcanbench/claude-2.1.321}
PIN_SHA=abc9feeeca8091d51b623291e044cf0528c2f6b74e0c1d6ef671d52c73f7cd3c
[ "$(shasum -a 256 "$PIN_DIR/claude" 2>/dev/null | cut -d' ' -f1)" = "$PIN_SHA" ] \
  || { echo "pinned Claude Code 2.1.321 missing or changed at $PIN_DIR/claude" >&2; exit 2; }
mkdir -p "$PIN_DIR/bin"
ln -sf ../claude "$PIN_DIR/bin/claude"
# The pinned CLI must win over the global install for the runs and for the
# sweep's quota probe alike.
export PATH="$PIN_DIR/bin:$HOME/.local/node/bin:$PATH"
export DISABLE_AUTOUPDATER=1
[ "$(claude --version 2>&1)" = "2.1.321 (Claude Code)" ] \
  || { echo "claude on PATH is not the 2.1.321 pin: $(command -v claude)" >&2; exit 2; }

unset CLAUDE_CODE_DISABLE_REFUSAL_FALLBACK
export VULCANBENCH_CLAUDE_THINKING_DISPLAY=summarized
export MODEL=claude-code:claude-fable-5-2
export PROBE=claude-fable-5-2
export OUTROOT=${OUTROOT:-$HOME/dev/VulcanBench/runs-effort-fable52}
export LEVELS=${LEVELS:-low}

echo "=== FABLE 5.2 CHAIN QUEUED $(date '+%F %H:%M:%S') from $PWD ($(git describe --tags --always --dirty 2>/dev/null)) levels: $LEVELS"
while pgrep -f "vulcanbench run" >/dev/null 2>&1 || pgrep -f "vulcanconduct run" >/dev/null 2>&1; do sleep 60; done
echo "=== claude-fable-5-2 SWEEP START $(date '+%F %H:%M:%S')"
while :; do
  bash scripts/cii-v4-board/run_effort_sweep.sh
  status=$?
  # Exit 1 is only the concurrency guard; anything else ends the chain.
  [ "$status" -ne 1 ] && break
  echo "=== sweep refused (a run is still active), retrying in 300s $(date '+%F %H:%M:%S')"
  sleep 300
done
echo "=== FABLE 5.2 CHAIN EXITED $status $(date '+%F %H:%M:%S')"
exit "$status"
