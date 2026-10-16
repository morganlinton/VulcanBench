"""Code quality maintenance v3.26.1: the Muse Wonder Full Medium top-up under the v3.26 judges.

v3.25 froze cellarcore and paddockcore at Medium as missing: the Muse Code
weekly limit had stopped the sweep. v3.26 kept that population and seated
Claude Opus 5.5 beside GPT-6.1 Sol (v3.25's seat, a scored sibling). The two
tasks then ran through Muse Code 1.4.9 in runs-effort-musewonder-full-v2-149
(owner decision 2026-10-16, docs/DECISIONS.md). This amendment judges those
runs, once they exist, under the same rubric, controls, quirk keys, gates and
seed, with both judges gated by the exams they already passed: Opus 5.5 by
v3.26's verdict and Sol by v3.25's (the v3.6.1 precedent: the exam is per
population, and these runs belong to it). Neither record is rewritten;
publication merges the directories and the Medium cell reaches 23.

Each top-up submission gets a primary and a repeat review from both judges and
no pairwise check. The frozen v3 implementation is reused as a library: this
module rebinds the v3 module's population and directory constants at import
time and replaces ``prepare``, the calibration gate, ``call`` (the Sol seat
runs on the Codex transport, as in v3.20 and v3.25) and ``parse_stream_for``.
Every other stage (run, probe, summarize) is v3's own code.

    python scripts/cii-v4-board/build_musewonder_population.py --top-up
    python -m harness.maintenance_review_v3261 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3261 \\
        python -m harness.maintenance_review_v3_resume run --panel claude
"""

from __future__ import annotations

import json
import random
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.26.1"
OUT = ROOT / "runs-code-quality-maintenance-v3.26.1"
BASE_ID = "code-quality-maintenance-v3.26"
BASE_OUT = ROOT / "runs-code-quality-maintenance-v3.26"  # the Opus seat and the population
SOL_ID = "code-quality-maintenance-v3.25"
SOL_OUT = ROOT / "runs-code-quality-maintenance-v3.25"  # the Sol seat
COMPARISON = ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison-topup.json"
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
MODEL = "musewonderfull"
MODELS = {MODEL: ("medium",)}
PANELS = ("claude", "sol")
# Where each judge's exam verdict lives, and the protocol it was taken under.
VERDICTS = {"claude": (BASE_OUT, BASE_ID), "sol": (SOL_OUT, SOL_ID)}


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
    v3.call = call
    v3.parse_stream_for = parse_stream_for


def panel_passed(panel: str) -> bool:
    """A judge is calibrated for this top-up if it passed its exam under that round's frozen protocol."""
    if panel not in VERDICTS:
        return False
    root, protocol_id = VERDICTS[panel]
    path = root / f"calibration-{panel}.json"
    if not path.exists() or v3.read(root / "protocol.json")["id"] != protocol_id:
        return False
    gate = v3.read(path)
    return bool(gate["passed"]) and gate["protocol_sha256"] == v3.sha(root / "protocol.json")


# --- transport ----------------------------------------------------------------

_frozen_call = v3.call
_frozen_parse_stream_for = v3.parse_stream_for


def call(panel: str, stage: str, name: str, kind: str, payload: dict, protocol: dict) -> dict:
    """The frozen call, with the Codex transport for the Sol seat (as in v3.20 and v3.25).

    v3's call selects the transport by panel name from a literal inside the
    function, so the Sol panel is routed here: the same receipts, binding,
    validation, retry and scoring path as the frozen function, with
    codex_vote as the transport. The Claude seat goes to the frozen call.
    """
    if panel != "sol":
        return _frozen_call(panel, stage, name, kind, payload, protocol)
    folder = v3.OUT / "calls" / panel / stage / name
    folder.mkdir(parents=True, exist_ok=True)
    text = v3.prompt(kind, payload)
    binding = {
        "protocol_sha256": v3.sha(v3.OUT / "protocol.json"),
        "prompt_sha256": base.digest(text.encode()),
        "kind": kind,
    }
    final = folder / "selected.json"
    if final.exists():
        vote = v3.read(final)
        if vote["binding"] != binding:
            raise ValueError("Cached response binding changed")
        v3.validate(kind, vote, payload)
        return vote
    settings = protocol["reviewers"][panel]
    for attempt in range(2):
        attempt_name = f"attempt-{attempt + 1}"
        receipt = folder / f"{attempt_name}.json"
        if receipt.exists():
            previous = v3.read(receipt)
            if previous.get("retryable") is True and previous.get("binding") == binding:
                continue
            raise RuntimeError("Prior non-retryable attempt requires operator review")
        try:
            vote = v3.codex_vote(text, folder, attempt_name, settings, v3.KIND_SCHEMA[kind])
            v3.validate(kind, vote, payload)
            if kind == "review":
                vote["reported_score"] = vote["score"]
                vote.update(v3.host_review_score(vote))
        except (json.JSONDecodeError, ValueError) as exc:
            retryable = isinstance(exc, json.JSONDecodeError) or str(exc) in v3.RETRYABLE
            base.save(
                receipt,
                {"status": "failed", "retryable": retryable, "error": str(exc), "binding": binding},
            )
            if not retryable:
                raise
            continue
        except Exception as exc:
            base.save(
                receipt,
                {"status": "failed", "retryable": False, "error": str(exc), "binding": binding},
            )
            raise
        vote.update(binding=binding, status="complete", stage=stage, panel=panel, kind=kind)
        vote.pop("api_equivalent_estimate_usd", None)
        base.save(receipt, vote)
        base.save(final, vote)
        print(
            base.canonical(
                {
                    "event": f"{kind}_complete",
                    "panel": panel,
                    "stage": stage,
                    "id": name,
                    "score": vote.get("score"),
                    "duration_s": vote.get("duration_s"),
                }
            ),
            flush=True,
        )
        return vote
    raise RuntimeError("Both response attempts failed; no further automatic retry")


def parse_stream_for(panel: str, text: str) -> dict:
    if panel == "sol":
        return v3.parse_codex_stream(text)
    return _frozen_parse_stream_for(panel, text)


# --- judges and population ----------------------------------------------------


def pinned_reviewers() -> tuple[dict, dict]:
    """Both seats' settings from the protocols they passed under; their binaries must still match."""
    base_protocol = v3.read(BASE_OUT / "protocol.json")
    sol_protocol = v3.read(SOL_OUT / "protocol.json")
    claude = base_protocol["reviewers"]["claude"]
    sol = sol_protocol["reviewers"]["sol"]
    binaries = {}
    claude_path = claude["claude"]
    if v3.sha(Path(claude_path)) != base_protocol["binaries"][claude_path]["sha256"]:
        raise ValueError("Claude CLI differs from the v3.26 pin")
    binaries[claude_path] = base_protocol["binaries"][claude_path]
    codex = sol_protocol["judge_pins"]["codex"]
    for name in ("launcher", "native"):
        path = codex[name]["path"]
        if not Path(path).is_file() or v3.sha(Path(path)) != codex[name]["sha256"]:
            raise ValueError(f"Codex {name} differs from the v3.25 pin")
        binaries[path] = {"sha256": codex[name]["sha256"]}
    return {"claude": claude, "sol": sol}, binaries


def check_population(
    record: dict, base_manifest: list[dict], frozen_missing: list[dict]
) -> Counter:
    """Only runs v3.26 froze as missing, none already judged, and each cell then reaches 23."""
    rows = record["rows"]
    excluded = record.get("excluded", [])
    missing = record.get("missing", [])
    counts = Counter((r["model"], r["effort"]) for r in rows)
    if not set(counts) <= {(m, e) for m, levels in MODELS.items() for e in levels}:
        raise ValueError(f"Unexpected cells {sorted(counts)}")
    already = {(r["model"], r["effort"], r["task"]) for r in base_manifest}
    allowed = {(m["model"], m["effort"], m["task"]) for m in frozen_missing}
    for r in rows:
        key = (r["model"], r["effort"], r["task"])
        if key in already:
            raise ValueError(f"{r['task']} at {r['effort']} was already judged under v3.26")
        if key not in allowed:
            raise ValueError(f"{r['task']} at {r['effort']} was not recorded missing under v3.26")
    judged = Counter((r["model"], r["effort"]) for r in base_manifest)
    gaps = Counter((e["model"], e["effort"]) for e in excluded + missing)
    for cell in counts:
        if counts[cell] + judged[cell] + gaps.get(cell, 0) != 23:
            raise ValueError(
                f"Cell {cell} has {counts[cell]} new rows, {judged[cell]} judged under v3.26 and "
                f"{gaps.get(cell, 0)} exclusions or missing, not 23"
            )
    return counts


def prepare() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    base_summary = v3.read(BASE_OUT / "summary.json")
    if not base_summary["ready_for_publication"] or base_summary["protocol"] != BASE_ID:
        raise ValueError("The v3.26 record is not final")
    if not all(panel_passed(p) for p in PANELS):
        raise ValueError(
            "Both judges must have passed their exams under their own frozen protocols"
        )
    reviewers, binaries = pinned_reviewers()
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    record = v3.read(COMPARISON)
    base_protocol = v3.read(BASE_OUT / "protocol.json")
    base_manifest = v3.read(BASE_OUT / "private-manifest.json")
    counts = check_population(record, base_manifest, base_protocol["population"]["missing"])
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
            ident = f"topup-{i + 1:03d}"
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
    repeats = [r["id"] for r in manifest]  # one repeat per submission in a top-up this small
    pairs: list[list[str]] = []
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
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "no new exam: Opus 5.5's v3.26 verdict and GPT-6.1 Sol's v3.25 verdict gate this top-up",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {
            "v3.25": "runs-code-quality-maintenance-v3.25",
            "v3.26": "runs-code-quality-maintenance-v3.26",
        },
        "top_up_of": {
            "directory": str(BASE_OUT.relative_to(ROOT)),
            "protocol_sha256": v3.sha(BASE_OUT / "protocol.json"),
            "summary_sha256": v3.sha(BASE_OUT / "summary.json"),
            "manifest_sha256": v3.sha(BASE_OUT / "private-manifest.json"),
            "calibration": {
                p: {
                    "path": str((root / f"calibration-{p}.json").relative_to(ROOT)),
                    "sha256": v3.sha(root / f"calibration-{p}.json"),
                    "protocol_id": protocol_id,
                    "protocol_sha256": v3.sha(root / "protocol.json"),
                }
                for p, (root, protocol_id) in VERDICTS.items()
            },
            "rule": "The v3.26 (Opus 5.5) and v3.25 (GPT-6.1 Sol) calibration verdicts gate this top-up; "
            "no new calibration calls. Publication merges the directories; neither record is rewritten.",
        },
        "population": {
            "description": "Top-up to the v3.26 Muse Wonder Full population: the Medium runs recorded "
            "missing under v3.25 and v3.26, made through Muse Code 1.4.9 after the quota reset "
            "(runs-effort-musewonder-full-v2-149; goal and verify reminder observers on Muse Spark 1.3, "
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
