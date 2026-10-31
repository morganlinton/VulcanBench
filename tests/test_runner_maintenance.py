"""Housekeeping: orphaned sandbox containers and bench tags cut through the script."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from backend.runner import maintenance
from backend.runner.config import RunnerConfig
from backend.runner.discovery import Discovery
from backend.runner.docker_ps import Container
from backend.runner.state import State
from tests.runner_fixtures import fake_repo, gate_job, git, harbor_proc, make_config


@pytest.fixture
def cfg(tmp_path: Path) -> RunnerConfig:
    return make_config(tmp_path)


@pytest.fixture
def state(cfg: RunnerConfig) -> State:
    return State(cfg.home / "runner.db")


def _c(id_: str, name: str, project: str | None = None, image: str = "x") -> Container:
    return Container(
        id=id_,
        name=name,
        status="Up 5 minutes",
        state="running",
        image=image,
        labels={"com.docker.compose.project": project} if project else {},
    )


def test_orphans_are_benchmark_containers_without_a_live_owner(
    cfg: RunnerConfig, state: State
) -> None:
    gate_job(cfg, "opus-done", task="t", trials=[{"id": "a1", "reward": {"reward": 1}}])
    gate_job(cfg, "opus-live", task="t", trials=[{"id": "b1", "running": True}], finished=False)
    containers = [
        _c("c1", "t__a1__env-main-1", "t__a1__env"),  # finished job's trial
        _c("c2", "t__b1__env-main-1", "t__b1__env"),  # live job's trial
        _c("c3", "vb-agent-abc", None, image="vulcanbench/agent-codex:latest"),
        _c("c4", "vb-legacy-foo-12345678-main-1", "vb-legacy-foo-12345678"),
        _c("c5", "postgres-1", "other", image="postgres"),
        _c("c6", "vb-gui-dashboard", None, image="vulcanbench/sandbox:v5-base"),
    ]
    disc = Discovery(cfg, procs=[harbor_proc("opus-live")], containers=containers, jobs=[])
    out = maintenance.orphan_containers(cfg, state, containers=containers, discovery=disc)
    by_id = {o["id"]: o for o in out["containers"]}
    assert set(by_id) == {"c1", "c3", "c4"}
    assert by_id["c1"]["run"] == "gate:opus-done" and "finished" in by_id["c1"]["reason"]
    assert by_id["c3"]["run"] is None and out["live_runs"] == ["gate:opus-live"]
    # Nothing live: the reason says so.
    disc = Discovery(cfg, procs=[], containers=containers[:1], jobs=[])
    out = maintenance.orphan_containers(cfg, state, containers=containers[:1], discovery=disc)
    assert out["containers"][0]["reason"] == "its run is finished"


def test_remove_touches_only_current_orphans(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[list[str]] = []

    def fake_run(argv: list[str], **_: Any) -> Any:
        calls.append(argv)

        class R:
            returncode = 0
            stdout = "c1\n" if argv[1] == "rm" else "net1\n"
            stderr = ""

        return R()

    monkeypatch.setattr(maintenance.subprocess, "run", fake_run)
    orphans = [{"id": "c1", "project": "t__a1__env"}, {"id": "c3", "project": None}]
    result = maintenance.remove_containers(["c1", "c9"], orphans)
    assert result["removed"] == ["c1"] and result["refused"] == ["c9"]
    assert result["networks"] == 1 and result["error"] is None
    rm = [c for c in calls if c[1:3] == ["rm", "-f"]]
    assert rm == [[rm[0][0], "rm", "-f", "c1"]]
    assert maintenance.remove_containers(["c9"], orphans)["removed"] == []


def test_tag_preflight_and_cut_through_the_script(tmp_path: Path) -> None:
    repo = fake_repo(tmp_path)
    bare = tmp_path / "origin.git"
    git(tmp_path, "init", "-q", "--bare", str(bare))
    git(repo, "remote", "add", "origin", str(bare))
    git(repo, "push", "-q", "-u", "origin", "main")
    pre = maintenance.tag_preflight(repo)
    assert pre["branch"] == "main" and pre["clean"] and pre["problems"] == []
    bad = maintenance.cut_bench_tag("Not Valid", push=False, repo=repo)
    assert bad["ok"] is False and "label" in bad["output"]
    out = maintenance.cut_bench_tag("gui-test", push=True, repo=repo)
    assert out["ok"] is True, out
    assert out["tag"] and out["tag"].startswith("bench/") and out["tag"].endswith("-gui-test")
    assert out["tag"] in git(repo, "tag", "-l", "bench/*")
    assert out["tag"] in git(bare, "tag", "-l", "bench/*")  # pushed
    again = maintenance.cut_bench_tag("gui-test", push=False, repo=repo)
    assert again["ok"] is False and "already exists" in again["output"]
    (repo / "harness" / "cli.py").write_text("dirty\n")
    pre = maintenance.tag_preflight(repo)
    assert "not clean" in " ".join(pre["problems"])
    refused = maintenance.cut_bench_tag("gui-dirty", push=False, repo=repo)
    assert refused["ok"] is False and "not clean" in refused["output"]
    assert not json.dumps(refused).count("sk-")
