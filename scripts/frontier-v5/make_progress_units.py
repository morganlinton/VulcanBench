#!/usr/bin/env python3
"""Derive each release 1 task's progress-score units from the task's own files.

usage: scripts/frontier-v5/make_progress_units.py [--check]

Writes ``scripts/frontier-v5/progress/<task>.json`` for the six board tasks,
then refreshes the per-task sha256 values in ``progress_rule.json``. With
``--check`` it writes nothing and exits 1 if a committed file differs from
what the task sources give (a test runs this).

The rule is the owner's option C (docs/DECISIONS.md 2026-11-19, PR #314):
a **unit** is either one ``fail_to_pass`` check, or one group of held-out
guards. A held-out guard is a ``pass_to_pass`` id that the agent's copy of
the repository does not contain (a held-out test, or a variant of a held-out
fixture); the guards of one fixture form one group, which passes only if
every id in it passes. Guards that are not variants of a shared fixture are
one id per group.

How each task's held-out guards are found, from the files that build the
agent's view:

- tree-sitter: test functions ``environment/agent_tests.patch`` deletes from
  ``query_test.rs`` that are graded as pass_to_pass; one group each.
- black: the seven hidden case files are all fail_to_pass; no guards.
- prettier: ids of the fixtures ``environment/agent_tests.patch`` deletes or
  edits (six fixture files deleted outright, and two whose held-out cases it
  removes, ``parenthesized-trailing-comment.js`` and ``class-implements.ts``),
  grouped per fixture file across its option sets and its BOM, line-ending,
  AST-compare and second-format variants.
- javaparser: pass_to_pass ids in the families' ``held_out_classes``; one
  group each.
- CLI11: pass_to_pass cases in ``HELD_OUT`` of ``tests/make_base.py``, which
  removes them from the agent image; one group each.
- pulldown-cmark: the regression examples 250 to 281 that ``tests/hidden``
  adds and that pass at base; one group each.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
SUITE = REPO / "tasks" / "frontier-v5"
OUT = HERE / "progress"
RULE = HERE / "progress_rule.json"


def families(task: str) -> dict[str, Any]:
    fam: dict[str, Any] = json.loads((SUITE / task / "tests" / "families.json").read_text())
    return fam


def treesitter(task: str, fam: dict[str, Any]) -> dict[str, list[str]]:
    patch = (SUITE / task / "environment" / "agent_tests.patch").read_text()
    names = re.findall(r"^-fn (test_\w+)\(", patch, re.M)
    p2p = set(fam["pass_to_pass"])
    ids = [f"cli::tests::query_test::{n}" for n in names]
    return {i: [i] for i in ids if i in p2p}


def black(task: str, fam: dict[str, Any]) -> dict[str, list[str]]:
    cases = sorted(p.stem for p in (SUITE / task / "tests" / "hidden_cases").glob("*.py"))
    leaked = [x for x in fam["pass_to_pass"] if any(f"[{c}]" in x for c in cases)]
    if leaked:
        raise SystemExit(f"black: hidden cases graded as pass_to_pass: {leaked}")
    return {}


def prettier(task: str, fam: dict[str, Any]) -> dict[str, list[str]]:
    patch = (SUITE / task / "environment" / "agent_tests.patch").read_text()
    fixtures = []
    for path in re.findall(r"^diff --git a/tests/format/(\S+) b/", patch, re.M):
        if "__snapshots__" in path:
            continue
        directory, name = path.rsplit("/", 1)
        fixtures.append(f"format/{directory}/format.test.js::{name}")
    groups: dict[str, list[str]] = {}
    for fx in fixtures:
        ids = [x for x in fam["pass_to_pass"] if x.startswith(fx + " ") or x.startswith(fx + "#")]
        if ids:
            groups[fx] = ids
    return groups


def javaparser(task: str, fam: dict[str, Any]) -> dict[str, list[str]]:
    classes = tuple(c + "::" for c in fam["held_out_classes"])
    return {i: [i] for i in fam["pass_to_pass"] if i.startswith(classes)}


def cli11(task: str, fam: dict[str, Any]) -> dict[str, list[str]]:
    src = (SUITE / task / "tests" / "make_base.py").read_text()
    start = src.index("HELD_OUT = {")
    held = set(re.findall(r'"([^"]+)"', src[start : src.index("}", start)]))
    out = {}
    for i in fam["pass_to_pass"]:
        prog, case = i.split("::", 1)
        if prog == "ConfigFileTest" and (case in held or case.split("/", 1)[0] in held):
            out[i] = [i]
    return out


def pulldown(task: str, fam: dict[str, Any]) -> dict[str, list[str]]:
    # tests/hidden/regression.txt appends examples 250 to 281 (DESIGN.md); the
    # generated regression_test_<n> ids must all be graded, else the range moved.
    ids = [f"lib::suite::regression::regression_test_{n}" for n in range(250, 282)]
    graded = set(fam["fail_to_pass"]) | set(fam["pass_to_pass"])
    if not all(i in graded for i in ids) or any(
        f"regression_test_{n}" in i for n in (249, 282) for i in fam["fail_to_pass"]
    ):
        raise SystemExit("pulldown-cmark: hidden regression examples are not 250 to 281")
    p2p = set(fam["pass_to_pass"])
    return {i: [i] for i in ids if i in p2p}


DERIVE = {
    "treesitter-query-anchor-matching": treesitter,
    "black-fmt-skip-region-handling": black,
    "prettier-comment-attachment-stability": prettier,
    "javaparser-member-resolution-scope": javaparser,
    "cli11-toml-config-round-trip": cli11,
    "pulldown-cmark-html-block-scanning": pulldown,
}


def task_version(task: str) -> str:
    m = re.search(r'^version = "([^"]+)"', (SUITE / task / "task.toml").read_text(), re.M)
    return m.group(1) if m else ""


def build(task: str) -> str:
    fam = families(task)
    guards = DERIVE[task](task, fam)
    f2p = set(fam["fail_to_pass"])
    if any(i in f2p for ids in guards.values() for i in ids):
        raise SystemExit(f"{task}: a held-out guard is also fail_to_pass")
    units = [{"unit": i, "kind": "fail_to_pass", "ids": [i]} for i in fam["fail_to_pass"]]
    units += [{"unit": g, "kind": "held_out_guard", "ids": ids} for g, ids in guards.items()]
    doc = {
        "task": task,
        "task_version": task_version(task),
        "rule": "C (docs/DECISIONS.md 2026-11-19)",
        "counts": {
            "units": len(units),
            "fail_to_pass_units": len(fam["fail_to_pass"]),
            "held_out_guard_units": len(guards),
            "held_out_guard_ids": sum(len(v) for v in guards.values()),
        },
        "units": units,
    }
    return json.dumps(doc, indent=1, ensure_ascii=False) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--check", action="store_true", help="exit 1 if a committed file is stale")
    args = ap.parse_args(argv)
    rule = json.loads(RULE.read_text())
    stale = []
    for task in DERIVE:
        text = build(task)
        path = OUT / f"{task}.json"
        sha = hashlib.sha256(text.encode()).hexdigest()
        if args.check:
            if not path.is_file() or path.read_text() != text:
                stale.append(path.name)
            if rule["tasks"][task].get("sha256") != sha:
                stale.append(f"progress_rule.json sha256 for {task}")
            continue
        OUT.mkdir(exist_ok=True)
        path.write_text(text)
        rule["tasks"][task]["units"] = f"progress/{task}.json"
        rule["tasks"][task]["sha256"] = sha
        counts = json.loads(text)["counts"]
        print(
            f"{task}: {counts['units']} units ({counts['fail_to_pass_units']} fail_to_pass, "
            f"{counts['held_out_guard_units']} guard groups of {counts['held_out_guard_ids']} ids)"
        )
    if args.check:
        for s in stale:
            print(f"stale: {s}", file=sys.stderr)
        return 1 if stale else 0
    RULE.write_text(json.dumps(rule, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
