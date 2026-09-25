#!/usr/bin/env bash
# Effort sweep on VulcanBench Frontier v4 (tasks/coding-intelligence-index-v4)
# through the Devin CLI on the signed-in Devin account, one model family at a
# time, one attempt per task per level.
#
# Devin selects effort through the model id (swe-2-medium / swe-2-high /
# swe-2-max), so the levels default to the three SWE-2 variants the account
# catalog lists; the adapter refuses any level the catalog lacks before a
# model call. Each level writes to <OUTROOT>/<level>/ so levels never mix.
# Every level is retried with --only-missing until all 23 tasks have a
# summary, waiting out plan-limit windows instead of burning retries.
#
#   MODEL=swe-2 OUTROOT=runs-effort-devin-swe2 bash scripts/cii-v4-board/run_devin_effort_sweep.sh
#
# Exit codes: 1 a run is already active (concurrency guard), 3 the Devin
# login expired (run `devin auth login`), 5 a level is blocked by
# vulcanbench.toml, 6 the Devin preflight failed (CLI too old or signed out).
set -u
cd "$(dirname "$0")/../.."
# Detached runs start with the bare macOS PATH. The Homebrew cask (3000.10.x)
# must win over the dead ~/.local/bin symlink to the May 2026 build.
export PATH="/opt/homebrew/bin:$PWD/.venv/bin:$PATH"

MODEL=${MODEL:-swe-2}
OUTROOT=${OUTROOT:?set OUTROOT, e.g. runs-effort-devin-swe2}
LEVELS=${LEVELS:-"medium high max"}
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

if ! vulcanbench harness doctor devin >/dev/null 2>&1; then
  vulcanbench harness doctor devin
  echo "refusing to start: the Devin preflight failed (see the table above)" >&2
  exit 6
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
    out=$(vulcanbench run --suite "$SUITE" --model "$MODEL" --harness devin --billing subscription \
      --sandbox local --no-judges --effort "$level" --only-missing -o "$outdir" 2>&1)
    status=$?
    echo "$out" | tail -3
    done_count=$(count_done "$outdir")
    if [ "$done_count" -ge 23 ]; then
      rm -f "$stamp"
      continue
    fi
    # The suite runner only reports "N run(s) errored"; the CLI's own refusal
    # (plan limit, expired login) is on the run's stream. Read the streams of
    # this attempt's runs so the case below can see them, and drop attempts
    # the CLI refused before any work (a trace and a stream, nothing else).
    streams=""
    pruned=0
    for d in $(find "$outdir" -mindepth 1 -maxdepth 1 -type d -name 'legacy-*' -newer "$stamp" 2>/dev/null); do
      [ -f "$d/cli-agent-stream.jsonl" ] || continue
      [ -e "$d/summary.json" ] && continue
      streams="$streams
$(tail -c 800 "$d/cli-agent-stream.jsonl")"
      if [ ! -e "$d/final.patch" ] \
         && grep -q -i -e "usage limit" -e "limit reached" -e "not authorized" -e "auth login" "$d/cli-agent-stream.jsonl" \
         && [ "$(ls "$d" | grep -v -e '^trace.jsonl$' -e '^cli-agent-stream.jsonl$' -e '^devin-session$' | wc -l | tr -d ' ')" -eq 0 ]; then
        rm -rf "$d"
        pruned=$((pruned + 1))
      fi
    done
    rm -f "$stamp"
    [ "$pruned" -gt 0 ] && echo "=== $MODEL effort=$level dropped $pruned attempt(s) the CLI refused before any work"
    case "$out$streams" in
      *"not authorized"*|*"Not authorized"*|*"auth login"*|*"not logged in"*)
        echo "=== $MODEL effort=$level AUTH FAILURE at $done_count/23: the Devin login has expired. Run 'devin auth login', then rerun this script. $(date '+%F %H:%M:%S')"
        exit 3 ;;
      *"plan limit"*|*"limit reached"*|*"usage limit"*|*"Usage limit"*|*"quota"*|*"Quota"*|*"rate limit"*)
        echo "=== $MODEL effort=$level plan limit at $done_count/23 (exit=$status), waiting ${QUOTA_WAIT}s $(date '+%H:%M:%S')"
        sleep "$QUOTA_WAIT" ;;
      *)
        echo "=== $MODEL effort=$level stopped at $done_count/23 (exit=$status), retrying in ${RETRY_WAIT}s $(date '+%H:%M:%S')"
        sleep "$RETRY_WAIT" ;;
    esac
  done
done
echo "=== SWEEP FINISHED $MODEL $(date '+%F %H:%M:%S')"
for level in $LEVELS; do echo "$level: $(count_done "$OUTROOT/$level")/23"; done
