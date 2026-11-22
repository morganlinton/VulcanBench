"""scripts/frontier-v5/grid_items.py and grid_plan.py on fixture trial dirs and grid files."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

REPO = Path(__file__).resolve().parents[1]
HERE = REPO / "scripts" / "frontier-v5"


def _load(name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


gi = _load("grid_items")
gp = _load("grid_plan")


def doc(**over: Any) -> dict[str, Any]:
    d: dict[str, Any] = {
        "schema": 1,
        "tamper_clean": True,
        "items": {
            "toml-multiline": {
                "kind": "defect",
                "done": True,
                "checks_passed": 3,
                "checks_total": 3,
            },
            "join-delimiter": {
                "kind": "defect",
                "done": False,
                "checks_passed": 1,
                "checks_total": 2,
            },
            "add-dump-flag": {
                "kind": "feature",
                "done": True,
                "checks_passed": 4,
                "checks_total": 4,
            },
            "parse-speed": {"kind": "perf", "done": False, "checks_passed": 0, "checks_total": 1},
        },
        "guards": {"total": 1098, "failed": ["ConfigFileTest::IniOutputSimple"]},
    }
    d.update(over)
    return d


def trial(root: Path, items: Any, reward: Any, name: str = "cli11-grid__abc") -> Path:
    t = root / name
    (t / "verifier").mkdir(parents=True)
    if items is not None:
        (t / "verifier" / "items.json").write_text(
            items if isinstance(items, str) else json.dumps(items)
        )
    if reward is not None:
        (t / "verifier" / "reward.json").write_text(json.dumps(reward))
    return t


def test_consistent_trial(tmp_path: Path) -> None:
    t = gi.read_trial(trial(tmp_path, doc(), {"reward": 0.5, "items_done": 2, "items_total": 4}))
    assert t.consistent and t.problems == []
    assert (t.items_done, t.items_total, t.reward) == (2, 4, 0.5)
    assert t.by_kind == {"defect": [1, 2], "feature": [1, 1], "perf": [0, 1]}
    assert t.guards_failed == ["ConfigFileTest::IniOutputSimple"] and t.guards_total == 1098
    assert "items" not in t.summary()
    assert gi.main([str(tmp_path / "cli11-grid__abc")]) == 0


def test_tamper_flag_zeroes_the_environment(tmp_path: Path) -> None:
    t = gi.read_trial(
        trial(tmp_path, doc(tamper_clean=False), {"reward": 0, "items_done": 0, "items_total": 4})
    )
    assert t.consistent and t.items_done == 0 and t.items_done_before_tamper == 2
    assert t.by_kind["feature"] == [0, 1]
    bad = gi.read_trial(
        trial(
            tmp_path,
            doc(tamper_clean=False),
            {"reward": 0.5, "items_done": 2, "items_total": 4},
            name="x__1",
        )
    )
    assert not bad.consistent and any("reward 0.5 is not 0/4" in p for p in bad.problems)


@pytest.mark.parametrize(
    "reward,problem",
    [
        ({"reward": 0.75, "items_done": 2, "items_total": 4}, "reward 0.75 is not 2/4"),
        ({"reward": 0.5, "items_done": 3, "items_total": 4}, "items_done=3"),
        ({"reward": 0.5, "items_done": 2}, "items_total=None"),
        ({"reward": True, "items_done": 2, "items_total": 4}, "no numeric reward"),
        (
            {"reward": 0.5, "items_done": 2, "items_total": 4, "guard_policy": "one_item_off"},
            "guard_policy='one_item_off' is not a finite number (Harbor rejects it)",
        ),
    ],
)
def test_reward_disagreements_are_inconsistent(tmp_path: Path, reward: Any, problem: str) -> None:
    t = gi.read_trial(trial(tmp_path, doc(), reward))
    assert not t.consistent and any(problem in p for p in t.problems), t.problems


def _broken(kind: str) -> Any:
    d = doc()
    if kind == "schema":
        d["schema"] = 2
    elif kind == "tamper":
        d["tamper_clean"] = 1
    elif kind == "empty":
        d["items"] = {}
    elif kind == "id":
        d["items"]["Bad_Id"] = d["items"].pop("parse-speed")
    elif kind == "kind":
        d["items"]["parse-speed"]["kind"] = "refactor"
    elif kind == "done-partial":
        d["items"]["join-delimiter"]["done"] = True
    elif kind == "counts":
        d["items"]["parse-speed"]["checks_total"] = 0
    elif kind == "guards":
        d["guards"] = {"total": 0, "failed": ["a"]}
    elif kind == "json":
        return "{not json"
    return d


@pytest.mark.parametrize(
    "kind,message",
    [
        ("schema", "schema must be 1"),
        ("tamper", "tamper_clean"),
        ("empty", "non-empty"),
        ("id", "kebab-case"),
        ("kind", "kind must be one of"),
        ("done-partial", "done with 1 of 2"),
        ("counts", "checks_total >= 1"),
        ("guards", "guards.total is 0"),
        ("json", "items.json"),
    ],
)
def test_malformed_items_are_reported(tmp_path: Path, kind: str, message: str) -> None:
    t = gi.read_trial(
        trial(tmp_path, _broken(kind), {"reward": 0.5, "items_done": 2, "items_total": 4})
    )
    assert not t.consistent and any(message in p for p in t.problems), t.problems


def test_missing_files(tmp_path: Path, capsys) -> None:
    assert gi.read_trial(trial(tmp_path, doc(), None)) is None
    t = gi.read_trial(trial(tmp_path, None, {"reward": 0}, name="y__1"))
    assert t.problems == ["the verifier wrote reward.json but no items.json"]
    assert gi.main([str(tmp_path / "cli11-grid__abc"), str(tmp_path / "y__1")]) == 1
    out = capsys.readouterr().out
    assert "not graded" in out and "INCONSISTENT" in out


# --- grid_plan -------------------------------------------------------------------

FREEZE = '[[verifier.collect]]\ncommand = "kill -KILL -1 2>/dev/null; true"\nuser = "root"\n'


def grid_env(root: Path, name: str, timeout: float = 10800.0, hook: str = FREEZE) -> None:
    d = root / name
    d.mkdir(parents=True)
    (d / "task.toml").write_text(
        f'schema_version = "1.4"\n\n[metadata]\ntimeout_sec = 5\n\n[agent]\n'
        f'timeout_sec = {timeout}\nnetwork_mode = "allowlist"\n\n[verifier]\ntimeout_sec = 900.0\n\n'
        + hook
    )


def grid_file(root: Path, **over: Any) -> Path:
    g: dict[str, Any] = {
        "schema": 1,
        "budget_min": 20,
        "budget_instruction": "Budget: {minutes} minutes.",
        "environments": {
            "a-grid": {"language": "cpp", "tier": "smaller"},
            "b-grid": {"language": "rust", "tier": "larger"},
        },
        "batches": [["a-grid", "b-grid"]],
    }
    g.update(over)
    p = root / "grid.json"
    p.write_text(json.dumps(g))
    return p


def test_plan_lines_and_instruction(tmp_path: Path, capsys) -> None:
    grid_env(tmp_path, "a-grid")
    grid_env(tmp_path, "b-grid")
    g = grid_file(tmp_path)
    assert gp.main([str(g), str(tmp_path)]) == 0
    idx, budget, source, mult, *envs = capsys.readouterr().out.strip().split("\t")
    assert (idx, budget, source, envs) == ("1", "1200", "grid", ["a-grid", "b-grid"])
    assert float(mult) * 10800 == pytest.approx(1200)
    assert gp.main([str(g), str(tmp_path), "--budget-min", "12.5", "--instruction"]) == 0
    assert capsys.readouterr().out.strip() == "Budget: 12.5 minutes."
    assert gp.main([str(g), str(tmp_path), "--budget-min", "0"]) == 2


def test_task_facts_reads_only_the_agent_table(tmp_path: Path) -> None:
    grid_env(tmp_path, "a-grid", timeout=3600)
    assert gp.task_facts(tmp_path / "a-grid" / "task.toml") == (3600.0, True)
    # A hook that is not root, or kills something else, is not the freeze hook.
    grid_env(tmp_path, "c", hook=FREEZE.replace('"root"', '"agent"'))
    assert gp.task_facts(tmp_path / "c" / "task.toml")[1] is False
    grid_env(tmp_path, "d", hook='[[verifier.collect]]\ncommand = "pkill node"\nuser = "root"\n')
    assert gp.task_facts(tmp_path / "d" / "task.toml")[1] is False


@pytest.mark.parametrize(
    "over,message",
    [
        ({"schema": 2}, "schema must be 1"),
        ({"budget_min": 0}, "budget_min"),
        ({"budget_min": True}, "budget_min"),
        ({"budget_instruction": "no placeholder"}, "{minutes}"),
        ({"environments": {"a-grid": {"language": "go", "tier": "smaller"}}}, "language"),
        ({"environments": {"a-grid": {"language": "c", "tier": "medium"}}}, "tier"),
        ({"environments": {"../x": {"language": "c", "tier": "smaller"}}}, "plain directory"),
        ({"batches": []}, "non-empty list"),
        ({"batches": [["a-grid", "nope"]]}, "not in environments"),
    ],
)
def test_grid_file_errors(tmp_path: Path, over: dict[str, Any], message: str) -> None:
    grid_env(tmp_path, "a-grid")
    grid_env(tmp_path, "b-grid")
    with pytest.raises(gp.GridError, match=message.replace("{", r"\{").replace("}", r"\}")):
        gp.load_grid(grid_file(tmp_path, **over), tmp_path)


@pytest.mark.skipif(not Path("/usr/bin/python3").exists(), reason="no system python3")
def test_grid_plan_runs_on_the_hosts_system_python(tmp_path: Path) -> None:
    # run_gate.sh calls grid_plan.py with the first python3 on its PATH, which
    # on the gate Macs is the system 3.9 (no tomllib).
    grid_env(tmp_path, "a-grid")
    grid_env(tmp_path, "b-grid")
    g = grid_file(tmp_path)
    r = subprocess.run(
        ["/usr/bin/python3", str(HERE / "grid_plan.py"), str(g), str(tmp_path)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert r.returncode == 0, r.stderr
    assert r.stdout.split("\t")[:3] == ["1", "1200", "grid"]


@pytest.mark.skipif(not Path("/usr/bin/python3").exists(), reason="no system python3")
@pytest.mark.parametrize("order", ["batch_last", "batch_first"])
def test_grid_plan_takes_a_batch_with_a_budget_on_the_system_python(
    tmp_path: Path, order: str
) -> None:
    # The pilot's first launch: `run_gate.sh --grid sol62 1` with
    # GATE_BUDGET_MIN set passed the batch number after --budget-min, which
    # argparse on Python 3.9 to 3.11 rejects under plain parse_args.
    grid_env(tmp_path, "a-grid")
    grid_env(tmp_path, "b-grid")
    g = grid_file(tmp_path)
    tail = ["--budget-min", "15", "1"] if order == "batch_last" else ["1", "--budget-min", "15"]
    r = subprocess.run(
        ["/usr/bin/python3", str(HERE / "grid_plan.py"), str(g), str(tmp_path), *tail],
        capture_output=True,
        text=True,
        check=False,
    )
    assert r.returncode == 0, r.stderr
    assert r.stdout.split("\t")[:3] == ["1", "900", "override"]
