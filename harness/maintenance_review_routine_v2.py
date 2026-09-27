"""Code quality maintenance for VulcanBench Routine v2, in five languages. DRAFT.

The draft amendment is ``docs/judging/drafts/routine-v2-code-quality-amendment.md``.
Until the owner approves it and it is appended to the v3 protocol document,
``DRAFT`` stays true and every stage that would call a judge refuses to run;
``prepare`` and ``summarize`` work, so the freeze and the arithmetic can be
dry-run on reference fixes with fake scores. At freeze the protocol id and
run directory take the next amendment number.

What differs from the v3.14 Routine runner, and nothing else:

- Evidence is in the task's language: every non-test source file in that
  language, the build manifest, Markdown, and every file the patch changed.
  v2's rule ("all Python source") would judge a C submission from its changed
  files alone. ``prepare`` refuses evidence over ``EVIDENCE_LIMIT_BYTES``
  rather than truncating it.
- Calibration runs per language. Python is the frozen v3 exam (stage
  ``calibration``, ``calibration-<panel>.json``). JavaScript, Rust, C++ and C
  use the translated controls in ``docs/judging/controls-v3/<language>/``:
  ten controls, five repeats, five pairs in both orders, gates 1 to 15 from
  v3's own gate code, the one-gate allowance per language (stage
  ``calibration-<language>``, ``calibration-<panel>-<language>.json``).
- Eligibility is per language: a judge reviews and publishes only the
  languages whose exam it passed. A language with no passing judge publishes
  no Code quality; its combined score is re-normalized over functional,
  automated quality and security for every model alike.
- A judge never publishes a score for its own lab's model; that submission
  uses the other judge alone and is marked single-judge.
- L2 is not applicable, as in v3.8: empty quirk keys, the zero-denominator
  rule, Code quality is L1. L4 signals are computed for Python only.

The record holds private task content, so the run directory lives in the
private VulcanRoutine repository. This file names no task.

    python scripts/cii-v4-board/build_routine_v2_population.py
    python -m harness.maintenance_review_routine_v2 prepare
    python -m harness.maintenance_review_routine_v2 calibrate --panel muse [--language rust]
    python -m harness.maintenance_review_routine_v2 run --panel muse
    python -m harness.maintenance_review_routine_v2 summarize
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import os
import random
import re
import statistics
import subprocess
import tempfile
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path

from harness import maintenance_review_v2 as v2
from harness import maintenance_review_v3 as v3
from harness import retrospective_judging as base

DRAFT = True
ROOT = v3.ROOT
ROUTINE = Path(os.environ.get("VULCANROUTINE_ROOT", ROOT.parent / "VulcanRoutine")).resolve()
PROTOCOL_ID = "code-quality-maintenance-routine-v2-draft"
OUT = Path(
    os.environ.get(
        "VB_ROUTINE_V2_JUDGING_OUT", ROUTINE / "judging/code-quality-maintenance-routine-v2-draft"
    )
).resolve()
COMPARISON = Path(
    os.environ.get(
        "VB_ROUTINE_V2_COMPARISON", ROUTINE / "results/private/routine-v2-comparison.json"
    )
).resolve()
TASKS = ROUTINE / "tasks/routine-v2"
SOURCE_DOC = ROOT / "docs/judging/code-quality-maintenance-v3.md"
AMENDMENT_DOC = ROOT / "docs/judging/drafts/routine-v2-code-quality-amendment.md"
# The judges' frozen settings and binary pins come from their v3.3 and v3.4 run
# directories, which live beside the main checkout (VB_JUDGE_PROTOCOL_ROOT).
JUDGE_PROTOCOL_ROOT = Path(os.environ.get("VB_JUDGE_PROTOCOL_ROOT", ROOT)).resolve()
GROK_PROTOCOL = JUDGE_PROTOCOL_ROOT / "runs-code-quality-maintenance-v3.3/protocol.json"
MUSE_PROTOCOL = JUDGE_PROTOCOL_ROOT / "runs-code-quality-maintenance-v3.4/protocol.json"
PANELS = ("muse", "grok")
PANEL_FAMILY = {"muse": "meta", "grok": "xai"}

ALL_LEVELS = tuple(base.LEVELS)
# model key -> (levels, lab). The default roster is Routine v1's (owner decision
# of September 26, 2026); additions are made before the sweep and before freeze.
MODELS = {
    "astra": (ALL_LEVELS, "openai"),
    "terra": (ALL_LEVELS, "openai"),
    "luna": (ALL_LEVELS, "openai"),
    "sol": (ALL_LEVELS, "openai"),
    "gpt55": (("low", "medium", "high", "extra-high"), "openai"),
    "fable": (ALL_LEVELS, "anthropic"),
    "opus55": (ALL_LEVELS, "anthropic"),
    "swe2": (("medium", "high", "max"), "cognition"),
}

LANGUAGES = ("python", "javascript", "rust", "cpp", "c")
NEW_LANGUAGES = LANGUAGES[1:]
TASKS_PER_LANGUAGE = 5
TASKS_PER_CELL = TASKS_PER_LANGUAGE * len(LANGUAGES)
EVIDENCE_LIMIT_BYTES = 200_000
SOURCE_SUFFIXES = {
    "python": frozenset({".py"}),
    "javascript": frozenset({".js", ".mjs", ".cjs"}),
    "rust": frozenset({".rs"}),
    "c": frozenset({".c", ".h"}),
    "cpp": frozenset({".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx"}),
}
BUILD_MANIFESTS = frozenset(
    {"pyproject.toml", "package.json", "Cargo.toml", "CMakeLists.txt", "Makefile"}
)
TEST_DIRECTORIES = frozenset({"test", "tests", "__tests__", "spec"})
_TEST_FILE = re.compile(r"(^test_.*\.py$|.*_test\.py$|.*\.(test|spec)\.[cm]?js$|.*_test\.rs$)")

CONTROL_EXTENSION = {"javascript": ".mjs", "rust": ".rs", "cpp": ".cpp", "c": ".c"}
CONTROL_MODULE_NAME = {
    "javascript": "ledger.mjs",
    "rust": "ledger.rs",
    "cpp": "ledger.cpp",
    "c": "ledger.c",
}
CALLS_PER_NEW_LANGUAGE = len(v3.CONTROL_FILES) * v3.REPEATS + 2 * len(v3.CALIBRATION_PAIRS)
PYTHON_EXAM_CALLS = CALLS_PER_NEW_LANGUAGE + 4 * v3.REPEATS
L2_NOT_APPLICABLE = (
    "Routine v2 tasks are admitted on one real ticket with no hidden contracts, so no legacy "
    "quirk exists to recover. The key is empty, the L2 denominator is zero for every submission, "
    "and v3's pre-registered zero-denominator rule moves the L2 share to L1."
)
NO_CODE_QUALITY = (
    "No judge passed this language's calibration exam, so this task has no Code quality; its "
    "combined score is re-normalized over functional, automated quality and security."
)
NO_NEUTRAL_JUDGE = (
    "The only judge that passed this language's exam is from the submitting model's lab, so this "
    "task has no Code quality for this model; its combined score is re-normalized."
)


def _bind() -> None:
    """Point the frozen v3 and v2 implementations at this protocol's population and directory."""
    v3.OUT = OUT
    v3.COMPARISON = COMPARISON
    v3.TASKS = TASKS
    v2.TASKS = TASKS
    v3.PROTOCOL_ID = PROTOCOL_ID
    v3.PANELS = PANELS
    v3.SCORED_SIBLING_OUT = {}
    v3.SCORED_PANELS = PANELS
    v3.SENSITIVITY_PANELS = ()
    v3.SENSITIVITY_OUT = OUT
    v3.DOC = OUT / "protocol-document.md"
    v3.prepare = prepare


# --- evidence -----------------------------------------------------------------


def task_language(task: str) -> str:
    language = v3.read(TASKS / task / "metadata.json")["languages"][0]
    if language not in LANGUAGES:
        raise ValueError(f"Unsupported task language {language!r}")
    return language


def is_test_path(name: str) -> bool:
    path = Path(name)
    return bool(TEST_DIRECTORIES.intersection(path.parts[:-1])) or bool(
        _TEST_FILE.fullmatch(path.name)
    )


def include_in_evidence(name: str, language: str, changed: set[str]) -> bool:
    """The Routine v2 evidence rule for one text file of the reconstructed repository."""
    path = Path(name)
    if name in changed or path.suffix == ".md" or path.name in BUILD_MANIFESTS:
        return True
    return path.suffix in SOURCE_SUFFIXES[language] and not is_test_path(name)


def evidence_for(row: dict, language: str) -> dict:
    """v2's reconstruction (saved patch applied to the starting repository), language-aware filter."""
    data = base.inputs(Path(row["source_directory"]), TASKS)
    if data["source_hashes"] != row["source_hashes"]:
        raise ValueError("Original evidence hash mismatch")
    changed = set()
    for line in data["patch"].splitlines():
        if line.startswith("diff --git "):
            match = re.fullmatch(r"diff --git a/(\S+) b/(\S+)", line)
            if not match:
                raise ValueError("Unsupported quoted patch path")
            for name in match.groups():
                if Path(name).is_absolute() or ".." in Path(name).parts:
                    raise ValueError("Unsafe patch path")
            changed.add(match.group(2))
    files, binary = {}, {}
    with tempfile.TemporaryDirectory(prefix="vb-review-rv2-") as temp:
        work = Path(temp) / "repo"
        v2.safe_copy(TASKS / row["task"] / "repo", work)
        subprocess.run(
            ["git", "apply", "--"], cwd=work, input=data["patch"].encode(), check=True,
            capture_output=True,
        )  # fmt: skip
        for p in sorted(work.rglob("*")):
            if p.is_symlink():
                raise ValueError("Symlink in candidate patch")
            if not p.is_file():
                continue
            name = p.relative_to(work).as_posix()
            raw = p.read_bytes()
            try:
                decoded = raw.decode("utf-8")
            except UnicodeDecodeError:
                decoded = None
            if b"\0" in raw or decoded is None:
                binary[name] = {"sha256": base.digest(raw), "bytes": len(raw)}
            elif include_in_evidence(name, language, changed):
                files[name] = decoded
    return {
        "issue": data["issue"],
        "candidate_patch": data["patch"],
        "final_files": files,
        "binary_manifest": binary,
    }


def evidence_bytes(evidence: dict) -> int:
    return len(base.canonical(evidence).encode())


# --- controls and calibration -------------------------------------------------


def language_controls(language: str) -> list[dict]:
    """The ten translated ledger controls for a new language, as review evidence."""
    folder = v3.CONTROLS_DIR / language
    spec = (folder / "SPEC.txt").read_text().strip()
    items = []
    for index, name in enumerate(v3.CONTROL_FILES):
        source = folder / (name.removesuffix(".py") + CONTROL_EXTENSION[language])
        issue = spec + (v3.LEDGER_QUIRK_SENTENCE if index == 7 else "")
        items.append(
            {
                "issue": issue,
                "candidate_patch": "Entire implementation is candidate-authored.",
                "final_files": {CONTROL_MODULE_NAME[language]: source.read_text()},
                "binary_manifest": {},
            }
        )
    return items


def language_control_sources(language: str) -> list[Path]:
    folder = v3.CONTROLS_DIR / language
    return [
        folder / (n.removesuffix(".py") + CONTROL_EXTENSION[language]) for n in v3.CONTROL_FILES
    ] + [folder / "SPEC.txt"]


def language_gates(votes: dict, pairs: dict) -> dict:
    """Gates 2 to 15 on a new language's controls, computed by v3's own gate code.

    Gate 16 scores the intent probe, which runs on the Python controls only.
    v3's function computes every gate in one pass, so it is fed placeholder
    probe and match results and its gate 16 entry is discarded unread.
    """
    probes = {(c, r): {"departures": []} for c in (7, 0) for r in range(v3.REPEATS)}
    matches = {(7, r): {"matches": [{"status": "recovered"}]} for r in range(v3.REPEATS)}
    gates = v3.gates_from_reviews(votes, pairs, probes, matches)
    gates.pop("g16_probe_recovers_documented_intent")
    return gates


def calibration_path(panel: str, language: str) -> Path:
    if language == "python":
        return OUT / f"calibration-{panel}.json"
    return OUT / f"calibration-{panel}-{language}.json"


def calibrate_language(panel: str, language: str, protocol: dict) -> dict:
    """One judge's exam in one new language; the Python exam is v3.calibrate_panel."""
    evidence = [
        v3.read(OUT / "controls" / language / f"control-{i}.json")
        for i in range(len(v3.CONTROL_FILES))
    ]
    stage = f"calibration-{language}"
    votes = {}
    for c, r in v3.read(OUT / "calibration-order.json"):
        votes[c, r] = v3.call(
            panel, stage, f"control-{c}-r{r + 1}", "review", evidence[c], protocol
        )
    pairs = {}
    for a, b in v3.CALIBRATION_PAIRS:
        forward = v3.call(
            panel, stage, f"pair-{a}-{b}", "pair", {"A": evidence[a], "B": evidence[b]}, protocol
        )
        reverse = v3.call(
            panel, stage, f"pair-{b}-{a}", "pair", {"A": evidence[b], "B": evidence[a]}, protocol
        )
        pairs[a, b] = (forward, reverse)
    gates = {"g01_validity": {"passed": True, "shortfall": 0.0}, **language_gates(votes, pairs)}
    result = {
        "panel": panel,
        "language": language,
        "human_calibrated": False,
        "gates": gates,
        **v3.calibration_verdict(gates),
        "protocol_sha256": v3.sha(OUT / "protocol.json"),
        "call_count": CALLS_PER_NEW_LANGUAGE,
        "control_means": {
            str(c): {
                k: statistics.mean(votes[c, r]["dimensions"][k]["score"] for r in range(v3.REPEATS))
                for k in v3.DIMENSIONS
            }
            for c in range(len(v3.CONTROL_FILES))
        },
    }
    base.save(calibration_path(panel, language), result)
    print(base.canonical({k: v for k, v in result.items() if k != "control_means"}), flush=True)
    return result


def language_passed(panel: str, language: str) -> bool:
    path = calibration_path(panel, language)
    if not path.exists():
        return False
    verdict = v3.read(path)
    return bool(verdict["passed"]) and verdict["protocol_sha256"] == v3.sha(OUT / "protocol.json")


def publishing_panels(model: str, language: str) -> list[str]:
    """Judges whose score publishes for this model and language."""
    lab = MODELS[model][1]
    return [p for p in PANELS if language_passed(p, language) and PANEL_FAMILY[p] != lab]


# --- freeze -------------------------------------------------------------------


def empty_key(task: str) -> dict:
    return {
        "task": task,
        "protocol": PROTOCOL_ID,
        "sources": ["VulcanRoutine docs/CHARTER-v2.md, admission rule 1 (a real ticket)"],
        "quirks": [],
        "not_applicable": L2_NOT_APPLICABLE,
    }


def select_pairs(manifest: list[dict], ordered_tasks: list[str]) -> list[list[str]]:
    """One pairwise diagnostic per task: the task at a model's lowest and highest level.

    Models rotate over the seeded task order, so every model's ladder is probed.
    """
    models = list(MODELS)
    pairs = []
    for i, task in enumerate(ordered_tasks):
        model = models[i % len(models)]
        ends = (MODELS[model][0][0], MODELS[model][0][-1])
        ids = [
            next(
                (
                    r["id"]
                    for r in manifest
                    if r["model"] == model and r["task"] == task and r["effort"] == effort
                ),
                None,
            )
            for effort in ends
        ]
        if all(ids):
            pairs.append(ids)
    return pairs


def _freeze_document(source: Path, name: str) -> Path:
    frozen = OUT / name
    if frozen.exists() and frozen.read_bytes() != source.read_bytes():
        raise ValueError(f"Frozen {name} already differs from the source; use a new directory")
    frozen.write_bytes(source.read_bytes())
    return frozen


def _check_population(record: dict) -> tuple[Counter, dict[str, str]]:
    rows = record["rows"]
    counts = Counter((r["model"], r["effort"]) for r in rows)
    expected = {(m, e) for m, (levels, _lab) in MODELS.items() for e in levels}
    if set(counts) - expected:
        raise ValueError(f"Unexpected cells {sorted(set(counts) - expected)}")
    gaps = Counter(
        (e["model"], e["effort"]) for e in record.get("excluded", []) + record.get("missing", [])
    )
    for cell in expected:
        if counts[cell] + gaps.get(cell, 0) != TASKS_PER_CELL:
            raise ValueError(
                f"Cell {cell} has {counts[cell]} rows and {gaps.get(cell, 0)} exclusions or "
                f"missing, not {TASKS_PER_CELL}"
            )
    tasks = sorted({r["task"] for r in rows})
    languages = {task: task_language(task) for task in tasks}
    per_language = Counter(languages.values())
    if len(tasks) != TASKS_PER_CELL or any(
        per_language[lang] != TASKS_PER_LANGUAGE for lang in LANGUAGES
    ):
        raise ValueError(f"Task coverage is not five tasks in each language: {dict(per_language)}")
    return counts, languages


def prepare() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    _freeze_document(SOURCE_DOC, "protocol-document.md")
    _freeze_document(AMENDMENT_DOC, "amendment-document.md")
    record = v3.read(COMPARISON)
    counts, languages = _check_population(record)
    rows = record["rows"]
    excluded, missing = record.get("excluded", []), record.get("missing", [])
    for task in sorted(languages):
        v3.freeze(OUT / "quirk-keys" / f"{task}.json", empty_key(task))
        if v3.load_key(task)["quirks"]:
            raise ValueError(f"Routine key for {task} is not empty")
    random.Random(v3.SEED).shuffle(rows)
    manifest, signals, sizes = [], {}, {}
    for i, row in enumerate(rows):
        ident = f"submission-{i + 1:03d}"
        language = languages[row["task"]]
        evidence = evidence_for(row, language)
        size = evidence_bytes(evidence)
        if size > EVIDENCE_LIMIT_BYTES:
            raise ValueError(f"{ident} evidence is {size} bytes, over {EVIDENCE_LIMIT_BYTES}")
        sizes[ident] = size
        v3.freeze(OUT / "evidence" / f"{ident}.json", evidence)
        manifest.append(
            {
                "id": ident,
                **row,
                "language": language,
                "evidence_sha256": v3.sha(OUT / "evidence" / f"{ident}.json"),
                "evidence_bytes": size,
                "passed_families": [],
            }
        )
        signals[ident] = {
            name: v3._signals(code)
            for name, code in evidence["final_files"].items()
            if language == "python" and name.endswith(".py")
        }
    v3.freeze(OUT / "private-manifest.json", manifest)
    v3.freeze(OUT / "signals.json", signals)
    for i, evidence in enumerate(v3.controls()):
        v3.freeze(OUT / "controls" / f"control-{i}.json", evidence)
    for language in NEW_LANGUAGES:
        for i, evidence in enumerate(language_controls(language)):
            v3.freeze(OUT / "controls" / language / f"control-{i}.json", evidence)
    v3.freeze(
        OUT / "calibration-order.json",
        v3.interleaved_order(v3.SEED, len(v3.CONTROL_FILES), v3.REPEATS),
    )
    expected = {(m, e) for m, (levels, _lab) in MODELS.items() for e in levels}
    present = sorted(cell for cell in expected if counts[cell])
    repeats = [
        next(r["id"] for r in manifest if (r["model"], r["effort"]) == cell) for cell in present
    ]
    ordered_tasks = sorted(
        languages, key=lambda t: hashlib.sha256(f"{v3.SEED}:{t}".encode()).hexdigest()
    )
    pairs = select_pairs(manifest, ordered_tasks)
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
        "draft": DRAFT,
        "human_calibrated": False,
        "humans_involved": False,
        "calibration": "automated held-out controls per language, five repeats; Python is the frozen v3 exam "
        "(gates 1 to 16); JavaScript, Rust, C++ and C take translated controls under gates 1 to 15; the "
        "one-gate 0.5 shortfall allowance applies per language",
        "gate_allowance": v3.GATE_ALLOWANCE,
        "amends": {"v3.14": "VulcanRoutine judging/code-quality-maintenance-v3.14"},
        "population": {
            "description": "VulcanBench Routine v2 (private, 25 routine tickets, five each in Python, "
            "JavaScript, Rust, C++ and C): every listed model at every level its harness offers, one "
            "attempt per task and level",
            "suite": "routine-v2",
            "models": {
                m: {"levels": list(levels), "lab": lab} for m, (levels, lab) in MODELS.items()
            },
            "cells": {f"{m}/{e}": counts[m, e] for m, e in sorted(expected)},
            "languages": {
                lang: sorted(t for t, tl in languages.items() if tl == lang) for lang in LANGUAGES
            },
            "excluded": excluded,
            "missing": missing,
        },
        "evidence_rule": "issue, saved patch, and the reconstructed repository's non-test source in the task's "
        "language, build manifest, Markdown, and every file the patch changed",
        "evidence_limit_bytes": EVIDENCE_LIMIT_BYTES,
        "evidence_suffixes": {k: sorted(v) for k, v in SOURCE_SUFFIXES.items()},
        "eligibility": "a judge reviews and publishes only languages whose exam it passed; a judge never "
        "publishes its own lab's model; a language with no passing judge has no Code quality and a "
        "re-normalized combined score",
        "panel_family": PANEL_FAMILY,
        "layers": {
            "l1_reviewed": "scored, unchanged",
            "l2_intent": "not applicable: " + L2_NOT_APPLICABLE,
            "l3_measured": "not run; fallback split in force, as in every v3.x amendment",
            "l4_signals": "Python submissions only; not computed for the other languages; no weight",
        },
        "comparability": "Routine v2 Code quality is the L1 reviewed score on routine tickets in five "
        "languages. It is never placed on one axis with Routine v1 or Frontier v4 Code quality.",
        "rubric": v3.RUBRIC,
        "system": v3.SYSTEM,
        "pair_instruction": v3.PAIR_INSTRUCTION,
        "probe_instruction": v3.PROBE_INSTRUCTION,
        "match_instruction": v3.MATCH_INSTRUCTION,
        "schemas": v3.KIND_SCHEMA,
        "seed": v3.SEED,
        "repeats": v3.REPEATS,
        "codex_config": list(base.CONFIG),
        "code_hashes": {str(p.relative_to(ROOT)): v3.sha(p) for p in code},
        "protocol_document_sha256": v3.sha(v3.DOC),
        "protocol_document_frozen_copy": str(v3.DOC.relative_to(OUT)),
        "protocol_document_source_sha256": v3.sha(SOURCE_DOC),
        "amendment_document_sha256": v3.sha(OUT / "amendment-document.md"),
        "source_comparison_sha256": v3.sha(COMPARISON),
        "manifest_sha256": v3.sha(OUT / "private-manifest.json"),
        "signals_sha256": v3.sha(OUT / "signals.json"),
        "selection_sha256": v3.sha(OUT / "diagnostic-selection.json"),
        "calibration_order_sha256": v3.sha(OUT / "calibration-order.json"),
        "control_source_hashes": {
            name: v3.sha(v3.CONTROLS_DIR / name) for name in v3.CONTROL_FILES
        },
        "control_hashes": {p.name: v3.sha(p) for p in sorted((OUT / "controls").glob("*.json"))},
        "language_control_source_hashes": {
            lang: {p.name: v3.sha(p) for p in language_control_sources(lang)}
            for lang in NEW_LANGUAGES
        },
        "language_control_hashes": {
            lang: {p.name: v3.sha(p) for p in sorted((OUT / "controls" / lang).glob("*.json"))}
            for lang in NEW_LANGUAGES
        },
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
            "calibration_per_panel": PYTHON_EXAM_CALLS
            + len(NEW_LANGUAGES) * CALLS_PER_NEW_LANGUAGE,
            "primary_per_panel": n + len(repeats) + 2 * len(pairs),
            "probe_and_match_per_panel": 0,
        },
        "single_panel_rule": "per language: publish from passing panels; a failed panel is disclosed as a "
        "sensitivity table; a language with no passing panel publishes no Code quality",
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
    by_language = {
        lang: [sizes[r["id"]] for r in manifest if r["language"] == lang] for lang in LANGUAGES
    }
    result = {
        "submissions": n,
        "cells": protocol["population"]["cells"],
        "excluded": len(excluded),
        "missing": len(missing),
        "pairs": len(pairs),
        "evidence_bytes": {
            lang: {"max": max(v), "median": statistics.median(v)}
            for lang, v in by_language.items()
            if v
        },
        "planned_calls": protocol["planned_calls"],
        "protocol_sha256": v3.sha(OUT / "protocol.json"),
    }
    v3.freeze(OUT / "preflight.json", result)
    print(base.canonical(result), flush=True)


def verify_frozen() -> dict:
    """v3's checks plus this protocol's language controls and amendment text."""
    protocol = v3.verify_frozen()
    if v3.sha(OUT / "amendment-document.md") != protocol["amendment_document_sha256"]:
        raise ValueError("Frozen amendment document changed")
    for lang, hashes in protocol["language_control_source_hashes"].items():
        for name, digest in hashes.items():
            if v3.sha(v3.CONTROLS_DIR / lang / name) != digest:
                raise ValueError(f"{lang} control source changed: {name}")
    for lang, hashes in protocol["language_control_hashes"].items():
        for name, digest in hashes.items():
            if v3.sha(OUT / "controls" / lang / name) != digest:
                raise ValueError(f"{lang} control changed: {name}")
    return protocol


# --- judge stages -------------------------------------------------------------


def _refuse_if_draft() -> None:
    if DRAFT:
        raise RuntimeError(
            "Routine v2 judging is a draft: no judge call until the owner approves the amendment "
            "and it is frozen under its amendment number"
        )


def calibrate(panel: str, languages: tuple[str, ...], protocol: dict) -> dict[str, bool]:
    _refuse_if_draft()
    verdicts = {}
    for language in languages:
        if language == "python":
            try:
                v3.calibrate_panel(panel, protocol)
            except RuntimeError as exc:
                if "calibration gates failed" not in str(exc):
                    raise
        else:
            calibrate_language(panel, language, protocol)
        verdicts[language] = language_passed(panel, language)
    return verdicts


def run_reviews(panel: str, protocol: dict) -> None:
    _refuse_if_draft()
    eligible = {lang for lang in LANGUAGES if language_passed(panel, lang)}
    if not eligible:
        raise RuntimeError(f"Panel {panel} has passed no language exam under the frozen protocol")
    manifest = v3.read(OUT / "private-manifest.json")
    language_of = {r["id"]: r["language"] for r in manifest}
    for row in manifest:
        if row["language"] in eligible:
            v3.call(
                panel, "primary", row["id"], "review",
                v3.read(OUT / "evidence" / f'{row["id"]}.json'), protocol,
            )  # fmt: skip
    selection = v3.read(OUT / "diagnostic-selection.json")
    for ident in selection["repeats"]:
        if language_of[ident] in eligible:
            v3.call(
                panel, "repeat", ident, "review", v3.read(OUT / "evidence" / f"{ident}.json"),
                protocol,
            )  # fmt: skip
    for first, second in selection["pairs"]:
        if language_of[first] not in eligible:
            continue
        for a, b in [(first, second), (second, first)]:
            payload = {
                "A": v3.read(OUT / "evidence" / f"{a}.json"),
                "B": v3.read(OUT / "evidence" / f"{b}.json"),
            }
            v3.call(panel, "pairwise", f"{a}-{b}", "pair", payload, protocol)


# --- summary ------------------------------------------------------------------


def composite_without_code_quality(row: dict) -> float:
    """The combined score re-normalized over the factors present (scorer rule)."""
    return 100 * (0.5 * row["functional"] + 0.085 * row["quality"] + 0.085 * row["security"]) / 0.67


def publish(
    row: dict,
    l1_by_panel: dict[str, float | None],
    panels: list[str],
    reason: str = NO_CODE_QUALITY,
) -> dict | None:
    """Published values for one submission, or ``None`` while a publishing judge is missing a score."""
    if not panels:
        return {
            "code_quality": None,
            "composite_v3": composite_without_code_quality(row),
            "judges": [],
            "status": reason,
        }
    scores = [l1_by_panel.get(p) for p in panels]
    if any(s is None for s in scores):
        return None
    l1 = statistics.mean(scores)
    return {
        "l1": l1,
        "code_quality": l1,
        "l2_redistributed": True,
        "composite_v3": v3._composite(row, l1, 0.33),
        "judges": list(panels),
        "single_judge": len(panels) == 1,
    }


def summarize() -> dict:
    verify_frozen()
    manifest = v3.read(OUT / "private-manifest.json")
    passed = {p: {lang: language_passed(p, lang) for lang in LANGUAGES} for p in PANELS}
    rows = []
    for row in manifest:
        panels = publishing_panels(row["model"], row["language"])
        l1 = {}
        for panel in PANELS:
            review = v3._selected(panel, "primary", row["id"])
            l1[panel] = review["score"] if review else None
        entry = {
            "id": row["id"],
            "model": row["model"],
            "effort": row["effort"],
            "task": row["task"],
            "language": row["language"],
            "l1_by_panel": l1,
            "published": publish(
                row,
                l1,
                panels,
                NO_NEUTRAL_JUDGE
                if any(passed[p][row["language"]] for p in PANELS)
                else NO_CODE_QUALITY,
            ),
        }
        rows.append(entry)
    complete = [r for r in rows if r["published"] is not None]

    def stats(items: list[dict], key: str) -> dict:
        return v3._stats(
            [r["published"][key] for r in items if r["published"].get(key) is not None]
        )

    groups: dict[str, list[dict]] = {}
    for r in complete:
        for key in (f"{r['model']}/{r['effort']}", f"{r['model']}/{r['effort']}/{r['language']}"):
            groups.setdefault(key, []).append(r)
    summary_groups = {
        key: {
            "n": len(items),
            "code_quality": stats(items, "code_quality"),
            "composite_v3": stats(items, "composite_v3"),
            "single_judge": sum(bool(r["published"].get("single_judge")) for r in items),
            "without_code_quality": sum(r["published"]["code_quality"] is None for r in items),
        }
        for key, items in sorted(groups.items())
    }
    result = {
        "protocol": PROTOCOL_ID,
        "draft": DRAFT,
        "human_calibrated": False,
        "humans_involved": False,
        "generated_at": datetime.now(UTC).isoformat(),
        "language_exams": passed,
        "languages_without_code_quality": [
            lang for lang in LANGUAGES if not any(passed[p][lang] for p in PANELS)
        ],
        "expected_submissions": len(manifest),
        "published_submissions": len(complete),
        "ready_for_publication": not DRAFT and len(complete) == len(manifest),
        "groups": summary_groups,
        "rows": rows,
    }
    base.save(OUT / "summary.json", result)
    return result


_bind()


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("action", choices=("prepare", "calibrate", "run", "summarize"))
    parser.add_argument("--panel", choices=PANELS)
    parser.add_argument("--language", choices=LANGUAGES)
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    if args.action == "prepare":
        prepare()
        return
    if args.action == "summarize":
        result = summarize()
        print(base.canonical({k: v for k, v in result.items() if k != "rows"}), flush=True)
        return
    if not args.panel:
        parser.error("--panel is required")
    protocol = verify_frozen()
    with (OUT / f".{args.panel}.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.action == "calibrate":
            languages = (args.language,) if args.language else LANGUAGES
            print(base.canonical(calibrate(args.panel, languages, protocol)), flush=True)
        else:
            run_reviews(args.panel, protocol)


if __name__ == "__main__":
    main()
