"""scripts/computer-use/validate_task.py: a cheat whose oracle crashed is INFRA, not caught."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "validate_task", REPO / "scripts" / "computer-use" / "validate_task.py"
)
assert SPEC and SPEC.loader
vt = importlib.util.module_from_spec(SPEC)
sys.modules["validate_task"] = vt  # dataclasses look the module up by name
SPEC.loader.exec_module(vt)

ZEROS = {"reward": 0, "artifact_present": 0, "overnight_flight": 0}


def make_task(root: Path) -> Path:
    task = root / "demo"
    (task / "tests").mkdir(parents=True)
    (task / "solution").mkdir()
    (task / "solution" / "solve.sh").write_text("#!/bin/sh\n")
    (task / "task.toml").write_text("[agent]\ntimeout_sec = 600\n")
    cheats = [
        {"name": "naive", "solution": "gold", "expect": {"reward": 0, "overnight_flight": 0}},
        {"name": "other", "solution": "gold", "expect": {"reward": 0}},
    ]
    (task / "tests" / "cheats.json").write_text(json.dumps({"cheats": cheats}))
    return task


def fake_harbor(oracle_rc: str, ran: list[str]):
    def sh(*argv: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        jobs = Path(argv[argv.index("-o") + 1])
        label = argv[argv.index("--job-name") + 1]
        ran.append(label)
        trial = jobs / label / "demo__abc"
        (trial / "verifier").mkdir(parents=True)
        (trial / "agent").mkdir()
        (trial / "verifier" / "reward.json").write_text(json.dumps(ZEROS))
        (trial / "agent" / "exit-code.txt").write_text(oracle_rc)
        return subprocess.CompletedProcess(list(argv), 0, "", "")

    return sh


@pytest.mark.parametrize(("oracle_rc", "infra"), [("1\n", True), ("0\n", False)])
def test_crashed_cheat_oracle_is_infra(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, oracle_rc: str, infra: bool
) -> None:
    ran: list[str] = []
    monkeypatch.setattr(vt, "sh", fake_harbor(oracle_rc, ran))
    session = vt.Session(make_task(tmp_path), tmp_path / "work", 1)
    session.check_cheats()
    assert session.infra is infra
    assert session.failed is False
    assert ran == ["cheat-naive", "cheat-other"]


def test_cheat_filter_runs_only_named(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    ran: list[str] = []
    monkeypatch.setattr(vt, "sh", fake_harbor("0\n", ran))
    session = vt.Session(make_task(tmp_path), tmp_path / "work", 1)
    session.cheat_names = {"naive"}
    session.check_cheats()
    assert ran == ["cheat-naive"]
