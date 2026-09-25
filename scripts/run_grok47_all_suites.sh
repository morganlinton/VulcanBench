#!/usr/bin/env bash
# Grok 4.7 across all three suites, on two harnesses (Cursor, Grok Build),
# so the harnesses can be compared on identical tasks. Owner request in chat,
# 2026-09-22.
#
# Serial, queued behind the Opus 5.5 chain (docs/DECISIONS.md 2026-09-13:
# speed cards rank by wall clock). The script waits until no Opus chain and
# no vulcanbench run is active, then runs every leg one after another.
#
# Levels: low, medium, high, extra-high. Neither harness exposes a max level
# for Grok 4.7, and ultra is blocked in vulcanbench.toml.
#
# Cursor: effort lives in the model id (grok-4.7-<level>, non-fast). The
# adapter's bracket form (grok-4.7-low[effort=low], grok-4.7[effort=high]) is
# rejected by Cursor for 4.7 (checked 2026-09-22), so Cursor legs pass no
# --effort; the level is carried by the id and the output directory.
# Grok Build: model grok-4.7 with --effort, mapped to --reasoning-effort
# (extra-high -> xhigh) by harness/effort.py.
#
# Every leg runs --billing subscription.
#
#   nohup bash scripts/run_grok47_all_suites.sh > logs/grok47-all-suites.log 2>&1 &
set -u
VB=/Users/morganlinton/dev/VulcanBench
CONDUCT=/Users/morganlinton/dev/VulcanConduct
ROUTINE=/Users/morganlinton/dev/VulcanRoutine
cd "$VB" || exit 1
export PATH="/Users/morganlinton/.nvm/versions/node/v22.11.0/bin:$VB/.venv/bin:$PATH"

LEVELS="low medium high extra-high"
QUOTA_WAIT=1800
RETRY_WAIT=120
MAX_ATTEMPTS=40

banner() { echo; echo "##### $1 $(date '+%F %H:%M:%S')"; echo; }

count_done() {
  find "$1" -name summary.json 2>/dev/null -exec \
    python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["task_id"])' {} \; \
    2>/dev/null | sort -u | wc -l | tr -d ' '
}

cursor_id() {
  case "$1" in
    extra-high) echo "grok-4.7-xhigh" ;;
    *) echo "grok-4.7-$1" ;;
  esac
}

# run_leg <harness> <suite> <expected> <outroot>
run_leg() {
  local harness=$1 suite=$2 expected=$3 outroot=$4
  for level in $LEVELS; do
    local outdir="$outroot/$level"
    mkdir -p "$outdir"
    local model effort_args
    if [ "$harness" = cursor ]; then
      model=$(cursor_id "$level"); effort_args=""
    else
      model=grok-4.7; effort_args="--effort $level"
    fi
    for attempt in $(seq 1 "$MAX_ATTEMPTS"); do
      local n; n=$(count_done "$outdir")
      if [ "$n" -ge "$expected" ]; then
        echo "=== $harness/$suite $level COMPLETE ($n/$expected) $(date '+%F %H:%M:%S')"
        break
      fi
      echo "=== $harness/$suite $level attempt $attempt, $n/$expected done $(date '+%F %H:%M:%S')"
      out=$(vulcanbench run --suite "$suite" --harness "$harness" --model "$model" \
        --billing subscription --sandbox local --no-judges $effort_args \
        --only-missing -o "$outdir" 2>&1)
      status=$?
      echo "$out" | tail -3
      [ "$(count_done "$outdir")" -ge "$expected" ] && continue
      case "$out" in
        *"quota"*|*"rate limit"*|*"usage limit"*|*"limit reached"*|*"SubscriptionQuota"*)
          echo "quota pause ${QUOTA_WAIT}s"; sleep "$QUOTA_WAIT" ;;
        *) echo "retry in ${RETRY_WAIT}s (exit $status)"; sleep "$RETRY_WAIT" ;;
      esac
    done
  done
}

banner "WAITING for the Opus 5.5 chain and any active vulcanbench run"
while pgrep -f run_opus55_all_suites.sh >/dev/null || pgrep -f "vulcanbench run --suite" >/dev/null; do
  sleep 600
done

for harness in cursor grok-build; do
  tag=${harness/grok-build/grokbuild}
  banner "$harness: Frontier v4 (23 x 4)"
  run_leg "$harness" coding-intelligence-index-v4 23 "runs-effort-grok47-$tag"
  banner "$harness: Routine v1 (12 x 4)"
  run_leg "$harness" routine-v1 12 "$ROUTINE/runs/runs-routine-v1-grok47-$tag"
  banner "$harness: Safety v1 / conduct-v1 (10 x 4)"
  run_leg "$harness" conduct-v1 10 "$CONDUCT/runs/runs-conduct-v1-grok47-$tag"
done

banner "GROK 4.7 CHAIN FINISHED"
