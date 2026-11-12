"""Code quality maintenance v3.30.1: Claude Fable 5.2 at Max, judged by Muse Spark 1.3 under its v3.30 verdict.

Low to Extra high were judged in v3.30. Grok 4.6 failed the v3.30 exam (gate
g14_pair_2_1) and, under the pre-registered single-panel rule, Muse Spark 1.3
published those levels alone; the owner chose that for Max too (chat,
2026-11-11). This round judges Max under the same rubric, controls, quirk
keys, gates, repeats, seed and weights, with Muse on the v3.30 settings and
binary and no new exam: Muse's v3.30 verdict gates it, because Max is another
level of the same Fable 5.2 sweep (the v3.26.5 practice). No earlier record
is rewritten.

Each submission gets a primary review, a probe and a match; the cell gets one
repeat review (the first submission in the seeded order) and no pairwise check,
as in v3.26.5. Submission ids are ``max-NNN`` so they never collide with
v3.30's ``submission-NNN``. GPT-6.1 Sol's later seat on this population is
v3.30.3, which reads this directory as Muse's scored sibling.

    python scripts/cii-v4-board/build_fable52_population.py --level max
    python -m harness.maintenance_review_v3301 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3301 \\
        python -m harness.maintenance_review_v3_resume run --panel muse
"""

from __future__ import annotations

import random
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.30.1"
OUT = ROOT / "runs-code-quality-maintenance-v3.30.1"
MODEL = "fable52"
LEVEL = "max"
MODELS = {MODEL: (LEVEL,)}
COMPARISON = ROOT / f"docs/results/swe-v4-fable52-2026-11/comparison-{LEVEL}.json"
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
PANELS = ("muse",)
SOLVER_CLI = "2.1.321 (Claude Code)"
BASE_ID = "code-quality-maintenance-v3.30"
BASE_OUT = ROOT / "runs-code-quality-maintenance-v3.30"
JUDGED = (BASE_OUT,)  # rounds whose runs this one must not judge again
# Where each judge's exam verdict lives, and the protocol it was taken under.
VERDICTS = {"muse": (BASE_OUT, BASE_ID)}
# Fields that must equal the v3.30 record byte for byte.
SAME_AS_BASE = (
    "rubric",
    "system",
    "probe_instruction",
    "match_instruction",
    "schemas",
    "seed",
    "repeats",
    "gate_allowance",
    "weights",
    "ledger_key",
    "control_source_hashes",
)
_FROZEN_CALL = v3.call
_FROZEN_PARSE = v3.parse_stream_for


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
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare
    v3.panel_passed = panel_passed
    v3.call = _FROZEN_CALL  # Muse goes through the frozen call
    v3.parse_stream_for = _FROZEN_PARSE
    check_muse_pin(OUT)


def verdict_passed(verdicts: dict, panel: str) -> bool:
    """A judge is calibrated if it passed its exam under the named round's frozen protocol."""
    if panel not in verdicts:
        return False
    root, protocol_id = verdicts[panel]
    path = root / f"calibration-{panel}.json"
    if not path.exists() or v3.read(root / "protocol.json")["id"] != protocol_id:
        return False
    gate = v3.read(path)
    return bool(gate["passed"]) and gate["protocol_sha256"] == v3.sha(root / "protocol.json")


def panel_passed(panel: str) -> bool:
    return verdict_passed(VERDICTS, panel)


def check_muse_pin(out: Path) -> None:
    """Before any stage after prepare: the frozen Muse binary must be unchanged."""
    protocol_path = out / "protocol.json"
    if not protocol_path.exists():
        return
    muse = v3.read(protocol_path)["reviewers"]["muse"]
    if not Path(muse["muse"]).is_file() or v3.sha(Path(muse["muse"])) != muse["binary_sha256"]:
        raise RuntimeError("Pinned Muse binary changed or is missing; stop the round")


def base_muse_settings() -> dict:
    """Muse exactly as v3.30 froze it, on the same binary matched by sha256."""
    muse = dict(v3.read(BASE_OUT / "protocol.json")["reviewers"]["muse"])
    muse_path, muse_sha = v3._pin_muse()
    if muse_sha != muse["binary_sha256"] or str(muse_path) != muse["muse"]:
        raise ValueError("Muse binary differs from the one v3.30 froze")
    return muse


def check_population(record: dict, judged: list[dict]) -> Counter:
    """One whole level, none of it judged before, rows plus gaps making the 23-task suite."""
    rows = record["rows"]
    excluded = record.get("excluded", [])
    missing = record.get("missing", [])
    counts = Counter((r["model"], r["effort"]) for r in rows)
    if set(counts) != {(MODEL, LEVEL)}:
        raise ValueError(f"Cells present {sorted(counts)}; expected only {MODEL}/{LEVEL}")
    already = {(r["model"], r["effort"], r["task"]) for r in judged}
    for r in rows:
        if (r["model"], r["effort"], r["task"]) in already:
            raise ValueError(f"{r['task']} at {r['effort']} was already judged")
    gaps = Counter((e["model"], e["effort"]) for e in excluded + missing)
    if counts[MODEL, LEVEL] + gaps.get((MODEL, LEVEL), 0) != 23:
        raise ValueError(
            f"{LEVEL} has {counts[MODEL, LEVEL]} rows and {gaps.get((MODEL, LEVEL), 0)} gaps, not 23"
        )
    if any(not e.get("reason") for e in excluded + missing):
        raise ValueError("Every exclusion and missing run needs a reason")
    if any(r.get("solver_cli_version") != SOLVER_CLI for r in rows):
        raise ValueError(f"Every {LEVEL} run must be from Claude Code {SOLVER_CLI}")
    return counts


def prepare() -> None:  # noqa: PLR0915, one linear freeze
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    base_summary = v3.read(BASE_OUT / "summary.json")
    if not base_summary["ready_for_publication"] or base_summary["protocol"] != BASE_ID:
        raise ValueError("The v3.30 record is not final")
    if not panel_passed("muse"):
        raise ValueError("Muse must have passed its v3.30 exam under the frozen v3.30 protocol")
    muse = base_muse_settings()
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    record = v3.read(COMPARISON)
    judged = [r for root in JUDGED for r in v3.read(root / "private-manifest.json")]
    counts = check_population(record, judged)
    rows = record["rows"]
    tasks = {r["task"] for r in rows}
    for task in sorted(tasks):
        if not (v3.KEYS_DIR / f"{task}.json").exists():
            raise ValueError(f"Missing quirk key for {task}")
        v3.freeze(OUT / "quirk-keys" / f"{task}.json", v3.read(v3.KEYS_DIR / f"{task}.json"))
        v3.load_key(task)
    random.Random(v3.SEED).shuffle(rows)
    manifest, signals = [], {}
    with v3._v2_writing_to(OUT):
        for i, row in enumerate(rows):
            ident = f"{LEVEL}-{i + 1:03d}"
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
    repeats = [manifest[0]["id"]]  # one repeat for the cell, as v3.30 gave each cell
    pairs: list[list[str]] = []  # one level: no pairwise check, as in v3.26.5
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
    base_protocol = v3.read(BASE_OUT / "protocol.json")
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "no new exam: Muse Spark 1.3's v3.30 verdict gates this level",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {"v3.30": str(BASE_OUT.relative_to(ROOT))},
        "extends": {
            "directories": [str(r.relative_to(ROOT)) for r in JUDGED],
            "protocol_sha256": {
                str(r.relative_to(ROOT)): v3.sha(r / "protocol.json") for r in JUDGED
            },
            "calibration": {
                p: {
                    "path": str((root / f"calibration-{p}.json").relative_to(ROOT)),
                    "sha256": v3.sha(root / f"calibration-{p}.json"),
                    "protocol_id": protocol_id,
                    "protocol_sha256": v3.sha(root / "protocol.json"),
                }
                for p, (root, protocol_id) in VERDICTS.items()
            },
            "rule": "A further level of the same Fable 5.2 sweep: Muse Spark 1.3's v3.30 calibration "
            "verdict gates it; no new calibration calls. Grok 4.6 failed the v3.30 exam and takes no "
            "part. Publication merges the directories; no earlier record is rewritten.",
        },
        "population": {
            "description": f"Claude Fable 5.2 at {LEVEL} through Claude Code 2.1.321 on the claude.ai "
            "subscription with the refusal fallback on and thinking summaries recorded, one attempt "
            "per task, run from bench/2026-11-09-fable52-frontier-v4 on 2026-11-10 and 2026-11-11 "
            "after the weekly Fable limit reset; rows carry the solver CLI version and fallback replies",
            "cells": {f"{m}/{e}": counts[m, e] for m, e in sorted(counts)},
            "excluded": record.get("excluded", []),
            "missing": record.get("missing", []),
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
        "protocol_document_frozen_copy": str(v3.DOC.relative_to(OUT)),
        "protocol_document_source_sha256": v3.sha(SOURCE_DOC),
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
        "reviewers": {"muse": muse},
        "scored_siblings": {},
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": {muse["muse"]: {"sha256": muse["binary_sha256"]}},
        "planned_calls": {
            "calibration_per_panel": 0,
            "primary_per_panel": n + len(repeats) + 2 * len(pairs),
            "probe_and_match_per_panel": 2 * n,
        },
        "single_panel_rule": "publish from passing panels; a failed panel is disclosed as a sensitivity table; "
        "both failing stops the revision",
        "comparability": "Muse Spark 1.3 alone, as v3.30 published Low to Extra high; GPT-6.1 Sol's seat "
        "follows in v3.30.3 if it passes its v3.30.2 exam",
        "weights": base_protocol["weights"],
        "invalid_response_retries": 1,
    }
    for field in SAME_AS_BASE:
        if protocol[field] != base_protocol[field]:
            raise ValueError(f"Protocol field {field} differs from v3.30")
    v3.freeze(OUT / "protocol.json", protocol)
    sizes = [
        len(v3.prompt("review", v3.read(OUT / "evidence" / f"{r['id']}.json"))) for r in manifest
    ]
    result = {
        "submissions": n,
        "cells": protocol["population"]["cells"],
        "excluded": len(protocol["population"]["excluded"]),
        "missing": len(protocol["population"]["missing"]),
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
