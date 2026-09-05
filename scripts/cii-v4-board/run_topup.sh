#!/usr/bin/env bash
# Repeat top-up for the Opus 5 effort ladder on the Coding Intelligence Index v4.
#
# The ladder was measured at one attempt per task per level, and eleven tasks
# changed verdict across levels, several non-monotonically. At n=1 that is the
# signature of run-to-run variance rather than an effort response, so two claims
# need hardening before publication:
#
#   Phase 1  max -> n=3 across all 23 tasks. The 95.7% headline rests on single
#            attempts and is the number people will quote.
#   Phase 2  the swing set (tasks whose verdict differs across low/medium/high/
#            max) -> n=3 at low, medium and high.
#
# Deficits are recomputed on every attempt, so the script is resumable and safe
# to rerun: a completed top-up simply finds nothing to do.
#
#   bash scripts/cii-v4-board/run_topup.sh
set -u
cd "$(dirname "$0")/../.."
export PATH="$PWD/.venv/bin:$PATH"

MODEL=${MODEL:-claude-code:claude-opus-5}
PROBE=${PROBE:-claude-opus-5}
OUTROOT=${OUTROOT:-runs-effort-opus5}
SUITE="coding-intelligence-index-v4"
TASKS_ROOT="tasks/$SUITE"
REPEAT=${REPEAT:-3}
WAIT=${WAIT:-1800}

if pgrep -f "vulcanbench run --suite $SUITE" >/dev/null 2>&1 \
   || pgrep -f "vulcanbench run --task" >/dev/null 2>&1; then
  echo "refusing to start: a vulcanbench run is already active" >&2
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

# Runs recorded for one task at one level.
count_runs() {
  find "$OUTROOT/$2" -name summary.json 2>/dev/null -exec \
    python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["task_id"])' {} \; \
    2>/dev/null | grep -c "^$1$" | tr -d ' '
}

# ---------- phase 1: max to n=REPEAT (suite-level --only-missing tops up) ----
for attempt in $(seq 1 60); do
  need=$(python3 - "$OUTROOT" "$REPEAT" <<'PY'
import json, glob, sys
from collections import Counter
root, repeat = sys.argv[1], int(sys.argv[2])
suite = json.load(open('tasks/coding-intelligence-index-v4/suite.json'))['tasks']
c = Counter(json.load(open(p))['task_id'] for p in glob.glob(f'{root}/max/*/summary.json'))
print(sum(max(0, repeat - c.get(t, 0)) for t in suite))
PY
)
  if [ "$need" -eq 0 ]; then
    echo "=== phase 1 COMPLETE: max at n=$REPEAT $(date '+%F %H:%M:%S')"
    break
  fi
  if ! quota_ready; then
    echo "=== phase 1 quota exhausted, $need runs short, waiting ${WAIT}s $(date '+%H:%M:%S')"
    sleep "$WAIT"; continue
  fi
  echo "=== phase 1 attempt $attempt, $need runs short $(date '+%F %H:%M:%S')"
  vulcanbench run --suite "$SUITE" --model "$MODEL" --sandbox local --no-judges \
    --effort max --repeat "$REPEAT" --only-missing -o "$OUTROOT/max" 2>&1 | tail -3
done

# ---------- phase 2: swing set to n=REPEAT at low / medium / high ------------
# Recomputed rather than hardcoded: the set is defined by the data.
mapfile -t SWING < <(python3 - "$OUTROOT" <<'PY'
import json, glob, sys
from collections import defaultdict
root = sys.argv[1]
lv = {}
for level in ('low','medium','high','max'):
    by = defaultdict(list)
    for p in glob.glob(f'{root}/{level}/*/summary.json'):
        s = json.load(open(p))
        by[s['task_id']].append(s['scores']['functional'])
    lv[level] = by
for t in sorted(lv['low']):
    v = {l: (1 if any(f == 1.0 for f in lv[l].get(t, [])) else 0) for l in lv}
    if len(set(v.values())) > 1:
        print(t)
PY
)
echo "=== phase 2 swing set: ${#SWING[@]} tasks"

for level in low medium high; do
  for task in "${SWING[@]}"; do
    for attempt in $(seq 1 40); do
      have=$(count_runs "$task" "$level")
      need=$((REPEAT - have))
      [ "$need" -le 0 ] && break
      if ! quota_ready; then
        echo "=== phase 2 quota exhausted ($level/$task at $have), waiting ${WAIT}s $(date '+%H:%M:%S')"
        sleep "$WAIT"; continue
      fi
      echo "=== phase 2 $level $task: $have/$REPEAT, running $need $(date '+%F %H:%M:%S')"
      vulcanbench run --task "$task" --tasks-root "$TASKS_ROOT" --model "$MODEL" \
        --sandbox local --no-judges --effort "$level" --repeat "$need" \
        -o "$OUTROOT/$level" 2>&1 | tail -2
    done
  done
done
echo "=== TOP-UP FINISHED $(date '+%F %H:%M:%S')"
