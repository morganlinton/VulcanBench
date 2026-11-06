#!/usr/bin/env bash
# ComputerUse v1 reference-model gate runner (docs/COMPUTER_USE_CHARTER.md).
#
# usage: scripts/computer-use/run_gate.sh claude <task> [<task> ...]
#        scripts/computer-use/run_gate.sh --refs
#        GATE_PREFLIGHT_ONLY=1 scripts/computer-use/run_gate.sh claude [<task>]
#
# Sibling of scripts/frontier-v5/run_gate.sh for tasks/computer-use-v1. The
# reference is Harbor's built-in computer-1 agent, which drives the desktop
# inside the task container and calls the vendor API from this host, so the
# only credential is an API key in the host environment (never in the
# container, never passed with --ae, never printed). Every attempt runs to
# the end: there is no early exit, and one trial runs at a time.
#
#   claude  harbor -a computer-1 -m <pins.references[claude].model>
#           (CU_GATE_MODEL overrides; the model and the computer-1 kwargs
#           come from tasks/computer-use-v1/suite.json pins)
#
# Authentication: ANTHROPIC_API_KEY= in CU_GATE_ENV (default
# ~/.config/vulcanbench/computer-use-gate.env), sourced into this script's
# environment so harbor inherits it. Preflight refuses without it.
#
# Preflight (also GATE_PREFLIGHT_ONLY=1, which prints one JSON line and
# exits 0 without launching): docker reachable, harbor 0.24.0 on PATH, the
# anthropic SDK importable from harbor's own python (the harbor[computer-1]
# extra; skipped when that python is not where uv puts it), the
# vulcanbench/sandbox:cu-base image present, the env file and key, the task
# directory, a resolved model.
#
# Infrastructure guard: an auth, quota, rate-limit, missing-SDK or desktop
# bring-up failure in a trial's agent logs, trial log or the job log stops
# the whole run; such trials are not gate results.
#
# The status file, solve counting, sandbox sweep, harbor stop and signal
# handling are copied from scripts/frontier-v5/run_gate.sh so that
# backend/runner/discovery.py reads $GATE_OUT/<job>.status.json unchanged
# (phases preflight, running, stopping, done, infra_failed, stopped; the
# same keys, with effort "n/a"). A shared scripts/lib/gate_common.sh is a
# later refactor once both scripts are stable.
#
# GATE_ATTEMPTS (default 3, 1 to 3) is harbor's -k. GATE_POLL_S (default 60)
# is the monitor interval. GATE_JOB_SUFFIX names a second job directory next
# to the first. Output: $GATE_OUT (default ~/vulcanbench-gate)/jobs/cu-<ref>-<task><suffix>.
# Exit codes: 0 done, 1 infrastructure failure, 2 refused before harbor
# started, 130 stopped on a signal.
set -u
if [ "${1:-}" = --refs ]; then
  echo '{"name": "claude", "agent": "computer-1", "model": "pins.references[claude].model in tasks/computer-use-v1/suite.json (CU_GATE_MODEL overrides)"}'
  exit 0
fi
preflight_only="${GATE_PREFLIGHT_ONLY:-0}"
ref="${1:?usage: run_gate.sh claude <task> [<task> ...]}"; shift
[ $# -gt 0 ] || [ "$preflight_only" = 1 ] || { echo "cu-gate: name at least one task" >&2; exit 2; }
here="$(cd "$(dirname "$0")/../.." && pwd)"
export PATH="$HOME/.local/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
OUT="${GATE_OUT:-$HOME/vulcanbench-gate}"
attempts="${GATE_ATTEMPTS:-3}"; suffix="${GATE_JOB_SUFFIX:-}"
case "$attempts" in 1|2|3) ;; *) echo "cu-gate: GATE_ATTEMPTS must be 1, 2 or 3 (got $attempts)" >&2; exit 2 ;; esac
mkdir -p "$OUT/jobs"
suite_json="$here/tasks/computer-use-v1/suite.json"
HARBOR_VERSION=0.24.0
IMAGE=vulcanbench/sandbox:cu-base

case "$ref" in
  claude)
    agent=computer-1
    model="${CU_GATE_MODEL:-}"
    if [ -z "$model" ] && [ -f "$suite_json" ]; then
      model=$(python3 -c '
import json, sys
pins = json.load(open(sys.argv[1])).get("pins", {})
for r in pins.get("references", []):
    if r.get("name") == "claude":
        print(r.get("model", "")); break' "$suite_json")
    fi
    [ -n "$model" ] || { echo "cu-gate: no model: set CU_GATE_MODEL or pins.references[claude].model in $suite_json" >&2; exit 2; }
    envf="${CU_GATE_ENV:-$HOME/.config/vulcanbench/computer-use-gate.env}"
    [ -f "$envf" ] || { echo "cu-gate: $envf missing (one line ANTHROPIC_API_KEY=...)" >&2; exit 2; }
    grep -q '^ANTHROPIC_API_KEY=.' "$envf" || { echo "cu-gate: $envf has no ANTHROPIC_API_KEY= line" >&2; exit 2; }
    set -a; . "$envf"; set +a ;;
  *) echo "cu-gate: reference must be claude" >&2; exit 2 ;;
esac

# computer-1 kwargs: CU_GATE_AGENT_KWARGS (space-separated key=value) wins,
# else suite.json pins.agent_kwargs, else the charter defaults.
ak_args=()
kwargs="${CU_GATE_AGENT_KWARGS:-}"
if [ -z "$kwargs" ] && [ -f "$suite_json" ]; then
  kwargs=$(python3 -c '
import json, sys
kw = json.load(open(sys.argv[1])).get("pins", {}).get("agent_kwargs", {})
print(" ".join("%s=%s" % (k, v) for k, v in kw.items()))' "$suite_json")
fi
[ -n "$kwargs" ] || kwargs="max_turns=200 desktop_width=1024 desktop_height=900 window_width=1024 window_height=900 start_url=http://127.0.0.1:8000/"
for kv in $kwargs; do ak_args+=(--ak "$kv"); done

preflight() { # host checks; prints one problem per line, returns 1 on any
  local rc=0 hv hpy
  docker info >/dev/null 2>&1 || { echo "cu-gate: docker is not reachable" >&2; rc=1; }
  command -v harbor >/dev/null || { echo "cu-gate: harbor not found (scripts/computer-use/setup-host.sh)" >&2; return 1; }
  hv=$(harbor --version 2>/dev/null)
  case "$hv" in *"$HARBOR_VERSION"*) ;; *) echo "cu-gate: harbor $HARBOR_VERSION required (got '$hv')" >&2; rc=1 ;; esac
  hpy="$HOME/.local/share/uv/tools/harbor/bin/python"
  if [ -x "$hpy" ]; then
    "$hpy" -c 'import anthropic' 2>/dev/null || { echo "cu-gate: harbor's python cannot import anthropic; run scripts/computer-use/setup-host.sh" >&2; rc=1; }
  fi
  docker image inspect "$IMAGE" >/dev/null 2>&1 || { echo "cu-gate: $IMAGE not built (make sandbox-image-computer-use)" >&2; rc=1; }
  return $rc
}

preflight || { echo "cu-gate: preflight failed" >&2; exit 2; }
for t in "$@"; do
  [ -f "$here/tasks/computer-use-v1/$t/task.toml" ] || { echo "cu-gate: no task $t under tasks/computer-use-v1" >&2; exit 2; }
done
if [ "$preflight_only" = 1 ]; then
  python3 -c '
import json, sys
print(json.dumps({"ref": sys.argv[1], "agent": sys.argv[2], "model": sys.argv[3], "status": "ready",
                  "would_start": True, "attempts": int(sys.argv[4]), "agent_kwargs": sys.argv[5].split()}))' \
    "$ref" "$agent" "$model" "$attempts" "$kwargs"
  exit 0
fi

# Exact failure texts: vendor auth and quota errors, the missing provider SDK,
# and computer-1's desktop bring-up timeouts (harbor/agents/computer_1/runtime.py).
INFRA_RE='authentication_error|invalid_api_key|insufficient_quota|rate_limit_error|overloaded_error|No module named .anthropic.|Chromium did not become ready|did not become ready within|X11 display :1 never appeared|not a computer-use model'

# --- copied from scripts/frontier-v5/run_gate.sh (status file, solves, sweep, stop, signals) ---
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
    "effort": "n/a", "pid": int(pid), "harbor_pid": int(hpid) if hpid else None,
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

infra_hit() { # job dir -> prints the first trial (or "job-log") with an infrastructure failure
  local tr
  for tr in "$1"/*__*; do
    [ -d "$tr" ] || continue
    if grep -rqE "$INFRA_RE" "$tr/agent" "$tr/trial.log" 2>/dev/null; then basename "$tr"; return; fi
  done
  if grep -qE "$INFRA_RE" "$OUT/$job.log" 2>/dev/null; then echo "job-log"; fi
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
# --- end of the copied helpers ---

for t in "$@"; do
  task="$here/tasks/computer-use-v1/$t"
  job="cu-$ref-$t$suffix"
  [ -e "$OUT/jobs/$job" ] && { echo "cu-gate: $OUT/jobs/$job exists; move it aside first" >&2; exit 2; }
  status_file="$OUT/$job.status.json"; hpid=""; hcode=""; started_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  write_status preflight
  if [ -n "$stop_flag" ]; then
    echo "!!! $t: stop requested (SIG$stop_flag) before harbor started"
    write_status stopped 130 "signal-$stop_flag"; exit 130
  fi
  echo "=== $t $(date '+%F %H:%M:%S') ref=$ref agent=$agent model=$model attempts=$attempts concurrency=1 kwargs=[$kwargs] harbor=$(harbor --version 2>/dev/null) repo=$(git -C "$here" rev-parse --short HEAD)"
  # harbor is exec'd through a python one-liner that restores SIGINT's default
  # disposition (a background list ignores it in bash 3.2), so $! is harbor's pid.
  (cd "$here" && exec python3 -c 'import os, signal, sys
signal.signal(signal.SIGINT, signal.SIG_DFL)
os.execvp(sys.argv[1], sys.argv[1:])' \
     harbor run -p "$task" -a "$agent" -m "$model" "${ak_args[@]}" \
     -k "$attempts" -n 1 -o "$OUT/jobs" --job-name "$job" -y -q) > "$OUT/$job.log" 2>&1 &
  hpid=$!
  write_status running
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
  done
  if [ -z "$hcode" ]; then wait "$hpid" 2>/dev/null; hcode=$?; fi
  stop_sandboxes "$t"
  bad=$(infra_hit "$OUT/jobs/$job")
  [ -n "$bad" ] && { echo "!!! $t: infrastructure failure in $bad; not a gate result"; write_status infra_failed "$hcode" "infra:$bad"; exit 1; }
  print_trials "$OUT/jobs/$job"
  echo "--- $t: $(solves "$OUT/jobs/$job") solve(s) of $attempts $(date '+%F %H:%M:%S')"
  write_status done "$hcode"
done
echo "=== DONE $(date '+%F %H:%M:%S')"
