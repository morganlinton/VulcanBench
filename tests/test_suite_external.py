"""Suites outside the checkout: resolution, provenance, the lock gate and runs."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from harness import suite as suite_mod
from harness.settings import load_settings
from harness.suite import (
    SUITES_HOME_ENV,
    load_suite,
    load_suite_dir,
    resolve_suites_home,
    run_suite,
)
from harness.suite_lock import SuiteLockError, freeze_dir, lock_digest, read_lock
from tests.test_suite import _make_task


def _external_suite(home: Path, suite_id: str, task_ids: list[str], **extra: object) -> Path:
    suite_dir = home / suite_id
    for tid in task_ids:
        _make_task(suite_dir, tid)
    suite_dir.mkdir(parents=True, exist_ok=True)
    manifest = {
        "id": suite_id,
        "display_name": f"Suite {suite_id}",
        "version": "0.1.0",
        "tasks": task_ids,
        **extra,
    }
    (suite_dir / "suite.json").write_text(json.dumps(manifest), encoding="utf-8")
    return suite_dir


def test_resolve_suites_home_order(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv(SUITES_HOME_ENV, raising=False)
    load_settings.cache_clear()
    # The settings file default, tilde-expanded and absolute.
    default = resolve_suites_home()
    assert default.is_absolute() and "~" not in str(default)
    assert default == Path("~/vulcanbench-suites").expanduser().absolute()
    # The environment wins over the file; the explicit flag wins over both.
    monkeypatch.setenv(SUITES_HOME_ENV, str(tmp_path / "from-env"))
    assert resolve_suites_home() == tmp_path / "from-env"
    assert resolve_suites_home(tmp_path / "flag") == tmp_path / "flag"
    assert resolve_suites_home("~/flag-home") == Path("~/flag-home").expanduser().absolute()


def test_external_suite_resolves_after_in_tree(tmp_path: Path) -> None:
    base = tmp_path / "tasks"
    home = tmp_path / "home"
    _make_task(base / "in-tree", "task-a")
    _external_suite(home, "ext", ["task-x"])

    ext = load_suite("ext", tasks_base=base, suites_home=home)
    assert ext.external is True
    assert ext.name == "ext" and ext.display_name == "Suite ext"
    assert ext.task_ids == ["task-x"]
    assert ext.tasks_root == home / "ext" and ext.suites_home == home
    assert ext.version == "0.1.0"

    tree = load_suite("in-tree", tasks_base=base, suites_home=home)
    assert tree.external is False and tree.suites_home is None and tree.version is None


def test_in_tree_wins_over_external_with_the_same_id(tmp_path: Path) -> None:
    base = tmp_path / "tasks"
    home = tmp_path / "home"
    _make_task(base / "demo", "tree-task")
    _external_suite(home, "demo", ["ext-task"])
    suite = load_suite("demo", tasks_base=base, suites_home=home)
    assert suite.external is False
    assert suite.task_ids == ["tree-task"]


def test_missing_suite_names_both_places(tmp_path: Path) -> None:
    base = tmp_path / "tasks"
    home = tmp_path / "home"
    base.mkdir()
    with pytest.raises(FileNotFoundError) as info:
        load_suite("nope", tasks_base=base, suites_home=home)
    assert str(base) in str(info.value) and str(home) in str(info.value)


def test_external_empty_task_list_is_an_empty_suite(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _external_suite(home, "empty", [])
    _make_task(suite_dir, "stray")  # a directory that is not admitted
    suite = load_suite("empty", tasks_base=tmp_path / "tasks", suites_home=home)
    assert suite.task_ids == []
    # In-tree semantics still fall back to every task directory.
    assert load_suite_dir(suite_dir).task_ids == ["stray"]


def test_external_suite_needs_a_manifest(tmp_path: Path) -> None:
    (tmp_path / "home" / "bare").mkdir(parents=True)
    with pytest.raises(FileNotFoundError, match="no manifest"):
        load_suite_dir(tmp_path / "home" / "bare", external=True)
    with pytest.raises(FileNotFoundError):
        load_suite("bare", tasks_base=tmp_path / "tasks", suites_home=tmp_path / "home")


def test_provenance_shape(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _external_suite(home, "prov", ["task-a", "task-b"])
    suite = load_suite("prov", tasks_base=tmp_path / "tasks", suites_home=home)
    unfrozen = suite.provenance()
    assert set(unfrozen) == {
        "kind",
        "id",
        "display_name",
        "suites_home",
        "suite_dir",
        "lock_version",
        "lock_digest",
        "frozen_at",
        "n_tasks",
    }
    assert unfrozen["kind"] == "external" and unfrozen["id"] == "prov"
    assert unfrozen["suites_home"] == str(home) and unfrozen["suite_dir"] == str(suite_dir)
    assert unfrozen["lock_version"] is None
    assert unfrozen["lock_digest"] is None and unfrozen["frozen_at"] is None
    assert unfrozen["n_tasks"] == 2

    freeze_dir(suite_dir)
    frozen = suite.provenance()
    lock = read_lock(suite_dir)
    assert lock is not None
    assert frozen["lock_version"] == "0.1.0"
    assert frozen["lock_digest"] == lock_digest(lock) and frozen["lock_digest"].startswith(
        "sha256:"
    )
    assert frozen["frozen_at"] == lock["frozen_at"]

    base = tmp_path / "tasks"
    _make_task(base / "tree", "t")
    assert load_suite("tree", tasks_base=base).provenance()["kind"] == "in-tree"


def test_run_suite_external_end_to_end(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _external_suite(home, "ext-run", ["task-a", "task-b"])
    freeze_dir(suite_dir)
    lock = read_lock(suite_dir)
    assert lock is not None
    out = tmp_path / "runs"

    result = run_suite(
        "ext-run",
        "mock:synthetic",
        output_dir=out,
        tasks_base=tmp_path / "tasks",
        suites_home=home,
        sandbox="local",
        judges=False,
    )
    assert result["suite"] == "ext-run" and result["n_tasks"] == 2
    assert {t["task_id"] for t in result["tasks"]} == {"task-a", "task-b"}
    assert result["source"]["suite"]["lock_digest"] == lock_digest(lock)
    assert result["source"]["suite"]["kind"] == "external"

    for row in result["tasks"]:
        summary = json.loads((out / row["run_id"] / "summary.json").read_text())
        assert summary["suite"] == "ext-run"
        assert summary["source"]["suite"]["lock_digest"] == lock_digest(lock)
        assert summary["source"]["suite"]["suite_dir"] == str(suite_dir)
        assert summary["source"]["harness"] is not None  # source_provenance() untouched
    suite_json = json.loads((out / result["suite_id"] / "suite.json").read_text())
    assert suite_json["source"]["suite"]["lock_digest"] == lock_digest(lock)


def test_run_suite_refuses_an_unfrozen_external_suite(tmp_path: Path) -> None:
    home = tmp_path / "home"
    _external_suite(home, "unfrozen", ["task-a"])
    with pytest.raises(SuiteLockError, match="vulcanbench suite freeze unfrozen"):
        run_suite(
            "unfrozen",
            "mock:synthetic",
            output_dir=tmp_path / "runs",
            tasks_base=tmp_path / "tasks",
            suites_home=home,
            judges=False,
        )
    assert not (tmp_path / "runs").exists()


def test_run_suite_refuses_a_drifted_external_suite(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _external_suite(home, "drift", ["task-a", "task-b"])
    freeze_dir(suite_dir)
    (suite_dir / "task-b" / "issue.md").write_text("Edited after freezing.")
    with pytest.raises(SuiteLockError) as info:
        run_suite(
            "drift",
            "mock:synthetic",
            output_dir=tmp_path / "runs",
            tasks_base=tmp_path / "tasks",
            suites_home=home,
            judges=False,
        )
    assert "task changed: task-b" in str(info.value)
    assert "task-a" not in str(info.value).split("task changed")[1]


def test_no_env_escape_hatch_for_the_lock_gate(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The dirty-checkout override does not reach the external lock check."""
    home = tmp_path / "home"
    _external_suite(home, "nohatch", ["task-a"])
    monkeypatch.setenv("VULCANBENCH_ALLOW_DIRTY", "1")
    with pytest.raises(SuiteLockError):
        run_suite(
            "nohatch",
            "mock:synthetic",
            output_dir=tmp_path / "runs",
            tasks_base=tmp_path / "tasks",
            suites_home=home,
            judges=False,
        )


def test_only_missing_counts_cached_runs_from_the_external_root(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _external_suite(home, "cached", ["task-a", "task-b"])
    freeze_dir(suite_dir)
    out = tmp_path / "runs"
    common = {
        "output_dir": out,
        "tasks_base": tmp_path / "tasks",
        "suites_home": home,
        "sandbox": "local",
        "judges": False,
    }
    first = run_suite("cached", "mock:synthetic", **common)
    assert first["n_runs"] == 2
    second = run_suite("cached", "mock:synthetic", only_missing=True, **common)
    assert second["n_runs"] == 0
    assert sorted(second["covered_cached"]) == ["task-a", "task-b"]


def test_run_suite_passes_suite_source_to_run_agent(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    home = tmp_path / "home"
    suite_dir = _external_suite(home, "src", ["task-a"])
    freeze_dir(suite_dir)
    seen: list[dict[str, object]] = []

    def fake_run_agent(**kwargs):  # type: ignore[no-untyped-def]
        seen.append(kwargs)
        return {
            "run_id": "task-a-x",
            "replay": "",
            "summary": {
                "run_id": "task-a-x",
                "task_id": "task-a",
                "model": kwargs["model"],
                "scores": {"functional": 1.0, "total": 1.0},
                "cost_usd": 0.0,
                "duration_s": 1.0,
            },
        }

    monkeypatch.setattr(suite_mod, "run_agent", fake_run_agent)
    run_suite(
        "src",
        "mock:synthetic",
        output_dir=tmp_path / "runs",
        tasks_base=tmp_path / "tasks",
        suites_home=home,
    )
    source = seen[0]["suite_source"]
    assert isinstance(source, dict)
    assert source["kind"] == "external" and source["suite_dir"] == str(suite_dir)
    assert seen[0]["tasks_root"] == suite_dir
