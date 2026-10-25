#!/usr/bin/env bash
# Frontier v5 admission gate runner (docs/frontier-v5/GATE-LOG.md).
#
# usage: scripts/frontier-v5/run_gate.sh <astra|fable|opus> <task> [<task> ...]
#
# Runs one reference model on each named task (tasks/frontier-v5/<task>)
# through the real `harbor run`, three attempts per task, per the protocol in
# docs/DECISIONS.md 2026-10-06, decision (b), with the wave-1 identifiers
# fixed in GATE-LOG.md:
#
#   astra  harbor -a codex       -m gpt-6-astra       Codex CLI 0.159.0,   medium
#   fable  harbor -a claude-code -m claude-fable-5-1  Claude Code 2.1.302, medium
#   opus   harbor -a claude-code -m claude-opus-5-5   Claude Code 2.1.302, medium
#          (second reference for wave 1 from 2026-10-25, docs/DECISIONS.md)
#
# Concurrency follows decision (e): Python and JavaScript tasks run two
# attempts at a time, compiled and JVM tasks one (16 GiB hosts).
# Early exit: two solves by the reference end a task's gate as a reject, so
# the Harbor job is stopped as soon as two trials have reward 1. Killing
# `harbor run` does not stop the running trial's sandbox containers, which
# keep the agent (and its quota use) alive until the task timeout (seen
# 2026-10-22, comrak), so stop_sandboxes removes them after every stop.
# Infrastructure guard: a finished trial whose agent log shows an auth,
# model-id, usage-credit or usage-limit failure stops the whole run; such
# trials are not gate results. Move their job directory aside (INFRA-*).
#
# Authentication (never read or printed by this script):
#   astra  the ChatGPT login in ~/.codex/auth.json (`codex login`); Harbor
#          injects it because CODEX_FORCE_AUTH_JSON=1 is set in its own
#          environment.
#   fable, opus  either a Claude subscription token in a private env file
#          (CLAUDE_GATE_ENV, default ~/.config/vulcanbench/claude-gate.env,
#          one line CLAUDE_CODE_OAUTH_TOKEN=..., from `claude setup-token`),
#          used with CLAUDE_FORCE_OAUTH=1; or, with FABLE_AUTH=api, an
#          ANTHROPIC_API_KEY line in that same file. A subscription needs
#          usage credits for Fable 5.1.
#          Before a claude-code run the script makes one one-turn call with the
#          host `claude` CLI to read the subscription's usage windows and
#          refuses to start above GATE_USAGE_MAX (default 0.85) unless
#          GATE_USAGE_OVERRIDE=1 (the 2026-10-24 black Fable run had to be
#          stopped by hand at 94% of the weekly window).
#
# Never pass flag values with --ae: Harbor treats --ae values as secrets and
# scrubs them from every file it writes (a value of 1 replaced every "1" in
# rewards, logs and artifacts on 2026-10-18).
#
# n=5 top-up (decision (b), a prong at exactly 1 of 3): GATE_ATTEMPTS=2
# GATE_JOB_SUFFIX=-topup runs two more attempts into a fresh job directory
# next to the first one; the early exit still fires at two solves (3 of 5).
#
# Output: $GATE_OUT (default ~/vulcanbench-gate, outside the repo)/jobs/<ref>-<task><suffix>,
# plus one summary line per trial on stdout. Record results in GATE-LOG.md
# after diff-reviewing every solve.
set -u
ref="${1:?usage: run_gate.sh <astra|fable> <task> [<task> ...]}"; shift
[ $# -gt 0 ] || { echo "run_gate: name at least one task" >&2; exit 2; }
here="$(cd "$(dirname "$0")/../.." && pwd)"
export PATH="$HOME/.local/bin:$HOME/.local/node/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
# Outside the repo by default: logs/ is not git-ignored here, and gate
# artifacts must never be committed.
OUT="${GATE_OUT:-$HOME/vulcanbench-gate}"
attempts="${GATE_ATTEMPTS:-3}"; suffix="${GATE_JOB_SUFFIX:-}"
case "$attempts" in 1|2|3) ;; *) echo "run_gate: GATE_ATTEMPTS must be 1, 2 or 3 (got $attempts)" >&2; exit 2 ;; esac
mkdir -p "$OUT/jobs"
command -v harbor >/dev/null || { echo "run_gate: harbor not found (scripts/frontier-v5/setup-host.sh installs it)" >&2; exit 2; }

case "$ref" in
  astra)
    agent=codex; model=gpt-6-astra; version=0.159.0
    [ -f "$HOME/.codex/auth.json" ] || { echo "run_gate: ~/.codex/auth.json missing; run \`codex login\`" >&2; exit 2; }
    auth_env=(CODEX_FORCE_AUTH_JSON=1); auth_args=() ;;
  fable|opus)
    agent=claude-code; version=2.1.302
    case "$ref" in fable) model=claude-fable-5-1 ;; opus) model=claude-opus-5-5 ;; esac
    envf="${CLAUDE_GATE_ENV:-$HOME/.config/vulcanbench/claude-gate.env}"
    [ -f "$envf" ] || { echo "run_gate: $envf missing (see the header for how to create it)" >&2; exit 2; }
    auth_args=(--env-file "$envf")
    if [ "${FABLE_AUTH:-oauth}" = api ]; then auth_env=(); else auth_env=(CLAUDE_FORCE_OAUTH=1); fi ;;
  *) echo "run_gate: reference must be astra, fable or opus" >&2; exit 2 ;;
esac

# Exact error texts seen in agent logs (2026-10-17/18) plus Codex's usage-limit
# message; kept narrow so ordinary output (code the agent reads) never matches.
INFRA_RE='"error":"authentication_failed"|OAuth access token is invalid|issue with the selected model|requires usage credits|hit your usage limit'

usage_preflight() { # claude-code refs: one one-turn call reads the subscription windows; refuse above GATE_USAGE_MAX
  local max="${GATE_USAGE_MAX:-0.85}" out rl_status rl_util
  command -v claude >/dev/null || { echo "run_gate: no claude CLI on PATH, skipping the usage preflight" >&2; return 0; }
  out=$( ( set -a; . "$envf"; set +a; claude -p --model "$model" --output-format stream-json --verbose --max-turns 1 'Reply with the single word ok.' 2>/dev/null ) \
    | grep '"rate_limit_event"' | tail -1 | python3 -c '
import json, sys
line = sys.stdin.read().strip()
if not line: sys.exit(0)
d = json.loads(line)["rate_limit_info"]; w = d.get("unifiedWindows") or {}
m = max([v.get("utilization") or 0 for v in w.values()] or [0])
print("%s %.2f %s" % (d.get("status"), m, " ".join("%s=%.2f" % (k, v.get("utilization") or 0) for k, v in w.items())))' )
  [ -n "$out" ] || { echo "run_gate: usage preflight saw no rate-limit event, continuing" >&2; return 0; }
  echo "=== usage preflight $model: $out"
  rl_status=${out%% *}; rl_util=$(printf '%s' "$out" | awk '{print $2}')
  if [ "${GATE_USAGE_OVERRIDE:-0}" != 1 ] && { [ "$rl_status" != allowed ] || awk -v u="$rl_util" -v m="$max" 'BEGIN{exit !(u > m)}'; }; then
    echo "!!! usage window at $rl_util (status $rl_status) is above GATE_USAGE_MAX=$max; not starting. Set GATE_USAGE_OVERRIDE=1 to run anyway." >&2
    exit 2
  fi
}

solves() { # job dir -> number of reward-1 trials
  python3 - "$1" <<'PY'
import glob, json, sys
n = 0
for p in glob.glob(sys.argv[1] + "/*__*/verifier/reward.json"):
    try:
        n += json.load(open(p)).get("reward") == 1
    except Exception:
        pass
print(n)
PY
}

stop_sandboxes() { # task name -> remove this task's leftover sandbox containers and networks
  # Harbor names a trial's compose project <first 32 chars of the task name>__<trial id>__env.
  local prefix ids n
  prefix=$(printf '%s' "${1:0:32}" | tr 'A-Z' 'a-z')
  ids=$(docker ps -aq --filter "name=^${prefix}__" 2>/dev/null)
  [ -n "$ids" ] && { echo "--- $1: removing leftover sandbox containers"; docker rm -f $ids >/dev/null 2>&1; }
  for n in $(docker network ls -q --filter "name=^${prefix}__" 2>/dev/null); do docker network rm "$n" >/dev/null 2>&1; done
  return 0
}

infra_hit() { # job dir -> prints the first trial with an infrastructure failure
  local tr
  for tr in "$1"/*__*; do
    [ -d "$tr/agent" ] || continue
    if grep -rqE "$INFRA_RE" "$tr/agent" 2>/dev/null; then basename "$tr"; return; fi
  done
}

for t in "$@"; do
  task="$here/tasks/frontier-v5/$t"
  [ -f "$task/task.toml" ] || { echo "run_gate: no task $t" >&2; exit 2; }
  lang=$(sed -n 's/^vulcanbench_language = "\(.*\)"/\1/p' "$task/task.toml")
  case "$lang" in python|javascript) n=2 ;; *) n=1 ;; esac
  job="$ref-$t$suffix"
  [ -e "$OUT/jobs/$job" ] && { echo "run_gate: $OUT/jobs/$job exists; move it aside first" >&2; exit 2; }
  [ "$agent" = claude-code ] && usage_preflight
  echo "=== $t $(date '+%F %H:%M:%S') ref=$ref agent=$agent model=$model version=$version effort=medium attempts=$attempts concurrency=$n harbor=$(harbor --version 2>/dev/null) repo=$(git -C "$here" rev-parse --short HEAD)"
  (cd "$here" && env "${auth_env[@]+"${auth_env[@]}"}" harbor run -p "$task" -a "$agent" -m "$model" --effort medium \
     --ak version="$version" -k "$attempts" -n "$n" "${auth_args[@]+"${auth_args[@]}"}" \
     -o "$OUT/jobs" --job-name "$job" -y -q) > "$OUT/$job.log" 2>&1 &
  hpid=$!
  stopped=""
  while kill -0 "$hpid" 2>/dev/null; do
    sleep 60
    bad=$(infra_hit "$OUT/jobs/$job")
    if [ -n "$bad" ]; then
      echo "!!! $t: infrastructure failure in $bad (matches: $INFRA_RE); stopping the run"
      kill -INT "$hpid" 2>/dev/null; sleep 20; kill "$hpid" 2>/dev/null; wait "$hpid" 2>/dev/null
      stop_sandboxes "$t"
      echo "!!! move $OUT/jobs/$job aside as INFRA-..., fix the cause, and rerun"; exit 1
    fi
    if [ "$(solves "$OUT/jobs/$job")" -ge 2 ]; then
      echo "--- $t: two solves, early reject; stopping the remaining attempt"
      kill -INT "$hpid" 2>/dev/null; sleep 20; kill "$hpid" 2>/dev/null; stopped=1
    fi
  done
  wait "$hpid" 2>/dev/null
  stop_sandboxes "$t"
  if [ -z "$stopped" ]; then
    bad=$(infra_hit "$OUT/jobs/$job")
    [ -n "$bad" ] && { echo "!!! $t: infrastructure failure in $bad; not a gate result"; exit 1; }
  fi
  for tr in "$OUT/jobs/$job"/*__*; do
    [ -d "$tr" ] || continue
    echo "  $(basename "$tr"): $(cat "$tr/verifier/reward.json" 2>/dev/null || echo 'stopped, no reward')"
  done
  echo "--- $t: $(solves "$OUT/jobs/$job") solve(s) $(date '+%F %H:%M:%S')"
done
echo "=== DONE $(date '+%F %H:%M:%S')"
