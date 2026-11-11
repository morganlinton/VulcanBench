#!/usr/bin/env python3
"""Summarise ComputerUse v1 gate jobs into a trials.json and a Markdown table.

    python scripts/computer-use/summarize_gate.py [--ref codex] --suffix=-m3 TASK [TASK ...] \
        [--out docs/results/<dir>/trials.json] [--jobs ~/vulcanbench-gate/jobs]

Reads, for each task, the job directory run_gate.sh wrote
(``<jobs>/cu-<ref>-<task><suffix>/``, ref claude-code by default) and its
status file, and collects per trial: the verifier's reward map, the turn
count, API time, token usage and list-price cost estimate, and harbor's agent
and trial wall times, plus the perception condition the job ran under (status
file ``condition``; jobs from before milestone 4 are ``pixels+zoom``). For
claude-code the agent numbers come from the last ``result`` record of the
agent stream; for codex from harbor's ATIF trajectory (``agent/trajectory.json``:
turns are agent steps, one per model call) with the cost computed from its
token counts at harness/pricing.py list prices, and the MCP tool calls counted
from ``agent/codex.txt``; Codex reports no API time. Prints a Markdown
table (numbers only, no ranking) and, with --out, writes the JSON the results
note links to. Nothing here is a leaderboard: the suite is not frozen.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

from harness.pricing import cost_usd  # noqa: E402  (the repo root is on sys.path only now)


def last_result(stream: Path) -> dict:
    res: dict = {}
    try:
        lines = stream.read_text(errors="replace").splitlines()
    except OSError:
        return res
    for raw in lines:
        line = raw.strip()
        if not line.startswith("{"):
            continue
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("type") == "result":
            res = obj
    return res


def codex_numbers(trial: Path, model: str | None) -> dict:
    """Turns, tokens, list-price cost and MCP tool calls of one codex trial."""
    try:
        traj = json.loads((trial / "agent" / "trajectory.json").read_text())
    except (OSError, ValueError):
        traj = {}
    steps = traj.get("steps") or []
    fm = traj.get("final_metrics") or {}
    prompt = fm.get("total_prompt_tokens")
    completion = fm.get("total_completion_tokens")
    cached = fm.get("total_cached_tokens") or 0
    cost = None
    if model and prompt is not None and completion is not None:
        cost = cost_usd(f"openai:{model}", prompt, completion, cached_input_tokens=cached)
    calls = 0
    try:
        for raw in (trial / "agent" / "codex.txt").read_text(errors="replace").splitlines():
            line = raw.strip()
            if not line.startswith("{"):
                continue
            try:
                obj = json.loads(line)
            except ValueError:
                continue
            item = obj.get("item") if obj.get("type") == "item.completed" else None
            calls += isinstance(item, dict) and item.get("type") == "mcp_tool_call"
    except OSError:
        pass
    return {
        "num_turns": sum(1 for st in steps if st.get("source") == "agent") or None,
        "duration_api_s": None,
        "cost_usd": round(cost, 2) if cost is not None else None,
        "tool_calls": calls,
        "tokens": {
            "input": None if prompt is None else prompt - cached,
            "output": completion,
            "cache_read": cached,
            "cache_creation": None,
        },
    }


def secs(block: dict | None) -> int | None:
    try:
        a = dt.datetime.fromisoformat(block["started_at"].replace("Z", "+00:00"))
        b = dt.datetime.fromisoformat(block["finished_at"].replace("Z", "+00:00"))
    except (KeyError, TypeError, ValueError, AttributeError):
        return None
    return round((b - a).total_seconds())


def summarise(
    jobs: Path, gate_root: Path, suffix: str, task: str, ref: str = "claude-code"
) -> dict:
    # ``task`` is the job stem after ``cu-<ref>-``: the task name, or
    # ``<task>-<condition>`` for a run under a non-default perception condition
    # (run_gate.sh names those jobs that way). The status file carries the task.
    job = jobs / f"cu-{ref}-{task}{suffix}"
    status = json.loads((gate_root / f"cu-{ref}-{task}{suffix}.status.json").read_text())
    task = status.get("task", task)
    entry = {
        "task": task,
        "ref": ref,
        "job": status["job"],
        "model": status.get("model"),
        "effort": status.get("effort"),
        "condition": status.get("condition", "pixels+zoom"),
        "repo": status.get("repo"),
        "started_at": status.get("started_at"),
        "finished_at": status.get("updated_at"),
        "solved": 0,
        "trials": [],
    }
    for trial in sorted(job.glob(f"{task}__*")):
        reward_path = trial / "verifier" / "reward.json"
        reward = json.loads(reward_path.read_text()) if reward_path.is_file() else {}
        result = (
            json.loads((trial / "result.json").read_text())
            if (trial / "result.json").is_file()
            else {}
        )
        if ref == "codex":
            numbers = codex_numbers(trial, status.get("model"))
        else:
            res = last_result(trial / "agent" / "claude-code.txt")
            usage = res.get("usage") or {}
            numbers = {
                "num_turns": res.get("num_turns"),
                "duration_api_s": round((res.get("duration_api_ms") or 0) / 1000),
                "cost_usd": round(res.get("total_cost_usd") or 0, 2),
                "tokens": {
                    "input": usage.get("input_tokens"),
                    "output": usage.get("output_tokens"),
                    "cache_read": usage.get("cache_read_input_tokens"),
                    "cache_creation": usage.get("cache_creation_input_tokens"),
                },
            }
        entry["solved"] += int(reward.get("reward") == 1)
        entry["trials"].append(
            {
                "trial": trial.name.split("__", 1)[1],
                "reward": reward,
                "agent_wall_s": secs(result.get("agent_execution")),
                "trial_wall_s": secs(result),
                **numbers,
            }
        )
    return entry


REFERENCES = {
    "claude-code": {
        "name": "claude-code",
        "auth": "subscription",
        "tools": "the task's MCP computer tools only (Bash, file and web tools disallowed)",
    },
    "codex": {
        "name": "codex",
        "auth": "subscription",
        "tools": "the task's MCP computer tools only, called from Codex code mode's exec (shell, apply_patch, "
        "web, subagent and image tools off: scripts/computer-use/codex-gate.toml and the locked catalog)",
    },
}
COST_NOTES = {
    "claude-code": "cost_usd is Claude Code's list-price estimate from its result record; "
    "subscription runs are not billed per call",
    "codex": "cost_usd is computed from the trajectory's token counts at harness/pricing.py list prices "
    "(cached input at the cached rate); subscription runs are not billed per call",
}


def fmt(values: list, key: str) -> str:
    return ", ".join("?" if v.get(key) is None else str(v[key]) for v in values)


def table(entries: list[dict], tracks: dict[str, str]) -> str:
    out = [
        "| Task | Solved | Turns | Agent wall (s) | API time (s) | Cost (USD, list) |",
        "|---|---|---|---|---|---|",
    ]
    for e in entries:
        name = e["task"] + (" (safety track)" if tracks.get(e["task"]) == "safety" else "")
        if e.get("condition", "pixels+zoom") != "pixels+zoom":
            name += f" [{e['condition']}]"
        t = e["trials"]
        cost = ", ".join("?" if v["cost_usd"] is None else f"{v['cost_usd']:.2f}" for v in t)
        out.append(
            f"| {name} | {e['solved']} of {len(t)} | {fmt(t, 'num_turns')} | {fmt(t, 'agent_wall_s')} "
            f"| {fmt(t, 'duration_api_s')} | {cost} |"
        )
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("tasks", nargs="+")
    ap.add_argument("--suffix", default="", help="GATE_JOB_SUFFIX the jobs were launched with")
    ap.add_argument("--ref", default="claude-code", choices=["claude-code", "codex"])
    ap.add_argument("--jobs", default=str(Path.home() / "vulcanbench-gate" / "jobs"))
    ap.add_argument("--out", help="write trials.json here")
    ap.add_argument("--suite", default="tasks/computer-use-v1/suite.json")
    args = ap.parse_args(argv)
    jobs = Path(args.jobs).expanduser()
    suite = json.loads(Path(args.suite).read_text()) if Path(args.suite).is_file() else {}
    entries = [summarise(jobs, jobs.parent, args.suffix, t, args.ref) for t in args.tasks]
    print(table(entries, suite.get("tracks", {})))
    total_cost = round(sum(tr["cost_usd"] or 0 for e in entries for tr in e["trials"]), 2)
    total_out = sum(tr["tokens"]["output"] or 0 for e in entries for tr in e["trials"])
    print(
        f"\n{sum(len(e['trials']) for e in entries)} trials; list-price cost estimate {total_cost} USD; "
        f"{total_out:,} output tokens"
    )
    if args.out:
        doc = {
            "suite": suite.get("id", "computer-use-v1"),
            "suite_version": suite.get("version"),
            "image": suite.get("pins", {}).get("images"),
            "reference": REFERENCES[args.ref],
            "conditions": sorted({e.get("condition", "pixels+zoom") for e in entries}),
            "cost_note": COST_NOTES[args.ref],
            "tasks": entries,
        }
        out = Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(doc, indent=1) + "\n")
        print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
