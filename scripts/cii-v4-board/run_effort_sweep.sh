#!/usr/bin/env bash
# Claude Fable 5.1 effort sweep on the Coding Intelligence Index v4.
#
# The shipped default level is already measured (runs-board/). This fills the
# remaining four levels, 23 tasks each, into runs-effort/<level>/ so they never
# mix with the default-effort board pool.
#
# Subscription quota is the binding constraint: a Max window carries only a
# fraction of a level, so each level is retried with --only-missing until it
# completes, waiting out "limit reached" windows rather than burning retries.
#
#   bash scripts/cii-v4-board/run_effort_sweep.sh
set -u
cd "$(dirname "$0")/../.."
export PATH="$PWD/.venv/bin:$PATH"

LEVELS=${LEVELS:-"low medium extra-high max"}
MODEL="claude-code:claude-fable-5-1"
SUITE="coding-intelligence-index-v4"
WAIT=${WAIT:-1800}

quota_ready() {
  local out
  out=$(echo "reply with the single word ok" \
    | claude -p --model claude-fable-5-1 --max-turns 1 2>&1 | tail -1)
  case "$out" in
    *"limit"*|*"Limit"*) return 1 ;;
    *) return 0 ;;
  esac
}

for level in $LEVELS; do
  outdir="runs-effort/$level"
  mkdir -p "$outdir"
  for attempt in $(seq 1 40); do
    done_count=$(find "$outdir" -name summary.json 2>/dev/null | wc -l | tr -d ' ')
    if [ "$done_count" -ge 23 ]; then
      echo "=== effort=$level COMPLETE ($done_count/23) $(date '+%F %H:%M:%S')"
      break
    fi
    if ! quota_ready; then
      echo "=== effort=$level quota exhausted at $done_count/23, waiting ${WAIT}s $(date '+%H:%M:%S')"
      sleep "$WAIT"
      continue
    fi
    echo "=== effort=$level attempt $attempt, $done_count/23 done $(date '+%F %H:%M:%S')"
    vulcanbench run --suite "$SUITE" --model "$MODEL" --sandbox local --no-judges \
      --effort "$level" --only-missing -o "$outdir" 2>&1 | tail -3
  done
done
echo "=== SWEEP FINISHED $(date '+%F %H:%M:%S')"
find runs-effort -name summary.json | wc -l
