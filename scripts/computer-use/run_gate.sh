#!/usr/bin/env bash
# ComputerUse v1 reference-model gate runner (docs/COMPUTER_USE_CHARTER.md).
#
# usage: scripts/computer-use/run_gate.sh <claude-code|codex|claude> <task> [<task> ...]
#        scripts/computer-use/run_gate.sh --refs
#        GATE_PREFLIGHT_ONLY=1 scripts/computer-use/run_gate.sh <ref> [<task>]
#
# Sibling of scripts/frontier-v5/run_gate.sh for tasks/computer-use-v1. Three
# references, three agent rows (suite.json pins.agent_rows):
#
#   claude-code  harbor -a claude-code -m <pins.references[claude-code].model>
#                --effort <CU_GATE_EFFORT, default medium>, Claude Code on the
#                Claude subscription, acting only through the task's MCP
#                computer tools: its own tools are passed as
#                --ak disallowed_tools=<pins...disallowed_tools>, web tools
#                are off. Auth exactly as the Frontier gate: CLAUDE_FORCE_OAUTH=1
#                with --env-file CLAUDE_GATE_ENV (default
#                ~/.config/vulcanbench/claude-gate.env, one line
#                CLAUDE_CODE_OAUTH_TOKEN=...), or FABLE_AUTH=api for an
#                ANTHROPIC_API_KEY line in that file. The same one-turn usage
#                preflight runs first and refuses above GATE_USAGE_MAX.
#   codex        harbor -a codex -m <pins.references[codex].model> --effort
#                <CU_GATE_EFFORT, default medium>, Codex CLI
#                <pins.references[codex].version> on the ChatGPT subscription,
#                acting only through the task's MCP computer tools. Auth as
#                the Frontier v5 gate: Harbor injects the auth.json that
#                CODEX_AUTH_JSON_PATH (set in harbor's environment, never
#                --ae) names: CU_GATE_CODEX_AUTH, else a dedicated gate login
#                at ~/.config/vulcanbench/codex-gate/auth.json, else
#                ~/.codex/auth.json. chatgpt.com, auth.openai.com and
#                api.openai.com are added to the task's agent allowlist with
#                --allow-agent-host. Codex's own tools are turned off by
#                scripts/computer-use/codex-gate.toml (--ak config=...) and by
#                a one-model catalog that clears apply_patch and the
#                subagent tools, built once per gate invocation by
#                scripts/computer-use/codex_catalog.py from `codex debug
#                models` on this host (CU_GATE_CODEX_CATALOG_SOURCE names a
#                file instead) and copied into every job's task copy (see
#                the condition note below; the codex row always runs a
#                copy, whose Dockerfile gains one COPY of the catalog) and
#                next to the status file as <job>.codex-catalog.json.
#   claude       harbor -a computer-1 -m <pins.references[claude].model>,
#                Anthropic's native computer-use tool called from this host,
#                so the only credential is ANTHROPIC_API_KEY= in CU_GATE_ENV
#                (default ~/.config/vulcanbench/computer-use-gate.env),
#                sourced into this script's environment so harbor inherits
#                it. computer-1 kwargs come from pins.agent_kwargs.
#
# CU_GATE_MODEL overrides any model; CU_GATE_AGENT_KWARGS (space-separated
# key=value) overrides the computer-1 kwargs. Credentials are never passed
# with --ae, never put in the container by this script, never printed. Every
# attempt runs to the end: there is no early exit, and one trial runs at a
# time.
#
# Preflight (also GATE_PREFLIGHT_ONLY=1, which prints one JSON line and
# exits 0 without launching): docker reachable, harbor 0.24.0 on PATH, the
# vulcanbench/sandbox:cu-base image present, the task directory, a resolved
# model, the reference's credential file; for claude the anthropic SDK in
# harbor's own python (the harbor[computer-1] extra), for claude-code the
# usage preflight, for codex the codex CLI on this host (for the catalog).
#
# Infrastructure guard: an auth, quota, rate-limit, missing-SDK or desktop
# bring-up failure in a trial's agent logs, trial log or the job log stops
# the whole run; such trials are not gate results.
#
# Tool-surface guard (codex): an event in a trial's codex.txt for a tool the
# row must not have (a shell command, a file change, a web search, a
# subagent, an image generation) stops the run with stop_reason
# tool-surface:<trial>; such trials are not gate results either, and the
# settings that let the tool through are a defect to fix before rerunning.
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
# to the first. Output: $GATE_OUT (default ~/vulcanbench-gate)/jobs/cu-<ref>-<task>[-<condition>]<suffix>.
#
# CU_GATE_CONDITION (default pixels+zoom) is the perception condition of the
# charter's table, applied by the MCP computer tools: pixels+zoom (what every
# reading before milestone 4 was taken under), pixels (no zoom tool) or
# pixels-lowres (screenshots scaled to 0.75, no zoom). Harbor forwards no
# arbitrary environment to the agent, so for a non-default condition the
# task is copied to $GATE_OUT/task-copies/<job>/<task> with "--condition"
# added to the MCP server's args in task.toml, and harbor runs that copy (as
# scripts/computer-use/validate_task.py installs a cheat probe). The copy is
# recorded in the status file (task_dir) and the condition in the job name
# and in the status file (condition). The claude-code and codex references
# only (both use the suite's MCP computer tools): computer-1 brings its own
# tool, which the suite does not change.
# Exit codes: 0 done, 1 infrastructure failure, 2 refused before harbor
# started, 130 stopped on a signal.
set -u
if [ "${1:-}" = --refs ]; then
  echo '{"name": "claude-code", "agent": "claude-code", "auth": "subscription", "model": "pins.references[claude-code].model in tasks/computer-use-v1/suite.json (CU_GATE_MODEL overrides)"}'
  echo '{"name": "codex", "agent": "codex", "auth": "subscription", "model": "pins.references[codex].model in tasks/computer-use-v1/suite.json (CU_GATE_MODEL overrides)"}'
  echo '{"name": "claude", "agent": "computer-1", "auth": "api-key", "model": "pins.references[claude].model in tasks/computer-use-v1/suite.json (CU_GATE_MODEL overrides)"}'
  exit 0
fi
preflight_only="${GATE_PREFLIGHT_ONLY:-0}"
ref="${1:?usage: run_gate.sh <claude-code|codex|claude> <task> [<task> ...]}"; shift
[ $# -gt 0 ] || [ "$preflight_only" = 1 ] || { echo "cu-gate: name at least one task" >&2; exit 2; }
here="$(cd "$(dirname "$0")/../.." && pwd)"
export PATH="$HOME/.local/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
OUT="${GATE_OUT:-$HOME/vulcanbench-gate}"
attempts="${GATE_ATTEMPTS:-3}"; suffix="${GATE_JOB_SUFFIX:-}"
case "$attempts" in 1|2|3) ;; *) echo "cu-gate: GATE_ATTEMPTS must be 1, 2 or 3 (got $attempts)" >&2; exit 2 ;; esac
condition="${CU_GATE_CONDITION:-pixels+zoom}"
case "$condition" in pixels+zoom|pixels|pixels-lowres) ;;
  *) echo "cu-gate: CU_GATE_CONDITION must be pixels+zoom, pixels or pixels-lowres (got $condition)" >&2; exit 2 ;; esac
mkdir -p "$OUT/jobs"
# CU_GATE_SUITE picks the suite: computer-use-v1 (Linux, default) or
# computer-use-macos-v1, whose tasks run under the Tart environment (a macOS
# VM per trial; docs/COMPUTER_USE_MACOS_CHARTER.md).
gate_suite="${CU_GATE_SUITE:-computer-use-v1}"
suite_json="$here/tasks/$gate_suite/suite.json"
HARBOR_VERSION=0.24.0
IMAGE=vulcanbench/sandbox:cu-base
macos=0
if [ "$gate_suite" = computer-use-macos-v1 ]; then
  macos=1; IMAGE=vulcanbench/sandbox:cu-mac
fi

pin() { # <ref> <field> -> pins.references[name==ref].field from suite.json, or empty
  [ -f "$suite_json" ] || return 0
  python3 -c '
import json, sys
pins = json.load(open(sys.argv[1])).get("pins", {})
for r in pins.get("references", []):
    if r.get("name") == sys.argv[2]:
        print(r.get(sys.argv[3], "")); break' "$suite_json" "$1" "$2"
}

auth_env=(); auth_args=(); extra_ak=(); effort_args=(); effort_label="n/a"
case "$ref" in
  claude-code)
    agent=claude-code
    model="${CU_GATE_MODEL:-$(pin claude-code model)}"
    version="${CU_GATE_CLAUDE_CODE_VERSION:-$(pin claude-code version)}"
    disallowed="${CU_GATE_DISALLOWED_TOOLS:-$(pin claude-code disallowed_tools)}"
    [ -n "$model" ] || { echo "cu-gate: no model: set CU_GATE_MODEL or pins.references[claude-code].model in $suite_json" >&2; exit 2; }
    [ -n "$version" ] || { echo "cu-gate: no Claude Code version: pins.references[claude-code].version in $suite_json" >&2; exit 2; }
    [ -n "$disallowed" ] || { echo "cu-gate: no disallowed_tools: pins.references[claude-code].disallowed_tools in $suite_json" >&2; exit 2; }
    effort_label="${CU_GATE_EFFORT:-medium}"
    effort_args=(--effort "$effort_label")
    extra_ak=(--ak "version=$version" --ak "disallowed_tools=$disallowed" --ak disable_web_search=true)
    envf="${CLAUDE_GATE_ENV:-$HOME/.config/vulcanbench/claude-gate.env}"
    [ -f "$envf" ] || { echo "cu-gate: $envf missing (one line CLAUDE_CODE_OAUTH_TOKEN=... from \`claude setup-token\`)" >&2; exit 2; }
    auth_args=(--env-file "$envf")
    if [ "${FABLE_AUTH:-oauth}" = api ]; then auth_env=(); else auth_env=(CLAUDE_FORCE_OAUTH=1); fi ;;
  codex)
    agent=codex
    model="${CU_GATE_MODEL:-$(pin codex model)}"
    version="${CU_GATE_CODEX_VERSION:-$(pin codex version)}"
    [ -n "$model" ] || { echo "cu-gate: no model: set CU_GATE_MODEL or pins.references[codex].model in $suite_json" >&2; exit 2; }
    [ -n "$version" ] || { echo "cu-gate: no Codex version: pins.references[codex].version in $suite_json" >&2; exit 2; }
    codex_cfg="$here/scripts/computer-use/codex-gate.toml"
    [ -f "$codex_cfg" ] || { echo "cu-gate: $codex_cfg missing" >&2; exit 2; }
    effort_label="${CU_GATE_EFFORT:-medium}"
    effort_args=(--effort "$effort_label")
    extra_ak=(--ak "version=$version" --ak "config=$codex_cfg")
    # The ChatGPT backend and its token refresh, on top of each task's agent
    # allowlist (which names only the claude-code reference's API host); the
    # Frontier v5 tasks allow the same three.
    for h in chatgpt.com auth.openai.com api.openai.com; do extra_ak+=(--allow-agent-host "$h"); done
    # A dedicated gate login when it exists, else the personal one (as the
    # Frontier v5 gate); harbor reads the file CODEX_AUTH_JSON_PATH names.
    codex_auth="${CU_GATE_CODEX_AUTH:-$HOME/.config/vulcanbench/codex-gate/auth.json}"
    if [ ! -f "$codex_auth" ]; then
      [ -n "${CU_GATE_CODEX_AUTH:-}" ] && { echo "cu-gate: CU_GATE_CODEX_AUTH=$codex_auth missing" >&2; exit 2; }
      codex_auth="$HOME/.codex/auth.json"
    fi
    [ -f "$codex_auth" ] || { echo "cu-gate: $codex_auth missing; run \`codex login\`" >&2; exit 2; }
    auth_env=(CODEX_AUTH_JSON_PATH="$codex_auth") ;;
  claude)
    agent=computer-1
    model="${CU_GATE_MODEL:-$(pin claude model)}"
    [ -n "$model" ] || { echo "cu-gate: no model: set CU_GATE_MODEL or pins.references[claude].model in $suite_json" >&2; exit 2; }
    envf="${CU_GATE_ENV:-$HOME/.config/vulcanbench/computer-use-gate.env}"
    [ -f "$envf" ] || { echo "cu-gate: $envf missing (one line ANTHROPIC_API_KEY=...)" >&2; exit 2; }
    grep -q '^ANTHROPIC_API_KEY=.' "$envf" || { echo "cu-gate: $envf has no ANTHROPIC_API_KEY= line" >&2; exit 2; }
    set -a; . "$envf"; set +a ;;
  *) echo "cu-gate: reference must be claude-code, codex or claude" >&2; exit 2 ;;
esac

if [ "$condition" != pixels+zoom ] && [ "$agent" = computer-1 ]; then
  echo "cu-gate: CU_GATE_CONDITION=$condition applies to the claude-code and codex references only (computer-1 brings its own tool)" >&2; exit 2
fi

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
if [ "$agent" = computer-1 ]; then
  for kv in $kwargs; do ak_args+=(--ak "$kv"); done
elif [ "$agent" = codex ]; then
  kwargs="(codex: tools from the task's MCP server; config=$(basename "$codex_cfg"); version=$version)"
else
  kwargs="(claude-code: tools from the task's MCP server; disallowed=$disallowed)"
fi

preflight() { # host checks; prints one problem per line, returns 1 on any
  local rc=0 hv hpy
  docker info >/dev/null 2>&1 || { echo "cu-gate: docker is not reachable" >&2; rc=1; }
  command -v harbor >/dev/null || { echo "cu-gate: harbor not found (scripts/computer-use/setup-host.sh)" >&2; return 1; }
  hv=$(harbor --version 2>/dev/null)
  case "$hv" in *"$HARBOR_VERSION"*) ;; *) echo "cu-gate: harbor $HARBOR_VERSION required (got '$hv')" >&2; rc=1 ;; esac
  hpy="$HOME/.local/share/uv/tools/harbor/bin/python"
  if [ "$agent" = computer-1 ] && [ -x "$hpy" ]; then
    "$hpy" -c 'import anthropic' 2>/dev/null || { echo "cu-gate: harbor's python cannot import anthropic; run scripts/computer-use/setup-host.sh" >&2; rc=1; }
  fi
  docker image inspect "$IMAGE" >/dev/null 2>&1 || { echo "cu-gate: $IMAGE not built (make sandbox-image-computer-use$( [ "$macos" = 1 ] && echo -macos))" >&2; rc=1; }
  if [ "$macos" = 1 ]; then
    command -v tart >/dev/null || { echo "cu-gate: tart not found (brew install openai/tools/tart)" >&2; rc=1; }
  fi
  if [ "$agent" = codex ] && [ -z "${CU_GATE_CODEX_CATALOG_SOURCE:-}" ]; then
    command -v codex >/dev/null || { echo "cu-gate: no codex CLI on PATH (needed for \`codex debug models\`; or set CU_GATE_CODEX_CATALOG_SOURCE)" >&2; rc=1; }
  fi
  return $rc
}

usage_preflight() { # claude-code: one one-turn call reads the subscription windows; refuse above GATE_USAGE_MAX
  # Copied from scripts/frontier-v5/run_gate.sh.
  local max="${GATE_USAGE_MAX:-0.85}" out rl_status rl_util verdict
  if ! command -v claude >/dev/null; then
    echo "cu-gate: no claude CLI on PATH, skipping the usage preflight" >&2
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
    echo "cu-gate: usage preflight saw no rate-limit event, continuing" >&2
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

preflight || { echo "cu-gate: preflight failed" >&2; exit 2; }
for t in "$@"; do
  [ -f "$here/tasks/computer-use-v1/$t/task.toml" ] || { echo "cu-gate: no task $t under tasks/computer-use-v1" >&2; exit 2; }
done
if [ "$preflight_only" = 1 ]; then
  [ "$agent" = claude-code ] && usage_preflight
  python3 -c '
import json, sys
print(json.dumps({"ref": sys.argv[1], "agent": sys.argv[2], "model": sys.argv[3], "status": "ready",
                  "would_start": True, "attempts": int(sys.argv[4]), "agent_kwargs": sys.argv[5].split(),
                  "condition": sys.argv[6]}))' \
    "$ref" "$agent" "$model" "$attempts" "$kwargs" "$condition"
  exit 0
fi

# codex: one locked catalog for the whole invocation, copied into each job.
codex_catalog=""
if [ "$agent" = codex ]; then
  codex_catalog="$OUT/codex-catalog-$(date -u '+%Y%m%dT%H%M%SZ')-$$.json"
  catalog_line=$(python3 "$here/scripts/computer-use/codex_catalog.py" --model "$model" --out "$codex_catalog" \
    ${CU_GATE_CODEX_CATALOG_SOURCE:+--source "$CU_GATE_CODEX_CATALOG_SOURCE"}) \
    || { echo "cu-gate: could not build the codex catalog for $model" >&2; exit 2; }
  echo "=== codex catalog: $catalog_line"
fi

# Exact failure texts: vendor auth and quota errors and the missing provider
# SDK (computer-1); the Claude Code auth, model and usage messages the
# Frontier gate matches; computer-1's desktop bring-up timeouts and the MCP
# server's display failure (harbor/agents/computer_1/runtime.py,
# sandbox/computer-use/ensure-display.sh).
INFRA_RE='authentication_error|invalid_api_key|insufficient_quota|rate_limit_error|overloaded_error|No module named .anthropic.|"error":"authentication_failed"|OAuth access token is invalid|issue with the selected model|requires usage credits|hit your usage limit|Chromium did not become ready|did not become ready within|X11 display :1 never appeared|not a computer-use model|refresh_token_reused|Your access token could not be refreshed|unexpected status 401|usage_limit_reached'
# Codex exec --json item types for tools the codex row must not have (the
# tool-surface guard; codex.txt only).
SURFACE_RE='"type": ?"(command_execution|file_change|web_search|collab_tool_call|image_generation)"'

# --- copied from scripts/frontier-v5/run_gate.sh (status file, solves, sweep, stop, signals) ---
job=""; t=""; task=""; status_file=""; hpid=""; hcode=""; started_at=""
stop_flag=""; spid=""

write_status() { # phase [exit_code] [stop_reason] -> atomic rewrite of $status_file
  [ -n "$status_file" ] || return 0
  python3 - "$status_file" "$1" "${2:-}" "${3:-}" "$job" "$t" "$ref" "$agent" "$model" \
    "$$" "$hpid" "$started_at" "$(git -C "$here" rev-parse --short HEAD 2>/dev/null)" "$OUT/jobs/$job" "$effort_label" \
    "$condition" "$task" <<'PY'
import glob, json, os, sys, time
(path, phase, code, reason, job, task, ref, agent, model,
 pid, hpid, started, repo, job_dir, effort, condition, task_dir) = sys.argv[1:18]
solves = 0
for p in glob.glob(job_dir + "/*__*/verifier/reward.json"):
    try:
        solves += json.load(open(p)).get("reward") == 1
    except Exception:
        pass
d = {
    "job": job, "task": task, "ref": ref, "agent": agent, "model": model,
    "effort": effort, "pid": int(pid), "harbor_pid": int(hpid) if hpid else None,
    "phase": phase, "solves": solves, "started_at": started,
    "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "exit_code": int(code) if code else None, "stop_reason": reason or None,
    "repo": repo or None, "job_dir": job_dir,
    "condition": condition, "task_dir": task_dir or None,
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
  if [ "$macos" = 1 ]; then
    # A trial VM the environment could not delete (harbor stopped mid-run).
    # Sweeps run one trial at a time, so a stopped trial VM is a leftover.
    local vm
    for vm in $(tart list 2>/dev/null | awk '$2 ~ /^vbmac-trial-/ && $NF == "stopped" {print $2}'); do
      echo "--- $1: deleting leftover VM $vm"; tart delete "$vm" >/dev/null 2>&1
    done
  fi
  return 0
}

surface_hit() { # job dir -> prints the first trial whose codex log shows a tool the row must not have
  local tr
  [ "$agent" = codex ] || return 0
  for tr in "$1"/*__*; do
    [ -f "$tr/agent/codex.txt" ] || continue
    if grep -qE "$SURFACE_RE" "$tr/agent/codex.txt" 2>/dev/null; then basename "$tr"; return; fi
  done
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
  task="$here/tasks/$gate_suite/$t"
  env_args=(); env_path=()
  if [ "$macos" = 1 ]; then
    vm_image=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["image"])' "$task/environment/vm.json") || exit 2
    tart list 2>/dev/null | grep -qw "$vm_image" || { echo "cu-gate: VM image $vm_image missing (scripts/computer-use-macos/build_task_image.py)" >&2; exit 2; }
    env_args=(--env vb_macos.env:TartEnvironment); env_path=("PYTHONPATH=$here")
  fi
  # computer-1 opens its browser on the task's start page (pins.start_urls);
  # native tasks have no entry and their app is already on the display.
  task_ak=()
  if [ "$agent" = computer-1 ] && [ -f "$suite_json" ] && ! printf '%s\n' "${ak_args[@]+"${ak_args[@]}"}" | grep -q '^start_url='; then
    url=$(python3 -c '
import json, sys
print(json.load(open(sys.argv[1])).get("pins", {}).get("start_urls", {}).get(sys.argv[2], ""))' "$suite_json" "$t")
    [ -n "$url" ] && task_ak=(--ak "start_url=$url")
  fi
  job="cu-$ref-$t$suffix"
  [ "$condition" != pixels+zoom ] && job="cu-$ref-$t-$condition$suffix"
  [ -e "$OUT/jobs/$job" ] && { echo "cu-gate: $OUT/jobs/$job exists; move it aside first" >&2; exit 2; }
  if [ "$condition" != pixels+zoom ] || [ -n "$codex_catalog" ]; then
    # harbor runs a copy of the task: its MCP computer tools start under the
    # condition, and for codex its image carries the locked catalog
    task=$(python3 - "$task" "$OUT/task-copies/$job" "$condition" "$codex_catalog" <<'PYCOPY'
import re, shutil, sys
from pathlib import Path
src, dest_root, condition, catalog = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], sys.argv[4]
copy = dest_root / src.name
if dest_root.exists():
    shutil.rmtree(dest_root)
shutil.copytree(src, copy, ignore=shutil.ignore_patterns("__pycache__"))
toml = copy / "task.toml"
text = toml.read_text()
if condition != "pixels+zoom" and "mcp_servers" in text:
    text, n = re.subn(r'("/opt/vbdesk/vb_computer_mcp.py")', r'\1, "--condition", "%s"' % condition, text, count=1)
    if n != 1:
        sys.exit("cu-gate: %s has mcp_servers but no vb_computer_mcp.py args to extend" % toml)
toml.write_text(text)
if catalog:
    env = copy / "environment"
    shutil.copy(catalog, env / "codex-catalog.json")
    dockerfile = env / "Dockerfile"
    dockerfile.write_text(
        dockerfile.read_text().rstrip("\n")
        + "\n\n# Added by scripts/computer-use/run_gate.sh for the codex reference: the locked\n"
        + "# one-model catalog scripts/computer-use/codex-gate.toml names.\n"
        + "COPY codex-catalog.json /opt/vbdesk/codex-catalog.json\n"
    )
print(copy)
PYCOPY
) || exit 2
    [ -n "$codex_catalog" ] && cp "$codex_catalog" "$OUT/$job.codex-catalog.json"
  fi
  status_file="$OUT/$job.status.json"; hpid=""; hcode=""; started_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  write_status preflight
  [ "$agent" = claude-code ] && usage_preflight
  if [ -n "$stop_flag" ]; then
    echo "!!! $t: stop requested (SIG$stop_flag) before harbor started"
    write_status stopped 130 "signal-$stop_flag"; exit 130
  fi
  echo "=== $t $(date '+%F %H:%M:%S') ref=$ref agent=$agent model=$model effort=$effort_label condition=$condition attempts=$attempts concurrency=1 kwargs=[$kwargs] harbor=$(harbor --version 2>/dev/null) repo=$(git -C "$here" rev-parse --short HEAD)"
  # harbor is exec'd through a python one-liner that restores SIGINT's default
  # disposition (a background list ignores it in bash 3.2), so $! is harbor's pid.
  (cd "$here" && exec python3 -c 'import os, signal, sys
signal.signal(signal.SIGINT, signal.SIG_DFL)
os.execvp(sys.argv[1], sys.argv[1:])' \
     env "${auth_env[@]+"${auth_env[@]}"}" "${env_path[@]+"${env_path[@]}"}" harbor run -p "$task" -a "$agent" -m "$model" "${env_args[@]+"${env_args[@]}"}" \
     "${effort_args[@]+"${effort_args[@]}"}" "${extra_ak[@]+"${extra_ak[@]}"}" "${ak_args[@]+"${ak_args[@]}"}" "${task_ak[@]+"${task_ak[@]}"}" \
     -k "$attempts" -n 1 "${auth_args[@]+"${auth_args[@]}"}" -o "$OUT/jobs" --job-name "$job" -y -q) > "$OUT/$job.log" 2>&1 &
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
    bad=$(surface_hit "$OUT/jobs/$job")
    if [ -n "$bad" ]; then
      echo "!!! $t: tool-surface breach in $bad (codex.txt matches: $SURFACE_RE); stopping the run"
      write_status stopping "" "tool-surface:$bad"
      stop_harbor
      write_status infra_failed "$hcode" "tool-surface:$bad"
      echo "!!! move $OUT/jobs/$job aside as INFRA-..., fix scripts/computer-use/codex-gate.toml or the catalog, and rerun"; exit 1
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
  bad=$(surface_hit "$OUT/jobs/$job")
  [ -n "$bad" ] && { echo "!!! $t: tool-surface breach in $bad; not a gate result"; write_status infra_failed "$hcode" "tool-surface:$bad"; exit 1; }
  bad=$(infra_hit "$OUT/jobs/$job")
  [ -n "$bad" ] && { echo "!!! $t: infrastructure failure in $bad; not a gate result"; write_status infra_failed "$hcode" "infra:$bad"; exit 1; }
  print_trials "$OUT/jobs/$job"
  echo "--- $t: $(solves "$OUT/jobs/$job") solve(s) of $attempts $(date '+%F %H:%M:%S')"
  write_status done "$hcode"
done
echo "=== DONE $(date '+%F %H:%M:%S')"
