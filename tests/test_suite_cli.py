"""``vulcanbench suite ...`` and ``vulcanbench run --suite <id> --suites-home``."""

from __future__ import annotations

import json
import shutil
from pathlib import Path
from typing import Any

import pytest

import harness.cli as cli_mod
from harness import suite_cli, validate
from harness.cli import main
from harness.sandbox import docker_executor
from harness.suite_lock import lock_digest
from harness.tasks import load_task, task_hash
from tests.cli_runner import CliRunner
from tests.test_suite import _make_task
from tests.test_validate_tasks import _SOLVING_GOLD, _full_task

runner = CliRunner()
_PYTEST = pytest.mark.skipif(shutil.which("pytest") is None, reason="pytest not on PATH")


def _json(capsys: pytest.CaptureFixture[str], args: list[str]) -> tuple[int, dict[str, Any]]:
    """Run the CLI with --json; stdout must hold exactly one JSON document."""
    code = 0
    # --json goes before a "--" separator, which ends option parsing.
    cut = args.index("--") if "--" in args else len(args)
    try:
        main([*args[:cut], "--json", *args[cut:]])
    except SystemExit as exc:
        code = int(exc.code or 0)
    out, _err = capsys.readouterr()
    return code, json.loads(out)


def _init(home: Path, suite_id: str = "acme-slice") -> Path:
    result = runner.invoke(
        main,
        [
            "suite",
            "init",
            suite_id,
            "--display-name",
            "Acme slice",
            "--repo",
            "acme/widgets",
            "--suites-home",
            str(home),
        ],
    )
    assert result.exit_code == 0, result.output
    return home / suite_id


def _validated(task_dir: Path, status: str = "PASS") -> None:
    (task_dir / "build").mkdir(exist_ok=True)
    (task_dir / "build" / "validate.json").write_text(
        json.dumps({"schema": "vulcanbench.suite.validate.v1", "status": status})
    )


# --- init ----------------------------------------------------------------------


def test_init_writes_manifest_and_readme(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    home = tmp_path / "home"
    code, doc = _json(
        capsys,
        [
            "suite",
            "init",
            "my-suite",
            "--display-name",
            "My suite",
            "--repo",
            "me/repo",
            "--version",
            "1.2.3",
            "--suites-home",
            str(home),
        ],
    )
    assert code == 0 and doc["schema"] == "vulcanbench.suite.init.v1"
    suite_dir = home / "my-suite"
    assert doc["path"] == str(suite_dir) and doc["suites_home"] == str(home)
    manifest = json.loads((suite_dir / "suite.json").read_text())
    assert manifest == doc["manifest"]
    assert set(manifest) == {
        "id",
        "display_name",
        "version",
        "repo",
        "created_at",
        "updated_at",
        "tasks",
        "builder",
    }
    assert manifest["id"] == "my-suite" and manifest["version"] == "1.2.3"
    assert manifest["repo"] == {
        "host": "github",
        "owner": "me",
        "name": "repo",
        "url": "https://github.com/me/repo",
    }
    assert manifest["tasks"] == [] and manifest["builder"] == {"schema": "vulcanbench.suite.v1"}
    readme = (suite_dir / "README.md").read_text()
    assert "build/" in readme and "never ship" in readme


@pytest.mark.parametrize("bad", ["Bad", "ab", "-abc", "a" * 65, "has_underscore"])
def test_init_refuses_bad_ids(tmp_path: Path, capsys: pytest.CaptureFixture[str], bad: str) -> None:
    code, doc = _json(
        capsys,
        [
            "suite",
            "init",
            "--display-name",
            "x",
            "--repo",
            "a/b",
            "--suites-home",
            str(tmp_path),
            # After "--" so argparse hands "-abc" over as the id, not an option.
            "--",
            bad,
        ],
    )
    assert code == 2
    assert doc["schema"] == "vulcanbench.suite.error.v1"
    assert doc["error"]["code"] == "invalid_id"
    assert not (tmp_path / bad).exists()


def test_init_refuses_in_tree_ids_and_aliases(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    repo_tasks = tmp_path / "tasks"
    (repo_tasks / "taken").mkdir(parents=True)
    monkeypatch.setattr(suite_cli, "REPO_TASKS", repo_tasks)
    for suite_id in ("taken", "cii", "v1-micro"):
        code, doc = _json(
            capsys,
            [
                "suite",
                "init",
                suite_id,
                "--display-name",
                "x",
                "--repo",
                "a/b",
                "--suites-home",
                str(tmp_path / "home"),
            ],
        )
        assert code == 2 and doc["error"]["code"] == "id_collides", suite_id
    result = runner.invoke(
        main,
        [
            "suite",
            "init",
            "fresh",
            "--display-name",
            "x",
            "--repo",
            "not-a-repo",
            "--suites-home",
            str(tmp_path / "home"),
        ],
    )
    assert result.exit_code == 2 and "owner/name" in result.output


def test_init_refuses_an_existing_directory(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    home = tmp_path / "home"
    _init(home)
    code, doc = _json(
        capsys,
        [
            "suite",
            "init",
            "acme-slice",
            "--display-name",
            "x",
            "--repo",
            "a/b",
            "--suites-home",
            str(home),
        ],
    )
    assert code == 8 and doc["error"]["code"] == "exists"
    assert doc["error"]["path"] == str(home / "acme-slice")
    human = runner.invoke(
        main,
        [
            "suite",
            "init",
            "acme-slice",
            "--display-name",
            "x",
            "--repo",
            "a/b",
            "--suites-home",
            str(home),
        ],
    )
    assert human.exit_code == 8 and human.output.startswith("error ")


# --- list / show -----------------------------------------------------------------


def test_list_json(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    home = tmp_path / "home"
    code, doc = _json(capsys, ["suite", "list", "--suites-home", str(home)])
    assert code == 0 and doc == {
        "schema": "vulcanbench.suite.list.v1",
        "suites_home": str(home),
        "suites": [],
    }

    frozen = _init(home, "frozen-one")
    _make_task(frozen, "t1")
    _validated(frozen / "t1")
    assert (
        runner.invoke(
            main, ["suite", "add-task", "frozen-one", "--task", "t1", "--suites-home", str(home)]
        ).exit_code
        == 0
    )
    assert (
        runner.invoke(main, ["suite", "freeze", "frozen-one", "--suites-home", str(home)]).exit_code
        == 0
    )
    _init(home, "unfrozen-one")

    code, doc = _json(capsys, ["suite", "list", "--suites-home", str(home)])
    assert code == 0
    rows = {r["id"]: r for r in doc["suites"]}
    assert set(rows) == {"frozen-one", "unfrozen-one"}
    assert set(rows["frozen-one"]) == {
        "id",
        "display_name",
        "version",
        "n_tasks",
        "locked",
        "lock_digest",
        "drift",
        "path",
    }
    assert rows["frozen-one"]["locked"] is True and rows["frozen-one"]["n_tasks"] == 1
    assert (
        rows["frozen-one"]["lock_digest"].startswith("sha256:")
        and rows["frozen-one"]["drift"] == []
    )
    assert rows["unfrozen-one"]["locked"] is False and rows["unfrozen-one"]["lock_digest"] is None
    assert rows["frozen-one"]["path"] == str(frozen)

    (frozen / "t1" / "issue.md").write_text("drifted")
    _code, doc = _json(capsys, ["suite", "list", "--suites-home", str(home)])
    assert {r["id"]: r["drift"] for r in doc["suites"]}["frozen-one"] == ["task changed: t1"]
    human = runner.invoke(main, ["suite", "list", "--suites-home", str(home)])
    assert human.exit_code == 0 and "drift" in human.output and "unfrozen" in human.output


def test_show_stages_and_missing(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "scaffolded")
    _make_task(suite_dir, "authored")
    (suite_dir / "authored" / "build").mkdir()
    (suite_dir / "authored" / "build" / "author.json").write_text("{}")
    _make_task(suite_dir, "validated")
    _validated(suite_dir / "validated", "WARN")
    _make_task(suite_dir, "failed")
    _validated(suite_dir / "failed", "FAIL")
    _make_task(suite_dir, "admitted")
    _validated(suite_dir / "admitted")
    assert (
        runner.invoke(
            main,
            ["suite", "add-task", "acme-slice", "--task", "admitted", "--suites-home", str(home)],
        ).exit_code
        == 0
    )
    manifest = json.loads((suite_dir / "suite.json").read_text())
    manifest["tasks"].append("ghost")
    (suite_dir / "suite.json").write_text(json.dumps(manifest))

    code, doc = _json(capsys, ["suite", "show", "acme-slice", "--suites-home", str(home)])
    assert code == 0 and doc["schema"] == "vulcanbench.suite.show.v1"
    assert doc["manifest"]["tasks"] == ["admitted", "ghost"]
    assert doc["lock"]["locked"] is False
    stages = {t["task_id"]: t["stage"] for t in doc["tasks"]}
    assert stages == {
        "scaffolded": "scaffolded",
        "authored": "authored",
        "validated": "validated",
        "failed": "scaffolded",
        "admitted": "admitted",
        "ghost": "missing",
    }
    by_id = {t["task_id"]: t for t in doc["tasks"]}
    assert by_id["validated"]["validation"]["status"] == "WARN"
    assert by_id["admitted"]["admitted"] is True and by_id["scaffolded"]["validation"] is None

    code, doc = _json(capsys, ["suite", "show", "nope", "--suites-home", str(home)])
    assert code == 5 and doc["error"]["code"] == "not_found"
    human = runner.invoke(main, ["suite", "show", "acme-slice", "--suites-home", str(home)])
    assert human.exit_code == 0 and "admitted" in human.output and "unfrozen" in human.output


# --- add-task / remove-task --------------------------------------------------------


def test_add_task_refuses_unvalidated_unless_forced(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "raw")
    base = ["suite", "add-task", "acme-slice", "--task", "raw", "--suites-home", str(home)]

    code, doc = _json(capsys, base)
    assert code == 1 and doc["error"]["code"] == "not_validated"
    assert "vulcanbench suite validate acme-slice --task raw" in doc["error"]["message"]
    assert json.loads((suite_dir / "suite.json").read_text())["tasks"] == []

    _validated(suite_dir / "raw", "FAIL")
    code, doc = _json(capsys, base)
    assert code == 1 and doc["error"]["validation_status"] == "FAIL"

    code, doc = _json(capsys, [*base, "--force"])
    assert code == 0 and doc["schema"] == "vulcanbench.suite.add-task.v1"
    assert doc["tasks"] == ["raw"] and doc["admitted_unvalidated"] == ["raw"]
    manifest = json.loads((suite_dir / "suite.json").read_text())
    assert manifest["tasks"] == ["raw"] and manifest["admitted_unvalidated"] == ["raw"]
    assert manifest["updated_at"] >= manifest["created_at"]

    # Validating later and re-adding clears the unvalidated mark; adding twice is idempotent.
    _validated(suite_dir / "raw", "PASS")
    code, doc = _json(capsys, base)
    assert code == 0 and doc["tasks"] == ["raw"] and doc["admitted_unvalidated"] == []
    assert "admitted_unvalidated" not in json.loads((suite_dir / "suite.json").read_text())

    code, doc = _json(
        capsys, ["suite", "add-task", "acme-slice", "--task", "ghost", "--suites-home", str(home)]
    )
    assert code == 5 and doc["error"]["code"] == "not_found"


def test_remove_task(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "t1")
    assert (
        runner.invoke(
            main,
            [
                "suite",
                "add-task",
                "acme-slice",
                "--task",
                "t1",
                "--force",
                "--suites-home",
                str(home),
            ],
        ).exit_code
        == 0
    )
    code, doc = _json(
        capsys, ["suite", "remove-task", "acme-slice", "--task", "t1", "--suites-home", str(home)]
    )
    assert code == 0 and doc["tasks"] == [] and doc["admitted_unvalidated"] == []
    manifest = json.loads((suite_dir / "suite.json").read_text())
    assert manifest["tasks"] == [] and "admitted_unvalidated" not in manifest
    assert (suite_dir / "t1").is_dir()  # the directory is left alone
    code, doc = _json(
        capsys, ["suite", "remove-task", "acme-slice", "--task", "t1", "--suites-home", str(home)]
    )
    assert code == 5 and doc["error"]["code"] == "not_found"


# --- freeze / check ----------------------------------------------------------------


def test_freeze_and_check_exit_codes(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    freeze = ["suite", "freeze", "acme-slice", "--suites-home", str(home)]
    check = ["suite", "check", "acme-slice", "--suites-home", str(home)]

    code, doc = _json(capsys, freeze)  # empty task list
    assert code == 2 and doc["error"]["code"] == "empty_suite"
    assert doc["error"]["problems"] == ["tasks list is empty"]

    code, doc = _json(capsys, check)  # no lock yet
    assert code == 1 and doc["ok"] is False and doc["lock_digest"] is None
    assert "vulcanbench suite freeze acme-slice" in doc["problems"][0]

    _make_task(suite_dir, "t1")
    _validated(suite_dir / "t1")
    assert (
        runner.invoke(
            main, ["suite", "add-task", "acme-slice", "--task", "t1", "--suites-home", str(home)]
        ).exit_code
        == 0
    )
    code, doc = _json(capsys, freeze)
    assert code == 0 and doc["schema"] == "vulcanbench.suite.freeze.v1"
    assert set(doc) == {"schema", "id", "path", "version", "lock_digest", "n_tasks"}
    assert doc["path"] == str(suite_dir / "suite.lock.json") and doc["n_tasks"] == 1
    assert doc["version"] == "0.1.0" and doc["lock_digest"].startswith("sha256:")
    lock = json.loads((suite_dir / "suite.lock.json").read_text())
    assert set(lock) == {"suite", "version", "frozen_at", "note", "tasks"} and set(
        lock["tasks"]
    ) == {"t1"}

    code, ok = _json(capsys, check)
    assert code == 0 and ok["ok"] is True and ok["problems"] == []
    assert ok["lock_digest"] == doc["lock_digest"]

    (suite_dir / "t1" / "issue.md").write_text("drift")
    code, drift = _json(capsys, check)
    assert code == 1 and drift["ok"] is False and drift["problems"] == ["task changed: t1"]
    human = runner.invoke(main, check)
    assert human.exit_code == 1 and "FAIL acme-slice" in human.output

    # A listed task whose directory vanished refuses to freeze.
    shutil.rmtree(suite_dir / "t1")
    code, doc = _json(capsys, freeze)
    assert code == 2 and doc["error"]["code"] == "task_missing"
    assert doc["error"]["problems"][0].startswith("task missing: t1")

    code, doc = _json(capsys, ["suite", "freeze", "nope", "--suites-home", str(home)])
    assert code == 5


# --- validate ----------------------------------------------------------------------


@_PYTEST
def test_validate_json_writes_build_record(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    task_dir = _full_task(suite_dir, task_id="fix-f", gold=_SOLVING_GOLD)
    (task_dir / "issue.md").write_text("# Fix m\n\n`f` must return 2.")

    code, doc = _json(
        capsys, ["suite", "validate", "acme-slice", "--task", "fix-f", "--suites-home", str(home)]
    )
    assert code == 0, doc
    assert doc["schema"] == "vulcanbench.suite.validate-report.v1"
    assert doc["id"] == "acme-slice" and doc["sandbox"] == "local" and doc["image"] is None
    assert doc["counts"] == {"pass": 1, "skip": 0, "warn": 0, "fail": 0}
    (record,) = doc["results"]
    assert set(record) == {
        "schema",
        "task_id",
        "status",
        "reasons",
        "sandbox",
        "image",
        "task_hash",
        "checked_at",
    }
    assert record["schema"] == "vulcanbench.suite.validate.v1"
    assert record["task_id"] == "fix-f" and record["status"] == "PASS"
    on_disk = json.loads((task_dir / "build" / "validate.json").read_text())
    assert on_disk == record and len(on_disk["task_hash"]) == 64

    # Now the task admits without --force, and the build/ folder stays out of the hash.
    assert (
        runner.invoke(
            main, ["suite", "add-task", "acme-slice", "--task", "fix-f", "--suites-home", str(home)]
        ).exit_code
        == 0
    )
    assert task_hash(load_task("fix-f", suite_dir)) == record["task_hash"]

    # Every admitted task, by default; a FAIL is exit 1.
    (task_dir / "gold_patch.diff").write_text("")
    code, doc = _json(capsys, ["suite", "validate", "acme-slice", "--suites-home", str(home)])
    assert code == 1 and doc["counts"]["fail"] == 1
    assert json.loads((task_dir / "build" / "validate.json").read_text())["status"] == "FAIL"


def test_validate_skip_docker_and_missing(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _full_task(suite_dir, task_id="fix-f", gold=_SOLVING_GOLD)
    monkeypatch.setattr(validate.shutil, "which", lambda name: None)
    code, doc = _json(
        capsys, ["suite", "validate", "acme-slice", "--task", "fix-f", "--suites-home", str(home)]
    )
    assert code == 4 and doc["counts"]["skip"] == 1 and doc["results"][0]["status"] == "SKIP"

    monkeypatch.setattr(docker_executor, "_docker_available", lambda: False)
    code, doc = _json(
        capsys,
        [
            "suite",
            "validate",
            "acme-slice",
            "--task",
            "fix-f",
            "--sandbox",
            "docker",
            "--suites-home",
            str(home),
        ],
    )
    assert code == 3 and doc["error"]["code"] == "docker_unavailable"

    code, doc = _json(
        capsys, ["suite", "validate", "acme-slice", "--task", "ghost", "--suites-home", str(home)]
    )
    assert code == 5 and doc["error"]["code"] == "not_found"
    code, doc = _json(
        capsys,
        [
            "suite",
            "validate",
            "acme-slice",
            "--task",
            "fix-f",
            "--sandbox",
            "vm",
            "--suites-home",
            str(home),
        ],
    )
    assert code == 2 and doc["error"]["code"] == "invalid_sandbox"


# --- vulcanbench run --suite <id> --suites-home ------------------------------------


def test_run_external_suite_end_to_end(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "t1")
    _make_task(suite_dir, "t2")
    for tid in ("t1", "t2"):
        _validated(suite_dir / tid)
        assert (
            runner.invoke(
                main, ["suite", "add-task", "acme-slice", "--task", tid, "--suites-home", str(home)]
            ).exit_code
            == 0
        )
    out = tmp_path / "runs"
    run = [
        "run",
        "--suite",
        "acme-slice",
        "--suites-home",
        str(home),
        "--model",
        "mock:synthetic",
        "--sandbox",
        "local",
        "--no-judges",
        "-o",
        str(out),
    ]
    refused = runner.invoke(main, run)
    assert refused.exit_code == 1 and "error suite 'acme-slice' is not frozen" in refused.output
    assert not out.exists()

    assert (
        runner.invoke(main, ["suite", "freeze", "acme-slice", "--suites-home", str(home)]).exit_code
        == 0
    )
    result = runner.invoke(main, run)
    assert result.exit_code == 0, result.output
    assert "running suite acme-slice" in result.output and "over 2 tasks" in result.output
    lock = json.loads((suite_dir / "suite.lock.json").read_text())
    summaries = [json.loads(p.read_text()) for p in out.glob("t*/summary.json")]
    assert len(summaries) == 2
    for summary in summaries:
        assert summary["suite"] == "acme-slice"
        assert summary["source"]["suite"]["lock_digest"] == lock_digest(lock)
        assert summary["source"]["suite"]["kind"] == "external"

    (suite_dir / "t2" / "issue.md").write_text("edited after freezing")
    drifted = runner.invoke(main, run)
    assert drifted.exit_code == 1 and "task changed: t2" in drifted.output


def test_run_dry_run_and_estimate_resolve_external_suites(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "t1")
    assert (
        runner.invoke(
            main,
            [
                "suite",
                "add-task",
                "acme-slice",
                "--task",
                "t1",
                "--force",
                "--suites-home",
                str(home),
            ],
        ).exit_code
        == 0
    )
    dry = runner.invoke(
        main,
        [
            "run",
            "--suite",
            "acme-slice",
            "--suites-home",
            str(home),
            "--model",
            "mock:synthetic",
            "--dry-run",
        ],
    )
    assert dry.exit_code == 0 and "dry-run would run suite=acme-slice" in dry.output
    est = runner.invoke(
        main,
        [
            "estimate",
            "--suite",
            "acme-slice",
            "--suites-home",
            str(home),
            "--model",
            "openai:gpt-6.1-terra",
            "--json",
        ],
    )
    assert est.exit_code == 0, est.output
    assert json.loads(est.output)["task_ids"] == ["t1"]
    missing = runner.invoke(
        main,
        [
            "estimate",
            "--suite",
            "nope",
            "--suites-home",
            str(home),
            "--model",
            "openai:gpt-6.1-terra",
        ],
    )
    assert missing.exit_code == 1 and "suites home" in missing.output


def test_compare_and_effort_sweep_accept_suites_home(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "t1")
    assert (
        runner.invoke(
            main,
            [
                "suite",
                "add-task",
                "acme-slice",
                "--task",
                "t1",
                "--force",
                "--suites-home",
                str(home),
            ],
        ).exit_code
        == 0
    )
    assert (
        runner.invoke(main, ["suite", "freeze", "acme-slice", "--suites-home", str(home)]).exit_code
        == 0
    )

    cmp = runner.invoke(
        main,
        [
            "compare",
            "--suite",
            "acme-slice",
            "--suites-home",
            str(home),
            "--runs-dir",
            str(tmp_path / "runs"),
            "--json",
        ],
    )
    assert cmp.exit_code == 0, cmp.output
    assert json.loads(cmp.output)["task_ids"] == ["t1"]

    seen: list[dict[str, Any]] = []

    def fake_run_suite(name: str, model: str, **kwargs: Any) -> dict[str, Any]:
        seen.append({"name": name, **kwargs})
        return {"suite": name, "model": model, "tasks": [], "errors": [], "aggregate": []}

    monkeypatch.setattr(cli_mod, "run_suite", fake_run_suite)
    sweep = runner.invoke(
        main,
        [
            "effort-sweep",
            "--suite",
            "acme-slice",
            "--suites-home",
            str(home),
            "--model",
            "mock:synthetic",
            "--efforts",
            "low",
            "-o",
            str(tmp_path / "runs"),
        ],
    )
    assert sweep.exit_code == 0, sweep.output
    assert seen[0]["suites_home"] == home


def test_effort_sweep_reports_a_lock_refusal(tmp_path: Path) -> None:
    home = tmp_path / "home"
    suite_dir = _init(home)
    _make_task(suite_dir, "t1")
    assert (
        runner.invoke(
            main,
            [
                "suite",
                "add-task",
                "acme-slice",
                "--task",
                "t1",
                "--force",
                "--suites-home",
                str(home),
            ],
        ).exit_code
        == 0
    )
    sweep = runner.invoke(
        main,
        [
            "effort-sweep",
            "--suite",
            "acme-slice",
            "--suites-home",
            str(home),
            "--model",
            "mock:synthetic",
            "--efforts",
            "low",
            "--sandbox",
            "local",
            "--no-judges",
            "-o",
            str(tmp_path / "runs"),
        ],
    )
    assert sweep.exit_code == 1 and "not frozen" in sweep.output


def test_cli_registers_the_group_with_the_gui_marker() -> None:
    source = Path(suite_cli.__file__).with_name("cli.py").read_text()
    assert "register_suite_commands(sub)  # GUI marker: suite command group" in source
    assert "# GUI marker: run --suites-home" in source
    assert runner.invoke(main, ["suite"]).exit_code == 0
    for name in ("init", "list", "show", "add-task", "remove-task", "freeze", "check", "validate"):
        assert runner.invoke(main, ["suite", name, "--help"]).exit_code == 0, name
