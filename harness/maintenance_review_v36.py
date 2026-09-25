"""Code quality maintenance v3.6: the same protocol applied to GPT-5.6 Terra.

Nothing about the rubric, controls, quirk keys, gates, repeats, seed or the
judges changes. What changes is the population: the September 2026 GPT-5.6
Terra effort sweep through Codex, five levels, 115 submissions when every
run exists. A task and level with no attempt (the Codex quota window) is
recorded under "missing" and the cell freezes with the submissions it has.
Both neutral judges, Muse Spark 1.3 and Grok 4.6, are scored in this one
directory, and there are no sensitivity panels.

The frozen v3 implementation is reused as a library. This module rebinds the
v3 module's population and directory constants at import time and replaces
only ``prepare``, which freezes the new manifest and protocol record. Every
other stage (calibrate, run, probe, summarize) is v3's own code, and the
protocol record lists this file beside the v3 files in ``code_hashes``.

    python -m harness.maintenance_review_v36 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v36 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel muse
"""

from __future__ import annotations

import hashlib
import random
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.6"
OUT = ROOT / "runs-code-quality-maintenance-v3.6"
COMPARISON = ROOT / "docs/results/swe-v4-terra-2026-09/comparison.json"
GROK_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.3/protocol.json"
MUSE_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.4/protocol.json"
MODELS = {"terra": tuple(base.LEVELS)}
PANELS = ("muse", "grok")


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {}
    v3.SCORED_PANELS = PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.prepare = prepare


def prepare() -> None:  # noqa: PLR0915, one linear freeze
    record = v3.read(COMPARISON)
    rows = record["rows"]
    excluded = record.get("excluded", [])
    missing = record.get("missing", [])
    counts = Counter((r["model"], r["effort"]) for r in rows)
    expected = {(m, e) for m, levels in MODELS.items() for e in levels}
    if set(counts) != expected:
        raise ValueError(f"Cells present {sorted(counts)} differ from expected {sorted(expected)}")
    gaps = Counter((e["model"], e["effort"]) for e in excluded + missing)
    for cell in expected:
        if counts[cell] + gaps.get(cell, 0) != 23:
            raise ValueError(
                f"Cell {cell} has {counts[cell]} rows and {gaps.get(cell, 0)} exclusions or missing, not 23"
            )
    tasks = {r["task"] for r in rows}
    if len(tasks) != 23:
        raise ValueError("Task coverage is not the 23-task suite")
    for task in sorted(tasks):
        if not (v3.KEYS_DIR / f"{task}.json").exists():
            raise ValueError(f"Missing quirk key for {task}")
        v3.freeze(OUT / "quirk-keys" / f"{task}.json", v3.read(v3.KEYS_DIR / f"{task}.json"))
        v3.load_key(task)
    random.Random(v3.SEED).shuffle(rows)
    manifest, signals = [], {}
    with v3._v2_writing_to(OUT):
        for i, row in enumerate(rows):
            ident = f"submission-{i + 1:03d}"
            evidence = v2.evidence_for(row)
            v3.freeze(OUT / "evidence" / f"{ident}.json", evidence)
            v3.probe_evidence(evidence)
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
    repeats = [
        next(r["id"] for r in manifest if (r["model"], r["effort"]) == cell)
        for cell in sorted(expected)
    ]
    ordered_tasks = sorted(
        tasks, key=lambda t: hashlib.sha256(f"{v3.SEED}:{t}".encode()).hexdigest()
    )
    levels = list(base.LEVELS)
    pairs = []
    for i, task in enumerate(ordered_tasks[: len(levels)]):
        # One model: pair the same task at two levels two steps apart.
        ids = [
            next(
                (
                    r["id"]
                    for r in manifest
                    if r["model"] == "terra" and r["task"] == task and r["effort"] == effort
                ),
                None,
            )
            for effort in (levels[i], levels[(i + 2) % len(levels)])
        ]
        if all(ids):
            pairs.append(ids)
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
        },
        "population": {
            "description": "GPT-5.6 Terra (five levels) through Codex on the coding-intelligence-index-v4 suite, "
            "one attempt per task and level; a task and level with no attempt is listed under missing",
            "cells": {f"{m}/{e}": counts[m, e] for m, e in sorted(expected)},
            "excluded": excluded,
            "missing": missing,
        },
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
            "probe_and_match_per_panel": 2 * n,
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
        "full_evidence": True,
        "prompt_characters_total_one_panel": sum(sizes),
        "largest_prompt_characters": max(sizes),
        "protocol_sha256": v3.sha(OUT / "protocol.json"),
    }
    v3.freeze(OUT / "preflight.json", result)
    print(base.canonical(result), flush=True)


_bind()


def main() -> None:
    v3.main()


if __name__ == "__main__":
    main()
