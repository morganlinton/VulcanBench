#!/usr/bin/env python3
"""Validate a Frontier v5 grid file and print the batches run_gate.sh runs.

usage: scripts/frontier-v5/grid_plan.py <grid.json> <tasks-dir> [--budget-min M]
                                        [--instruction] [<batch> ...]

The grid file (PLAN-v6 sections 2 and 4) fixes, for every model, which
environments exist, where each sits in the grid, the wall-clock budget and
the batches that run together:

    {"schema": 1,
     "budget_min": 20,
     "budget_instruction": "... {minutes} minutes ..." | null,
     "environments": {"<env>": {"language": "cpp", "tier": "smaller"}},
     "batches": [["<env>", "<env>", "<env>", "<env>"], ...]}

``<env>`` is the environment's directory under ``<tasks-dir>`` (Harbor's
task name filter matches directory names). Rules checked here:

- ``language`` is one of c, cpp, java, javascript, python, rust (TypeScript
  repos sit in the javascript row); ``tier`` is smaller or larger.
- Every environment is in exactly one batch; a batch holds 1 to 4.
- Every environment directory has a task.toml whose ``[agent] timeout_sec``
  is the same across its batch (the budget is applied to a whole Harbor job
  as one agent-timeout multiplier) and that declares the freeze hook: a
  ``[[verifier.collect]]`` entry run as root whose command kills every
  process in the agent container (``kill -KILL -1``). Harbor's agent timeout
  cancels the ``docker compose exec`` client but not the agent process in
  the container, so without the hook the agent keeps writing while the tree
  is collected for grading (docs/frontier-v5/GRID-ENGINE.md).
- ``budget_instruction`` is null (the environment's own instruction states
  the budget) or a sentence with ``{minutes}``, appended to every
  environment's instruction with Harbor's ``--extra-instruction``.

Output: one tab-separated line per selected batch (all batches by default;
``<batch>`` numbers are 1-based):

    <index> <budget_sec> <budget_source> <agent-timeout-multiplier> <env> [<env> ...]

``budget_source`` is ``grid`` or ``override`` (``--budget-min``, for the
pilot's calibration runs). With ``--instruction`` it prints the budget
sentence instead (empty when there is none). Exit 2 with a message on any
violation. Runs on the host's python3 (3.9 and later; no tomllib).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

LANGUAGES = ("c", "cpp", "java", "javascript", "python", "rust")
TIERS = ("smaller", "larger")
MAX_BATCH = 4
ENV_NAME = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
FREEZE_MARK = "kill -KILL -1"


class GridError(ValueError):
    pass


def toml_tables(text: str) -> list[tuple[str, dict[str, str]]]:
    """(header, {key: raw value}) for each table, in order; enough for task.toml's flat keys."""
    tables: list[tuple[str, dict[str, str]]] = [("", {})]
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        m = re.match(r"^(\[\[?)\s*([^\]]+?)\s*\]\]?$", line)
        if m:
            tables.append((m.group(1) + m.group(2) + "]" * len(m.group(1)), {}))
            continue
        k, sep, v = line.partition("=")
        if sep:
            tables[-1][1][k.strip()] = v.strip()
    return tables


def _unquote(v: str) -> str:
    v = v.strip()
    if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
        return v[1:-1]
    return v


def task_facts(task_toml: Path) -> tuple[float, bool]:
    """(agent timeout_sec, has the freeze hook) from a task.toml."""
    tables = toml_tables(task_toml.read_text())
    timeout = None
    freeze = False
    for header, kv in tables:
        if header == "[agent]" and "timeout_sec" in kv:
            timeout = float(kv["timeout_sec"])
        if header == "[[verifier.collect]]":
            cmd = _unquote(kv.get("command", ""))
            user = _unquote(kv.get("user", ""))
            service = _unquote(kv.get("service", "main"))
            if FREEZE_MARK in cmd and user in ("root", "0") and service == "main":
                freeze = True
    if timeout is None or timeout <= 0:
        raise GridError(f"{task_toml}: no positive [agent] timeout_sec")
    return timeout, freeze


def _check_header(path: Path, grid: Any) -> None:
    if not isinstance(grid, dict) or grid.get("schema") != 1:
        raise GridError(f"{path}: schema must be 1")
    budget = grid.get("budget_min")
    if isinstance(budget, bool) or not isinstance(budget, (int, float)) or budget <= 0:
        raise GridError(f"{path}: budget_min must be a positive number of minutes")
    note = grid.get("budget_instruction")
    if note is not None and (not isinstance(note, str) or "{minutes}" not in note):
        raise GridError(f"{path}: budget_instruction must be null or contain {{minutes}}")


def _check_environments(path: Path, envs: Any) -> None:
    if not isinstance(envs, dict) or not envs:
        raise GridError(f"{path}: environments must be a non-empty object")
    for name, cell in envs.items():
        if not ENV_NAME.match(name):
            raise GridError(f"{path}: environment name {name!r} is not a plain directory name")
        if not isinstance(cell, dict):
            raise GridError(f"{path}: {name}: expected an object")
        if cell.get("language") not in LANGUAGES:
            raise GridError(f"{path}: {name}: language must be one of {', '.join(LANGUAGES)}")
        if cell.get("tier") not in TIERS:
            raise GridError(f"{path}: {name}: tier must be smaller or larger")


def _check_batches(path: Path, batches: Any, envs: dict[str, Any]) -> None:
    if not isinstance(batches, list) or not batches:
        raise GridError(f"{path}: batches must be a non-empty list")
    seen: dict[str, int] = {}
    for i, batch in enumerate(batches, 1):
        if not isinstance(batch, list) or not 1 <= len(batch) <= MAX_BATCH:
            raise GridError(f"{path}: batch {i} must list 1 to {MAX_BATCH} environments")
        for name in batch:
            if name not in envs:
                raise GridError(f"{path}: batch {i}: {name!r} is not in environments")
            if name in seen:
                raise GridError(f"{path}: {name} is in batches {seen[name]} and {i}")
            seen[name] = i
    missing = sorted(set(envs) - set(seen))
    if missing:
        raise GridError(f"{path}: not in any batch: {', '.join(missing)}")


def _check_tasks(
    batches: list[list[str]], envs: dict[str, Any], tasks_dir: Path
) -> dict[str, float]:
    facts: dict[str, tuple[float, bool]] = {}
    for name in envs:
        toml = tasks_dir / name / "task.toml"
        if not toml.is_file():
            raise GridError(f"no environment {name} ({toml} missing)")
        facts[name] = task_facts(toml)
    no_freeze = sorted(n for n, (_, f) in facts.items() if not f)
    if no_freeze:
        raise GridError(
            "no freeze hook ([[verifier.collect]] as root running "
            f"'{FREEZE_MARK}') in: {', '.join(no_freeze)}; see docs/frontier-v5/GRID-ENGINE.md"
        )
    for i, batch in enumerate(batches, 1):
        if len({facts[n][0] for n in batch}) > 1:
            raise GridError(
                f"batch {i}: [agent] timeout_sec differs across its environments "
                f"({', '.join(f'{n}={facts[n][0]:g}' for n in batch)})"
            )
    return {n: t for n, (t, _) in facts.items()}


def load_grid(path: Path, tasks_dir: Path) -> dict[str, Any]:
    try:
        grid = json.loads(path.read_text())
    except (OSError, ValueError) as exc:
        raise GridError(f"{path}: {exc}") from exc
    _check_header(path, grid)
    _check_environments(path, grid.get("environments"))
    _check_batches(path, grid.get("batches"), grid["environments"])
    grid["_timeouts"] = _check_tasks(grid["batches"], grid["environments"], tasks_dir)
    return dict(grid)


def plan(
    grid: dict[str, Any], budget_min: float | None, select: list[int]
) -> list[tuple[int, float, str, str, list[str]]]:
    batches: list[list[str]] = grid["batches"]
    budget = float(budget_min if budget_min is not None else grid["budget_min"])
    source = "override" if budget_min is not None else "grid"
    picks = select or list(range(1, len(batches) + 1))
    out = []
    for i in picks:
        if not 1 <= i <= len(batches):
            raise GridError(f"no batch {i} (the grid has {len(batches)})")
        envs = batches[i - 1]
        timeout = grid["_timeouts"][envs[0]]
        budget_sec = budget * 60
        out.append((i, budget_sec, source, repr(budget_sec / timeout), envs))
    return out


def instruction(grid: dict[str, Any], budget_min: float | None) -> str:
    note = grid.get("budget_instruction")
    if not note:
        return ""
    minutes = budget_min if budget_min is not None else grid["budget_min"]
    return str(note).replace("{minutes}", f"{float(minutes):g}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("grid", type=Path)
    ap.add_argument("tasks_dir", type=Path)
    ap.add_argument("batches", nargs="*", type=int)
    ap.add_argument("--budget-min", type=float, default=None)
    ap.add_argument("--instruction", action="store_true")
    # Intermixed parsing: batch numbers may follow an option. Plain
    # parse_args rejects that on Python 3.9 to 3.11 (the host's python3).
    args = ap.parse_intermixed_args(argv)
    try:
        if args.budget_min is not None and args.budget_min <= 0:
            raise GridError("--budget-min must be positive")
        grid = load_grid(args.grid, args.tasks_dir)
        if args.instruction:
            print(instruction(grid, args.budget_min))
            return 0
        for i, budget_sec, source, mult, envs in plan(grid, args.budget_min, args.batches):
            print("\t".join([str(i), f"{budget_sec:g}", source, mult, *envs]))
    except GridError as exc:
        print(f"grid: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
