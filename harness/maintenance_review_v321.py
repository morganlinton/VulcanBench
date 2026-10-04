"""Code quality maintenance v3.21: the v3.20 protocol applied to Grok 4.7 in Grok Build on Frontier v4.

The Grok Build half of the Grok 4.7 harness comparison. Everything is v3.20's:
rubric, controls, quirk keys, gates, repeats, seed, weights, and the judges
(Muse Spark 1.3 under its v3.4 pin, GPT-6.1 Sol at medium through Codex CLI
0.159.0 on v3.10's Sol seat transport). Only the population changes: the
Grok 4.7 effort sweep through xAI's Grok Build CLI on the Grok subscription,
four levels (low, medium, high, extra-high), 92 runs. Runs that hit the flat
3-hour task bound are unfinished and excluded as incomplete source runs; they
still count as failed tasks in pass rates. Both judges retake the exam under
this protocol before any counted call.

    python scripts/cii-v4-board/build_grok47grokbuild_population.py
    python -m harness.maintenance_review_v321 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v321 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel muse
"""

from __future__ import annotations

import hashlib
import json
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
PROTOCOL_ID = "code-quality-maintenance-v3.21"
OUT = ROOT / "runs-code-quality-maintenance-v3.21"
COMPARISON = ROOT / "docs/results/swe-v4-grok47-grokbuild-2026-10/comparison.json"
MUSE_PROTOCOL = ROOT / "runs-code-quality-maintenance-v3.4/protocol.json"
MODELS = {"grok47grokbuild": ("low", "medium", "high", "extra-high")}
CODEX = Path(
    "/Users/morganlinton/.local/vulcanbench-codex-0.159.0/lib/node_modules/@openai/codex/bin/codex.js"
)
SOL_MODEL = "gpt-6.1-sol"
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
PANELS = ("muse", "sol")


def _bind() -> None:
    """Point the frozen v3 implementation at this protocol's population and directory."""
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


_frozen_call = v3.call
_frozen_parse_stream_for = v3.parse_stream_for


def call(panel: str, stage: str, name: str, kind: str, payload: dict, protocol: dict) -> dict:
    """The frozen call, with the Codex transport for the Sol seat.

    v3's call selects the transport by panel name from a literal inside the
    function, so the Sol panel is routed here: the same receipts, binding,
    validation, retry and scoring path as the frozen function, with
    codex_vote (the transport v3 already uses for its Astra sensitivity
    panel) as the transport. Every other panel goes to the frozen call.
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
    levels = list(MODELS["grok47grokbuild"])
    pairs = []
    for i, task in enumerate(ordered_tasks[: len(levels)]):
        # One model: pair the same task at two levels two steps apart.
        ids = [
            next(
                (
                    r["id"]
                    for r in manifest
                    if r["model"] == "grok47grokbuild"
                    and r["task"] == task
                    and r["effort"] == effort
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
    muse_path, muse_sha = v3._pin_muse()
    if str(muse_path) != muse_settings["muse"] or muse_sha != muse_settings["binary_sha256"]:
        raise ValueError("Muse binary pin differs from v3.4")
    if not CODEX.exists():
        raise ValueError(f"Codex CLI not found at {CODEX}")
    codex_sha = v3.sha(CODEX)
    sol_settings = {
        "model": SOL_MODEL,
        "effort": "medium",
        "codex": str(CODEX),
        "codex_version": "0.159.0",
        "binary_sha256": codex_sha,
        "timeout": 900,
        "lab": "OpenAI",
        "prompt_delivery": "stdin",
        "system_prompt": "model_instructions_file",
        "identity": "requested-only",
        "neutrality": "neutral for xAI submissions",
        "seat": "fills the seat Grok 4.6 holds for non-xAI submissions; Grok 4.6 is not neutral for Grok 4.7",
    }
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
            "v3.18": "runs-code-quality-maintenance-v3.18",
            "v3.19": "runs-code-quality-maintenance-v3.19",
            "v3.20": "runs-code-quality-maintenance-v3.20",
        },
        "population": {
            "description": "Grok 4.7 (four levels: low, medium, high, extra-high) through the Grok Build CLI "
            f"{', '.join(sorted({r['solver_cli_version'] for r in rows}))} on the Grok subscription, on the "
            "coding-intelligence-index-v4 suite, one attempt per task and level; runs stopped by the flat "
            "3-hour task bound are incomplete source runs and are listed under excluded",
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
        "reviewers": {"muse": muse_settings, "sol": sol_settings},
        "scored_siblings": {},
        "sensitivity_panels": {},
        "locate_matcher": None,
        "reader": "dropped in v3.3 after failing gate 17 under v3.2; not part of this protocol",
        "binaries": {str(muse_path): {"sha256": muse_sha}, str(CODEX): {"sha256": codex_sha}},
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
