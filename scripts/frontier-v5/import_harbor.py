#!/usr/bin/env python3
"""Import Harbor jobs from run_gate.sh into Frontier v5 per-run records.

usage: scripts/frontier-v5/import_harbor.py --out DIR [--bases DIR] [--repo DIR]
                                            [--include-infra] <job-dir> [...]

Every trial of every named Harbor job (``$GATE_OUT/jobs/<job>``, written by
``scripts/frontier-v5/run_gate.sh``) becomes one run directory,
``<out>/<job>/<trial>/``, holding:

  run.json            the per-run record (schema ``vulcanbench-frontier-v5-run/1``)
  checks.json         every held-out and guard id with its status (progress_score.py)
  tool-calls.jsonl    the run's tool calls in the shared schema
                      (``harness/agent/tool_calls.py``, owner rule 2026-10-29)
  public-tool-calls.jsonl
                      the same calls as public trace events: names, kinds,
                      timing and outcomes, never inputs or outputs
  patch.diff          the agent's change to the graded artifact, when the
                      task's base tree is given (``--bases``)

The record carries: the job, trial, reference, agent and CLI version, model,
effort and attempt number; the job's concurrency (only serial runs count
toward speed); agent minutes (Harbor's agent-execution phase); tokens and the
API-equivalent cost at list price; the verifier's reward and the progress
score with its per-check summary; the patch summary; the tool-call summary;
and whether the trial is scored.

A trial is **scored** when its verifier wrote a reward and nothing marks it
as infrastructure. Infrastructure is the ``VULCANBENCH_INFRA:`` marker the
dsh agent writes, an exception Harbor raises for the account or the model id
(authentication, usage limit, model not found), a trial whose verifier never
ran, or a job set aside as ``INFRA-*`` (skipped unless ``--include-infra``).
Other exceptions are recorded (``exception``), never hidden: an agent timeout
is a scored attempt. So is a session a provider safety classifier stopped
(docs/DECISIONS.md 2026-09-24): it is graded on what the agent wrote and
disclosed, with ``safety_refusal: true``, the provider's message
(``safety_message``) and the ``safety-refusal`` flag. Detected from Harbor's
``AgentSafetyRefusalError``, Codex's "flagged for possible cybersecurity
risk" error event, and Claude Code's refusal-fallback events and
"safeguards flagged this session" stop.

Tool calls are read from each harness's own record. Codex: the ``exec
--json`` stream (``codex.txt``), timed from the rollout's ``item_completed``
events, whose tool items are in the same order (zipped only when the item
kinds line up one for one). Claude Code: the ``stream-json`` output
(``claude-code.txt``), timed from the session logs by message uuid. dsh: the
session logs under ``dsh-session-logs/`` (``harbor_dsh_agent.py``).

Costs are API-equivalent at list price, the way the board has always priced
columns: Codex and dsh from Harbor's token totals with ``harness.pricing``
(cache reads at the cached rate; the long-context tier is not inferred);
Claude Code from its final ``modelUsage`` with every cache write at the
5-minute rate. Harbor's own figure is kept beside it (``harness_reported_usd``).

``--bases DIR`` holds each task's pristine artifact trees at
``DIR/<task>/<artifact source path>`` (for tree-sitter,
``DIR/treesitter-query-anchor-matching/app/lib/src``), copied once from the
task's agent image. Without them ``patch`` records why it is missing.
"""

from __future__ import annotations

import argparse
import difflib
import json
import statistics
import sys
import tempfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REPO))

import progress_score  # noqa: E402

from harness.agent import tool_calls  # noqa: E402
from harness.pricing import cost_usd  # noqa: E402

SCHEMA = "vulcanbench-frontier-v5-run/1"
#: Harbor agent name -> (tool_calls harness id, agent log, price prefix)
AGENTS = {
    "codex": ("codex", "codex.txt", "openai"),
    "claude-code": ("claude-code", "claude-code.txt", "anthropic"),
    "dsh": ("dsh", "dsh.txt", "deepseek"),
}
#: USD per million tokens: uncached input, cache read, 5m cache write, output.
CLAUDE_RATES = {
    "claude-fable-5-2": (8.0, 0.2, 10.0, 40.0),
    "claude-fable-5-1": (10.0, 0.25, 12.5, 50.0),
    "claude-opus-5-5": (4.0, 0.2, 5.0, 20.0),
}
INFRA_EXCEPTIONS = {
    "AgentAuthenticationError",
    "ApiUsageLimitError",
    "ModelNotFoundError",
    "NetworkConnectionError",
}
SAFETY_EXCEPTION = "AgentSafetyRefusalError"
INFRA_MARK = "VULCANBENCH_INFRA:"
SKIP_DIRS = {"__pycache__", "node_modules", "target", ".git"}


def _load(path: Path) -> dict[str, Any]:
    try:
        d = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}
    return d if isinstance(d, dict) else {}


def _ts(value: Any) -> datetime | None:
    if not isinstance(value, str):
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def _minutes(phase: Any) -> float | None:
    if not isinstance(phase, dict):
        return None
    a, b = _ts(phase.get("started_at")), _ts(phase.get("finished_at"))
    if a is None or b is None:
        return None
    return round((b - a).total_seconds() / 60, 3)


def _jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return rows
    for line in lines:
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows


# --- tool calls ----------------------------------------------------------------

_CODEX_KINDS = {
    "CommandExecution": "command_execution",
    "FileChange": "file_change",
    "McpToolCall": "mcp_tool_call",
    "WebSearch": "web_search",
}


def _iso_ms(ms: Any) -> str | None:
    if not isinstance(ms, (int, float)) or isinstance(ms, bool):
        return None
    return datetime.fromtimestamp(ms / 1000, UTC).isoformat()


def codex_trace(agent_dir: Path) -> list[dict[str, Any]]:
    """Synthetic trace events timing Codex tool items from the rollout.

    The exec stream's tool items and the rollout's ``item_completed`` tool
    items are the same calls in the same order, under different ids. They are
    zipped only when the sequences of kinds match one for one; otherwise no
    times are given (the calls are still recorded, untimed).
    """
    stream = []
    for ev in _jsonl(agent_dir / "codex.txt"):
        item = ev.get("item") or {}
        if ev.get("type") == "item.completed" and item.get("type") in _CODEX_KINDS.values():
            stream.append((item.get("id"), item["type"]))
    rollout = []
    for path in sorted(agent_dir.glob("sessions/**/rollout-*.jsonl")):
        for ev in _jsonl(path):
            p = ev.get("payload") or {}
            if ev.get("type") == "event_msg" and p.get("type") == "item_completed":
                kind = _CODEX_KINDS.get(str((p.get("item") or {}).get("type")))
                if kind:
                    rollout.append((kind, p.get("started_at_ms"), p.get("completed_at_ms")))
    if not stream or [k for _, k in stream] != [k for k, _, _ in rollout]:
        return []
    trace = []
    for (item_id, kind), (_, start, end) in zip(stream, rollout, strict=True):
        if _iso_ms(start):
            trace.append(
                {"type": "tool_call", "ts": _iso_ms(start), "data": {"id": item_id, "tool": kind}}
            )
        if _iso_ms(end):
            trace.append(
                {
                    "type": "tool_observation",
                    "ts": _iso_ms(end),
                    "data": {"id": item_id, "tool": kind},
                }
            )
    return trace


def claude_stream(agent_dir: Path) -> list[dict[str, Any]]:
    """Claude Code's stream-json events, each given its session-log timestamp by uuid."""
    stamps: dict[str, str] = {}
    for path in sorted(agent_dir.glob("sessions/projects/**/*.jsonl")):
        for ev in _jsonl(path):
            if ev.get("uuid") and ev.get("timestamp"):
                stamps[str(ev["uuid"])] = str(ev["timestamp"])
    out = []
    for ev in _jsonl(agent_dir / "claude-code.txt"):
        stamp = stamps.get(str(ev.get("uuid")))
        out.append({**ev, "timestamp": stamp} if stamp and "timestamp" not in ev else ev)
    return out


def extract_tool_calls(agent_dir: Path, agent: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Normalized tool calls and their summary block for one trial's agent directory."""
    harness = AGENTS[agent][0]
    with tempfile.TemporaryDirectory(prefix="vb-v5-calls-") as tmp:
        stage = Path(tmp)
        if agent == "codex":
            src = agent_dir / "codex.txt"
            if src.is_file():
                (stage / tool_calls.STREAM).write_bytes(src.read_bytes())
            trace = codex_trace(agent_dir)
            (stage / tool_calls.TRACE).write_text("".join(json.dumps(e) + "\n" for e in trace))
        elif agent == "claude-code":
            if (agent_dir / "claude-code.txt").is_file():
                rows = claude_stream(agent_dir)
                (stage / tool_calls.STREAM).write_text("".join(json.dumps(e) + "\n" for e in rows))
        else:
            stage = agent_dir
        try:
            calls = tool_calls.extract(stage, harness)
        except tool_calls.CaptureUnavailable as exc:
            return [], {
                "schema": tool_calls.SCHEMA,
                "harness": harness,
                "capture": "unavailable",
                "reason": str(exc),
            }
    return calls, tool_calls.summary_block(calls, harness)


# --- safety refusals --------------------------------------------------------------


def safety_refusal(agent_dir: Path, agent: str, exception: str | None) -> str | None:
    """The provider's message when a safety classifier stopped the session, else None.

    Only the harness's own top-level events count (a tool result quoting the
    same words is not a refusal). Harbor's exception alone gives a generic
    message.
    """
    if agent == "codex":
        for ev in _jsonl(agent_dir / "codex.txt"):
            msg = ev.get("message") if ev.get("type") == "error" else None
            if isinstance(msg, str) and "flagged for possible cybersecurity risk" in msg:
                return msg
    elif agent == "claude-code":
        for ev in _jsonl(agent_dir / "claude-code.txt"):
            sub = ev.get("subtype")
            if sub in ("model_refusal_fallback", "model_refusal_no_fallback"):
                to = ev.get("fallback_model") or ev.get("model")
                return f"Claude Code {sub}" + (f" to {to}" if to else "")
            res = ev.get("result")
            if ev.get("type") == "result" and isinstance(res, str) and "safeguards flagged" in res:
                return res
    if exception == SAFETY_EXCEPTION:
        return "Harbor AgentSafetyRefusalError (provider safety classifier)"
    return None


# --- tokens and cost -------------------------------------------------------------


def claude_usage(agent_dir: Path) -> dict[str, dict[str, int]] | None:
    """Per-model usage from Claude Code's final result event (cumulative for the session)."""
    final = None
    for ev in _jsonl(agent_dir / "claude-code.txt"):
        if ev.get("type") == "result" and isinstance(ev.get("modelUsage"), dict):
            final = ev["modelUsage"]
    return final


def economics(
    agent: str, model: str, agent_dir: Path, result: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    ar = result.get("agent_result") or {}
    tokens = {
        "input": ar.get("n_input_tokens"),
        "cached_input": ar.get("n_cache_tokens"),
        "output": ar.get("n_output_tokens"),
    }
    cost: dict[str, Any] = {"harness_reported_usd": ar.get("cost_usd"), "api_equivalent_usd": None}
    if agent == "claude-code":
        usage = claude_usage(agent_dir)
        if usage:
            total, writes, unpriced = 0.0, 0, []
            for name, u in usage.items():
                rates = CLAUDE_RATES.get(name)
                if rates is None:
                    unpriced.append(name)
                    continue
                ip, cr, w5, op = rates
                w = int(u.get("cacheCreationInputTokens") or 0)
                writes += w
                total += (
                    int(u.get("inputTokens") or 0) * ip
                    + int(u.get("cacheReadInputTokens") or 0) * cr
                    + w * w5
                    + int(u.get("outputTokens") or 0) * op
                ) / 1e6
            tokens["cache_write"] = writes
            cost.update(
                api_equivalent_usd=None if unpriced else round(total, 6),
                basis="list price; Claude Code modelUsage; cache writes at the 5-minute rate",
                **({"unpriced_models": unpriced} if unpriced else {}),
            )
        return tokens, cost
    prefix = AGENTS[agent][2]
    if None not in (tokens["input"], tokens["output"]):
        priced = cost_usd(
            f"{prefix}:{model}",
            int(tokens["input"] or 0),
            int(tokens["output"] or 0),
            cached_input_tokens=int(tokens["cached_input"] or 0),
        )
        cost.update(
            api_equivalent_usd=None if priced is None else round(priced, 6),
            basis="list price; harness.pricing on Harbor's token totals; cache reads at the cached rate",
        )
    return tokens, cost


# --- patch ---------------------------------------------------------------------


def _files(root: Path) -> dict[str, Path]:
    out: dict[str, Path] = {}
    if not root.is_dir():
        return out
    for p in root.rglob("*"):
        if p.is_file() and not (set(p.relative_to(root).parts) & SKIP_DIRS):
            out[p.relative_to(root).as_posix()] = p
    return out


def tree_diff(base: Path, new: Path, label: str) -> tuple[str, dict[str, int]]:
    """Unified diff of two trees, paths prefixed a/<label>/ and b/<label>/."""
    a, b = _files(base), _files(new)
    chunks: list[str] = []
    stats = {"files_changed": 0, "lines_added": 0, "lines_removed": 0}
    for rel in sorted(set(a) | set(b)):
        old = a[rel].read_bytes() if rel in a else b""
        cur = b[rel].read_bytes() if rel in b else b""
        if old == cur:
            continue
        stats["files_changed"] += 1
        an = f"a/{label}/{rel}" if rel in a else "/dev/null"
        bn = f"b/{label}/{rel}" if rel in b else "/dev/null"
        try:
            ot, nt = old.decode(), cur.decode()
        except UnicodeDecodeError:
            chunks.append(f"Binary files {an} and {bn} differ\n")
            continue
        lines = list(difflib.unified_diff(ot.splitlines(True), nt.splitlines(True), an, bn))
        for line in lines[2:]:
            if line.startswith("+"):
                stats["lines_added"] += 1
            elif line.startswith("-"):
                stats["lines_removed"] += 1
        chunks.append(f"diff --vb {an} {bn}\n")
        chunks += [
            ln if ln.endswith("\n") else ln + "\n\\ No newline at end of file\n" for ln in lines
        ]
    return "".join(chunks), stats


def build_patch(trial: Path, task: str, bases: Path | None) -> tuple[str | None, dict[str, Any]]:
    manifest = (
        json.loads((trial / "artifacts" / "manifest.json").read_text())
        if (trial / "artifacts" / "manifest.json").is_file()
        else []
    )
    sources = [
        m
        for m in manifest
        if m.get("status") == "ok" and not str(m.get("source", "")).startswith("/logs/")
    ]
    if not sources:
        return None, {"missing": "no graded artifact was collected"}
    if bases is None:
        return None, {"missing": "no base trees given (--bases)"}
    parts, total = [], {"files_changed": 0, "lines_added": 0, "lines_removed": 0}
    for m in sources:
        rel = str(m["source"]).lstrip("/")
        base = bases / task / rel
        if not base.is_dir():
            return None, {"missing": f"no base tree at {base}"}
        art = trial / str(m["destination"])
        text, st = tree_diff(base, art, rel)
        parts.append(text)
        for k in total:
            total[k] += st[k]
    return "".join(parts), {"file": "patch.diff", **total}


# --- one trial -------------------------------------------------------------------


def ref_of(job: str, task: str, effort: str | None) -> str:
    cut = job.find("-" + task) if task else -1
    name = job[:cut] if cut > 0 else job.split("-", 1)[0]
    if effort and name.endswith("-" + effort):
        name = name[: -len(effort) - 1]
    return name


def import_trial(
    trial: Path, job_cfg: dict[str, Any], out: Path, repo: Path, bases: Path | None, attempt: int
) -> dict[str, Any]:
    result = _load(trial / "result.json")
    cfg = _load(trial / "config.json")
    agent_cfg = cfg.get("agent") or {}
    kwargs = agent_cfg.get("kwargs") or {}
    agent = (result.get("agent_info") or {}).get("name") or agent_cfg.get("name") or ""
    agent = "dsh" if "harbor_dsh_agent" in str(agent) else agent
    model = agent_cfg.get("model_name") or ""
    task = progress_score.task_from_trial(trial) or ""
    effort = kwargs.get("reasoning_effort")
    job = job_cfg.get("job_name") or trial.parent.name
    agent_dir = trial / "agent"
    exc = (result.get("exception_info") or {}).get("exception_type")

    flags: list[str] = []
    infra: list[str] = []
    safety = safety_refusal(agent_dir, agent, exc)
    if safety:
        flags.append("safety-refusal")
    if exc in INFRA_EXCEPTIONS:
        infra.append(f"exception {exc}")
    marker = agent_dir / "vb-infra.txt"
    if marker.is_file():
        infra += [
            ln[len(INFRA_MARK) :].strip()
            for ln in marker.read_text().splitlines()
            if ln.startswith(INFRA_MARK)
        ]
    if trial.parent.name.startswith("INFRA-"):
        infra.append("job set aside as INFRA")

    score: dict[str, Any] | None = None
    checks: dict[str, str] = {}
    if (trial / "verifier" / "reward.json").is_file() and task in progress_score.load_rule()[
        "tasks"
    ]:
        s = progress_score.score_trial(trial, repo, task=task)
        score, checks = s.summary(), s.checks
    else:
        infra.append("the verifier did not run")

    run_dir = out / job / trial.name
    run_dir.mkdir(parents=True, exist_ok=True)
    calls, calls_summary = (
        extract_tool_calls(agent_dir, agent)
        if agent in AGENTS
        else ([], {"capture": "unavailable", "reason": f"unknown agent {agent!r}"})
    )
    if calls:
        tool_calls.save(run_dir, calls)
        origin = (result.get("agent_execution") or {}).get("started_at")
        if origin:
            events = tool_calls.public_events(calls, _ts(origin) or origin)
            (run_dir / "public-tool-calls.jsonl").write_text(
                "".join(json.dumps(e) + "\n" for e in events)
            )
    tokens, cost = economics(agent, model, agent_dir, result) if agent in AGENTS else ({}, {})
    patch_text, patch = build_patch(trial, task, bases)
    if patch_text is not None:
        (run_dir / "patch.diff").write_text(patch_text)
    (run_dir / "checks.json").write_text(json.dumps(checks, indent=0, sort_keys=True) + "\n")

    record = {
        "schema": SCHEMA,
        "suite": "frontier-v5",
        "job": job,
        "trial": trial.name,
        "task": task,
        "task_checksum": result.get("task_checksum"),
        "ref": ref_of(job, task, effort),
        "agent": agent,
        "agent_version": (result.get("agent_info") or {}).get("version") or kwargs.get("version"),
        "model": model,
        "effort": effort or "medium",
        "attempt": attempt,
        "concurrency": job_cfg.get("n_concurrent_trials"),
        "started_at": result.get("started_at"),
        "finished_at": result.get("finished_at"),
        "agent_minutes": _minutes(result.get("agent_execution")),
        "exception": exc,
        "flags": flags,
        "safety_refusal": safety is not None,
        "safety_message": safety,
        "infra": infra,
        "scored": score is not None and not infra,
        "reward": None if score is None else score["reward"],
        "passed": None if score is None else score["passed"],
        "progress": None if score is None else score["progress"],
        "checks": score,
        "tokens": tokens,
        "cost": cost,
        "patch": patch,
        "tool_calls": calls_summary,
    }
    (run_dir / "run.json").write_text(json.dumps(record, indent=1, sort_keys=True) + "\n")
    return record


def import_job(job: Path, out: Path, repo: Path, bases: Path | None) -> list[dict[str, Any]]:
    cfg = _load(job / "config.json")
    trials = []
    for d in sorted(job.glob("*__*")):
        if not d.is_dir():
            continue
        if not _load(d / "result.json").get("finished_at"):
            print(f"skip {job.name}/{d.name}: still running (no finished_at)", file=sys.stderr)
            continue
        trials.append(d)
    trials.sort(key=lambda d: (_load(d / "result.json").get("started_at") or "", d.name))
    return [import_trial(t, cfg, out, repo, bases, i) for i, t in enumerate(trials, 1)]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("jobs", nargs="+", type=Path, help="Harbor job directories")
    ap.add_argument("--out", type=Path, required=True, help="where run directories are written")
    ap.add_argument("--repo", type=Path, default=REPO, help="checkout whose tasks/ the runs used")
    ap.add_argument("--bases", type=Path, default=None, help="pristine artifact trees per task")
    ap.add_argument("--include-infra", action="store_true", help="also import INFRA-* jobs")
    args = ap.parse_args(argv)
    records = []
    for job in args.jobs:
        if job.name.startswith("INFRA-") and not args.include_infra:
            print(f"skip {job.name} (set aside as INFRA)", file=sys.stderr)
            continue
        records += import_job(job, args.out, args.repo, args.bases)
    bad = [r for r in records if r["checks"] and not r["checks"]["consistent"]]
    for r in records:
        mins = r["agent_minutes"]
        print(
            f"{r['job']}/{r['trial']}: scored={r['scored']} reward={r['reward']} "
            f"progress={r['progress']} minutes={mins} calls={r['tool_calls'].get('count')}"
            f"{' flags=' + ','.join(r['flags']) if r['flags'] else ''}"
            f"{' infra=' + '; '.join(r['infra']) if r['infra'] else ''}"
        )
    if records:
        scored = [r for r in records if r["scored"]]
        med = statistics.median(
            [r["agent_minutes"] for r in scored if r["agent_minutes"] is not None] or [0]
        )
        print(
            f"import_harbor: {len(records)} trial(s), {len(scored)} scored, median {med} agent minutes"
        )
    if bad:
        print(f"import_harbor: {len(bad)} trial(s) disagree with their reward", file=sys.stderr)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
