"""Code quality maintenance v3.13: Devin SWE-2 gets a second judge, Claude Opus 5.

Nothing about the rubric, controls, quirk keys, gates, repeats, seed or the
weights changes. What changes from v3.11 is the panel:

- Population: the v3.11 Devin SWE-2 freeze, byte for byte (65 judged
  submissions; prepare refuses unless the manifest and signals match).
- Panel: Muse Spark 1.3 is scored from its v3.11 pass as a scored sibling,
  and Claude Opus 5 takes the second seat under the settings, guards and
  CLI pin v3.2 froze for it. Grok 4.6 (v3.9) and GPT-5.6 Sol (v3.10) failed
  the exam for this population, which left v3.11 publishing from one judge;
  Opus 5 passed the identical exam under v3.12 with no allowance used. It is
  neutral for a Cognition model and is not admitted as a neutral judge of
  Anthropic submissions. It retakes the exam here before any counted call;
  if it fails, v3.11's single-judge publication stands.
- The protocol document is frozen as a copy inside this run directory, as
  v3.8 does, so concurrent protocols stop colliding on one file.

The frozen v3 implementation is reused as a library. This module rebinds the
v3 module's population and directory constants at import time and replaces
only ``prepare``, which freezes the new manifest and protocol record. Every
other stage (calibrate, run, probe, summarize) is v3's own code, and the
protocol record lists this file beside the v3 files in ``code_hashes``.

    python -m harness.maintenance_review_v313 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v39 \\
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
PROTOCOL_ID = "code-quality-maintenance-v3.13"
OUT = ROOT / "runs-code-quality-maintenance-v3.13"
MUSE_OUT = ROOT / "runs-code-quality-maintenance-v3.11"  # scored sibling: Muse on this population
# v3.11 reused Muse's v3.9 verdict (the v3.6.1 precedent), so the verdict that gates
# Muse here is the same file v3.11 pinned, not a copy inside the v3.11 directory.
MUSE_CALIBRATION = ROOT / "runs-code-quality-maintenance-v3.9/calibration-muse.json"
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
OPUS_PROTOCOL = (
    ROOT / "runs-code-quality-maintenance-v3.2/protocol.json"
)  # the Opus 5 panel settings and CLI pin
COMPARISON = ROOT / "docs/results/swe-v4-devin-swe2-2026-09/comparison-judged.json"
MODELS = {"swe2": ("medium", "high", "max")}
PANELS = ("claude",)
SCORED_PANELS = ("claude", "muse")


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {"muse": MUSE_OUT}
    v3.SCORED_PANELS = SCORED_PANELS
    v3.DOC = OUT / "protocol-document.md"
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.prepare = prepare
    v3.panel_passed = panel_passed


def panel_passed(panel: str) -> bool:
    """Opus 5 is gated by its exam here; Muse by the v3.9 verdict that gated its v3.11 pass."""
    if panel == "muse":
        gate = v3.read(MUSE_CALIBRATION)
        pinned = v3.read(MUSE_OUT / "protocol.json")["calibration"]["muse"]["sha256"]
        return bool(gate["passed"]) and v3.sha(MUSE_CALIBRATION) == pinned
    path = OUT / f"calibration-{panel}.json"
    if not path.exists():
        return False
    gate = v3.read(path)
    return bool(gate["passed"]) and gate["protocol_sha256"] == v3.sha(OUT / "protocol.json")


def prepare() -> None:  # noqa: PLR0912, PLR0915, one linear freeze
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
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
    # A task with no judged submission at any level (paddockcore: every Muse attempt
    # was unfinished or changed no source) is covered by the exclusions, not the rows.
    covered = tasks | {e["task"] for e in excluded + missing}
    if len(covered) != 23:
        raise ValueError("Judged and excluded tasks together are not the 23-task suite")
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
    for name in ("private-manifest.json", "signals.json"):
        if v3.sha(OUT / name) != v3.sha(MUSE_OUT / name):
            raise ValueError(
                f"{name} must be byte-identical to the v3.11 freeze Muse was judged under"
            )
    if not panel_passed("muse"):
        raise ValueError("Muse's v3.9 verdict does not gate this pass")
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
    levels = list(MODELS["swe2"])
    pairs = []
    for i, task in enumerate(ordered_tasks[: len(levels)]):
        # One model: pair the same task at two levels two steps apart.
        ids = [
            next(
                (
                    r["id"]
                    for r in manifest
                    if r["model"] == "swe2" and r["task"] == task and r["effort"] == effort
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
    opus_protocol = v3.read(OPUS_PROTOCOL)
    claude_settings = dict(opus_protocol["reviewers"]["claude"])
    claude_settings.update(
        lab="Anthropic",
        neutrality="neutral for Cognition submissions; not admitted as a neutral judge of Anthropic submissions",
        seat="second seat for the Devin SWE-2 population, after Grok 4.6 (v3.9) and GPT-5.6 Sol (v3.10) failed the exam",
    )
    claude_path = Path(claude_settings["claude"])
    claude_sha = v3.sha(claude_path)
    if claude_sha != opus_protocol["binaries"][str(claude_path)]["sha256"]:
        raise ValueError("Claude CLI differs from the v3.2 pin")
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "automated held-out controls, five repeats, gates 1 to 17 with a one-gate 0.5 shortfall allowance, "
        "line-level excerpt rule with elision markers; the judge retakes the exam under this protocol",
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
            "v3.8": "VulcanRoutine judging/code-quality-maintenance-v3.8 (private repository)",
            "v3.9": "runs-code-quality-maintenance-v3.9",
            "v3.10": "runs-code-quality-maintenance-v3.10",
            "v3.11": "runs-code-quality-maintenance-v3.11",
            "v3.12": "runs-code-quality-maintenance-v3.12",
        },
        "population": {
            "description": "Devin SWE-2 (medium, high and max) through the Devin CLI on the coding-intelligence-index-v4 suite, "
            "one attempt per task and level; a run that did not finish, or that changed no recognized source file, is listed under excluded",
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
        "reviewers": {
            "claude": claude_settings,
            "muse": v3.read(MUSE_OUT / "protocol.json")["reviewers"]["muse"],
        },
        "scored_siblings": {
            "muse": {
                "directory": str(MUSE_OUT.relative_to(ROOT)),
                "protocol_sha256": v3.sha(MUSE_OUT / "protocol.json"),
                "calibration_sha256": v3.sha(MUSE_CALIBRATION),
                "calibration_path": str(MUSE_CALIBRATION.relative_to(ROOT)),
                "role": "scored neutral panel judged under v3.11 on the identical population",
            }
        },
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "reviewer_fallback_policy": "retain and disclose reviewer fallbacks (owner decision 2026-09-07); the card reports the count",
        "binaries": {
            str(claude_path): {
                "sha256": claude_sha,
                "version": opus_protocol["binaries"][str(claude_path)].get("version"),
            }
        },
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
