"""Code quality maintenance v3.8: the same protocol on the private Routine v1 suite.

Nothing about the rubric, controls, gates, repeats, seed, weights or the
judges changes. What changes is the population and one layer's applicability:

- Population: VulcanBench Routine v1, twelve private routine tickets, every
  board model at every effort level it offers (GPT-6 Astra, GPT-5.6 Terra,
  Luna and Sol, GPT-5.5 and Claude Fable 5.1 through their own CLIs, SWE-2
  through the Devin CLI at medium, high and max). One attempt per task and
  level. A task and level with no finished attempt is recorded under
  "missing" or "excluded" and the cell freezes with the submissions it has.
- L2 intent recovery is not applicable. It scores whether a reviewer can
  recover a task's frozen legacy quirks from the source. Routine tasks are
  admitted on the opposite gate (one clear ticket, no hidden contracts), so
  no quirk exists to recover. Each routine task freezes an empty quirk key,
  which puts every submission's L2 denominator at zero, and the rule v3
  pre-registered for that case applies unchanged: "the 6% moves to L1 for
  this submission only". Code quality is therefore the L1 reviewed score of
  the passing panels. No probe or match call is made.
- Consequence, stated wherever these numbers appear: Routine Code quality
  and Frontier Code quality are different constructs (L1 only versus L1 plus
  L2) on different task shapes. They are not comparable across the two
  suites. Within Routine v1 every model and level is scored identically.

Both neutral judges, Muse Spark 1.3 and Grok 4.6, retake the identical
calibration exam (controls, probe and match calls included) under this
protocol before any counted call. There are no sensitivity panels.

Privacy: the frozen record holds task content, so it lives in the private
VulcanRoutine repository (``judging/code-quality-maintenance-v3.8``), never
in this tree. This file names no task.

The frozen v3 implementation is reused as a library. This module rebinds the
v3 and v2 population, task and directory constants at import time and
replaces only ``prepare``. Every other stage (calibrate, run, summarize) is
v3's own code, and the protocol record lists this file beside the v3 files
in ``code_hashes``.

    python -m harness.maintenance_review_v38 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v38 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel muse
"""

from __future__ import annotations

import hashlib
import os
import random
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

ROOT = v3.ROOT
ROUTINE = Path(os.environ.get("VULCANROUTINE_ROOT", ROOT.parent / "VulcanRoutine")).resolve()
PROTOCOL_ID = "code-quality-maintenance-v3.8"
OUT = ROUTINE / "judging/code-quality-maintenance-v3.8"
COMPARISON = ROUTINE / "results/private/routine-v1-comparison.json"
TASKS = ROUTINE / "tasks/routine-v1"
GROK_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.3/protocol.json"
MUSE_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.4/protocol.json"
ALL_LEVELS = tuple(base.LEVELS)
MODELS = {
    "astra": ALL_LEVELS,
    "terra": ALL_LEVELS,
    "luna": ALL_LEVELS,
    "sol": ALL_LEVELS,
    "gpt55": ("low", "medium", "high", "extra-high"),
    "fable": ALL_LEVELS,
    "swe2": ("medium", "high", "max"),
}
TASKS_PER_CELL = 12
PANELS = ("muse", "grok")
L2_NOT_APPLICABLE = (
    "Routine v1 tasks are admitted on one clear ticket with no hidden contracts, so no legacy "
    "quirk exists to recover. The key is empty, the L2 denominator is zero for every submission, "
    "and v3's pre-registered zero-denominator rule moves the L2 share to L1."
)


def _bind() -> None:
    """Point the frozen v3 and v2 implementations at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.TASKS = TASKS
    v2.TASKS = TASKS
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {}
    v3.SCORED_PANELS = PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.prepare = prepare


def empty_key(task: str) -> dict:
    return {
        "task": task,
        "protocol": PROTOCOL_ID,
        "sources": ["VulcanRoutine docs/CHARTER.md, admission gate 1 (one clear ticket)"],
        "quirks": [],
        "not_applicable": L2_NOT_APPLICABLE,
    }


def select_pairs(manifest: list[dict], ordered_tasks: list[str]) -> list[list[str]]:
    """One pairwise diagnostic per task: the same task at a model's lowest and highest level.

    Models rotate over the seeded task order, so every model's effort ladder
    is probed on at least one task.
    """
    models = list(MODELS)
    pairs = []
    for i, task in enumerate(ordered_tasks):
        model = models[i % len(models)]
        ends = (MODELS[model][0], MODELS[model][-1])
        ids = [
            next(
                (
                    r["id"]
                    for r in manifest
                    if r["model"] == model and r["task"] == task and r["effort"] == effort
                ),
                None,
            )
            for effort in ends
        ]
        if all(ids):
            pairs.append(ids)
    return pairs


def prepare() -> None:  # noqa: PLR0915, one linear freeze
    record = v3.read(COMPARISON)
    rows = record["rows"]
    excluded = record.get("excluded", [])
    missing = record.get("missing", [])
    counts = Counter((r["model"], r["effort"]) for r in rows)
    expected = {(m, e) for m, levels in MODELS.items() for e in levels}
    if set(counts) - expected:
        raise ValueError(f"Unexpected cells {sorted(set(counts) - expected)}")
    gaps = Counter((e["model"], e["effort"]) for e in excluded + missing)
    for cell in expected:
        if counts[cell] + gaps.get(cell, 0) != TASKS_PER_CELL:
            raise ValueError(
                f"Cell {cell} has {counts[cell]} rows and {gaps.get(cell, 0)} exclusions or "
                f"missing, not {TASKS_PER_CELL}"
            )
    tasks = {r["task"] for r in rows}
    if len(tasks) != TASKS_PER_CELL:
        raise ValueError(f"Task coverage is not the {TASKS_PER_CELL}-task suite")
    for task in sorted(tasks):
        v3.freeze(OUT / "quirk-keys" / f"{task}.json", empty_key(task))
        if v3.load_key(task)["quirks"]:
            raise ValueError(f"Routine key for {task} is not empty")
    random.Random(v3.SEED).shuffle(rows)
    manifest, signals = [], {}
    with v3._v2_writing_to(OUT):
        for i, row in enumerate(rows):
            ident = f"submission-{i + 1:03d}"
            evidence = v2.evidence_for(row)
            v3.freeze(OUT / "evidence" / f"{ident}.json", evidence)
            manifest.append(
                {
                    "id": ident,
                    **row,
                    "evidence_sha256": v3.sha(OUT / "evidence" / f"{ident}.json"),
                    "passed_families": sorted(v3.passed_families(Path(row["source_directory"]))),
                }
            )
            signals[ident] = {
                name: v3._signals(code)
                for name, code in evidence["final_files"].items()
                if name.endswith(".py")
            }
    v3.freeze(OUT / "private-manifest.json", manifest)
    v3.freeze(OUT / "signals.json", signals)
    for i, evidence in enumerate(v3.controls()):
        v3.freeze(OUT / "controls" / f"control-{i}.json", evidence)
    v3.freeze(
        OUT / "calibration-order.json",
        v3.interleaved_order(v3.SEED, len(v3.CONTROL_FILES), v3.REPEATS),
    )
    present = sorted(cell for cell in expected if counts[cell])
    repeats = [
        next(r["id"] for r in manifest if (r["model"], r["effort"]) == cell) for cell in present
    ]
    ordered_tasks = sorted(
        tasks, key=lambda t: hashlib.sha256(f"{v3.SEED}:{t}".encode()).hexdigest()
    )
    pairs = select_pairs(manifest, ordered_tasks)
    v3.freeze(OUT / "diagnostic-selection.json", {"repeats": repeats, "pairs": pairs})
    code = [
        Path(__file__),
        Path(v3.__file__),
        Path(v2.__file__),
        Path(base.__file__),
        Path(v3.claude.__file__),
        ROOT / "harness/claude_review_guard.py",
        ROOT / "harness/tasks.py",
        ROOT / "harness/evaluator/readability_signals.py",
    ]
    muse_settings = v3.read(MUSE_PROTOCOL)["reviewers"]["muse"]
    grok_settings = v3.read(GROK_PROTOCOL)["reviewers"]["grok"]
    muse_path, muse_sha = v3._pin_muse()
    if str(muse_path) != muse_settings["muse"] or muse_sha != muse_settings["binary_sha256"]:
        raise ValueError("Muse binary pin differs from v3.4")
    cursor = Path(grok_settings["cursor"])
    cursor_sha = v3.sha(cursor)
    if cursor_sha != v3.read(GROK_PROTOCOL)["binaries"][str(cursor)]["sha256"]:
        raise ValueError("Cursor binary differs from the v3.3 pin")
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "automated held-out controls, five repeats, gates 1 to 17 with a one-gate 0.5 shortfall allowance, "
        "line-level excerpt rule with elision markers; both judges retake the exam under this protocol",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {
            "v3": "runs-code-quality-maintenance-v3",
            "v3.1": "runs-code-quality-maintenance-v3.1",
            "v3.2": "runs-code-quality-maintenance-v3.2",
            "v3.3": "runs-code-quality-maintenance-v3.3",
            "v3.4": "runs-code-quality-maintenance-v3.4",
            "v3.5": "runs-code-quality-maintenance-v3.5",
            "v3.6": "runs-code-quality-maintenance-v3.6",
            "v3.6.1": "runs-code-quality-maintenance-v3.6.1",
            "v3.7": "runs-code-quality-maintenance-v3.7",
        },
        "population": {
            "description": "VulcanBench Routine v1 (private, twelve routine tickets): every board model at every "
            "effort level it offers through its own CLI, one attempt per task and level; a task and level with "
            "no finished attempt is listed under excluded or missing",
            "suite": "routine-v1",
            "cells": {f"{m}/{e}": counts[m, e] for m, e in sorted(expected)},
            "excluded": excluded,
            "missing": missing,
        },
        "layers": {
            "l1_reviewed": "scored, unchanged",
            "l2_intent": "not applicable: " + L2_NOT_APPLICABLE,
            "l3_measured": "not run; fallback split in force, as in every v3.x amendment",
            "l4_signals": "frozen at prepare time, unchanged",
        },
        "comparability": "Routine Code quality is the L1 reviewed score alone; Frontier Code quality is L1 plus L2 "
        "on legacy reconstruction tasks. The two are not comparable across suites.",
        "rubric": v3.RUBRIC,
        "system": v3.SYSTEM,
        "pair_instruction": v3.PAIR_INSTRUCTION,
        "probe_instruction": v3.PROBE_INSTRUCTION,
        "match_instruction": v3.MATCH_INSTRUCTION,
        "reader_instruction": v3.READER_INSTRUCTION,
        "schemas": v3.KIND_SCHEMA,
        "seed": v3.SEED,
        "repeats": v3.REPEATS,
        "codex_config": list(base.CONFIG),
        "code_hashes": {str(p.relative_to(ROOT)): v3.sha(p) for p in code},
        "protocol_document_sha256": v3.sha(v3.DOC),
        "source_comparison_sha256": v3.sha(COMPARISON),
        "manifest_sha256": v3.sha(OUT / "private-manifest.json"),
        "signals_sha256": v3.sha(OUT / "signals.json"),
        "selection_sha256": v3.sha(OUT / "diagnostic-selection.json"),
        "calibration_order_sha256": v3.sha(OUT / "calibration-order.json"),
        "control_source_hashes": {
            name: v3.sha(v3.CONTROLS_DIR / name) for name in v3.CONTROL_FILES
        },
        "control_hashes": {p.name: v3.sha(p) for p in sorted((OUT / "controls").glob("*.json"))},
        "quirk_key_hashes": {
            p.name: v3.sha(p) for p in sorted((OUT / "quirk-keys").glob("*.json"))
        },
        "ledger_key": v3.LEDGER_KEY,
        "reviewers": {"muse": muse_settings, "grok": grok_settings},
        "scored_siblings": {},
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": {str(muse_path): {"sha256": muse_sha}, str(cursor): {"sha256": cursor_sha}},
        "planned_calls": {
            "calibration_per_panel": 80,
            "primary_per_panel": n + len(repeats) + 2 * len(pairs),
            "probe_and_match_per_panel": 0,
        },
        "single_panel_rule": "publish from passing panels; a failed panel is disclosed as a sensitivity table; "
        "both failing stops the revision",
        "weights": {
            "functional": 0.50,
            "quality": 0.085,
            "security": 0.085,
            "code_quality": {
                "total": 0.33,
                "l1_reviewed": 0.15,
                "l2_intent": 0.06,
                "l3_measured": 0.12,
                "fallback_without_l3": {"l1_reviewed": 0.24, "l2_intent": 0.09},
            },
        },
        "invalid_response_retries": 1,
    }
    v3.freeze(OUT / "protocol.json", protocol)
    sizes = [
        len(v3.prompt("review", v3.read(OUT / "evidence" / f"{r['id']}.json"))) for r in manifest
    ]
    result = {
        "submissions": n,
        "cells": protocol["population"]["cells"],
        "excluded": len(excluded),
        "missing": len(missing),
        "quirk_keys": len(protocol["quirk_key_hashes"]),
        "quirks": 0,
        "pairs": len(pairs),
        "full_evidence": True,
        "prompt_characters_total_one_panel": sum(sizes),
        "largest_prompt_characters": max(sizes),
        "protocol_sha256": v3.sha(OUT / "protocol.json"),
    }
    v3.freeze(OUT / "preflight.json", result)
    print(base.canonical(result), flush=True)


_bind()


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    v3.main()


if __name__ == "__main__":
    main()
