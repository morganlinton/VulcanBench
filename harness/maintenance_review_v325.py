"""Code quality maintenance v3.25: the same protocol applied to Muse Wonder Full on Frontier v4.

Nothing about the rubric, controls, quirk keys, gates, repeats, seed or weights
changes from v3.7. What changes is the population and both judges. The
population is the October 2026 Muse Wonder Full sweep through Muse Code 1.4.6
in Contributor mode with the skill reminder off, Low and Medium, run from
bench/2026-10-13-musewonder-full-v2-low. Medium freezes at 21 of 23: the two
tasks the Muse Code weekly limit stopped are recorded as missing and are judged
later in a v3.25.1 top-up that leaves this record untouched.

Muse Spark 1.3 is a Meta model and is not neutral for a Meta submission (owner
decision in chat, 2026-10-15), so both seats change:

- "grok": Grok 4.7 at medium through Cursor (model id ``grok-4.7-medium``,
  display name ``Grok 4.7 256K Medium``, the label Cursor reports at run time), on
  v3's frozen Cursor transport. Cursor is pinned by version directory as in
  v3.24, to the same version and digest v3.24 pinned. Grok 4.7 has never
  judged before.
- "sol": GPT-6.1 Sol at medium through Codex CLI 0.159.0, on the transport
  v3.20 built for a Sol seat (v3's frozen call with ``codex_vote``). Both the
  ``codex.js`` launcher and the native binary it runs are pinned by sha256.

Both judges take the exam under this protocol before any counted call. If one
fails, the other publishes alone and the failure is disclosed; two failures stop
the round. There are no sensitivity panels. The protocol document is frozen as a
copy inside the run directory. Every stage re-checks the Cursor version digest
and both Codex hashes before any call.

The frozen v3 implementation is reused as a library. This module rebinds the v3
module's population and directory constants at import time and replaces
``prepare``, ``call`` and ``parse_stream_for``. Every other stage (calibrate,
run, probe, summarize) is v3's own code.

    python scripts/cii-v4-board/build_musewonder_population.py --check
    python -m harness.maintenance_review_v325 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v325 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel grok
"""

from __future__ import annotations

import hashlib
import json
import random
from collections import Counter
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

ROOT = v3.ROOT
PROTOCOL_ID = "code-quality-maintenance-v3.25"
OUT = ROOT / "runs-code-quality-maintenance-v3.25"
COMPARISON = ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison.json"
MODEL = "musewonderfull"
MODELS = {MODEL: ("low", "medium")}
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
PANELS = ("grok", "sol")
SUBMISSION_LAB = "Meta"
EXCLUDED_JUDGES = {
    "muse": "Muse Spark 1.3 is a Meta model and is not neutral for a Meta submission "
    "(owner decision in chat, 2026-10-15)",
}
TOP_UP = {
    "round": "v3.25.1",
    "protocol_id": "code-quality-maintenance-v3.25.1",
    "directory": "runs-code-quality-maintenance-v3.25.1",
    "population": "docs/results/swe-v4-musewonder-full-2026-10/comparison-topup.json, built by "
    "scripts/cii-v4-board/build_musewonder_population.py --top-up",
    "rule": "only the runs listed under this protocol's population.missing, once they exist; this "
    "round's calibration verdicts gate them with no new exam (the v3.6.1 precedent); each top-up "
    "submission gets a primary and a repeat review and no pairwise check; publication merges both "
    "directories and this record is not rewritten",
}

GROK_MODEL = "grok-4.7-medium"
GROK_DISPLAY = "Grok 4.7 256K Medium"  # the label Cursor reports at run time
CURSOR_VERSION = "2026.10.01-e373342"
CURSOR_DIGEST = "dedbb877a8405b20e38a3dc32f38bf614bcd96d6e557f73eb6e99ff8c87b8a73"
CURSOR_PINNED_LAUNCHER = Path.home() / ".local/share/cursor-agent/versions" / CURSOR_VERSION / "cursor-agent"

SOL_MODEL = "gpt-6.1-sol"
CODEX_VERSION = "0.159.0"
CODEX_ROOT = Path.home() / ".local/share/vulcanbench/codex-0.159.0/node_modules/@openai"
CODEX = CODEX_ROOT / "codex/bin/codex.js"
CODEX_NATIVE = CODEX_ROOT / "codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex"
CODEX_SHA256 = "61b0194f3bb6534439c8d26a3ed57d0805f84b884588b761795323eeb92fcf70"
CODEX_NATIVE_SHA256 = "e89718aa1969bfc4a471277bdc4679a3a3529293de0a309909822dfd67ddb77a"


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
    if "muse" in PANELS:
        raise ValueError("Muse Spark 1.3 is excluded from judging a Meta submission")
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {}
    v3.SCORED_PANELS = PANELS
    v3.call = call
    v3.parse_stream_for = parse_stream_for
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare
    _check_pins()


# --- judge pins ---------------------------------------------------------------


def cursor_version_digest(version_dir: Path) -> str:
    """sha256 over every shipped file in a Cursor version directory, skipping its transient lock folder.

    The same digest v3.24 records (harness/maintenance_review_v324.py).
    """
    h = hashlib.sha256()
    for p in sorted(version_dir.rglob("*")):
        rel = p.relative_to(version_dir)
        if not p.is_file() or rel.parts[0] == ".running":
            continue
        h.update(rel.as_posix().encode())
        h.update(b"\0")
        h.update(hashlib.sha256(p.read_bytes()).digest())
    return h.hexdigest()


def pinned_cursor(link: Path | None = None) -> dict:
    """Require the pinned Cursor version's own launcher and the digest v3.24 pinned.

    Cursor updates itself when it runs and repoints its shared launcher link
    (~/.local/bin/cursor-agent) at the newest version; on 2026-10-15 it moved to
    2026.10.14-941492e a minute after the first v3.25 launch. Judge calls go
    through the pinned version's own launcher, so that launcher is what is pinned;
    where the shared link points is recorded, not required.
    """
    launcher = (link or CURSOR_PINNED_LAUNCHER).resolve()
    version_dir = launcher.parent
    if version_dir.name != CURSOR_VERSION:
        raise ValueError(f"Cursor resolves to {version_dir.name}, not the pinned {CURSOR_VERSION}")
    digest = cursor_version_digest(version_dir)
    if digest != CURSOR_DIGEST:
        raise ValueError(f"Cursor {CURSOR_VERSION} digest {digest} differs from the pin")
    return {
        "version": version_dir.name,
        "directory": str(version_dir),
        "launcher": str(launcher),
        "digest": digest,
        "same_as": "code-quality-maintenance-v3.24",
        "shared_link_points_to": str(v3.CURSOR.resolve()) if v3.CURSOR.exists() else None,
        "rule": "judged through this versioned launcher for the whole round; re-checked before every stage",
    }


def pinned_codex(launcher: Path = CODEX, native: Path = CODEX_NATIVE) -> dict:
    """Both Codex files must exist and match their pinned sha256."""
    record = {}
    for name, path, pinned in (
        ("launcher", launcher, CODEX_SHA256),
        ("native", native, CODEX_NATIVE_SHA256),
    ):
        if not path.is_file():
            raise ValueError(f"Codex {name} not found at {path}")
        if v3.sha(path) != pinned:
            raise ValueError(f"Codex {name} at {path} differs from its pinned sha256")
        record[name] = {"path": str(path), "sha256": pinned}
    return {
        "version": CODEX_VERSION,
        **record,
        "rule": "both files re-checked by sha256 before every stage",
    }


def _check_pins() -> None:
    """Before any stage after prepare: the pinned Cursor version and both Codex files must be unchanged."""
    protocol_path = OUT / "protocol.json"
    if not protocol_path.exists():
        return
    pins = v3.read(protocol_path)["judge_pins"]
    cursor = pins["cursor_version"]
    if cursor_version_digest(Path(cursor["directory"])) != cursor["digest"]:
        raise RuntimeError(
            f"Pinned Cursor {cursor['version']} changed or is missing; stop the round"
        )
    for name in ("launcher", "native"):
        path = Path(pins["codex"][name]["path"])
        if not path.is_file() or v3.sha(path) != pins["codex"][name]["sha256"]:
            raise RuntimeError(f"Pinned Codex {name} changed or is missing; stop the round")


def grok_settings(cursor: dict) -> dict:
    return {
        "model": GROK_MODEL,
        "display_name": GROK_DISPLAY,
        "effort": "medium",
        "cursor": cursor["launcher"],
        "timeout": 900,
        "lab": "xAI",
        "prompt_delivery": "stdin",
        "system_prompt": "folded into prompt",
        "identity": "requested-only, display name checked",
        "display_name_source": "the label Cursor reports for grok-4.7-medium at run time (its model list "
        "renders it as 'Grok 4.7  Medium' without the 256K context label); checked on every response",
        "neutrality": "neutral for Meta submissions",
        "seat": "first time as a judge; takes the exam under this protocol before any counted call",
    }


def sol_settings(codex: dict) -> dict:
    return {
        "model": SOL_MODEL,
        "effort": "medium",
        "codex": codex["launcher"]["path"],
        "codex_version": CODEX_VERSION,
        "binary_sha256": codex["launcher"]["sha256"],
        "native_binary": codex["native"]["path"],
        "native_binary_sha256": codex["native"]["sha256"],
        "timeout": 900,
        "lab": "OpenAI",
        "prompt_delivery": "stdin",
        "system_prompt": "model_instructions_file",
        "identity": "requested-only",
        "neutrality": "neutral for Meta submissions",
        "seat": "the v3.20 Sol seat and transport; takes the exam under this protocol before any counted call",
    }


def pinned_reviewers() -> tuple[dict, dict, dict]:
    cursor = pinned_cursor()
    codex = pinned_codex()
    reviewers = {"grok": grok_settings(cursor), "sol": sol_settings(codex)}
    if set(reviewers) != set(PANELS) or any(r["lab"] == SUBMISSION_LAB for r in reviewers.values()):
        raise ValueError("Judges must be the two panels and neutral for a Meta submission")
    binaries = {
        cursor["launcher"]: {"sha256": v3.sha(Path(cursor["launcher"]))},
        codex["launcher"]["path"]: {"sha256": codex["launcher"]["sha256"]},
        codex["native"]["path"]: {"sha256": codex["native"]["sha256"]},
    }
    return reviewers, {"cursor_version": cursor, "codex": codex}, binaries


# --- transport ----------------------------------------------------------------

_frozen_call = v3.call
_frozen_parse_stream_for = v3.parse_stream_for


def call(panel: str, stage: str, name: str, kind: str, payload: dict, protocol: dict) -> dict:
    """The frozen call, with the Codex transport for the Sol seat (as in v3.20).

    v3's call selects the transport by panel name from a literal inside the
    function, so the Sol panel is routed here: the same receipts, binding,
    validation, retry and scoring path as the frozen function, with
    codex_vote as the transport. Every other panel goes to the frozen call.
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


# --- population and freeze ----------------------------------------------------


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
    """One repeat per cell; one Low-versus-Medium pair per level.

    With two levels v3's "two steps apart" rule wraps onto the same level, so
    each pair sets a task's Low submission against its Medium one. Tasks are
    taken in the seeded order, skipping any task without both submissions, so
    a missing Medium run never shrinks the diagnostics.
    """
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


def prepare() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    frozen_doc = OUT / "protocol-document.md"
    if frozen_doc.exists() and frozen_doc.read_bytes() != SOURCE_DOC.read_bytes():
        raise ValueError(
            "Frozen protocol document already differs from the source; use a new directory"
        )
    reviewers, pins_record, binaries = pinned_reviewers()
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
        "line-level excerpt rule with elision markers; both judges take the exam under this protocol "
        "(Grok 4.7 for the first time)",
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
        },
        "population": {
            "description": "Muse Wonder Full (Low and Medium) through Muse Code 1.4.6 in Contributor mode with "
            "the skill reminder off, on the coding-intelligence-index-v4 suite, one attempt per task and level, "
            "run from bench/2026-10-13-musewonder-full-v2-low; Medium tasks stopped by the Muse Code weekly "
            "limit are listed under missing and judged in the v3.25.1 top-up",
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
        "judge_pins": pins_record,
        "scored_siblings": {},
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": binaries,
        "planned_calls": {
            "calibration_per_panel": 80,
            "primary_per_panel": n + len(selection["repeats"]) + 2 * len(selection["pairs"]),
            "probe_and_match_per_panel": 2 * n,
        },
        "single_panel_rule": "publish from passing panels; a failed panel is disclosed as a sensitivity table; "
        "both failing stops the revision",
        "comparability": "a Muse Wonder Full score is a mean of Grok 4.7 and GPT-6.1 Sol; other models' "
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
