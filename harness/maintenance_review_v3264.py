"""Code quality maintenance v3.26.4: the Extra-high depotcore rerun, under the v3.26 judges.

Extra-high depotcore first hit the 3-hour bound while another session's
Frontier v5 jobs ran beside it for 145 of its 180 minutes, so v3.26.3 judged
Extra-high on 22 runs. Under the owner's overlap rule (flag overlapped tasks,
rerun any that look inflated) it was rerun alone on Muse Code 1.4.9; the
overlapped attempt is kept in the sweep's overlap-superseded/ folder and
disclosed (docs/DECISIONS.md, 2026-10-17). This top-up judges only the rerun,
under the same rubric, controls, quirk keys, gates and seed, with the same two
judges and no new exam (Opus 5.5 by its v3.26 verdict, GPT-6.1 Sol by its
v3.25 verdict), on the v3.26.1 precedent: a primary and a repeat review from
both judges, a probe and a match, no pairwise check. No earlier record is
rewritten; publication merges this directory with v3.26.3 and Extra-high
reaches 23 judged runs.

    python scripts/cii-v4-board/build_musewonder_population.py --level extra-high \\
        --only legacy-depotcore-binary-parity
    python -m harness.maintenance_review_v3264 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3264 \\
        python -m harness.maintenance_review_v3_resume run --panel claude
"""

from __future__ import annotations

import random
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import maintenance_review_v3261 as v3261
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.26.4"
OUT = ROOT / "runs-code-quality-maintenance-v3.26.4"
LEVEL = "extra-high"
COMPARISON = ROOT / f"docs/results/swe-v4-musewonder-full-2026-10/comparison-{LEVEL}-topup.json"
SOURCE_DOC = v3261.SOURCE_DOC
MODEL = v3261.MODEL
MODELS = {MODEL: (LEVEL,)}
PANELS = v3261.PANELS
BASE_OUT = v3261.BASE_OUT
BASE_ID = v3261.BASE_ID
TOPUP_OUT = v3261.OUT
HIGH_OUT = ROOT / "runs-code-quality-maintenance-v3.26.2"
LEVEL_OUT = ROOT / "runs-code-quality-maintenance-v3.26.3"  # Extra-high, 22 judged; this tops it up
LEVEL_ID = "code-quality-maintenance-v3.26.3"
JUDGED = (
    BASE_OUT,
    TOPUP_OUT,
    HIGH_OUT,
    LEVEL_OUT,
)  # rounds whose runs this one must not judge again


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3261._bind()  # the v3.26.1 judge gates, Sol routing and stream parser
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare


def check_population(
    record: dict, judged: list[dict], allowed: set[tuple[str, str, str]]
) -> Counter:
    """Only runs v3.26.3 excluded, none judged before, and the level then reaching 23 with v3.26.3's rows."""
    rows = record["rows"]
    excluded = record.get("excluded", [])
    missing = record.get("missing", [])
    counts = Counter((r["model"], r["effort"]) for r in rows)
    if set(counts) != {(MODEL, LEVEL)}:
        raise ValueError(f"Cells present {sorted(counts)}; expected only {MODEL}/{LEVEL}")
    already = {(r["model"], r["effort"], r["task"]) for r in judged}
    for r in rows:
        key = (r["model"], r["effort"], r["task"])
        if key in already:
            raise ValueError(f"{r['task']} at {r['effort']} was already judged")
        if key not in allowed:
            raise ValueError(f"{r['task']} at {r['effort']} was not excluded under v3.26.3")
    in_level = sum(1 for r in judged if (r["model"], r["effort"]) == (MODEL, LEVEL))
    gaps = Counter((e["model"], e["effort"]) for e in excluded + missing)
    if counts[MODEL, LEVEL] + in_level + gaps.get((MODEL, LEVEL), 0) != 23:
        raise ValueError(
            f"{LEVEL} has {counts[MODEL, LEVEL]} new rows, {in_level} judged before and "
            f"{gaps.get((MODEL, LEVEL), 0)} gaps, not 23"
        )
    if any(not e.get("reason") for e in excluded + missing):
        raise ValueError("Every exclusion and missing run needs a reason")
    if any(r.get("solver_cli_version") != "Muse Code 1.4.9 (1.4.9-R6297.1)" for r in rows):
        raise ValueError(f"Every {LEVEL} run must be from Muse Code 1.4.9")
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
        raise ValueError("The v3.26 record is not final")
    if not all(v3261.panel_passed(p) for p in PANELS):
        raise ValueError(
            "Both judges must have passed their exams under their own frozen protocols"
        )
    reviewers, binaries = v3261.pinned_reviewers()
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    record = v3.read(COMPARISON)
    judged = [r for root in JUDGED for r in v3.read(root / "private-manifest.json")]
    level_protocol = v3.read(LEVEL_OUT / "protocol.json")
    if level_protocol["id"] != LEVEL_ID:
        raise ValueError(f"{LEVEL_OUT} is not the {LEVEL_ID} record")
    allowed = {
        (e["model"], e["effort"], e["task"]) for e in level_protocol["population"]["excluded"]
    }
    counts = check_population(record, judged, allowed)
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
            ident = f"{LEVEL}-topup-{i + 1:03d}"
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
    repeats = [r["id"] for r in manifest]  # a top-up this small repeats every submission (v3.26.1)
    pairs: list[list[str]] = []
    v3.freeze(OUT / "diagnostic-selection.json", {"repeats": repeats, "pairs": pairs})
    code = [
        Path(__file__),
        Path(v3261.__file__),
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
        "calibration": "no new exam: Opus 5.5's v3.26 verdict and GPT-6.1 Sol's v3.25 verdict gate this level",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {
            "v3.25": "runs-code-quality-maintenance-v3.25",
            "v3.26": "runs-code-quality-maintenance-v3.26",
            "v3.26.1": "runs-code-quality-maintenance-v3.26.1",
            "v3.26.2": "runs-code-quality-maintenance-v3.26.2",
            "v3.26.3": "runs-code-quality-maintenance-v3.26.3",
        },
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
                for p, (root, protocol_id) in v3261.VERDICTS.items()
            },
            "rule": "A top-up of v3.26.3: only the Extra-high runs it excluded, rerun clean after an "
            "overlapped 3-hour timeout. The v3.26 (Opus 5.5) and v3.25 (GPT-6.1 Sol) calibration verdicts "
            "gate it; no new calibration calls. Publication merges the directories; no earlier record is rewritten.",
        },
        "population": {
            "description": f"Muse Wonder Full at {LEVEL}: the run v3.26.3 excluded, rerun alone through Muse Code 1.4.9 in "
            "runs-effort-musewonder-full-v2-149 (goal and verify reminder observers on Muse Spark 1.3, "
            "recorded per row; docs/DECISIONS.md 2026-10-16)",
            "solver": record["solver"],
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
        "reviewers": reviewers,
        "scored_siblings": {},
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": binaries,
        "planned_calls": {
            "calibration_per_panel": 0,
            "primary_per_panel": n + len(repeats) + 2 * len(pairs),
            "probe_and_match_per_panel": 2 * n,
        },
        "single_panel_rule": "publish from passing panels; a failed panel is disclosed as a sensitivity table; "
        "both failing stops the revision",
        "comparability": "a Muse Wonder Full score is a mean of Opus 5.5 and GPT-6.1 Sol, as in v3.26",
        "weights": base_protocol["weights"],
        "invalid_response_retries": 1,
    }
    for field in (
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
    ):
        if protocol[field] != base_protocol[field]:
            raise ValueError(f"Protocol field {field} differs from v3.26")
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
