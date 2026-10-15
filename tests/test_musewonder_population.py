"""Muse Wonder Full judging population (v3.25): the builder's checks, gaps, top-up and check modes."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from types import ModuleType

import pytest

ROOT = Path(__file__).resolve().parents[1]
CELLAR = "legacy-cellarcore-binary-parity"
PADDOCK = "legacy-paddockcore-binary-parity"
BLEND = "legacy-blendcore-binary-parity"


def _builder() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "build_musewonder_population", ROOT / "scripts/cii-v4-board/build_musewonder_population.py"
    )
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def builder(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> ModuleType:
    """The builder pointed at a fake checkout with three tasks and a fake sweep folder."""
    b = _builder()
    tasks = tmp_path / "tasks"
    tasks.mkdir()
    lock = {"tasks": {t: f"hash-{t}" for t in (BLEND, CELLAR, PADDOCK)}}
    (tasks / "suite.lock.json").write_text(json.dumps(lock))
    sweep = tmp_path / b.RUNS
    sweep.mkdir()
    (sweep / "protocol.json").write_text(
        json.dumps(
            {
                "model": b.SWEEP_MODELS[b.MODEL],
                "cli_model": "muse-wonder-full-1.0",
                "harness_version": b.SOLVER_CLI,
                "contributor_mode": "on",
                "muse_settings_plugins": {"skill-reminder": {"enabled": False}},
                "binary_sha256": "bin",
                "muse_settings_sha256": "settings",
            }
        )
    )
    monkeypatch.setattr(b, "ROOT", tmp_path)
    monkeypatch.setattr(b, "TASKS", tasks)
    monkeypatch.setattr(b, "SUITE_LOCK", tasks / "suite.lock.json")
    monkeypatch.setattr(b, "TASK_IDS", [BLEND, CELLAR, PADDOCK])
    monkeypatch.setattr(b, "OUTPUT", tmp_path / "out/comparison.json")
    monkeypatch.setattr(b, "TOPUP_OUTPUT", tmp_path / "out/comparison-topup.json")

    def fake_inputs(run: Path, _tasks: Path) -> dict:
        summary = json.loads((run / "summary.json").read_text())
        if not summary["finished"]:
            raise ValueError(f"Incomplete source run: {run}")
        return {"source_hashes": {"summary": run.name, "task": summary["task_hash"]}}

    monkeypatch.setattr(b.base, "inputs", fake_inputs)
    monkeypatch.setattr(b, "_evidence_rebuilds", lambda run, summary, data: True)
    return b


def add_run(root: Path, level: str, task: str, **changes: object) -> Path:
    run = root / "runs-effort-musewonder-full-v2" / level / f"{task}-{level[:3]}0001"
    run.mkdir(parents=True)
    (run / "cli-agent-stream.jsonl").write_text('{"type": "stdout"}\n')
    summary = {
        "task_id": task,
        "model": "muse-code:muse-wonder-full-1.0-contributor",
        "effort": {"requested": level, "provider": "muse-code"},
        "cli_agent": {"harness_version": "Muse Code 1.4.6 (1.4.6-R6000.1)"},
        "source": {
            "harness": {"dirty": False, "describe": "bench/2026-10-13-musewonder-full-v2-low"}
        },
        "task_hash": f"hash-{task}",
        "finished": True,
        "scores": {"functional": 1.0, "quality": 0.8, "security": 1.0},
        "duration_s": 100.0,
        "total_tokens": 1200,
        "tokens": {"prompt": 1000, "completion": 200, "cached_input": 900},
        "started_at": "2026-10-14T00:00:00+00:00",
        "finished_at": "2026-10-14T00:01:40+00:00",
        "economics": {"api_equivalent_cost_usd": None},
    }
    for key, value in changes.items():
        summary[key] = value
    (run / "summary.json").write_text(json.dumps(summary))
    return run


def _full_sweep(root: Path) -> None:
    for task in (BLEND, CELLAR, PADDOCK):
        add_run(root, "low", task)
    add_run(root, "medium", BLEND)


def test_rows_match_the_muse_record_shape_and_gaps_carry_reasons(
    builder: ModuleType, tmp_path: Path
) -> None:
    _full_sweep(tmp_path)
    aborted = tmp_path / "runs-effort-musewonder-full-v2/quota-aborted/medium-cellar"
    aborted.mkdir(parents=True)  # interrupted attempts live outside <level>/ and are never read
    (tmp_path / "runs-effort-musewonder-full-v2/medium/legacy-x-unfinished").mkdir()
    record = builder.build()
    assert record["cells"] == {"musewonderfull/low": 3, "musewonderfull/medium": 1}
    row = record["rows"][0]
    muse_fields = {
        "model", "effort", "task", "run_id", "functional", "quality", "security", "fallback",
        "duration_s", "reported_tokens", "solver_receipt", "source_directory", "source_hashes",
        "started_at", "finished_at", "solver_cli_version", "api_equivalent_cost_usd",
    }  # fmt: skip
    assert set(row) == muse_fields
    assert row["solver_receipt"]["raw_tokens"] == 1200 and row["fallback"] is False
    assert Path(row["source_directory"]).is_absolute()
    assert [(x["effort"], x["task"]) for x in record["missing"]] == [
        ("medium", CELLAR),
        ("medium", PADDOCK),
    ]
    cellar, paddock = record["missing"]
    for gap in (cellar, paddock):
        assert "2026-10-19" in gap["reason"] and "v3.25.1" in gap["reason"]
    assert "quota-aborted" in cellar["reason"] and "quota-aborted" not in paddock["reason"]
    assert record["solver"]["run_from"] == ["bench/2026-10-13-musewonder-full-v2-low"]
    assert record["solver"]["contributor_mode"] == "on"
    assert len(record["solver"]["sweep_protocol_sha256"]) == 64


def test_exclusions_keep_their_reasons(builder: ModuleType, tmp_path: Path) -> None:
    add_run(tmp_path, "low", BLEND, finished=False)
    add_run(tmp_path, "low", CELLAR, scores={"functional": 0.0, "quality": None, "security": None})
    record = builder.build()
    reasons = {e["task"]: e["reason"] for e in record["excluded"]}
    assert reasons[BLEND].startswith("Incomplete source run")
    assert reasons[CELLAR] == "No recognized source file changed; nothing to judge"
    assert not record["rows"]


def test_unreconstructible_evidence_is_excluded(
    builder: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    add_run(tmp_path, "low", BLEND)
    monkeypatch.setattr(builder, "_evidence_rebuilds", lambda run, summary, data: False)
    record = builder.build()
    assert record["excluded"][0]["reason"] == "Evidence not reconstructible from the saved patch"


def test_a_failed_receipt_is_recorded_not_raised(builder: ModuleType, tmp_path: Path) -> None:
    add_run(tmp_path, "low", BLEND, total_tokens=999)
    assert "does not sum" in builder.build()["rows"][0]["solver_receipt"]["error"]


@pytest.mark.parametrize(
    ("change", "message"),
    [
        ({"model": "muse-code:muse-spark-1.3-contributor"}, "unexpected solver"),
        ({"effort": {"requested": "high"}}, "effort"),
        ({"cli_agent": {"harness_version": "Muse Code 1.4.7"}}, "solver CLI"),
        ({"source": {"harness": {"dirty": True}}}, "dirty"),
        ({"task_hash": "other"}, "suite lock"),
    ],
)
def test_runs_outside_the_sweep_conditions_stop_the_build(
    builder: ModuleType, tmp_path: Path, change: dict, message: str
) -> None:
    add_run(tmp_path, "low", BLEND, **change)
    with pytest.raises(ValueError, match=message):
        builder.build()


def test_a_changed_sweep_protocol_stops_the_build(builder: ModuleType, tmp_path: Path) -> None:
    path = tmp_path / "runs-effort-musewonder-full-v2/protocol.json"
    protocol = json.loads(path.read_text())
    protocol["muse_settings_plugins"] = {"skill-reminder": {"enabled": True}}
    path.write_text(json.dumps(protocol))
    with pytest.raises(ValueError, match="muse_settings_plugins"):
        builder.build()


def test_two_finished_runs_for_one_cell_stop_the_build(builder: ModuleType, tmp_path: Path) -> None:
    add_run(tmp_path, "low", BLEND)
    second = tmp_path / "runs-effort-musewonder-full-v2/low/legacy-blendcore-binary-parity-zz"
    second.mkdir()
    first = next((tmp_path / "runs-effort-musewonder-full-v2/low").glob(f"{BLEND}-low*"))
    for name in ("summary.json", "cli-agent-stream.jsonl"):
        (second / name).write_text((first / name).read_text())
    with pytest.raises(ValueError, match="Two finished runs"):
        builder.build()


def test_top_up_keeps_only_runs_v325_froze_as_missing(builder: ModuleType, tmp_path: Path) -> None:
    _full_sweep(tmp_path)
    add_run(tmp_path, "medium", CELLAR)  # ran after the weekly reset
    frozen = tmp_path / "protocol.json"
    frozen.write_text(
        json.dumps(
            {
                "population": {
                    "missing": [
                        {"model": "musewonderfull", "effort": "medium", "task": CELLAR},
                        {"model": "musewonderfull", "effort": "medium", "task": PADDOCK},
                    ]
                }
            }
        )
    )
    record = builder.build(builder.frozen_missing(frozen))
    assert [(r["effort"], r["task"]) for r in record["rows"]] == [("medium", CELLAR)]
    assert [(x["effort"], x["task"]) for x in record["missing"]] == [("medium", PADDOCK)]


def test_main_writes_and_check_mode_compares(builder: ModuleType, tmp_path: Path) -> None:
    _full_sweep(tmp_path)
    assert builder.main(["--check"]) == 1  # nothing written yet
    assert builder.main([]) == 0
    assert json.loads(builder.OUTPUT.read_text())["cells"]["musewonderfull/low"] == 3
    assert builder.main(["--check"]) == 0
    add_run(tmp_path, "medium", CELLAR)
    assert builder.main(["--check"]) == 1
    assert json.loads(builder.OUTPUT.read_text())["cells"]["musewonderfull/medium"] == 1


@pytest.mark.skipif(
    not (ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison.json").exists(),
    reason="the population record lands with the results PR (#197), not this engine PR",
)
def test_committed_record_names_the_sweep_and_its_gaps() -> None:
    record = json.loads(
        (ROOT / "docs/results/swe-v4-musewonder-full-2026-10/comparison.json").read_text()
    )
    assert record["levels"] == {"musewonderfull": ["low", "medium"]}
    assert record["cells"] == {"musewonderfull/low": 23, "musewonderfull/medium": 21}
    assert len(record["rows"]) == 44 and not record["excluded"]
    assert {(x["effort"], x["task"]) for x in record["missing"]} == {
        ("medium", CELLAR),
        ("medium", PADDOCK),
    }
    assert all(
        "/runs-effort-musewonder-full-v2/" in r["source_directory"]
        and "/.claude/worktrees/" not in r["source_directory"]
        for r in record["rows"]
    )
