"""Code quality maintenance v3.30.2: GPT-6.1 Sol takes the second seat on Claude Fable 5.2, Low to Extra high.

Grok 4.6 failed the v3.30 exam (gate g14_pair_2_1, an order-inconsistent pair)
and Muse Spark 1.3 published Low to Extra high alone. The owner chose GPT-6.1
Sol for the empty seat across every level (chat, 2026-11-11), as GPT-5.6 Sol
filled Grok's seat after the v3.9 exam (v3.10). Sol is neutral for an
Anthropic submission.

Nothing about the rubric, controls, quirk keys, gates, repeats, seed, weights
or the population changes: the population is the v3.30 freeze, byte for byte.
Its manifest, signals, diagnostic selection, evidence and quirk keys are copied
from the v3.30 directory and checked against the hashes the v3.30 protocol
recorded; controls and the calibration order are rebuilt and must match too.
Muse's v3.30 calibration, reviews and probes are scored from the v3.30
directory as a scored sibling. Only Sol takes the exam and the full pass here,
through the v3.20 Sol transport and the Codex CLI 0.159.0 pins of v3.27.

If Sol fails its exam it makes no counted call, and Muse alone stands as
v3.30 published it. Max gets Sol's seat in v3.30.3 under this verdict.

    python -m harness.maintenance_review_v3302 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3302 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel sol
"""

from __future__ import annotations

from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import maintenance_review_v327 as v327
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.30.2"
OUT = ROOT / "runs-code-quality-maintenance-v3.30.2"
BASE_ID = "code-quality-maintenance-v3.30"
BASE_OUT = ROOT / "runs-code-quality-maintenance-v3.30"  # scored sibling: Muse under v3.30
COMPARISON = ROOT / "docs/results/swe-v4-fable52-2026-11/comparison.json"
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
MODELS = {"fable52": ("low", "medium", "high", "extra-high")}
PANELS = ("sol",)
SCORED_PANELS = ("sol", "muse")
SUBMISSION_LAB = "Anthropic"
SAME_AS_BASE = (
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
    "ledger_key",
    "control_source_hashes",
    "control_hashes",
    "quirk_key_hashes",
    "calibration_order_sha256",
    "manifest_sha256",
    "signals_sha256",
    "selection_sha256",
    "source_comparison_sha256",
)
COPIED = ("private-manifest.json", "signals.json", "diagnostic-selection.json")


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {"muse": BASE_OUT}
    v3.SCORED_PANELS = SCORED_PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare
    v3.panel_passed = panel_passed
    v3.call = v327.call  # the Sol seat through Codex; anything else goes to the frozen call
    v3.parse_stream_for = v327.parse_stream_for
    check_codex_pins(OUT)


# Where each judge's exam verdict lives, and the protocol it was taken under.
VERDICTS = {"muse": (BASE_OUT, BASE_ID), "sol": (OUT, PROTOCOL_ID)}


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
    """Muse by its v3.30 verdict, Sol by the exam it takes here."""
    return verdict_passed(VERDICTS, panel)


def check_codex_pins(out: Path) -> None:
    """Before any stage after prepare: both pinned Codex files must be unchanged."""
    protocol_path = out / "protocol.json"
    if not protocol_path.exists():
        return
    codex = v3.read(protocol_path)["judge_pins"]["codex"]
    for name in ("launcher", "native"):
        path = Path(codex[name]["path"])
        if not path.is_file() or v3.sha(path) != codex[name]["sha256"]:
            raise RuntimeError(f"Pinned Codex {name} changed or is missing; stop the round")


def sol_reviewer() -> tuple[dict, dict]:
    """GPT-6.1 Sol at medium through the pinned Codex CLI 0.159.0, as v3.27 and v3.28 ran it."""
    codex = v327.pinned_codex()
    settings = {
        **v327.sol_settings(codex),
        "neutrality": "neutral for Anthropic submissions",
        "seat": "the v3.20 Sol seat and transport, in the seat Grok 4.6 left when it failed the v3.30 "
        "exam (g14_pair_2_1); owner choice, 2026-11-11; takes the exam under v3.30.2",
    }
    if settings["lab"] == SUBMISSION_LAB:
        raise ValueError("The Sol seat must be neutral for an Anthropic submission")
    return settings, codex


def copy_freeze(source: Path, out: Path, comparison: Path) -> dict:
    """Copy a judged round's population freeze byte for byte and check it against that round's record.

    Manifest, signals, selection, evidence and quirk keys are copied; controls and
    the calibration order are rebuilt from the v3 sources. Every file must hash to
    what the source protocol recorded. Returns the source protocol.
    """
    src = v3.read(source / "protocol.json")
    if v3.sha(comparison) != src["source_comparison_sha256"]:
        raise ValueError(f"{comparison.name} differs from the record {source.name} froze")
    out.mkdir(parents=True, exist_ok=True)
    for name in COPIED:
        v3.freeze(out / name, v3.read(source / name))
    manifest = v3.read(out / "private-manifest.json")
    for row in manifest:
        v3.freeze(out / "evidence" / f"{row['id']}.json", v3.read(source / "evidence" / f"{row['id']}.json"))
        if v3.sha(out / "evidence" / f"{row['id']}.json") != row["evidence_sha256"]:
            raise ValueError(f"Evidence {row['id']} differs from the {source.name} freeze")
    for name in src["quirk_key_hashes"]:
        v3.freeze(out / "quirk-keys" / name, v3.read(source / "quirk-keys" / name))
    for i, evidence in enumerate(v3.controls()):
        v3.freeze(out / "controls" / f"control-{i}.json", evidence)
    v3.freeze(
        out / "calibration-order.json",
        v3.interleaved_order(v3.SEED, len(v3.CONTROL_FILES), v3.REPEATS),
    )
    checks = {
        "manifest_sha256": v3.sha(out / "private-manifest.json"),
        "signals_sha256": v3.sha(out / "signals.json"),
        "selection_sha256": v3.sha(out / "diagnostic-selection.json"),
        "calibration_order_sha256": v3.sha(out / "calibration-order.json"),
        "control_hashes": {p.name: v3.sha(p) for p in sorted((out / "controls").glob("*.json"))},
        "quirk_key_hashes": {p.name: v3.sha(p) for p in sorted((out / "quirk-keys").glob("*.json"))},
    }
    for key, value in checks.items():
        if value != src[key]:
            raise ValueError(f"{key} differs from the {source.name} freeze")
    return src


def code_files() -> list[Path]:
    return [
        Path(__file__),
        Path(v327.__file__),
        Path(v3.__file__),
        Path(v2.__file__),
        Path(base.__file__),
        Path(v3.claude.__file__),
        ROOT / "harness/claude_review_guard.py",
        ROOT / "harness/tasks.py",
        ROOT / "harness/evaluator/readability_signals.py",
    ]


def prepare() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    base_protocol = v3.read(BASE_OUT / "protocol.json")
    base_summary = v3.read(BASE_OUT / "summary.json")
    if base_protocol["id"] != BASE_ID or not base_summary["ready_for_publication"]:
        raise ValueError("The v3.30 record is not final")
    if not v3.read(BASE_OUT / "calibration-muse.json")["passed"]:
        raise ValueError("Muse did not pass under v3.30")
    if v3.read(BASE_OUT / "calibration-grok.json")["passed"]:
        raise ValueError("Grok passed under v3.30; there is no empty seat")
    sol, codex = sol_reviewer()
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    copy_freeze(BASE_OUT, OUT, COMPARISON)
    manifest = v3.read(OUT / "private-manifest.json")
    selection = v3.read(OUT / "diagnostic-selection.json")
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "automated held-out controls, five repeats, gates 1 to 17 with a one-gate 0.5 shortfall "
        "allowance, line-level excerpt rule with elision markers; Sol takes the exam under this protocol",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {"v3.30": str(BASE_OUT.relative_to(ROOT))},
        "population": {
            "description": "the v3.30 freeze of Claude Fable 5.2, Low to Extra high, byte for byte",
            "cells": base_protocol["population"]["cells"],
            "excluded": base_protocol["population"]["excluded"],
            "missing": base_protocol["population"]["missing"],
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
        "code_hashes": {str(p.relative_to(ROOT)): v3.sha(p) for p in code_files()},
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
        "reviewers": {"sol": sol, "muse": base_protocol["reviewers"]["muse"]},
        "judge_pins": {"codex": codex},
        "scored_siblings": {
            "muse": {
                "directory": str(BASE_OUT.relative_to(ROOT)),
                "protocol_sha256": v3.sha(BASE_OUT / "protocol.json"),
                "calibration_sha256": v3.sha(BASE_OUT / "calibration-muse.json"),
                "role": "scored neutral panel judged under v3.30 on the identical population",
            }
        },
        "failed_panels_disclosed": {
            "grok": {
                "directory": str(BASE_OUT.relative_to(ROOT)),
                "calibration_sha256": v3.sha(BASE_OUT / "calibration-grok.json"),
                "outcome": "failed calibration gate g14_pair_2_1 under v3.30; no counted call; seat filled by sol",
            }
        },
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": {
            codex["launcher"]["path"]: {"sha256": codex["launcher"]["sha256"]},
            codex["native"]["path"]: {"sha256": codex["native"]["sha256"]},
        },
        "planned_calls": {
            "calibration_per_panel": 80,
            "primary_per_panel": n + len(selection["repeats"]) + 2 * len(selection["pairs"]),
            "probe_and_match_per_panel": 2 * n,
        },
        "single_panel_rule": "publish from passing panels; if Sol fails its exam it makes no counted call and "
        "Muse alone stands as v3.30 published it",
        "comparability": "a Fable 5.2 score becomes a Muse Spark 1.3 and GPT-6.1 Sol mean; the Claude Opus 5.5, "
        "GPT-6.1 Sol and GPT-6.1 Terra columns are Muse Spark 1.3 and Grok 4.6 means, so cards carry a panel note",
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
        "quirk_keys": len(protocol["quirk_key_hashes"]),
        "copied_from": str(BASE_OUT.relative_to(ROOT)),
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
