#!/usr/bin/env bash
# Frontier v5 admission gate runner (docs/frontier-v5/GATE-LOG.md).
#
# usage: scripts/frontier-v5/run_gate.sh <astra|fable|opus> <task> [<task> ...]
#        scripts/frontier-v5/run_gate.sh --refs
#        GATE_PREFLIGHT_ONLY=1 scripts/frontier-v5/run_gate.sh <ref> [<task>]
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
# GATE_CONCURRENCY=1 overrides that for a task whose verifiers cannot share
# the Docker VM (prettier, docs/DECISIONS.md 2026-10-27); the value is
# printed on the === line.
# Early exit: two solves by the reference end a task's gate as a reject, so
# the Harbor job is stopped as soon as two trials have reward 1. Killing
# `harbor run` does not stop the running trial's sandbox containers, which
# keep the agent (and its quota use) alive until the task timeout (seen
# 2026-10-22, comrak), so stop_sandboxes removes them after every stop.
# Web tools are off for both references (`--ak disable_web_search=true`:
# Codex `web_search=disabled`, Claude Code `--disallowedTools WebSearch,WebFetch`),
# and Codex runs with scripts/frontier-v5/codex-gate.toml, which also turns
# off the account's connected apps, plugins and browser tools.
# They run on the provider's side, so the task's network allowlist never sees
# them; on 2026-10-30 an Astra attempt downloaded the upstream source of the
# code an F2 task had removed through Codex's `web__run` (docs/DECISIONS.md).
# Infrastructure guard: a finished trial whose agent log shows an auth,
# model-id, usage-credit or usage-limit failure stops the whole run; such
# trials are not gate results. Move their job directory aside (INFRA-*).
#
# Stopping: SIGINT or SIGTERM to this script stops the run cleanly. Harbor
# is started as a direct child with SIGINT at its default disposition (a
# background list in bash 3.2 ignores SIGINT, which is why the early exit
# used to leave harbor running, docs/DECISIONS.md 2026-10-29), so one SIGINT
# makes harbor cancel its trial and run `docker compose down`; the script
# waits up to 90 s, escalates to SIGTERM then SIGKILL, and sweeps the task's
# sandbox containers either way. Exit codes: 0 done, 1 infrastructure
# failure, 2 refused before harbor started, 130 stopped on a signal.
#
# Status file: $GATE_OUT/<job>.status.json is rewritten atomically at every
# phase change (preflight, running, stopping, done, infra_failed, stopped)
# with the pids, solves, harbor's exit code and the stop reason, so a
# watcher (the local run GUI, backend/runner) never has to infer state from
# harbor's result.json, which has no finished_at on a cancelled job.
#
# Authentication (never read or printed by this script):
#   astra  a dedicated ChatGPT login for gate runs, with no connected apps,
#          in ~/.config/vulcanbench/codex-gate/auth.json
#          (`CODEX_HOME=~/.config/vulcanbench/codex-gate codex login`), or the
#          file GATE_CODEX_AUTH names; without one, the personal login in
#          ~/.codex/auth.json. Harbor injects the file CODEX_AUTH_JSON_PATH
#          names, set in its own environment.
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
#          GATE_PREFLIGHT_ONLY=1 runs just that check and prints one JSON
#          line (model, status, max_utilization, windows, would_start);
#          exit 0 if a run would start, 2 if it would be refused.
#
# GATE_POLL_S (default 60) is how often the monitor loop looks for solves and
# infrastructure failures; the script's own tests set it low.
#
# --refs prints one JSON line per reference (name, agent, model, version)
# and exits, so a caller never has to parse this file.
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
if [ "${1:-}" = --refs ]; then
  echo '{"name": "astra", "agent": "codex", "model": "gpt-6-astra", "version": "0.159.0"}'
  echo '{"name": "fable", "agent": "claude-code", "model": "claude-fable-5-1", "version": "2.1.302"}'
  echo '{"name": "opus", "agent": "claude-code", "model": "claude-opus-5-5", "version": "2.1.302"}'
  exit 0
fi
preflight_only="${GATE_PREFLIGHT_ONLY:-0}"
ref="${1:?usage: run_gate.sh <astra|fable|opus> <task> [<task> ...]}"; shift
[ $# -gt 0 ] || [ "$preflight_only" = 1 ] || { echo "run_gate: name at least one task" >&2; exit 2; }
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
    # A dedicated gate login (no connected apps) when it exists, else the
    # personal one; Harbor reads the file named by CODEX_AUTH_JSON_PATH.
    codex_auth="${GATE_CODEX_AUTH:-$HOME/.config/vulcanbench/codex-gate/auth.json}"
    if [ ! -f "$codex_auth" ]; then
      [ -n "${GATE_CODEX_AUTH:-}" ] && { echo "run_gate: GATE_CODEX_AUTH=$codex_auth missing" >&2; exit 2; }
      codex_auth="$HOME/.codex/auth.json"
    fi
    [ -f "$codex_auth" ] || { echo "run_gate: $codex_auth missing; run \`codex login\`" >&2; exit 2; }
    auth_env=(CODEX_AUTH_JSON_PATH="$codex_auth")
    # Provider-side tools off: web search, the account's connected apps
    # (codex_apps), plugins and browser tools (docs/DECISIONS.md 2026-10-31).
    codex_cfg="$here/scripts/frontier-v5/codex-gate.toml"
    [ -f "$codex_cfg" ] || { echo "run_gate: $codex_cfg missing" >&2; exit 2; }
    auth_args=(--ak "config=$codex_cfg") ;;
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
# The last three alternatives are a reference reaching a provider-side tool
# (web search, the account's connected apps) that run_gate.sh turns off; such
# a trial is not a gate result either (docs/DECISIONS.md 2026-10-31).
INFRA_RE='"error":"authentication_failed"|OAuth access token is invalid|issue with the selected model|requires usage credits|hit your usage limit|"server": ?"codex_apps"|tools[.]mcp__codex_apps__|tools[.]web__run[(]'

# Per-task state the helpers below read; set in the task loop.
job=""; t=""; status_file=""; hpid=""; hcode=""; started_at=""
stop_flag=""; spid=""

write_status() { # phase [exit_code] [stop_reason] -> atomic rewrite of $status_file
  [ -n "$status_file" ] || return 0
  python3 - "$status_file" "$1" "${2:-}" "${3:-}" "$job" "$t" "$ref" "$agent" "$model" \
    "$$" "$hpid" "$started_at" "$(git -C "$here" rev-parse --short HEAD 2>/dev/null)" "$OUT/jobs/$job" <<'PY'
import glob, json, os, sys, time
(path, phase, code, reason, job, task, ref, agent, model,
 pid, hpid, started, repo, job_dir) = sys.argv[1:15]
solves = 0
for p in glob.glob(job_dir + "/*__*/verifier/reward.json"):
    try:
        solves += json.load(open(p)).get("reward") == 1
    except Exception:
        pass
d = {
    "job": job, "task": task, "ref": ref, "agent": agent, "model": model,
    "effort": "medium", "pid": int(pid), "harbor_pid": int(hpid) if hpid else None,
    "phase": phase, "solves": solves, "started_at": started,
    "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "exit_code": int(code) if code else None, "stop_reason": reason or None,
    "repo": repo or None, "job_dir": job_dir,
}
tmp = path + ".tmp"
with open(tmp, "w") as fh:
    json.dump(d, fh, indent=1)
os.replace(tmp, path)
PY
}

usage_preflight() { # claude-code refs: one one-turn call reads the subscription windows; refuse above GATE_USAGE_MAX
  local max="${GATE_USAGE_MAX:-0.85}" out rl_status rl_util verdict
  if ! command -v claude >/dev/null; then
    echo "run_gate: no claude CLI on PATH, skipping the usage preflight" >&2
    [ "$preflight_only" = 1 ] && { printf '{"model": "%s", "status": "unknown", "would_start": true, "note": "no claude CLI on PATH"}\n' "$model"; exit 0; }
    return 0
  fi
  out=$( ( set -a; . "$envf"; set +a; claude -p --model "$model" --output-format stream-json --verbose --max-turns 1 'Reply with the single word ok.' 2>/dev/null ) \
    | grep '"rate_limit_event"' | tail -1 | python3 -c '
import json, sys
line = sys.stdin.read().strip()
if not line: sys.exit(0)
d = json.loads(line)["rate_limit_info"]; w = d.get("unifiedWindows") or {}
m = max([v.get("utilization") or 0 for v in w.values()] or [0])
print("%s %.2f %s" % (d.get("status"), m, " ".join("%s=%.2f" % (k, v.get("utilization") or 0) for k, v in w.items())))' )
  if [ -z "$out" ]; then
    echo "run_gate: usage preflight saw no rate-limit event, continuing" >&2
    [ "$preflight_only" = 1 ] && { printf '{"model": "%s", "status": "unknown", "would_start": true, "note": "no rate-limit event in the probe"}\n' "$model"; exit 0; }
    return 0
  fi
  rl_status=${out%% *}; rl_util=$(printf '%s' "$out" | awk '{print $2}')
  verdict=start
  if [ "${GATE_USAGE_OVERRIDE:-0}" != 1 ] && { [ "$rl_status" != allowed ] || awk -v u="$rl_util" -v m="$max" 'BEGIN{exit !(u > m)}'; }; then
    verdict=refuse
  fi
  if [ "$preflight_only" = 1 ]; then
    python3 -c '
import json, sys
words = sys.argv[1].split()
windows = {}
for w in words[2:]:
    k, _, v = w.partition("=")
    windows[k] = float(v)
print(json.dumps({"model": sys.argv[2], "status": words[0], "max_utilization": float(words[1]),
                  "windows": windows, "max_allowed": float(sys.argv[3]),
                  "would_start": sys.argv[4] == "start"}))' "$out" "$model" "$max" "$verdict"
    [ "$verdict" = start ] && exit 0 || exit 2
  fi
  echo "=== usage preflight $model: $out"
  if [ "$verdict" = refuse ]; then
    echo "!!! usage window at $rl_util (status $rl_status) is above GATE_USAGE_MAX=$max; not starting. Set GATE_USAGE_OVERRIDE=1 to run anyway." >&2
    write_status infra_failed 2 usage
    exit 2
  fi
}

if [ "$preflight_only" = 1 ]; then
  if [ "$agent" = claude-code ]; then usage_preflight; fi
  printf '{"model": "%s", "status": "not_applicable", "would_start": true, "note": "no usage preflight for %s"}\n' "$model" "$agent"
  exit 0
fi

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

stop_harbor() { # SIGINT harbor (its cancel path runs `docker compose down`), escalate, reap, sweep
  local i
  trap '' INT TERM
  [ -n "$hpid" ] || return 0
  kill -INT "$hpid" 2>/dev/null
  i=0; while [ "$i" -lt 90 ] && kill -0 "$hpid" 2>/dev/null; do sleep 1; i=$((i + 1)); done
  if kill -0 "$hpid" 2>/dev/null; then
    echo "!!! $t: harbor did not stop within 90 s of SIGINT; sending SIGTERM"
    kill -TERM "$hpid" 2>/dev/null
    i=0; while [ "$i" -lt 15 ] && kill -0 "$hpid" 2>/dev/null; do sleep 1; i=$((i + 1)); done
  fi
  kill -0 "$hpid" 2>/dev/null && { echo "!!! $t: harbor did not stop; sending SIGKILL"; kill -KILL "$hpid" 2>/dev/null; }
  wait "$hpid" 2>/dev/null; hcode=$?
  stop_sandboxes "$t"
}

print_trials() { # job dir -> one summary line per trial
  local tr
  for tr in "$1"/*__*; do
    [ -d "$tr" ] || continue
    echo "  $(basename "$tr"): $(cat "$tr/verifier/reward.json" 2>/dev/null || echo 'stopped, no reward')"
  done
}

on_signal() { # record the signal; the monitor loop acts on it after its current step
  stop_flag=$1
  [ -n "$spid" ] && kill "$spid" 2>/dev/null
  return 0
}
trap 'on_signal INT' INT
trap 'on_signal TERM' TERM

for t in "$@"; do
  task="$here/tasks/frontier-v5/$t"
  [ -f "$task/task.toml" ] || { echo "run_gate: no task $t" >&2; exit 2; }
  lang=$(sed -n 's/^vulcanbench_language = "\(.*\)"/\1/p' "$task/task.toml")
  case "$lang" in python|javascript) n=2 ;; *) n=1 ;; esac
  case "${GATE_CONCURRENCY:-}" in "") ;; 1|2) n=$GATE_CONCURRENCY ;; *) echo "run_gate: GATE_CONCURRENCY must be 1 or 2" >&2; exit 2 ;; esac
  job="$ref-$t$suffix"
  [ -e "$OUT/jobs/$job" ] && { echo "run_gate: $OUT/jobs/$job exists; move it aside first" >&2; exit 2; }
  status_file="$OUT/$job.status.json"; hpid=""; hcode=""; started_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  write_status preflight
  [ "$agent" = claude-code ] && usage_preflight
  if [ -n "$stop_flag" ]; then
    echo "!!! $t: stop requested (SIG$stop_flag) before harbor started"
    write_status stopped 130 "signal-$stop_flag"; exit 130
  fi
  echo "=== $t $(date '+%F %H:%M:%S') ref=$ref agent=$agent model=$model version=$version effort=medium attempts=$attempts concurrency=$n harbor=$(harbor --version 2>/dev/null) repo=$(git -C "$here" rev-parse --short HEAD)"
  # A background list runs with SIGINT ignored (bash 3.2 has no `trap - INT`
  # that survives the fork), so harbor is exec'd through a python one-liner
  # that restores the default disposition. $! is then harbor's own pid.
  (cd "$here" && exec python3 -c 'import os, signal, sys
signal.signal(signal.SIGINT, signal.SIG_DFL)
os.execvp(sys.argv[1], sys.argv[1:])' \
     env "${auth_env[@]+"${auth_env[@]}"}" harbor run -p "$task" -a "$agent" -m "$model" --effort medium \
     --ak version="$version" --ak disable_web_search=true -k "$attempts" -n "$n" "${auth_args[@]+"${auth_args[@]}"}" \
     -o "$OUT/jobs" --job-name "$job" -y -q) > "$OUT/$job.log" 2>&1 &
  hpid=$!
  write_status running
  stopped=""
  while kill -0 "$hpid" 2>/dev/null; do
    if [ -z "$stop_flag" ]; then
      sleep "${GATE_POLL_S:-60}" & spid=$!
      wait "$spid" 2>/dev/null; kill "$spid" 2>/dev/null; spid=""
    fi
    if [ -n "$stop_flag" ]; then
      echo "!!! $t: stop requested (SIG$stop_flag); stopping harbor and the sandboxes"
      write_status stopping "" "signal-$stop_flag"
      stop_harbor
      print_trials "$OUT/jobs/$job"
      write_status stopped "$hcode" "signal-$stop_flag"
      exit 130
    fi
    bad=$(infra_hit "$OUT/jobs/$job")
    if [ -n "$bad" ]; then
      echo "!!! $t: infrastructure failure in $bad (matches: $INFRA_RE); stopping the run"
      write_status stopping "" "infra:$bad"
      stop_harbor
      write_status infra_failed "$hcode" "infra:$bad"
      echo "!!! move $OUT/jobs/$job aside as INFRA-..., fix the cause, and rerun"; exit 1
    fi
    if [ "$(solves "$OUT/jobs/$job")" -ge 2 ]; then
      echo "--- $t: two solves, early reject; stopping the remaining attempt"
      write_status stopping "" early-reject
      stop_harbor; stopped=1
      trap 'on_signal INT' INT; trap 'on_signal TERM' TERM
    fi
  done
  if [ -z "$hcode" ]; then wait "$hpid" 2>/dev/null; hcode=$?; fi
  stop_sandboxes "$t"
  if [ -z "$stopped" ]; then
    bad=$(infra_hit "$OUT/jobs/$job")
    [ -n "$bad" ] && { echo "!!! $t: infrastructure failure in $bad; not a gate result"; write_status infra_failed "$hcode" "infra:$bad"; exit 1; }
  fi
  print_trials "$OUT/jobs/$job"
  echo "--- $t: $(solves "$OUT/jobs/$job") solve(s) $(date '+%F %H:%M:%S')"
  write_status done "$hcode" "${stopped:+early-reject}"
done
echo "=== DONE $(date '+%F %H:%M:%S')"
