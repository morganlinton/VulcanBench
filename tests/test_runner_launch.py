"""Launch plans: the hard rules are enforced server-side, before anything is shown."""

from __future__ import annotations

from pathlib import Path

import pytest

from backend.runner import launch
from backend.runner.config import RunnerConfig
from tests.runner_fixtures import fake_repo, fake_worktree, git, make_config

TAG = "bench/2026-10-29-test"


@pytest.fixture
def repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    r = fake_repo(tmp_path)
    monkeypatch.setattr(launch, "REPO_ROOT", r)
    return r


@pytest.fixture
def cfg(tmp_path: Path) -> RunnerConfig:
    return make_config(tmp_path)


def test_child_env_is_an_allowlist(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("VULCANBENCH_ALLOW_DIRTY", "1")
    monkeypatch.setenv("VULCANBENCH_RUNNER", "1")
    monkeypatch.setenv("DATABASE_URL", "postgres://x")
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-not-real")
    monkeypatch.setenv("CLAUDE_CODE_OAUTH_TOKEN", "not-real")
    monkeypatch.setenv("CLAUDE_GATE_ENV", "/tmp/claude-gate.env")
    monkeypatch.setenv("CODEX_HOME", "/tmp/codex-home")
    env = launch.child_env({"RUN_WORKTREES": "/tmp/wt"})
    assert set(env) <= {"HOME", "PATH", "USER", "LANG", "TMPDIR", "CODEX_HOME", "RUN_WORKTREES"}
    assert env["RUN_WORKTREES"] == "/tmp/wt" and env["CODEX_HOME"] == "/tmp/codex-home"
    assert "CLAUDE_GATE_ENV" not in env  # the gate script's knob no longer passes
    assert not any(k.startswith("VULCANBENCH_") for k in env)
    for bad in ("VULCANBENCH_ALLOW_DIRTY", "DATABASE_URL", "FOO_API_KEY", "X_TOKEN", "MY_SECRET"):
        with pytest.raises(launch.LaunchError):
            launch.child_env({bad: "1"})


def test_options_lists_tags_and_worktree_state(repo: Path, cfg: RunnerConfig) -> None:
    opts = launch.options(cfg)
    assert set(opts) == {"tags", "tag", "worktree", "run_worktrees"}
    assert opts["tags"] == [TAG] and opts["tag"] == TAG
    wt = opts["worktree"]
    assert (
        wt["exists"] is False
        and wt["problems"] == []
        and wt["path"] == str(cfg.run_worktrees / "bench-2026-10-29-test")
    )
    assert "script" not in wt
    assert "ultra" not in str(opts)


def test_prepare_plan_shape_and_hash(repo: Path, cfg: RunnerConfig) -> None:
    spec = launch.build_prepare_plan(cfg, TAG)
    wt = cfg.run_worktrees / "bench-2026-10-29-test"
    assert spec["kind"] == "prepare" and spec["job_name"] == "prepare-bench-2026-10-29-test"
    assert spec["argv"] is None and spec["needs_prep"] is True
    assert spec["prep_argv"] == ["bash", str(repo / "scripts" / "run_worktree.sh"), TAG]
    assert spec["prep_env"] == {"RUN_WORKTREES": str(cfg.run_worktrees)}
    assert spec["worktree"] == str(wt) and spec["usage_check_required"] is False
    assert "no model calls" in spec["estimate"]["billing"]
    assert spec["hash"] == launch.plan_hash(spec) and len(spec["hash"]) == 16
    assert launch.build_prepare_plan(cfg, TAG)["hash"] == spec["hash"]
    assert launch.validate_before_launch(cfg, spec) == []


def test_prepare_plan_refusals(
    repo: Path, cfg: RunnerConfig, monkeypatch: pytest.MonkeyPatch
) -> None:
    def err(tag: str) -> tuple[int, str]:
        with pytest.raises(launch.LaunchError) as exc:
            launch.build_prepare_plan(cfg, tag)
        return exc.value.status, exc.value.detail

    assert err("main")[0] == 422
    assert err("bench/missing")[0] == 422
    fake_worktree(repo, cfg)
    status, detail = err(TAG)
    assert status == 409 and "already exists" in detail


def test_validate_before_launch_checks_prepare_specs_only(repo: Path, cfg: RunnerConfig) -> None:
    spec = launch.build_prepare_plan(cfg, TAG)
    problems = launch.validate_before_launch(cfg, {**spec, "tag": "bench/missing"})
    assert "no tag bench/missing" in problems
    moved = {**spec, "worktree": str(cfg.run_worktrees / "elsewhere")}
    assert launch.validate_before_launch(cfg, moved) == ["worktree path does not match the tag"]
    for kind in ("gate", "suite", "sweep", None):
        assert launch.validate_before_launch(cfg, {**spec, "kind": kind}) == ["unsupported kind"]


def test_dirty_or_moved_worktree_is_refused(repo: Path, cfg: RunnerConfig) -> None:
    wt = fake_worktree(repo, cfg)
    assert launch.validate_worktree(cfg, TAG) == []
    (wt / "harness" / "extra.py").write_text("x = 1\n")
    problems = launch.validate_worktree(cfg, TAG)
    assert any("uncommitted changes" in p for p in problems)
    assert launch.worktree_status(cfg, TAG)["dirty"] == ["?? harness/extra.py"]
    (wt / "harness" / "extra.py").unlink()
    assert launch.validate_worktree(cfg, TAG) == []
    # HEAD moved off the tag (someone committed in the worktree).
    (wt / "notes.txt").write_text("hi\n")
    git(wt, "add", "notes.txt")
    git(wt, "commit", "-q", "-m", "drift")
    assert any("not at bench/" in p for p in launch.validate_worktree(cfg, TAG))


def test_backend_checkout_is_never_a_launch_dir(
    repo: Path, cfg: RunnerConfig, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Point run_worktrees so that the tag's worktree path IS the repo itself.
    monkeypatch.setattr(launch, "worktree_dir", lambda _cfg, _tag: repo)
    info = launch.worktree_status(cfg, TAG)
    assert "the worktree is the backend's own checkout" in info["problems"]
