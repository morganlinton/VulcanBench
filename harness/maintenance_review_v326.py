"""Code quality maintenance v3.26: Opus 5.5 takes the second seat for Muse Wonder Full on Frontier v4.

v3.25 seated Grok 4.7 (through Cursor) and GPT-6.1 Sol (through Codex). Sol took
the exam, passed it and finished every review, probe and match under v3.25. Grok
stopped on its first exam call because the Cursor account ran out of usage
("You're out of usage", 2026-10-15 17:57), so v3.25 cannot summarize. The owner
replaced the Grok seat with Opus 5.5 (chat, 2026-10-15).

Changing a seat changes the protocol, and v3.25's votes are bound to its
protocol hash, so the new seat runs as its own round with Sol attached as a
scored sibling (the v3.4 and v3.13 pattern): Sol's v3.25 directory is read as
it stands, its protocol hash is recorded here and re-checked before every
stage, and its evidence must be byte-identical to this round's. Nothing in
v3.25 is rewritten; its Grok seat stays recorded as stopped.

- "claude": Claude Opus 5.5 at medium through Claude Code 2.1.300 on the
  subscription, on v3's frozen Claude transport (``claude_vote`` with the
  identity, isolation and overage guards). Claude Code updates itself in place,
  so the judge runs from a copy of the 2.1.300 binary under
  ~/.local/share/vulcanbench, pinned by sha256. Opus 5.5 is neutral for a Meta
  submission. It is also one of the models the comparison card sets beside Muse
  Wonder Full; its own published score comes from its own round's panel, not
  from this one. It takes the exam under this protocol before any counted call.
- "sol": GPT-6.1 Sol, the v3.25 seat, unchanged and not re-run.

Rubric, controls, quirk keys, gates, repeats, seed, weights and population are
v3.25's. The two missing Medium runs are judged later in a v3.26.1 top-up gated
by this round's Opus verdict and v3.25's Sol verdict.

The frozen v3 implementation is reused as a library. This module rebinds the v3
module's population and directory constants at import time and replaces
``prepare``. Every other stage (calibrate, run, probe, summarize) is v3's own
code, and the Claude seat uses v3's own ``call``.

    cp -a <v3.25 run worktree>/runs-code-quality-maintenance-v3.25 .
    python -m harness.maintenance_review_v326 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v326 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel claude
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
PROTOCOL_ID = "code-quality-maintenance-v3.26"
OUT = ROOT / "runs-code-quality-maintenance-v3.26"
SOL_OUT = ROOT / "runs-code-quality-maintenance-v3.25"  # scored sibling: Sol on this population
SOL_PROTOCOL_ID = "code-quality-maintenance-v3.25"
COMPARISON = ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison.json"
MODEL = "musewonderfull"
MODELS = {MODEL: ("low", "medium")}
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
PANELS = ("claude",)
SCORED_SIBLING_OUT = {"sol": SOL_OUT}
SCORED_PANELS = ("claude", "sol")
SUBMISSION_LAB = "Meta"
EXCLUDED_JUDGES = {
    "muse": "Muse Spark 1.3 is a Meta model and is not neutral for a Meta submission "
    "(owner decision in chat, 2026-10-15)",
}
REPLACED_SEATS = {
    "grok": "Grok 4.7 through Cursor, seated in v3.25, stopped on its first exam call when the Cursor "
    "account ran out of usage (2026-10-15 17:57); replaced by Opus 5.5 (owner decision in chat, 2026-10-15)",
}
TOP_UP = {
    "round": "v3.26.1",
    "protocol_id": "code-quality-maintenance-v3.26.1",
    "directory": "runs-code-quality-maintenance-v3.26.1",
    "population": "docs/results/swe-v4-musewonder-full-2026-10/comparison-topup.json, built by "
    "scripts/cii-v4-board/build_musewonder_population.py --top-up",
    "rule": "only the runs listed under this protocol's population.missing, once they exist; this "
    "round's Opus verdict and v3.25's Sol verdict gate them with no new exam (the v3.6.1 precedent); "
    "each top-up submission gets a primary and a repeat review from both seats and no pairwise check; "
    "publication merges the directories and neither record is rewritten",
}

CLAUDE_MODEL = "claude-opus-5-5"
CLAUDE_VERSION = "2.1.300"
CLAUDE = Path.home() / ".local/share/vulcanbench/claude-2.1.300/claude"
CLAUDE_SHA256 = "475517caa611e3318a2f298fa071287ec98955538df78c58f709ecc01e5ae2d3"


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    if "muse" in SCORED_PANELS:
        raise ValueError("Muse Spark 1.3 is excluded from judging a Meta submission")
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = SCORED_SIBLING_OUT
    v3.SCORED_PANELS = SCORED_PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare


# --- judge pin ----------------------------------------------------------------


def pinned_claude(path: Path = CLAUDE) -> dict:
    """The copied 2.1.300 binary must exist and match its sha256; v3's verify_frozen re-checks it every stage."""
    if not path.is_file():
        raise ValueError(f"Claude CLI not found at {path}")
    if v3.sha(path) != CLAUDE_SHA256:
        raise ValueError(f"Claude CLI at {path} differs from its pinned sha256")
    return {"path": str(path), "version": CLAUDE_VERSION, "sha256": CLAUDE_SHA256}


def claude_settings(pin: dict) -> dict:
    return {
        "model": CLAUDE_MODEL,
        "effort": "medium",
        "claude": pin["path"],
        "claude_version": pin["version"],
        "timeout": 900,
        "lab": "Anthropic",
        "tier": "Claude subscription, overage disabled; the v3 guard stops on any overage or a quota above 80%",
        "prompt_delivery": "stdin",
        "system_prompt": "--system-prompt",
        "identity": "every assistant message must report claude-opus-5-5; any model fallback fails the call",
        "neutrality": "neutral for Meta submissions",
        "comparability": "Opus 5.5 also appears on the comparison card as a compared model, scored by its own round",
        "seat": "replaces the v3.25 Grok seat; takes the exam under this protocol before any counted call",
    }


def sibling_record(root: Path = SOL_OUT) -> dict:
    """Sol's v3.25 directory as it stands: its own protocol, a passed exam, and the same population."""
    protocol = v3.read(root / "protocol.json")
    if protocol["id"] != SOL_PROTOCOL_ID:
        raise ValueError(f"{root} is not the {SOL_PROTOCOL_ID} record")
    gate = v3.read(root / "calibration-sol.json")
    if gate["passed"] is not True or gate["protocol_sha256"] != v3.sha(root / "protocol.json"):
        raise ValueError("Sol has no passing exam under its own frozen protocol")
    return {
        "directory": str(root),
        "protocol_id": SOL_PROTOCOL_ID,
        "protocol_sha256": v3.sha(root / "protocol.json"),
        "calibration_sha256": v3.sha(root / "calibration-sol.json"),
        "model": protocol["reviewers"]["sol"]["model"],
        "role": "scored panel with equal weight, run under its own frozen protocol",
    }


# --- population (as v3.25; copied, since importing that module rebinds v3) ----


def check_population(record: dict) -> tuple[list[dict], list[dict], list[dict], Counter]:
    """Every expected cell present, and each cell's rows plus gaps make the 23-task suite."""
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
    if any(not e.get("reason") for e in excluded + missing):
        raise ValueError("Every exclusion and missing run needs a reason")
    if len({r["task"] for r in rows}) != 23:
        raise ValueError("Task coverage is not the 23-task suite")
    return rows, excluded, missing, counts


def diagnostic_selection(manifest: list[dict], tasks: set[str]) -> dict:
    """One repeat per cell; one Low-versus-Medium pair per level, as v3.25 selects them."""
    expected = sorted({(r["model"], r["effort"]) for r in manifest})
    repeats = [
        next(r["id"] for r in manifest if (r["model"], r["effort"]) == cell) for cell in expected
    ]
    ordered_tasks = sorted(
        tasks, key=lambda t: hashlib.sha256(f"{v3.SEED}:{t}".encode()).hexdigest()
    )
    levels = list(MODELS[MODEL])
    pairs: list[list[str]] = []
    for task in ordered_tasks:
        if len(pairs) == len(levels):
            break
        i = len(pairs)
        ids = [
            next(
                (
                    r["id"]
                    for r in manifest
                    if r["model"] == MODEL and r["task"] == task and r["effort"] == effort
                ),
                None,
            )
            for effort in (levels[i], levels[(i + 1) % len(levels)])
        ]
        if all(ids):
            pairs.append([str(x) for x in ids])
    return {"repeats": repeats, "pairs": pairs}


# Files that must be byte-identical to the sibling's, so both seats judge the same prompts.
# The protocol document is not among them: it gains this round's amendment section.
SHARED_FILES = (
    "private-manifest.json",
    "signals.json",
    "diagnostic-selection.json",
    "calibration-order.json",
)
# Protocol fields that decide what a judge sees and how it is scored; they must match the sibling's.
SHARED_PROTOCOL_FIELDS = (
    "rubric",
    "system",
    "pair_instruction",
    "probe_instruction",
    "match_instruction",
    "schemas",
    "seed",
    "repeats",
    "gate_allowance",
    "weights",
    "source_comparison_sha256",
    "manifest_sha256",
    "signals_sha256",
    "selection_sha256",
    "calibration_order_sha256",
    "control_source_hashes",
    "control_hashes",
    "quirk_key_hashes",
    "ledger_key",
    "population",
)


def require_same_protocol(protocol: dict, sibling: dict) -> None:
    for field in SHARED_PROTOCOL_FIELDS:
        if protocol[field] != sibling[field]:
            raise ValueError(f"Protocol field {field} differs from {SOL_PROTOCOL_ID}")


def require_same_inputs(out: Path = OUT, sibling: Path = SOL_OUT) -> None:
    for name in SHARED_FILES:
        if v3.sha(out / name) != v3.sha(sibling / name):
            raise ValueError(
                f"{name} differs from {sibling}; both seats must judge the same inputs"
            )
    for folder in ("evidence", "controls", "quirk-keys"):
        mine = {p.name: v3.sha(p) for p in sorted((out / folder).glob("*.json"))}
        theirs = {p.name: v3.sha(p) for p in sorted((sibling / folder).glob("*.json"))}
        if mine != theirs:
            raise ValueError(
                f"{folder}/ differs from {sibling}; both seats must judge the same inputs"
            )


def prepare() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    pin = pinned_claude()
    reviewers = {"claude": claude_settings(pin)}
    if any(r["lab"] == SUBMISSION_LAB for r in reviewers.values()):
        raise ValueError("Judges must be neutral for a Meta submission")
    sibling = sibling_record()
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    rows, excluded, missing, counts = check_population(v3.read(COMPARISON))
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
    selection = diagnostic_selection(manifest, tasks)
    v3.freeze(OUT / "diagnostic-selection.json", selection)
    require_same_inputs()
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
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "automated held-out controls, five repeats, gates 1 to 17 with a one-gate 0.5 shortfall allowance, "
        "line-level excerpt rule with elision markers; Opus 5.5 takes the exam under this protocol; Sol's "
        "v3.25 verdict gates the sibling seat",
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
            "v3.20": "runs-code-quality-maintenance-v3.20",
            "v3.24": "runs-code-quality-maintenance-v3.24",
            "v3.25": "runs-code-quality-maintenance-v3.25",
        },
        "population": {
            # v3.25's description, cells and missing list verbatim; require_same_protocol checks them
            **v3.read(SOL_OUT / "protocol.json")["population"],
            "cells": {f"{m}/{e}": counts[m, e] for m, e in sorted(counts)},
            "excluded": excluded,
            "missing": missing,
        },
        "top_up": {
            **TOP_UP,
            "cells": {
                f"{m}/{e}": sorted(
                    x["task"] for x in missing if (x["model"], x["effort"]) == (m, e)
                )
                for m, e in sorted({(x["model"], x["effort"]) for x in missing})
            },
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
        "excluded_judges": EXCLUDED_JUDGES,
        "replaced_seats": REPLACED_SEATS,
        "judge_pins": {"claude": pin},
        "scored_siblings": {"sol": sibling},
        "shares_evidence_with": {
            "protocol_sha256": sibling["protocol_sha256"],
            "manifest_sha256": v3.sha(SOL_OUT / "private-manifest.json"),
        },
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": {pin["path"]: {"sha256": pin["sha256"], "version": pin["version"]}},
        "planned_calls": {
            "calibration_per_panel": 80,
            "primary_per_panel": n + len(selection["repeats"]) + 2 * len(selection["pairs"]),
            "probe_and_match_per_panel": 2 * n,
        },
        "single_panel_rule": "publish from passing panels; a failed panel is disclosed as a sensitivity table; "
        "both failing stops the revision",
        "comparability": "a Muse Wonder Full score is a mean of Opus 5.5 and GPT-6.1 Sol; other models' "
        "published scores come from their own rounds and panels, and any card placing them side by side says so",
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
    require_same_protocol(protocol, v3.read(SOL_OUT / "protocol.json"))
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
        "same_inputs_as": str(SOL_OUT.relative_to(ROOT)),
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
