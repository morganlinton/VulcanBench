"""Code quality maintenance v3.30.3: GPT-6.1 Sol's seat on Claude Fable 5.2 at Max.

The second half of the owner's choice of GPT-6.1 Sol for the seat Grok 4.6
left (chat, 2026-11-11): v3.30.2 seats Sol on Low to Extra high, this round
seats it on Max. The population is the v3.30.1 freeze, byte for byte, copied
and checked as v3.30.2 copies v3.30. Muse Spark 1.3's v3.30.1 reviews and
probes are scored from the v3.30.1 directory as a scored sibling, gated by its
v3.30 verdict; Sol makes the full pass here with no new exam, gated by its
v3.30.2 verdict (the v3.26.5 practice for a further level of the same sweep).

    python -m harness.maintenance_review_v3303 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3303 \\
        python -m harness.maintenance_review_v3_resume run --panel sol
"""

from __future__ import annotations

from pathlib import Path

from harness import maintenance_review_v3 as v3
from harness import maintenance_review_v327 as v327
from harness import maintenance_review_v3302 as v3302
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.30.3"
OUT = ROOT / "runs-code-quality-maintenance-v3.30.3"
MAX_ID = "code-quality-maintenance-v3.30.1"
MAX_OUT = ROOT / "runs-code-quality-maintenance-v3.30.1"  # scored sibling: Muse at Max
COMPARISON = ROOT / "docs/results/swe-v4-fable52-2026-11/comparison-max.json"
SOURCE_DOC = v3302.SOURCE_DOC
MODELS = {"fable52": ("max",)}
PANELS = ("sol",)
SCORED_PANELS = ("sol", "muse")
VERDICTS = {
    "muse": (v3302.BASE_OUT, v3302.BASE_ID),
    "sol": (v3302.OUT, v3302.PROTOCOL_ID),
}


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {"muse": MAX_OUT}
    v3.SCORED_PANELS = SCORED_PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare
    v3.panel_passed = panel_passed
    v3.call = v327.call
    v3.parse_stream_for = v327.parse_stream_for
    v3302.check_codex_pins(OUT)


def panel_passed(panel: str) -> bool:
    """Muse by its v3.30 verdict, Sol by its v3.30.2 verdict."""
    return v3302.verdict_passed(VERDICTS, panel)


def prepare() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    max_protocol = v3.read(MAX_OUT / "protocol.json")
    max_summary = v3.read(MAX_OUT / "summary.json")
    if max_protocol["id"] != MAX_ID or not max_summary["ready_for_publication"]:
        raise ValueError("The v3.30.1 record is not final")
    if not panel_passed("muse"):
        raise ValueError("Muse must have passed its v3.30 exam")
    if not panel_passed("sol"):
        raise ValueError("Sol must have passed its v3.30.2 exam before it judges Max")
    sol_protocol = v3.read(v3302.OUT / "protocol.json")
    sol, codex = v3302.sol_reviewer()
    if sol != sol_protocol["reviewers"]["sol"] or codex != sol_protocol["judge_pins"]["codex"]:
        raise ValueError("Sol settings or Codex pins differ from the ones v3.30.2 examined")
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    v3302.copy_freeze(MAX_OUT, OUT, COMPARISON)
    manifest = v3.read(OUT / "private-manifest.json")
    selection = v3.read(OUT / "diagnostic-selection.json")
    n = len(manifest)
    protocol = {
        "id": PROTOCOL_ID,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "no new exam: GPT-6.1 Sol's v3.30.2 verdict and Muse Spark 1.3's v3.30 verdict gate this level",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {
            "v3.30.1": str(MAX_OUT.relative_to(ROOT)),
            "v3.30.2": str(v3302.OUT.relative_to(ROOT)),
        },
        "extends": {
            "calibration": {
                p: {
                    "path": str((root / f"calibration-{p}.json").relative_to(ROOT)),
                    "sha256": v3.sha(root / f"calibration-{p}.json"),
                    "protocol_id": protocol_id,
                    "protocol_sha256": v3.sha(root / "protocol.json"),
                }
                for p, (root, protocol_id) in VERDICTS.items()
            },
            "rule": "A further level of the same Fable 5.2 sweep; no new calibration calls. Publication "
            "merges v3.30.2 (Low to Extra high) and this directory (Max); no earlier record is rewritten.",
        },
        "population": {
            "description": "the v3.30.1 freeze of Claude Fable 5.2 at Max, byte for byte",
            "cells": max_protocol["population"]["cells"],
            "excluded": max_protocol["population"]["excluded"],
            "missing": max_protocol["population"]["missing"],
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
        "code_hashes": {
            str(p.relative_to(ROOT)): v3.sha(p) for p in [Path(__file__), *v3302.code_files()]
        },
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
        "reviewers": {"sol": sol, "muse": max_protocol["reviewers"]["muse"]},
        "judge_pins": {"codex": codex},
        "scored_siblings": {
            "muse": {
                "directory": str(MAX_OUT.relative_to(ROOT)),
                "protocol_sha256": v3.sha(MAX_OUT / "protocol.json"),
                "role": "scored neutral panel judged under v3.30.1 on the identical population",
            }
        },
        "failed_panels_disclosed": {
            "grok": {
                "directory": str(v3302.BASE_OUT.relative_to(ROOT)),
                "calibration_sha256": v3.sha(v3302.BASE_OUT / "calibration-grok.json"),
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
            "calibration_per_panel": 0,
            "primary_per_panel": n + len(selection["repeats"]) + 2 * len(selection["pairs"]),
            "probe_and_match_per_panel": 2 * n,
        },
        "single_panel_rule": "publish from passing panels; a failed panel is disclosed as a sensitivity table; "
        "both failing stops the revision",
        "comparability": "a Fable 5.2 Max score is a Muse Spark 1.3 and GPT-6.1 Sol mean, as v3.30.2 makes Low "
        "to Extra high",
        "weights": max_protocol["weights"],
        "invalid_response_retries": 1,
    }
    for field in v3302.SAME_AS_BASE:
        if protocol[field] != max_protocol[field]:
            raise ValueError(f"Protocol field {field} differs from v3.30.1")
    v3.freeze(OUT / "protocol.json", protocol)
    sizes = [
        len(v3.prompt("review", v3.read(OUT / "evidence" / f"{r['id']}.json"))) for r in manifest
    ]
    result = {
        "submissions": n,
        "cells": protocol["population"]["cells"],
        "quirk_keys": len(protocol["quirk_key_hashes"]),
        "copied_from": str(MAX_OUT.relative_to(ROOT)),
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
