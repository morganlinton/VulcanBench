"""Code quality maintenance v3.30.4: GPT-6.1 Sol reviews Claude Fable 5.1, for a matched panel against Fable 5.2.

Claude Fable 5.2 is published from Muse Spark 1.3 and GPT-6.1 Sol (v3.30.2 and
v3.30.3); Claude Fable 5.1 from Muse Spark 1.3 and Grok 4.6 (v3.4, with Grok's
reviews from v3.3). Sol scores higher than Grok, so the published columns do not
share a panel. The owner asked for Sol on the earlier columns too (chat,
2026-11-12), so a Fable 5.2 card can compare Muse and Sol means on both sides.

This round is a comparison record only. It does not amend v3.4: Fable 5.1's
published board scores stay the Muse and Grok means.

Population: the Fable 5.1 rows of the v3.4 freeze. v3.4 judged Fable 5.1 and
GPT-6 Astra together; Sol is an OpenAI model and may not review Astra, so this
round keeps the 115 Fable 5.1 rows, byte for byte, and drops Astra's rows. Every
v3.4 diagnostic pair sets an Astra submission against a Fable one, so the pairs
are dropped too; the repeats keep the Fable cells. The v3.4 manifest, signals,
selection and each copied evidence file are checked against the hashes the v3.4
protocol recorded before anything is derived. Controls, quirk keys and the
calibration order are copied or rebuilt and must match v3.4 exactly.

Judges: no new exam. Every frozen protocol field v3.30.2 examined Sol under
(rubric, system prompt, instructions, schemas, seed, repeats, gate allowance,
weights, ledger key, controls, quirk keys, calibration order) equals v3.4's, so
Sol is gated by its v3.30.2 verdict and must use the settings and Codex pins
v3.30.2 examined. Muse's v3.4 reviews and probes are scored from the v3.4
directory as a scored sibling, gated by its v3.4 verdict.

    python -m harness.maintenance_review_v3304 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v3304 \\
        python -m harness.maintenance_review_v3_resume run --panel sol
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from harness import maintenance_review_v3 as v3
from harness import maintenance_review_v327 as v327
from harness import maintenance_review_v3302 as v3302
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.30.4"
OUT = ROOT / "runs-code-quality-maintenance-v3.30.4"
SOURCE_ID = "code-quality-maintenance-v3.4"
SOURCE_OUT = ROOT / "runs-code-quality-maintenance-v3.4"  # scored sibling: Muse on Fable 5.1
COMPARISON = ROOT / "docs/results/swe-v4-astra-fable51-2026-09/comparison.json"
SOURCE_DOC = v3302.SOURCE_DOC
KEPT_MODEL = "fable"  # Claude Fable 5.1's model id in the v3.4 population
DROPPED_MODEL = "astra"  # GPT-6 Astra: same lab as Sol
MODELS = {KEPT_MODEL: ("low", "medium", "high", "extra-high", "max")}
PANELS = ("sol",)
SCORED_PANELS = ("sol", "muse")
VERDICTS = {
    "muse": (SOURCE_OUT, SOURCE_ID),
    "sol": (v3302.OUT, v3302.PROTOCOL_ID),
}
# Fields that must equal the source protocol. The manifest, signals and
# selection are derived (the Fable rows only), so they are checked separately.
SAME_AS_SOURCE = tuple(
    f
    for f in v3302.SAME_AS_BASE
    if f not in ("manifest_sha256", "signals_sha256", "selection_sha256")
)
# Fields Sol was examined under in v3.30.2 that must also hold here.
SAME_AS_SOL_EXAM = (
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
)


@dataclass(frozen=True)
class Round:
    """One Sol matched-panel round: where it writes, what it reviews, which verdicts gate it."""

    protocol_id: str
    out: Path
    source_id: str
    source_out: Path
    comparison: Path
    verdicts: dict
    module: Path
    # Source runs were frozen on the earlier host: (recorded prefix, local root
    # holding the restored runs, or None when the runs no longer exist).
    source_runs: tuple[str, Path | None]


EARLIER_HOST = "/Users/morganlinton/dev/VulcanBench"
FROZEN_VERIFY = v3.verify_frozen  # the frozen check, captured before any round rebinds it


class _RelocatedSources:
    """The frozen ``base`` module, except that source runs are read from where they now live.

    The frozen v3 check re-reads every source run by the absolute path the
    freeze recorded. The rounds this module serves were frozen on the earlier
    host, so the recorded prefix is moved to the local root. When the runs no
    longer exist, the re-read is skipped and the recorded hashes stand; the
    evidence file every judge reads is still checked against its frozen hash.
    """

    def __init__(self, rnd: Round) -> None:
        self.rnd = rnd
        manifest = v3.read(rnd.out / "private-manifest.json")
        self.recorded = {row["source_directory"]: row["source_hashes"] for row in manifest}

    def __getattr__(self, name: str):
        return getattr(base, name)

    def inputs(self, run: Path, tasks_root: Path) -> dict:
        prefix, local = self.rnd.source_runs
        text = str(run)
        if not text.startswith(prefix + "/"):
            return base.inputs(run, tasks_root)
        if local is None:
            return {"source_hashes": self.recorded[text]}
        return base.inputs(local / text[len(prefix) + 1 :], tasks_root)


def verify_frozen_for(rnd: Round) -> dict:
    """Run the frozen check with source runs relocated; any other protocol gets the frozen check untouched."""
    if rnd.protocol_id != v3.PROTOCOL_ID:
        return FROZEN_VERIFY()
    saved = v3.base
    v3.base = _RelocatedSources(rnd)
    try:
        return FROZEN_VERIFY()
    finally:
        v3.base = saved


def source_runs_record(rnd: Round) -> dict:
    prefix, local = rnd.source_runs
    if local is None:
        return {
            "recorded_prefix": prefix,
            "status": "the source runs did not survive the move from the earlier host; the frozen evidence "
            "is checked against its recorded hash before every stage, and the source re-read is skipped",
        }
    return {
        "recorded_prefix": prefix,
        "status": "restored from the private backup and re-read before every stage",
        "local_root": str(local.relative_to(ROOT)) if local.is_relative_to(ROOT) else str(local),
    }


def verify_frozen() -> dict:
    return verify_frozen_for(ROUND)


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {"muse": SOURCE_OUT}
    v3.SCORED_PANELS = SCORED_PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare
    v3.panel_passed = panel_passed
    v3.call = v327.call
    v3.parse_stream_for = v327.parse_stream_for
    v3.verify_frozen = verify_frozen
    v3302.check_codex_pins(OUT)


def panel_passed(panel: str) -> bool:
    """Muse by its v3.4 verdict, Sol by its v3.30.2 verdict."""
    return v3302.verdict_passed(VERDICTS, panel)


def subset(manifest: list, signals: dict, selection: dict, model: str) -> tuple[list, dict, dict]:
    """Keep one model's rows of a judged freeze, unchanged, with its repeats and no cross-model pairs."""
    rows = [row for row in manifest if row["model"] == model]
    if not rows:
        raise ValueError(f"No {model} rows in the source freeze")
    kept = {row["id"] for row in rows}
    selected = {
        "repeats": [i for i in selection["repeats"] if i in kept],
        "pairs": [p for p in selection["pairs"] if set(p) <= kept],
    }
    return rows, {i: signals[i] for i in sorted(kept)}, selected


def copy_subset(source: Path, out: Path, comparison: Path, model: str) -> dict:
    """Derive one model's freeze from a judged round, checking every source file against its record."""
    src = v3.read(source / "protocol.json")
    if v3.sha(comparison) != src["source_comparison_sha256"]:
        raise ValueError(f"{comparison.name} differs from the record {source.name} froze")
    for name, key in (
        ("private-manifest.json", "manifest_sha256"),
        ("signals.json", "signals_sha256"),
        ("diagnostic-selection.json", "selection_sha256"),
    ):
        if v3.sha(source / name) != src[key]:
            raise ValueError(f"{name} differs from the {source.name} record")
    rows, signals, selection = subset(
        v3.read(source / "private-manifest.json"),
        v3.read(source / "signals.json"),
        v3.read(source / "diagnostic-selection.json"),
        model,
    )
    out.mkdir(parents=True, exist_ok=True)
    v3.freeze(out / "private-manifest.json", rows)
    v3.freeze(out / "signals.json", signals)
    v3.freeze(out / "diagnostic-selection.json", selection)
    for row in rows:
        target = out / "evidence" / f"{row['id']}.json"
        v3.freeze(target, v3.read(source / "evidence" / f"{row['id']}.json"))
        if v3.sha(target) != row["evidence_sha256"]:
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
        "calibration_order_sha256": v3.sha(out / "calibration-order.json"),
        "control_hashes": {p.name: v3.sha(p) for p in sorted((out / "controls").glob("*.json"))},
        "quirk_key_hashes": {
            p.name: v3.sha(p) for p in sorted((out / "quirk-keys").glob("*.json"))
        },
    }
    for key, value in checks.items():
        if value != src[key]:
            raise ValueError(f"{key} differs from the {source.name} freeze")
    return src


def check_sol_exam_applies(protocol: dict) -> None:
    """Sol's v3.30.2 verdict gates this round only if it was examined under the same frozen fields."""
    exam = v3.read(v3302.OUT / "protocol.json")
    for field in SAME_AS_SOL_EXAM:
        if protocol[field] != exam[field]:
            raise ValueError(f"Protocol field {field} differs from the one Sol was examined under")
    sol, codex = v3302.sol_reviewer()
    if sol != exam["reviewers"]["sol"] or codex != exam["judge_pins"]["codex"]:
        raise ValueError("Sol settings or Codex pins differ from the ones v3.30.2 examined")


def build_protocol(rnd: Round, src: dict, extra: dict) -> dict:
    """The frozen protocol for a Sol matched-panel round over a copied or derived freeze."""
    out = rnd.out
    manifest = v3.read(out / "private-manifest.json")
    selection = v3.read(out / "diagnostic-selection.json")
    sol, codex = v3302.sol_reviewer()
    n = len(manifest)
    return {
        "id": rnd.protocol_id,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "no new exam: GPT-6.1 Sol's v3.30.2 verdict and Muse Spark 1.3's "
        f"{src['id'].rsplit('-', 1)[-1]} verdict gate this round",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "purpose": "matched-panel comparison for Claude Fable 5.2 (Muse Spark 1.3 and GPT-6.1 Sol on both "
        "sides); does not amend the source round's publication",
        "extends": {
            "calibration": {
                p: {
                    "path": str((root / f"calibration-{p}.json").relative_to(ROOT)),
                    "sha256": v3.sha(root / f"calibration-{p}.json"),
                    "protocol_id": protocol_id,
                    "protocol_sha256": v3.sha(root / "protocol.json"),
                }
                for p, (root, protocol_id) in rnd.verdicts.items()
            },
            "rule": "Sol reviews a judged freeze for comparison with v3.30.2 and v3.30.3; no calibration calls; "
            "no earlier record is rewritten.",
        },
        **extra,
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
        "code_hashes": {str(p.relative_to(ROOT)): v3.sha(p) for p in [rnd.module, *code_files()]},
        "protocol_document_sha256": v3.sha(out / "protocol-document.md"),
        "protocol_document_frozen_copy": "protocol-document.md",
        "protocol_document_source_sha256": v3.sha(SOURCE_DOC),
        "source_comparison_sha256": v3.sha(rnd.comparison),
        "manifest_sha256": v3.sha(out / "private-manifest.json"),
        "signals_sha256": v3.sha(out / "signals.json"),
        "selection_sha256": v3.sha(out / "diagnostic-selection.json"),
        "calibration_order_sha256": v3.sha(out / "calibration-order.json"),
        "control_source_hashes": {
            name: v3.sha(v3.CONTROLS_DIR / name) for name in v3.CONTROL_FILES
        },
        "control_hashes": {p.name: v3.sha(p) for p in sorted((out / "controls").glob("*.json"))},
        "quirk_key_hashes": {
            p.name: v3.sha(p) for p in sorted((out / "quirk-keys").glob("*.json"))
        },
        "ledger_key": v3.LEDGER_KEY,
        "reviewers": {"sol": sol, "muse": src["reviewers"]["muse"]},
        "judge_pins": {"codex": codex},
        "scored_siblings": {
            "muse": {
                "directory": str(rnd.source_out.relative_to(ROOT)),
                "protocol_sha256": v3.sha(rnd.source_out / "protocol.json"),
                "role": f"scored neutral panel judged under {src['id']} on these submissions",
            }
        },
        "source_runs": source_runs_record(rnd),
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
        "single_panel_rule": "publish from passing panels; Sol is gated by its v3.30.2 verdict and Muse by its "
        "source-round verdict",
        "weights": src["weights"],
        "invalid_response_retries": 1,
    }


def code_files() -> list[Path]:
    return [Path(__file__), *v3302.code_files()]


def preflight(rnd: Round, protocol: dict) -> None:
    out = rnd.out
    manifest = v3.read(out / "private-manifest.json")
    sizes = [
        len(v3.prompt("review", v3.read(out / "evidence" / f"{r['id']}.json"))) for r in manifest
    ]
    result = {
        "submissions": len(manifest),
        "cells": protocol["population"]["cells"],
        "quirk_keys": len(protocol["quirk_key_hashes"]),
        "copied_from": str(rnd.source_out.relative_to(ROOT)),
        "planned_calls": protocol["planned_calls"],
        "prompt_characters_total_one_panel": sum(sizes),
        "largest_prompt_characters": max(sizes),
        "protocol_sha256": v3.sha(out / "protocol.json"),
    }
    v3.freeze(out / "preflight.json", result)
    print(base.canonical(result), flush=True)


def start(rnd: Round) -> dict:
    """Common prepare checks: a final source record, both verdicts, a clean frozen document."""
    source, source_id = rnd.source_out, rnd.source_id
    rnd.out.mkdir(parents=True, exist_ok=True)
    frozen_doc = rnd.out / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    src = v3.read(source / "protocol.json")
    if src["id"] != source_id or not v3.read(source / "summary.json")["ready_for_publication"]:
        raise ValueError(f"The {source_id} record is not final")
    if not v3302.verdict_passed(rnd.verdicts, "muse"):
        raise ValueError(f"Muse must have passed its {source_id} exam")
    if not v3302.verdict_passed(rnd.verdicts, "sol"):
        raise ValueError("Sol must have passed its v3.30.2 exam")
    frozen_doc.write_bytes(SOURCE_DOC.read_bytes())
    return src


ROUND = Round(
    PROTOCOL_ID,
    OUT,
    SOURCE_ID,
    SOURCE_OUT,
    COMPARISON,
    VERDICTS,
    Path(__file__),
    (EARLIER_HOST, None),  # Fable 5.1's runs were lost with the earlier host
)


def prepare() -> None:
    src = start(ROUND)
    copy_subset(SOURCE_OUT, OUT, COMPARISON, KEPT_MODEL)
    manifest = v3.read(OUT / "private-manifest.json")
    if any(row["model"] == DROPPED_MODEL for row in manifest):
        raise ValueError("An Astra row reached the Sol population")
    cells: dict[str, int] = {}
    for row in manifest:
        key = f"{row['model']}/{row['effort']}"
        cells[key] = cells.get(key, 0) + 1
    if set(cells) != {f"{KEPT_MODEL}/{e}" for e in MODELS[KEPT_MODEL]}:
        raise ValueError(f"Unexpected cells {sorted(cells)}")
    protocol = build_protocol(
        ROUND,
        src,
        {
            "amends": {},
            "population": {
                "description": "the Claude Fable 5.1 rows of the v3.4 freeze, byte for byte; GPT-6 Astra's rows "
                "and every Astra-against-Fable diagnostic pair are dropped because Sol is an OpenAI model",
                "cells": cells,
                "derived_from": {
                    "directory": str(SOURCE_OUT.relative_to(ROOT)),
                    "manifest_sha256": src["manifest_sha256"],
                    "signals_sha256": src["signals_sha256"],
                    "selection_sha256": src["selection_sha256"],
                },
                "dropped": {
                    "model": DROPPED_MODEL,
                    "reason": "same lab as GPT-6.1 Sol; Astra keeps its published Muse Spark 1.3 and Grok 4.6 means",
                },
            },
            "comparability": "Claude Fable 5.1 becomes a Muse Spark 1.3 and GPT-6.1 Sol mean, matching Fable 5.2 "
            "under v3.30.2 and v3.30.3; GPT-6 Astra stays Muse and Grok, so cards footnote it",
        },
    )
    for field in SAME_AS_SOURCE:
        if protocol[field] != src[field]:
            raise ValueError(f"Protocol field {field} differs from {SOURCE_ID}")
    check_sol_exam_applies(protocol)
    v3.freeze(OUT / "protocol.json", protocol)
    preflight(ROUND, protocol)


_bind()


def main() -> None:
    v3.main()


if __name__ == "__main__":
    main()
