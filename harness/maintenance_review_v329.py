"""Code quality maintenance v3.29: the same protocol applied to GPT-6.1 Terra on Frontier v4.

Nothing about the rubric, controls, quirk keys, gates, repeats, seed, weights
or the judges changes from v3.24. What changes is the population: the
October 2026 GPT-6.1 Terra effort sweep through Codex CLI 0.159.0 on the
ChatGPT subscription, five levels, 23 tasks each, run from the tagged
worktree bench/2026-10-27-gpt61-terra-frontier-v4 (so no task-hash bridge).

The judges are Muse Spark 1.3 and Grok 4.6, the pair that judged GPT-6.1 Sol
(v3.18) and Claude Opus 5.5 (v3.15), so Terra's Code quality is comparable
with those columns. Both are neutral for an OpenAI submission (owner choice in
chat, 2026-10-28) and retake the exam under this protocol before any counted
call. Settings come from the recovered v3.3 and v3.4 protocols as in v3.24.

One pin is tightened. Cursor updated itself to 2026.10.14-941492e after v3.24,
so resolving ``~/.local/bin/cursor-agent`` would now pick a CLI no round has
judged on. As v3.25 does, this round requires the version v3.24 pinned
(2026.10.01-e373342, still on disk) and its digest, judges through that
version's own launcher, and re-checks the digest before every stage.

    python scripts/cii-v4-board/build_gpt61terra_population.py
    python -m harness.maintenance_review_v329 prepare
    VB_MAINT_MODULE=harness.maintenance_review_v329 \\
        python -m harness.maintenance_review_v3_resume calibrate --panel muse
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
PROTOCOL_ID = "code-quality-maintenance-v3.29"
OUT = ROOT / "runs-code-quality-maintenance-v3.29"
COMPARISON = ROOT / "docs/results/swe-v4-gpt61-terra-2026-10/comparison.json"
RECOVERED = {
    "v3.3": (
        ROOT / "docs/judging/recovered/code-quality-maintenance-v3.3-protocol.json",
        "b82da5987bb555443c9006e51e5a4d555c41858dc710140f0571bd90ed8a5eec",
    ),
    "v3.4": (
        ROOT / "docs/judging/recovered/code-quality-maintenance-v3.4-protocol.json",
        "1d80e0974526f8d825f14921c9102c8eda47ec312a0981783475149e31fd8524",
    ),
}
MODELS = {"gpt61terra": tuple(base.LEVELS)}
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
PANELS = ("muse", "grok")
CURSOR_VERSION = "2026.10.01-e373342"
CURSOR_DIGEST = "dedbb877a8405b20e38a3dc32f38bf614bcd96d6e557f73eb6e99ff8c87b8a73"
CURSOR_PINNED_LAUNCHER = Path.home() / ".local/share/cursor-agent/versions" / CURSOR_VERSION / "cursor-agent"


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
    _check_cursor_pin()


def cursor_version_digest(version_dir: Path) -> str:
    """sha256 over every shipped file in a Cursor version directory, skipping its transient lock folder."""
    h = hashlib.sha256()
    for p in sorted(version_dir.rglob("*")):
        rel = p.relative_to(version_dir)
        if not p.is_file() or rel.parts[0] == ".running":
            continue
        h.update(rel.as_posix().encode())
        h.update(b"\0")
        h.update(hashlib.sha256(p.read_bytes()).digest())
    return h.hexdigest()


def _check_cursor_pin() -> None:
    """Before any stage after prepare: the pinned Cursor version must be unchanged."""
    protocol_path = OUT / "protocol.json"
    if not protocol_path.exists():
        return
    pin = v3.read(protocol_path)["judge_pins"]["cursor_version"]
    if cursor_version_digest(Path(pin["directory"])) != pin["digest"]:
        raise RuntimeError(f"Pinned Cursor {pin['version']} changed or is missing; stop the round")


def pinned_cursor(link: Path | None = None) -> dict:
    """Require the pinned Cursor version's own launcher and the digest v3.24 pinned.

    Cursor updates itself when it runs and repoints its shared launcher link
    (~/.local/bin/cursor-agent) at the newest version, which is 2026.10.14-941492e
    when this round is written. Judge calls go through the pinned version's own
    launcher, so that launcher is what is pinned; where the shared link points
    is recorded, not required (as in v3.25).
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


def pinned_reviewers() -> tuple[dict, dict, dict]:
    """Reviewer settings from the recovered protocols, Muse hash-checked, Cursor pinned by version."""
    sources = {}
    for name, (path, published) in RECOVERED.items():
        if v3.sha(path) != published:
            raise ValueError(f"Recovered {name} protocol differs from its published sha256")
        sources[name] = v3.read(path)
    grok = {k: v for k, v in sources["v3.3"]["reviewers"]["grok"].items() if k != "cursor"}
    muse = {k: v for k, v in sources["v3.4"]["reviewers"]["muse"].items() if k != "muse"}
    muse_path, muse_sha = v3._pin_muse()
    if muse_sha != muse["binary_sha256"]:
        raise ValueError("Muse binary differs from the v3.4 pin")
    if grok["model"] != v3.GROK_MODEL or grok["display_name"] != v3.GROK_DISPLAY:
        raise ValueError("Grok reviewer settings disagree with the v3 constants")
    if muse["model"] != v3.MUSE_MODEL:
        raise ValueError("Muse reviewer settings disagree with the v3 constants")
    cursor = pinned_cursor()
    muse_settings = {**muse, "muse": str(muse_path)}
    grok_settings = {**grok, "cursor": cursor["launcher"]}
    record = {
        "settings_sources": {
            name: {"file": str(path.relative_to(ROOT)), "sha256": published}
            for name, (path, published) in RECOVERED.items()
        },
        "muse": "same binary as v3.4, matched by sha256",
        "cursor_version": cursor,
        "paths_resolved_on_host": {"muse": str(muse_path), "cursor": cursor["launcher"]},
    }
    return muse_settings, grok_settings, record


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
                    if r["model"] == "gpt61terra" and r["task"] == task and r["effort"] == effort
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
    muse_settings, grok_settings, pins_record = pinned_reviewers()
    muse_path = Path(muse_settings["muse"])
    cursor = Path(grok_settings["cursor"])
    muse_sha = muse_settings["binary_sha256"]
    cursor_sha = v3.sha(cursor)
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
            "v3.18": "runs-code-quality-maintenance-v3.18",
            "v3.23": "runs-code-quality-maintenance-v3.23",
            "v3.24": "runs-code-quality-maintenance-v3.24",
        },
        "population": {
            "description": "GPT-6.1 Terra (five levels) through Codex CLI 0.159.0 on the ChatGPT subscription, "
            "on the coding-intelligence-index-v4 suite, one attempt per task and level, run from "
            "bench/2026-10-27-gpt61-terra-frontier-v4 on 2026-10-27 with no pause; rows carry the solver CLI "
            "version",
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
        "judge_pins": pins_record,
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
