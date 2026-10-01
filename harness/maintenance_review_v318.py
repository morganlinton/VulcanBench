"""Code quality maintenance v3.18: the same protocol applied to GPT-6.1 Sol on Frontier v4.

Nothing about the rubric, controls, quirk keys, gates, repeats, seed, weights
or the judges changes from v3.7. What changes is the population: the
September 2026 GPT-6.1 Sol effort sweep through Codex CLI 0.159.0 (pinned for
this model, docs/DECISIONS.md 2026-09-29), five levels, 115 runs. Runs that
hit the flat 3-hour task bound are unfinished and are excluded as incomplete
source runs; they still count as failed tasks in pass rates.

Both neutral judges, Muse Spark 1.3 and Grok 4.6, are neutral for an OpenAI
submission and retake the exam under this protocol before any counted call.
There are no sensitivity panels. The protocol document is frozen as a copy
inside the run directory, as v3.15 to v3.17 do.

The judging runs with no solver sweep active: the owner held the Grok 4.7
chain until it finishes (docs/DECISIONS.md, 2026-09-29), because the Grok
judge and the Grok 4.7 chain both run through Cursor.

The frozen v3 implementation is reused as a library. This module rebinds the
v3 module's population and directory constants at import time and replaces
only ``prepare``. Every other stage (calibrate, run, probe, summarize) is v3's
own code.

    python scripts/cii-v4-board/build_gpt61sol_population.py
    python -m harness.maintenance_review_v318 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v318 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel muse
"""

from __future__ import annotations

import hashlib
import random
import re
import subprocess
import tempfile
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.18"
OUT = ROOT / "runs-code-quality-maintenance-v3.18"
COMPARISON = ROOT / "docs/results/swe-v4-gpt61-sol-2026-09/comparison.json"
GROK_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.3/protocol.json"
MUSE_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.4/protocol.json"
MODELS = {"gpt61sol": tuple(base.LEVELS)}
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
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
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare


def evidence_for(row: dict) -> dict:  # noqa: PLR0912, one branch per precondition
    """v2's reconstruction, with one fallback for runs that add binary test fixtures.

    First seen under v3.18: three GPT-6.1 Sol runs wrote test fixtures that git
    treats as binary (or whose carriage returns the text-mode capture altered),
    so the saved text patch cannot be re-applied: v2's own recovery reapplies a
    plain diff, which carries no binary data. The fallback keeps v2's provenance
    check exactly (the saved workspace's staged text diff, normalized the way the
    original capture was, must equal the run's patch) and then applies the same
    index's ``--binary`` diff instead. Every file is then read and classified as
    v2 does; the method is frozen under reconstruction/.
    """
    try:
        return v2.evidence_for(row)
    except subprocess.CalledProcessError:
        pass
    data = base.inputs(Path(row["source_directory"]), v2.TASKS)
    if data["source_hashes"] != row["source_hashes"]:
        raise ValueError("Original evidence hash mismatch")
    paths = []
    for line in data["patch"].splitlines():
        if line.startswith("diff --git "):
            match = re.fullmatch(r"diff --git a/(\S+) b/(\S+)", line)
            if not match:
                raise ValueError("Unsupported quoted patch path")
            for name in match.groups():
                if Path(name).is_absolute() or ".." in Path(name).parts:
                    raise ValueError("Unsafe patch path")
            paths.append(match.group(2))
    saved = Path(row["source_directory"]) / "workspace"
    diff = ["git", "diff", "--cached", "--no-ext-diff", "--no-textconv"]
    text = subprocess.check_output(diff, cwd=saved)
    normalized = text.decode("utf-8", errors="replace").replace("\r\n", "\n").replace("\r", "\n")
    if normalized != data["patch"]:
        raise ValueError(f"Cannot reconstruct {row['run_id']}: snapshot differs from saved patch")
    binary_diff = subprocess.check_output([*diff, "--binary", "--full-index"], cwd=saved)
    files, binary = {}, {}
    with tempfile.TemporaryDirectory(prefix="vb-review-reconstruct-") as temp:
        work = Path(temp) / "repo"
        v2.safe_copy(v2.TASKS / row["task"] / "repo", work)
        subprocess.run(
            ["git", "apply", "--"], cwd=work, input=binary_diff, check=True, capture_output=True
        )
        for p in sorted(work.rglob("*")):
            if p.is_symlink():
                raise ValueError("Symlink in candidate patch")
            if not p.is_file():
                continue
            name = p.relative_to(work).as_posix()
            if ".git" in p.relative_to(work).parts:
                raise ValueError("Unexpected git directory")
            raw = p.read_bytes()
            try:
                decoded = raw.decode("utf-8")
            except UnicodeDecodeError:
                decoded = None
            if b"\0" in raw or decoded is None:
                binary[name] = {"sha256": base.digest(raw), "bytes": len(raw)}
            elif p.suffix in {".py", ".md"} or name in paths:
                files[name] = decoded
    v3.freeze(
        OUT / "reconstruction" / f"{row['run_id']}.json",
        {
            "method": "saved index --binary diff applied; its text-mode diff, normalized as the original "
            "capture was, equals the run's patch",
            "text_diff_sha256": base.digest(text),
            "binary_diff_sha256": base.digest(binary_diff),
        },
    )
    return {
        "issue": data["issue"],
        "candidate_patch": data["patch"],
        "final_files": files,
        "binary_manifest": binary,
    }


def prepare() -> None:  # noqa: PLR0915, one linear freeze
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
            evidence = evidence_for(row)
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
                    if r["model"] == "gpt61sol" and r["task"] == task and r["effort"] == effort
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
            "v3.6": "runs-code-quality-maintenance-v3.6",
            "v3.6.1": "runs-code-quality-maintenance-v3.6.1",
            "v3.7": "runs-code-quality-maintenance-v3.7",
            "v3.15": "runs-code-quality-maintenance-v3.15",
            "v3.16": "runs-code-quality-maintenance-v3.16",
            "v3.17": "runs-code-quality-maintenance-v3.17",
        },
        "population": {
            "description": "GPT-6.1 Sol (five levels) through Codex CLI 0.159.0 on the ChatGPT subscription, "
            "on the coding-intelligence-index-v4 suite, one attempt per task and level; runs stopped by the "
            "flat 3-hour task bound are incomplete source runs and are listed under excluded",
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
