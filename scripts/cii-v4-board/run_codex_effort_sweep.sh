#!/usr/bin/env bash
# Effort sweep on the Coding Intelligence Index v4 through the Codex CLI on a
# ChatGPT subscription, one model at a time, one attempt per task per level.
#
# Each level writes to <OUTROOT>/<level>/ so levels never mix. Every level is
# retried with --only-missing until all 23 tasks have a summary, waiting out
# subscription "limit reached" windows instead of burning retries.
#
#   MODEL=gpt-5.5 OUTROOT=runs-effort-gpt55 bash scripts/cii-v4-board/run_codex_effort_sweep.sh
#   MODEL=gpt-5.6-luna OUTROOT=runs-effort-luna bash scripts/cii-v4-board/run_codex_effort_sweep.sh
set -u
cd "$(dirname "$0")/../.."
# Detached runs start with the bare macOS PATH; include the authenticated Codex CLI.
export PATH="/Users/morganlinton/.nvm/versions/node/v22.11.0/bin:$PWD/.venv/bin:$PATH"

MODEL=${MODEL:?set MODEL, e.g. gpt-5.5 or gpt-5.6-luna}
OUTROOT=${OUTROOT:?set OUTROOT, e.g. runs-effort-gpt55}
LEVELS=${LEVELS:-"low medium high extra-high max"}
SUITE="coding-intelligence-index-v4"
QUOTA_WAIT=${QUOTA_WAIT:-1800}
RETRY_WAIT=${RETRY_WAIT:-120}
MAX_ATTEMPTS=${MAX_ATTEMPTS:-40}

# vulcanbench.toml [effort].blocked: levels that never run. The harness refuses
# them too; checking here keeps a mistyped LEVELS from even starting a loop.
for level in $LEVELS; do
  if python3 -c 'import sys, tomllib; blocked = tomllib.load(open("vulcanbench.toml", "rb")).get("effort", {}).get("blocked", []); sys.exit(0 if sys.argv[1] in blocked else 1)' "$level"; then
    echo "refusing to start: effort '$level' is blocked by vulcanbench.toml" >&2
    exit 5
  fi
done

if pgrep -f "vulcanbench run --suite $SUITE" >/dev/null 2>&1; then
  echo "refusing to start: a vulcanbench run is already active (pgrep -fl 'vulcanbench run')" >&2
  exit 1
fi

count_done() {
  # Distinct tasks with a summary, not summary files: a resumed sweep can race
  # a completing run into a duplicate.
  find "$1" -name summary.json 2>/dev/null -exec \
    python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["task_id"])' {} \; \
    2>/dev/null | sort -u | wc -l | tr -d ' '
}

for level in $LEVELS; do
  outdir="$OUTROOT/$level"
  mkdir -p "$outdir"
  for attempt in $(seq 1 "$MAX_ATTEMPTS"); do
    done_count=$(count_done "$outdir")
    if [ "$done_count" -ge 23 ]; then
      echo "=== $MODEL effort=$level COMPLETE ($done_count/23) $(date '+%F %H:%M:%S')"
      break
    fi
    echo "=== $MODEL effort=$level attempt $attempt, $done_count/23 done $(date '+%F %H:%M:%S')"
    stamp=$(mktemp)
    out=$(vulcanbench run --suite "$SUITE" --model "$MODEL" --harness codex --billing subscription \
      --sandbox local --no-judges --effort "$level" --only-missing -o "$outdir" 2>&1)
    status=$?
    echo "$out" | tail -3
    done_count=$(count_done "$outdir")
    if [ "$done_count" -ge 23 ]; then
      rm -f "$stamp"
      continue
    fi
    # The suite runner only reports "N run(s) errored"; the API's own refusal
    # (quota window, expired login) is in each run's stream. Read the streams
    # of this attempt's runs so the case below can see them, and drop attempts
    # the API refused before any work (a trace and a stream, nothing else).
    streams=""
    pruned=0
    for d in $(find "$outdir" -mindepth 1 -maxdepth 1 -type d -name 'legacy-*' -newer "$stamp" 2>/dev/null); do
      [ -f "$d/cli-agent-stream.jsonl" ] || continue
      [ -e "$d/summary.json" ] && continue
      streams="$streams
$(tail -c 800 "$d/cli-agent-stream.jsonl")"
      if [ ! -e "$d/final.patch" ] \
         && grep -q -e "usage limit" -e "limit reached" -e "refresh token has expired" "$d/cli-agent-stream.jsonl" \
         && [ "$(ls "$d" | grep -v -e '^trace.jsonl$' -e '^cli-agent-stream.jsonl$' | wc -l | tr -d ' ')" -eq 0 ]; then
        rm -rf "$d"
        pruned=$((pruned + 1))
      fi
    done
    rm -f "$stamp"
    [ "$pruned" -gt 0 ] && echo "=== $MODEL effort=$level dropped $pruned attempt(s) the API refused before any work"
    case "$out$streams" in
      *"refresh token has expired"*|*"sign in again"*|*"Please log out"*|*"not logged in"*)
        echo "=== $MODEL effort=$level AUTH FAILURE at $done_count/23: the Codex subscription login has expired. Run 'codex login', then rerun this script. $(date '+%F %H:%M:%S')"
        exit 3 ;;
      *"limit reached"*|*"usage limit"*|*"quota"*|*"Quota"*|*"rate limit"*)
        echo "=== $MODEL effort=$level quota exhausted at $done_count/23 (exit=$status), waiting ${QUOTA_WAIT}s $(date '+%H:%M:%S')"
        sleep "$QUOTA_WAIT" ;;
      *)
        echo "=== $MODEL effort=$level stopped at $done_count/23 (exit=$status), retrying in ${RETRY_WAIT}s $(date '+%H:%M:%S')"
        sleep "$RETRY_WAIT" ;;
    esac
  done
done
echo "=== SWEEP FINISHED $MODEL $(date '+%F %H:%M:%S')"
for level in $LEVELS; do echo "$level: $(count_done "$OUTROOT/$level")/23"; done
