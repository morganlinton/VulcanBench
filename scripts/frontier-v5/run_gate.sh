#!/usr/bin/env bash
# Frontier v5 admission gate runner (docs/frontier-v5/GATE-LOG.md).
#
# usage: scripts/frontier-v5/run_gate.sh <ref> <task> [<task> ...]
#        scripts/frontier-v5/run_gate.sh --grid <ref> [<batch> ...]
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
#   sol62  harbor -a codex       -m gpt-6.2-sol       Codex CLI 0.166.0,   medium
#          (not a gate reference: the release 1 top-effort measurement of
#          decision R3, docs/DECISIONS.md 2026-11-19; Codex 0.159.0 refuses
#          gpt-6.2-sol, 0.166.0 is its Frontier v4 pin)
#   terra  harbor -a codex       -m gpt-6.1-terra     Codex CLI 0.159.0
#          (not a gate reference: the launch lineup's Terra, R9, on its
#          Frontier v4 pin, docs/DECISIONS.md 2026-10-26; first used for the
#          grid pilot's calibration runs)
#
# Release 1 board lineup (decision R9 plus the owner's Claude funding,
# docs/DECISIONS.md 2026-11-19; each on its lab's native harness and its
# Frontier v4 CLI pin, at its highest unblocked effort via GATE_EFFORT):
#
#   sol62    as above
#   fable52  harbor -a claude-code -m claude-fable-5-2  Claude Code 2.1.321
#            (its Frontier v4 pin, DECISIONS.md 2026-11-09)
#   terra    harbor -a codex       -m gpt-6.1-terra     Codex CLI 0.159.0
#            (its Frontier v4 pin, DECISIONS.md 2026-10-26)
#   dsflash  harbor -a harbor_dsh_agent:DeepSeekHarness -m deepseek-flash
#            DeepSeek Harness (dsh) 0.2.1-rc.2 (its Frontier v4 pin,
#            DECISIONS.md 2026-10-19). Harbor 0.24.0 has no dsh agent, so
#            scripts/frontier-v5/harbor_dsh_agent.py is loaded from harbor's
#            PYTHONPATH. dsh takes only low, high or max, so GATE_EFFORT must
#            name one; the agent phase may also reach api.deepseek.com
#            (--allow-agent-host). Web tools, session-log upload and
#            telemetry are off, as on Frontier v4.
#
# GATE_EFFORT (default medium) sets the reasoning effort: low, medium, high,
# xhigh or max. Levels in vulcanbench.toml's [effort].blocked ("ultra") are
# refused. Every gate admission ran at medium, so any other level is a
# measurement, not a gate result; its job name carries the level
# (<ref>-<effort>-<task>), and the === line and status file record it.
#
# Concurrency follows decision (e): Python and JavaScript tasks run two
# attempts at a time, compiled and JVM tasks one (16 GiB hosts).
# GATE_CONCURRENCY=1 overrides that for a task whose verifiers cannot share
# the Docker VM (prettier, docs/DECISIONS.md 2026-10-27); the value is
# printed on the === line.
# Early exit: two solves by the reference end a task's gate as a reject, so
# the Harbor job is stopped as soon as two trials have reward 1
# (GATE_EARLY_EXIT=0 turns that off, for confirmation runs that score every
# attempt, PLAN-v4.md section 4). Killing
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
# Safety refusals (Codex's cybersecurity flag, Claude Code's refusal fallback)
# are recorded, not infrastructure: a note per trial on stdout and
# safety_refusals in the status file (SAFETY_RE below).
# Infrastructure guard: a finished trial whose agent log shows an auth
# (including an organization policy that disables subscription access),
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
#   astra, sol62, terra  a dedicated ChatGPT login for gate runs, with no connected apps,
#          in ~/.config/vulcanbench/codex-gate/auth.json
#          (`CODEX_HOME=~/.config/vulcanbench/codex-gate codex login`), or the
#          file GATE_CODEX_AUTH names; without one, the personal login in
#          ~/.codex/auth.json. Harbor injects the file CODEX_AUTH_JSON_PATH
#          names, set in its own environment.
#   fable, fable52, opus  either a Claude subscription token in a private env file
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
#          The check fails closed: no claude CLI, or a probe with no
#          rate-limit event, refuses unless GATE_USAGE_OVERRIDE=1 (an
#          API-key run has no subscription windows and needs the override),
#          and a probe whose result is an error (auth, organization policy,
#          usage limit) refuses even with it. On 2026-11-03 an organization
#          that had disabled subscription access produced no event, the old
#          check let the run start, and 21 attempts scored fake zeros.
#   dsflash  a DeepSeek API key: one line DEEPSEEK_API_KEY=... in a private
#          env file (DSH_GATE_ENV, default ~/.config/vulcanbench/deepseek.env),
#          passed to harbor with --env-file; runs are metered API spend. No
#          usage pre-flight (the API has no subscription windows); an account
#          failure stops the run through the infrastructure guard.
#          GATE_PREFLIGHT_ONLY=1 runs just that check and prints one JSON
#          line (model, status, max_utilization, windows, would_start; status
#          "unknown" or "probe_error" with a note when it could not read the
#          windows); exit 0 if a run would start, 2 if it would be refused.
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
#
# Grid runs (PLAN-v6 section 4, docs/frontier-v5/GRID-ENGINE.md):
# `--grid <ref> [<batch> ...]` runs one budgeted agent session per grid
# environment, a batch of up to four environments at a time as one Harbor
# job (`-p <tasks dir> -i <env> ...`, one attempt each, all four
# concurrent), batches in the order and grouping of the grid file
# (GATE_GRID_FILE, default <tasks dir>/grid.json; GATE_TASKS_DIR, default
# tasks/frontier-v5), so every model meets the same contention. Batch
# numbers are 1-based; no number runs every batch. scripts/frontier-v5/
# grid_plan.py validates the file first (exit 2 on any problem).
# The budget is the grid file's budget_min (GATE_BUDGET_MIN overrides it
# for calibration runs; the === line and status file say which), applied
# as Harbor's --agent-timeout-multiplier over the environments' [agent]
# timeout_sec. At the budget Harbor cancels the agent, records
# AgentTimeoutError, collects the tree and still runs the verifier; each
# environment's freeze hook (a [[verifier.collect]] kill as root, checked by
# grid_plan.py) stops the agent's processes in the container before the
# tree is collected. The grid file's budget_instruction, when set, is
# appended to every instruction with --extra-instruction. A grid job is
# <ref>-<effort>-grid-b<n><suffix>; no early exit; the infrastructure guard
# stops the whole batch (rerun the whole batch, so contention stays equal),
# and the status file adds the batch, its environments, the budget, items
# done and total (from each reward.json) and budget_reached (trials Harbor
# stopped at the budget).
set -u
if [ "${1:-}" = --refs ]; then
  echo '{"name": "astra", "agent": "codex", "model": "gpt-6-astra", "version": "0.159.0"}'
  echo '{"name": "fable", "agent": "claude-code", "model": "claude-fable-5-1", "version": "2.1.302"}'
  echo '{"name": "opus", "agent": "claude-code", "model": "claude-opus-5-5", "version": "2.1.302"}'
  echo '{"name": "sol62", "agent": "codex", "model": "gpt-6.2-sol", "version": "0.166.0"}'
  echo '{"name": "fable52", "agent": "claude-code", "model": "claude-fable-5-2", "version": "2.1.321"}'
  echo '{"name": "terra", "agent": "codex", "model": "gpt-6.1-terra", "version": "0.159.0"}'
  echo '{"name": "dsflash", "agent": "dsh", "model": "deepseek-flash", "version": "0.2.1-rc.2"}'
  exit 0
fi
preflight_only="${GATE_PREFLIGHT_ONLY:-0}"
grid=0
[ "${1:-}" = --grid ] && { grid=1; shift; }
ref="${1:?usage: run_gate.sh [--grid] <astra|sol62|fable|opus|fable52|terra|dsflash> <task|batch> [...]}"; shift
[ $# -gt 0 ] || [ "$preflight_only" = 1 ] || [ "$grid" = 1 ] || { echo "run_gate: name at least one task" >&2; exit 2; }
here="$(cd "$(dirname "$0")/../.." && pwd)"
tasks_dir="${GATE_TASKS_DIR:-$here/tasks/frontier-v5}"
export PATH="$HOME/.local/bin:$HOME/.local/node/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
# Outside the repo by default: logs/ is not git-ignored here, and gate
# artifacts must never be committed.
OUT="${GATE_OUT:-$HOME/vulcanbench-gate}"
attempts="${GATE_ATTEMPTS:-3}"; suffix="${GATE_JOB_SUFFIX:-}"
case "$attempts" in 1|2|3) ;; *) echo "run_gate: GATE_ATTEMPTS must be 1, 2 or 3 (got $attempts)" >&2; exit 2 ;; esac
early_exit="${GATE_EARLY_EXIT:-1}"
case "$early_exit" in 0|1) ;; *) echo "run_gate: GATE_EARLY_EXIT must be 0 or 1 (got $early_exit)" >&2; exit 2 ;; esac
effort="${GATE_EFFORT:-medium}"
# Blocked levels never run on any model (vulcanbench.toml [effort].blocked,
# docs/DECISIONS.md 2026-09-16); the list is read from the operator file.
blocked=$(sed -n 's/^blocked *= *\[\(.*\)\]/\1/p' "$here/vulcanbench.toml" 2>/dev/null | tr -d '" ' | tr ',' ' ')
for b in $blocked ultra; do
  [ "$effort" = "$b" ] && { echo "run_gate: effort $effort is blocked (vulcanbench.toml [effort].blocked)" >&2; exit 2; }
done
case "$effort" in low|medium|high|xhigh|max) ;; *) echo "run_gate: GATE_EFFORT must be low, medium, high, xhigh or max (got $effort)" >&2; exit 2 ;; esac
mkdir -p "$OUT/jobs"
command -v harbor >/dev/null || { echo "run_gate: harbor not found (scripts/frontier-v5/setup-host.sh installs it)" >&2; exit 2; }

# What harbor's -a gets: the agent name, or an import path for a custom agent.
agent_arg=""; harbor_env=(); extra_args=()
case "$ref" in
  astra|sol62|terra)
    agent=codex
    case "$ref" in
      astra) model=gpt-6-astra; version=0.159.0 ;;
      sol62) model=gpt-6.2-sol; version=0.166.0 ;;
      terra) model=gpt-6.1-terra; version=0.159.0 ;;
    esac
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
  fable|opus|fable52)
    agent=claude-code; version=2.1.302
    case "$ref" in
      fable) model=claude-fable-5-1 ;;
      opus) model=claude-opus-5-5 ;;
      fable52) model=claude-fable-5-2; version=2.1.321 ;;
    esac
    envf="${CLAUDE_GATE_ENV:-$HOME/.config/vulcanbench/claude-gate.env}"
    [ -f "$envf" ] || { echo "run_gate: $envf missing (see the header for how to create it)" >&2; exit 2; }
    auth_args=(--env-file "$envf")
    if [ "${FABLE_AUTH:-oauth}" = api ]; then auth_env=(); else auth_env=(CLAUDE_FORCE_OAUTH=1); fi ;;
  dsflash)
    agent=dsh; model=deepseek-flash; version=0.2.1-rc.2
    # dsh's DeepSeek route takes low, high or max only (no medium, no xhigh).
    case "$effort" in low|high|max) ;; *) echo "run_gate: dsh takes GATE_EFFORT low, high or max (got $effort)" >&2; exit 2 ;; esac
    agent_arg="harbor_dsh_agent:DeepSeekHarness"
    [ -f "$here/scripts/frontier-v5/harbor_dsh_agent.py" ] || { echo "run_gate: scripts/frontier-v5/harbor_dsh_agent.py missing" >&2; exit 2; }
    harbor_env=(PYTHONPATH="$here/scripts/frontier-v5${PYTHONPATH:+:$PYTHONPATH}")
    envf="${DSH_GATE_ENV:-$HOME/.config/vulcanbench/deepseek.env}"
    [ -f "$envf" ] || { echo "run_gate: $envf missing (one line DEEPSEEK_API_KEY=...)" >&2; exit 2; }
    auth_args=(--env-file "$envf")
    auth_env=()
    extra_args=(--allow-agent-host api.deepseek.com) ;;
  *) echo "run_gate: reference must be astra, sol62, fable, opus, fable52, terra or dsflash" >&2; exit 2 ;;
esac
agent_arg="${agent_arg:-$agent}"

# Exact error texts seen in agent logs (2026-10-17/18) plus Codex's usage-limit
# message; kept narrow so ordinary output (code the agent reads) never matches.
# Claude Code's own failures are matched on its structured fields, which sit
# unescaped only in its top-level stream-json lines (the same text inside a
# tool result is JSON-escaped and does not match): the synthetic assistant
# message's "error" (authentication_failed, billing_error, rate_limit, and
# oauth_org_not_allowed, the organization-disabled subscription of
# 2026-11-03) and the result line's api_error_status 401 or 403; plus the
# literal texts Claude Code prints for those cases.
# The next three alternatives are a reference reaching a provider-side tool
# (web search, the account's connected apps) that run_gate.sh turns off; such
# a trial is not a gate result either (docs/DECISIONS.md 2026-10-31).
# The last alternative is dsh: harbor_dsh_agent.py reads dsh's own records
# after the run and writes a VULCANBENCH_INFRA: line to vb-infra.txt for an
# error event with an account or API code (AUTH, QUOTA, RATE_LIMIT, SERVER,
# TRANSPORT, MISSING_CREDENTIAL, the codes the harness adapter treats as
# infrastructure), a plugin that did not activate, or a finished run with no
# usage receipt. Matching the marker rather than dsh's error line keeps the
# pattern off Codex's own "error" events. Every agent log line is JSON, so
# no other record starts with the marker.
INFRA_RE='"error":"(authentication_failed|billing_error|rate_limit|oauth_org_not_allowed)"|"api_error_status":40[13][,}]|disabled Claude subscription access|Invalid API key[^"]{1,8}Please run /login|OAuth token has expired[.] Please obtain a new token|OAuth token revoked|Credit balance is too low|OAuth access token is invalid|issue with the selected model|requires usage credits|hit your usage limit|"server": ?"codex_apps"|tools[.]mcp__codex_apps__|tools[.]web__run[(]|^VULCANBENCH_INFRA: '

# A provider safety classifier stopping the session is NOT infrastructure:
# such an attempt counts, graded on what the agent wrote, and the stop is
# disclosed (docs/DECISIONS.md 2026-09-24, the Claude refusal-fallback
# convention, applied again for Haiku v3.24). It is recorded, never matched by
# INFRA_RE: Codex's top-level error event "flagged for possible cybersecurity
# risk" (Harbor raises AgentSafetyRefusalError for it, kept in the trial's
# result.json), and Claude Code's refusal-fallback events (subtype
# model_refusal_fallback, or model_refusal_no_fallback with the fallback off)
# and its "safeguards flagged this session" stop. The same text inside a tool
# result is JSON-escaped or mid-line and does not match.
SAFETY_RE='^[{]"type": ?"(error|turn[.]failed)".*flagged for possible cybersecurity risk|"subtype": ?"model_refusal_(no_)?fallback"|^[{]"type": ?"result".*safeguards flagged this session'

# Per-task state the helpers below read; set in the task loop. In grid mode
# t is the batch label (grid-b<n>), job_tasks its environments.
job=""; t=""; status_file=""; hpid=""; hcode=""; started_at=""
stop_flag=""; spid=""; grid_meta=""; job_tasks=()

safety_trials() { # job dir -> one line per trial a provider safety classifier stopped
  local tr
  for tr in "$1"/*__*; do
    [ -d "$tr" ] || continue
    if grep -qs '"exception_type": *"AgentSafetyRefusalError"' "$tr/result.json" \
      || { [ -d "$tr/agent" ] && grep -rqE "$SAFETY_RE" "$tr/agent" 2>/dev/null; }; then
      basename "$tr"
    fi
  done
}

write_status() { # phase [exit_code] [stop_reason] -> atomic rewrite of $status_file
  [ -n "$status_file" ] || return 0
  python3 - "$status_file" "$1" "${2:-}" "${3:-}" "$job" "$t" "$ref" "$agent" "$model" \
    "$$" "$hpid" "$started_at" "$(git -C "$here" rev-parse --short HEAD 2>/dev/null)" "$OUT/jobs/$job" "$effort" \
    "$( [ -n "$job" ] && safety_trials "$OUT/jobs/$job" | tr '\n' ' ')" "$grid_meta" <<'PY'
import glob, json, os, sys, time
(path, phase, code, reason, job, task, ref, agent, model,
 pid, hpid, started, repo, job_dir, effort, safety, grid_meta) = sys.argv[1:18]
solves = 0
items_done = items_total = budget_reached = 0
for p in glob.glob(job_dir + "/*__*/verifier/reward.json"):
    try:
        r = json.load(open(p))
        solves += r.get("reward") == 1
        items_done += int(r.get("items_done") or 0)
        items_total += int(r.get("items_total") or 0)
    except Exception:
        pass
for p in glob.glob(job_dir + "/*__*/result.json"):
    try:
        e = json.load(open(p)).get("exception_info") or {}
        budget_reached += e.get("exception_type") == "AgentTimeoutError"
    except Exception:
        pass
d = {
    "job": job, "task": task, "ref": ref, "agent": agent, "model": model,
    "effort": effort, "pid": int(pid), "harbor_pid": int(hpid) if hpid else None,
    "phase": phase, "solves": solves, "started_at": started,
    "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "exit_code": int(code) if code else None, "stop_reason": reason or None,
    "repo": repo or None, "job_dir": job_dir,
    "safety_refusals": len(safety.split()), "safety_refusal_trials": safety.split(),
}
if grid_meta:
    d.update(json.loads(grid_meta))
    d.update(items_done=items_done, items_total=items_total, budget_reached=budget_reached)
    del d["solves"]
tmp = path + ".tmp"
with open(tmp, "w") as fh:
    json.dump(d, fh, indent=1)
os.replace(tmp, path)
PY
}

usage_preflight() { # claude-code refs: one one-turn call reads the subscription windows; refuse above GATE_USAGE_MAX
  # Fails closed: a probe that shows no
  # rate-limit event proves nothing about the account (on 2026-11-03 a
  # disabled subscription produced none, and 21 attempts scored fake zeros),
  # so it refuses unless GATE_USAGE_OVERRIDE=1. A probe whose result is an
  # error (auth, organization policy, usage limit) refuses even with the
  # override: no attempt could make a model call.
  local max="${GATE_USAGE_MAX:-0.85}" out rl_status rl_util rl_windows probe_err verdict
  if ! command -v claude >/dev/null; then
    usage_unknown "no claude CLI on PATH"; return 0
  fi
  out=$( ( set -a; . "$envf"; set +a; claude -p --model "$model" --output-format stream-json --verbose --max-turns 1 'Reply with the single word ok.' 2>/dev/null ) \
    | python3 -c '
import json, sys
rl = None; err = ""
for line in sys.stdin:
    try:
        d = json.loads(line)
    except ValueError:
        continue
    if not isinstance(d, dict):
        continue
    if d.get("type") == "rate_limit_event":
        rl = d.get("rate_limit_info") or {}
    elif d.get("type") == "assistant" and d.get("error") and not err:
        err = str(d["error"])
    elif d.get("type") == "result" and d.get("is_error"):
        err = " ".join(str(x) for x in (d.get("api_error_status"), d.get("api_error_code"), d.get("result")) if x)
err = " ".join(err.replace("|", "/").split())[:300]
if rl is None:
    print("|||" + err)
else:
    w = rl.get("unifiedWindows") or {}
    m = max([v.get("utilization") or 0 for v in w.values()] or [0])
    print("%s|%.2f|%s|%s" % (rl.get("status"), m, " ".join("%s=%.2f" % (k, v.get("utilization") or 0) for k, v in w.items()), err))' )
  IFS='|' read -r rl_status rl_util rl_windows probe_err <<EOF
$out
EOF
  if [ -n "$probe_err" ]; then
    if [ "$preflight_only" = 1 ]; then
      python3 -c 'import json, sys; print(json.dumps({"model": sys.argv[1], "status": "probe_error", "would_start": False, "note": sys.argv[2]}))' "$model" "$probe_err"
      exit 2
    fi
    echo "!!! usage preflight $model: the probe call failed ($probe_err); not starting. GATE_USAGE_OVERRIDE does not cover this: fix the credential." >&2
    write_status infra_failed 2 probe-error
    exit 2
  fi
  if [ -z "$rl_status" ]; then
    usage_unknown "no rate-limit event in the probe"; return 0
  fi
  out="$rl_status $rl_util $rl_windows"
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

usage_unknown() { # note -> the probe could not read the usage windows: refuse unless GATE_USAGE_OVERRIDE=1
  local start=false
  [ "${GATE_USAGE_OVERRIDE:-0}" = 1 ] && start=true
  if [ "$preflight_only" = 1 ]; then
    printf '{"model": "%s", "status": "unknown", "would_start": %s, "note": "%s"}\n' "$model" "$start" "$1"
    [ "$start" = true ] && exit 0 || exit 2
  fi
  if [ "$start" = true ]; then
    echo "run_gate: usage preflight: $1; starting anyway (GATE_USAGE_OVERRIDE=1)" >&2
    return 0
  fi
  echo "!!! usage preflight $model: $1; not starting. Check the credential, or set GATE_USAGE_OVERRIDE=1 to run anyway." >&2
  write_status infra_failed 2 usage-unknown
  exit 2
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

stop_job_sandboxes() { # every task of the current job
  local x
  for x in "${job_tasks[@]}"; do stop_sandboxes "$x"; done
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
  stop_job_sandboxes
}

print_trials() { # job dir -> one summary line per trial
  local tr
  for tr in "$1"/*__*; do
    [ -d "$tr" ] || continue
    echo "  $(basename "$tr"): $(cat "$tr/verifier/reward.json" 2>/dev/null || echo 'stopped, no reward')"
  done
  for tr in $(safety_trials "$1"); do
    echo "  $tr: safety refusal: a provider safety classifier stopped the session; the attempt counts as graded and the stop is disclosed"
  done
}

on_signal() { # record the signal; the monitor loop acts on it after its current step
  stop_flag=$1
  [ -n "$spid" ] && kill "$spid" 2>/dev/null
  return 0
}
trap 'on_signal INT' INT
trap 'on_signal TERM' TERM

run_job() { # start harbor with harbor_cmd, monitor it; sets hcode and stopped; exits on a signal or infra
  # A background list runs with SIGINT ignored (bash 3.2 has no `trap - INT`
  # that survives the fork), so harbor is exec'd through a python one-liner
  # that restores the default disposition. $! is then harbor's own pid.
  (cd "$here" && exec python3 -c 'import os, signal, sys
signal.signal(signal.SIGINT, signal.SIG_DFL)
os.execvp(sys.argv[1], sys.argv[1:])' \
     env "${harbor_env[@]+"${harbor_env[@]}"}" "${auth_env[@]+"${auth_env[@]}"}" "${harbor_cmd[@]}") > "$OUT/$job.log" 2>&1 &
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
    if [ "$job_early_exit" = 1 ] && [ "$(solves "$OUT/jobs/$job")" -ge 2 ]; then
      echo "--- $t: two solves, early reject; stopping the remaining attempt"
      write_status stopping "" early-reject
      stop_harbor; stopped=1
      trap 'on_signal INT' INT; trap 'on_signal TERM' TERM
    fi
  done
  if [ -z "$hcode" ]; then wait "$hpid" 2>/dev/null; hcode=$?; fi
  stop_job_sandboxes
  if [ -z "$stopped" ]; then
    bad=$(infra_hit "$OUT/jobs/$job")
    if [ -n "$bad" ]; then
      echo "!!! $t: infrastructure failure in $bad; not a ${grid_word} result"
      write_status infra_failed "$hcode" "infra:$bad"; exit 1
    fi
  fi
}

begin_job() { # after job, status_file, grid_meta: refuse an existing job dir, preflight, honour a pending stop
  [ -e "$OUT/jobs/$job" ] && { echo "run_gate: $OUT/jobs/$job exists; move it aside first" >&2; exit 2; }
  hpid=""; hcode=""; started_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  write_status preflight
  [ "$agent" = claude-code ] && usage_preflight
  if [ -n "$stop_flag" ]; then
    echo "!!! $t: stop requested (SIG$stop_flag) before harbor started"
    write_status stopped 130 "signal-$stop_flag"; exit 130
  fi
}

if [ "$grid" = 1 ]; then
  grid_word=grid
  grid_file="${GATE_GRID_FILE:-$tasks_dir/grid.json}"
  for b in "$@"; do
    case "$b" in *[!0-9]*|"") echo "run_gate: grid batches are numbers (got $b)" >&2; exit 2 ;; esac
  done
  budget_args=()
  [ -n "${GATE_BUDGET_MIN:-}" ] && budget_args=(--budget-min "$GATE_BUDGET_MIN")
  # Batch numbers go before the options: on Python 3.9 to 3.11 argparse
  # rejects a positional that follows an option ("unrecognized arguments").
  plan=$(python3 "$here/scripts/frontier-v5/grid_plan.py" "$grid_file" "$tasks_dir" \
    "$@" "${budget_args[@]+"${budget_args[@]}"}") || exit 2
  note=$(python3 "$here/scripts/frontier-v5/grid_plan.py" "$grid_file" "$tasks_dir" \
    "${budget_args[@]+"${budget_args[@]}"}" --instruction) || exit 2
  note_args=()
  [ -n "$note" ] && note_args=(--extra-instruction "$note")
  job_early_exit=0
  # The plan is read on fd 3 so nothing in the loop (the claude probe,
  # harbor) can consume it from stdin.
  while IFS="$(printf '\t')" read -r idx budget_sec budget_source mult envs_line <&3; do
    [ -n "$idx" ] || continue
    envs_line=$(printf '%s' "$envs_line" | tr '\t' ' ')
    job_tasks=($envs_line)
    t="grid-b$idx"
    job="$ref-$effort-grid-b$idx$suffix"
    status_file="$OUT/$job.status.json"
    grid_meta=$(python3 -c 'import json, sys
print(json.dumps({"mode": "grid", "batch": int(sys.argv[1]), "environments": sys.argv[2].split(),
                  "budget_sec": float(sys.argv[3]), "budget_source": sys.argv[4],
                  "agent_timeout_multiplier": float(sys.argv[5]), "grid_file": sys.argv[6],
                  "budget_instruction": bool(sys.argv[7])}))' \
      "$idx" "$envs_line" "$budget_sec" "$budget_source" "$mult" "$grid_file" "$note")
    begin_job
    echo "=== $t $(date '+%F %H:%M:%S') ref=$ref agent=$agent model=$model version=$version effort=$effort environments=$envs_line budget=${budget_sec}s ($budget_source) multiplier=$mult concurrency=${#job_tasks[@]} harbor=$(harbor --version 2>/dev/null) repo=$(git -C "$here" rev-parse --short HEAD)"
    include=()
    for e in "${job_tasks[@]}"; do include+=(-i "$e"); done
    harbor_cmd=(harbor run -p "$tasks_dir" "${include[@]}" -a "$agent_arg" -m "$model" --effort "$effort"
      --ak version="$version" --ak disable_web_search=true -k 1 -n "${#job_tasks[@]}"
      --agent-timeout-multiplier "$mult" "${note_args[@]+"${note_args[@]}"}"
      "${auth_args[@]+"${auth_args[@]}"}" "${extra_args[@]+"${extra_args[@]}"}"
      -o "$OUT/jobs" --job-name "$job" -y -q)
    run_job
    print_trials "$OUT/jobs/$job"
    write_status done "$hcode"
    echo "--- $t: $(python3 -c 'import json, sys; d = json.load(open(sys.argv[1])); print("%d of %d items, %d at the budget" % (d["items_done"], d["items_total"], d["budget_reached"]))' "$status_file" 2>/dev/null) $(date '+%F %H:%M:%S')"
  done 3<<EOF
$plan
EOF
  echo "=== DONE $(date '+%F %H:%M:%S')"
  exit 0
fi

grid_word=gate
for t in "$@"; do
  task="$tasks_dir/$t"
  [ -f "$task/task.toml" ] || { echo "run_gate: no task $t" >&2; exit 2; }
  lang=$(sed -n 's/^vulcanbench_language = "\(.*\)"/\1/p' "$task/task.toml")
  case "$lang" in python|javascript) n=2 ;; *) n=1 ;; esac
  case "${GATE_CONCURRENCY:-}" in "") ;; 1|2) n=$GATE_CONCURRENCY ;; *) echo "run_gate: GATE_CONCURRENCY must be 1 or 2" >&2; exit 2 ;; esac
  if [ "$effort" = medium ]; then job="$ref-$t$suffix"; else job="$ref-$effort-$t$suffix"; fi
  status_file="$OUT/$job.status.json"; job_tasks=("$t")
  begin_job
  echo "=== $t $(date '+%F %H:%M:%S') ref=$ref agent=$agent model=$model version=$version effort=$effort attempts=$attempts concurrency=$n harbor=$(harbor --version 2>/dev/null) repo=$(git -C "$here" rev-parse --short HEAD)"
  [ "$early_exit" = 0 ] && echo "=== early exit off (GATE_EARLY_EXIT=0): every attempt runs to the end"
  job_early_exit=$early_exit
  harbor_cmd=(harbor run -p "$task" -a "$agent_arg" -m "$model" --effort "$effort"
    --ak version="$version" --ak disable_web_search=true -k "$attempts" -n "$n" "${auth_args[@]+"${auth_args[@]}"}"
    "${extra_args[@]+"${extra_args[@]}"}"
    -o "$OUT/jobs" --job-name "$job" -y -q)
  run_job
  print_trials "$OUT/jobs/$job"
  echo "--- $t: $(solves "$OUT/jobs/$job") solve(s) $(date '+%F %H:%M:%S')"
  write_status done "$hcode" "${stopped:+early-reject}"
done
echo "=== DONE $(date '+%F %H:%M:%S')"
