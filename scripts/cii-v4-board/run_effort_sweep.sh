#!/usr/bin/env bash
# Effort sweep on the Coding Intelligence Index v4, one model at a time.
#
# The shipped default level is measured separately (the board pool, or for
# Opus 5 the frontier-gate pool). This fills the remaining four levels, 23
# tasks each, into <OUTROOT>/<level>/ so they never mix with default-effort
# results.
#
# Defaults to Fable 5.1; override for another model, e.g.
#   MODEL=claude-code:claude-opus-5 OUTROOT=runs-effort-opus5 \
#     PROBE=claude-opus-5 bash scripts/cii-v4-board/run_effort_sweep.sh
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
MODEL=${MODEL:-claude-code:claude-fable-5-1}
OUTROOT=${OUTROOT:-runs-effort}
PROBE=${PROBE:-claude-fable-5-1}
SUITE="coding-intelligence-index-v4"
WAIT=${WAIT:-1800}

if pgrep -f "vulcanbench run --suite $SUITE" >/dev/null 2>&1; then
  echo "refusing to start: a vulcanbench run is already active (pgrep -fl 'vulcanbench run')" >&2
  exit 1
fi

quota_ready() {
  local out
  out=$(echo "reply with the single word ok" \
    | claude -p --model "$PROBE" --max-turns 1 2>&1 | tail -1)
  case "$out" in
    *"limit"*|*"Limit"*) return 1 ;;
    *) return 0 ;;
  esac
}

for level in $LEVELS; do
  outdir="$OUTROOT/$level"
  mkdir -p "$outdir"
  for attempt in $(seq 1 40); do
    # Count DISTINCT tasks, not summary files: a killed-and-resumed sweep can
    # race a completing run into a duplicate, and counting files would then
    # declare the level done while a task was still missing.
    done_count=$(find "$outdir" -name summary.json 2>/dev/null -exec \
      python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["task_id"])' {} \; \
      2>/dev/null | sort -u | wc -l | tr -d ' ')
    if [ "$done_count" -ge 23 ]; then
      echo "=== $MODEL effort=$level COMPLETE ($done_count/23) $(date '+%F %H:%M:%S')"
      break
    fi
    if ! quota_ready; then
      echo "=== $MODEL effort=$level quota exhausted at $done_count/23, waiting ${WAIT}s $(date '+%H:%M:%S')"
      sleep "$WAIT"
      continue
    fi
    echo "=== $MODEL effort=$level attempt $attempt, $done_count/23 done $(date '+%F %H:%M:%S')"
    vulcanbench run --suite "$SUITE" --model "$MODEL" --sandbox local --no-judges \
      --effort "$level" --only-missing -o "$outdir" 2>&1 | tail -3
  done
done
echo "=== SWEEP FINISHED $MODEL $(date '+%F %H:%M:%S')"
find "$OUTROOT" -name summary.json | wc -l
