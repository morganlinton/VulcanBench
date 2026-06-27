"""Dataset-shape guards for the v1 task corpus.

These assert the *shape* of the shipped benchmark (size, difficulty spread,
category and language coverage, required files, honest provenance labeling) so
the corpus cannot silently shrink or lose coverage. They do not run the tasks —
that is `scripts/validate_tasks.py` (gold-solves / fail-to-pass / determinism).

Claims ledger (promise -> proving test):
- "39 real, multi-language tasks"                      -> test_minimum_real_task_count
- "difficulty spans easy/medium/hard"                  -> test_difficulty_spread
- "categories incl. refactor & concurrency"            -> test_category_coverage
- "medium/large navigation tasks for large-codebase"   -> test_repo_scale_coverage
- "task complexity is labeled"                         -> test_task_complexity_labels
- "every real task is structurally complete"           -> test_every_real_task_has_required_files
- "provenance is labeled and the OSS task is honest"   -> test_provenance_labeling
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

# Floors, not targets: the suite must not silently shrink below what is shipped.
# v1 was pruned from 52 -> 43 -> 31 (9 unsolvable scaffolds, then 12
# zero-discrimination "Double" one-liners), then grew to 48 as hard,
# discriminating tasks were added (py-expr-eval, go-parallel-map,
# py-sliding-window-max, go-ttl-lru-cache, py-url-normalize, py-semver-compare,
# py-config-parse, py-reactive-sheet [first real multi_file], py-txn-kvstore
# [second multi_file: nested-transaction undo journal across two files],
# py-bytecode-vm [third multi_file: bytecode compiler + stack VM sharing an ISA],
# py-event-ledger [fourth multi_file: event-sourced ledger, reducer + command side]).
# The large/navigation tier shed mislabeled one-liners (12 -> 7). Raise these as
# more discriminating tasks land.
TASKS_ROOT = Path("tasks/v1")
M3_MIN_REAL_TASKS = 48
M2_MIN_MEDIUM_LARGE = 7


def _real_task_dirs() -> list[Path]:
    """Task dirs that are real benchmark tasks (declarative tests + gold patch)."""
    out: list[Path] = []
    for d in sorted(TASKS_ROOT.iterdir()):
        if not d.is_dir() or not (d / "metadata.json").exists():
            continue
        meta = json.loads((d / "metadata.json").read_text())
        if meta.get("tests") and (d / "gold_patch.diff").exists():
            out.append(d)
    return out


def _meta(d: Path) -> dict:
    return json.loads((d / "metadata.json").read_text())


def test_minimum_real_task_count() -> None:
    dirs = _real_task_dirs()
    assert len(dirs) >= M3_MIN_REAL_TASKS, (
        f"expected >={M3_MIN_REAL_TASKS} real tasks, found {len(dirs)}"
    )


def test_difficulty_spread() -> None:
    diffs = {_meta(d).get("difficulty") for d in _real_task_dirs()}
    assert {"easy", "medium", "hard"} <= diffs, f"missing difficulty tiers: {diffs}"


def test_category_coverage() -> None:
    cats = {_meta(d).get("category") for d in _real_task_dirs()}
    assert {"bug_fix", "feature", "refactor", "concurrency"} <= cats, f"missing categories: {cats}"


def test_language_coverage() -> None:
    langs: set[str] = set()
    for d in _real_task_dirs():
        langs.update(_meta(d).get("languages", []))
    assert {"python", "go", "typescript"} <= langs, f"missing languages: {langs}"


def test_repo_scale_coverage() -> None:
    manifest = json.loads((TASKS_ROOT / "suite.json").read_text())
    large_tier = manifest.get("large") or []
    assert len(large_tier) >= M2_MIN_MEDIUM_LARGE, (
        f"expected >={M2_MIN_MEDIUM_LARGE} navigation-stress tasks in suite large tier, "
        f"found {len(large_tier)}"
    )
    loaded = __import__("harness.suite", fromlist=["load_suite"]).load_suite(
        "v1-large", TASKS_ROOT.parent
    )
    assert len(loaded.task_ids) >= M2_MIN_MEDIUM_LARGE


def test_task_complexity_labels() -> None:
    allowed = {"localized", "multi_file", "system", "architecture"}
    values = {_meta(d).get("task_complexity") for d in _real_task_dirs()}
    assert values <= allowed
    assert "localized" in values


# --- composition discipline: keep the suite from regressing to easy filler ---

MIN_HARD_TASKS = 16
MIN_MEDIUM_OR_HARD_FRACTION = 0.6
MIN_NON_LOCALIZED_TASKS = 10


def test_hard_task_floor() -> None:
    hard = [d for d in _real_task_dirs() if _meta(d).get("difficulty") == "hard"]
    assert len(hard) >= MIN_HARD_TASKS, (
        f"expected >={MIN_HARD_TASKS} hard tasks, found {len(hard)} — the suite needs a"
        " ceiling that separates strong models"
    )


def test_not_dominated_by_easy_tasks() -> None:
    dirs = _real_task_dirs()
    med_or_hard = sum(1 for d in dirs if _meta(d).get("difficulty") in {"medium", "hard"})
    fraction = med_or_hard / len(dirs)
    assert fraction >= MIN_MEDIUM_OR_HARD_FRACTION, (
        f"only {fraction:.0%} of tasks are medium/hard (want >="
        f"{MIN_MEDIUM_OR_HARD_FRACTION:.0%}); trivially-easy tasks waste a run without"
        " discriminating models"
    )


def test_non_localized_coverage_floor() -> None:
    # Guard against the navigation-heavy tier silently going to zero. This floor
    # is low on purpose — growing multi_file/system/architecture coverage is
    # tracked in the roadmap — but it must not regress.
    non_localized = sum(
        1 for d in _real_task_dirs() if _meta(d).get("task_complexity") != "localized"
    )
    assert non_localized >= MIN_NON_LOCALIZED_TASKS, (
        f"expected >={MIN_NON_LOCALIZED_TASKS} non-localized (multi_file/system/architecture)"
        f" tasks, found {non_localized}"
    )


@pytest.mark.parametrize("task_dir", _real_task_dirs(), ids=lambda d: d.name)
def test_every_real_task_has_required_files(task_dir: Path) -> None:
    assert (task_dir / "issue.md").exists()
    assert (task_dir / "expected_metrics.json").exists()
    assert (task_dir / "gold_patch.diff").read_text().strip(), "gold_patch.diff is empty"
    assert (task_dir / "tests").is_dir(), "missing hidden tests/ dir"
    has_repo = (task_dir / "repo").is_dir() or (task_dir / "repo_snapshot.tar.gz").exists()
    assert has_repo, "missing repo/ or repo_snapshot.tar.gz"
    meta = _meta(task_dir)
    assert isinstance(meta.get("decontaminated"), bool), "decontaminated must be a bool"
    assert meta.get("task_complexity") in {"localized", "multi_file", "system", "architecture"}
    spec = meta["tests"]
    assert spec.get("fail_to_pass"), "fail_to_pass must be non-empty"
    if meta.get("source") == "oss":
        assert meta.get("base_commit"), "oss tasks require base_commit"
        upstream = meta.get("upstream")
        assert isinstance(upstream, dict) and upstream.get("url"), "oss tasks require upstream.url"


def test_provenance_labeling() -> None:
    oss = [d for d in _real_task_dirs() if _meta(d).get("source") == "oss"]
    assert len(oss) >= 1, "expected at least one honestly OSS-sourced task"
    for d in oss:
        meta = _meta(d)
        assert meta.get("decontaminated") is False
        notes = meta.get("decontamination_notes", "")
        upstream = meta.get("upstream") or {}
        provenance = f"{notes} {upstream.get('url', '')} {upstream.get('issue', '')}"
        assert "http" in provenance
        repo = d / "repo"
        licenses = [p for p in repo.rglob("*") if "license" in p.name.lower() and p.is_file()]
        assert licenses, f"{d.name}: OSS task must preserve an upstream LICENSE"


def test_suite_manifest_lists_tiers() -> None:
    manifest = json.loads((TASKS_ROOT / "suite.json").read_text())
    assert len(manifest.get("full", [])) >= M3_MIN_REAL_TASKS
    assert manifest.get("micro") and manifest.get("large")
