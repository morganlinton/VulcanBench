"""Launch plans: the hard rules are enforced server-side, before anything is shown."""

from __future__ import annotations

import os
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
    launch._refs_cache.clear()
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
    monkeypatch.setenv("FABLE_AUTH", "api")
    env = launch.child_env({"GATE_OUT": "/tmp/gate", "GATE_ATTEMPTS": "2"})
    assert set(env) <= {
        "HOME",
        "PATH",
        "USER",
        "LANG",
        "TMPDIR",
        "CLAUDE_GATE_ENV",
        "FABLE_AUTH",
        "GATE_OUT",
        "GATE_ATTEMPTS",
    }
    assert env["GATE_OUT"] == "/tmp/gate" and env["CLAUDE_GATE_ENV"] == "/tmp/claude-gate.env"
    assert not any(k.startswith("VULCANBENCH_") for k in env)
    for bad in ("VULCANBENCH_ALLOW_DIRTY", "DATABASE_URL", "FOO_API_KEY", "X_TOKEN", "MY_SECRET"):
        with pytest.raises(launch.LaunchError):
            launch.child_env({bad: "1"})


def test_options_lists_tags_tasks_and_worktree_state(repo: Path, cfg: RunnerConfig) -> None:
    opts = launch.options(cfg)
    assert opts["tags"] == [TAG] and opts["tag"] == TAG
    assert [r["name"] for r in opts["refs"]] == ["astra", "fable", "opus"]
    assert opts["tasks"] == [{"name": "demo-task", "language": "python", "default_concurrency": 2}]
    wt = opts["worktree"]
    assert (
        wt["exists"] is False
        and wt["problems"] == []
        and wt["path"] == str(cfg.run_worktrees / "bench-2026-10-29-test")
    )
    assert opts["gate_effort"] == "medium" and set(opts["credentials"]) == {
        "codex_auth",
        "claude_gate_env",
    }
    assert "ultra" not in str(opts)


def test_plan_without_worktree_adds_the_prepare_step(repo: Path, cfg: RunnerConfig) -> None:
    spec = launch.build_gate_plan(cfg, launch.GateForm(ref="astra", task="demo-task", tag=TAG))
    wt = cfg.run_worktrees / "bench-2026-10-29-test"
    assert spec["needs_prep"] is True
    assert spec["prep_argv"] == ["bash", str(repo / "scripts" / "run_worktree.sh"), TAG]
    assert spec["prep_env"] == {"RUN_WORKTREES": str(cfg.run_worktrees)}
    assert spec["argv"] == [
        "bash",
        str(wt / "scripts" / "frontier-v5" / "run_gate.sh"),
        "astra",
        "demo-task",
    ]
    assert spec["cwd"] == str(wt) and "--allow-dirty" not in spec["argv"]
    assert spec["env"] == {"GATE_OUT": str(cfg.gate_out), "GATE_ATTEMPTS": "3"}
    assert spec["job_name"] == "astra-demo-task" and spec["model"] == "gpt-6-astra"
    assert spec["effort"] == "medium" and spec["concurrency"] == 2
    assert spec["estimate"]["max_wall_s"] == 2 * 3 * 3600 and spec["usage_check_required"] is False
    assert any("does not exist yet" in w for w in spec["warnings"])
    assert spec["hash"] == launch.plan_hash(spec) and len(spec["hash"]) == 16


def test_plan_with_worktree_and_knobs(repo: Path, cfg: RunnerConfig) -> None:
    wt = fake_worktree(repo, cfg)
    form = launch.GateForm(
        ref="opus", task="demo-task", tag=TAG, attempts=2, concurrency=1, suffix="-topup"
    )
    spec = launch.build_gate_plan(cfg, form)
    assert spec["needs_prep"] is False and spec["prep_argv"] is None
    assert spec["env"] == {
        "GATE_OUT": str(cfg.gate_out),
        "GATE_ATTEMPTS": "2",
        "GATE_CONCURRENCY": "1",
        "GATE_JOB_SUFFIX": "-topup",
    }
    assert spec["job_name"] == "opus-demo-task-topup" and spec["worktree"] == str(wt)
    assert (
        spec["usage_check_required"] is True and spec["script_features"]["preflight_only"] is True
    )
    assert spec["estimate"]["max_wall_s"] == 2 * 3 * 3600
    assert launch.validate_before_launch(cfg, spec) == []
    # The same form gives the same hash; a different knob gives another.
    assert launch.build_gate_plan(cfg, form)["hash"] == spec["hash"]
    other = launch.build_gate_plan(
        cfg, launch.GateForm(ref="opus", task="demo-task", tag=TAG, attempts=1, suffix="-x")
    )
    assert other["hash"] != spec["hash"]


def test_plan_refusals(repo: Path, cfg: RunnerConfig) -> None:
    def err(form: launch.GateForm) -> tuple[int, str]:
        with pytest.raises(launch.LaunchError) as exc:
            launch.build_gate_plan(cfg, form)
        return exc.value.status, exc.value.detail

    assert err(launch.GateForm(ref="ultra", task="demo-task", tag=TAG))[0] == 422
    assert err(launch.GateForm(ref="astra", task="../x", tag=TAG))[0] == 422
    assert err(launch.GateForm(ref="astra", task="nope", tag=TAG))[0] == 422
    assert err(launch.GateForm(ref="astra", task="demo-task", tag="main"))[0] == 422
    assert err(launch.GateForm(ref="astra", task="demo-task", tag="bench/missing"))[0] == 422
    assert err(launch.GateForm(ref="astra", task="demo-task", tag=TAG, attempts=4))[0] == 422
    assert (
        err(launch.GateForm(ref="astra", task="demo-task", tag=TAG, suffix="bad suffix"))[0] == 422
    )
    (cfg.gate_jobs_dir / "astra-demo-task").mkdir()
    status, detail = err(launch.GateForm(ref="astra", task="demo-task", tag=TAG))
    assert status == 409 and "exists" in detail


def test_dirty_or_moved_worktree_is_refused(repo: Path, cfg: RunnerConfig) -> None:
    wt = fake_worktree(repo, cfg)
    form = launch.GateForm(ref="astra", task="demo-task", tag=TAG)
    spec = launch.build_gate_plan(cfg, form)
    (wt / "harness" / "extra.py").write_text("x = 1\n")
    with pytest.raises(launch.LaunchError) as exc:
        launch.build_gate_plan(cfg, form)
    assert exc.value.status == 409 and "uncommitted changes" in exc.value.detail
    assert any("uncommitted" in p for p in launch.validate_before_launch(cfg, spec))
    (wt / "harness" / "extra.py").unlink()
    assert launch.validate_before_launch(cfg, spec) == []
    # HEAD moved off the tag (someone committed in the worktree).
    (wt / "notes.txt").write_text("hi\n")
    git(wt, "add", "notes.txt")
    git(wt, "commit", "-q", "-m", "drift")
    assert any("not at bench/" in p for p in launch.validate_before_launch(cfg, spec))
    tampered = dict(spec, argv=[*spec["argv"], "--allow-dirty"])
    assert any("run_gate.sh invocation" in p for p in launch.validate_before_launch(cfg, tampered))
    tampered = dict(spec, env={"VULCANBENCH_ALLOW_DIRTY": "1"})
    assert any("unexpected env override" in p for p in launch.validate_before_launch(cfg, tampered))


def test_backend_checkout_is_never_a_launch_dir(
    repo: Path, cfg: RunnerConfig, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Point run_worktrees so that the tag's worktree path IS the repo itself.
    monkeypatch.setattr(launch, "worktree_dir", lambda _cfg, _tag: repo)
    info = launch.worktree_status(cfg, TAG)
    assert "the worktree is the backend's own checkout" in info["problems"]


def test_tasks_come_from_git_when_the_worktree_is_missing(repo: Path, cfg: RunnerConfig) -> None:
    assert [t["name"] for t in launch.gate_tasks(cfg, TAG)] == ["demo-task"]
    fake_worktree(repo, cfg)
    assert [t["name"] for t in launch.gate_tasks(cfg, TAG)] == ["demo-task"]


def test_usage_preflight_uses_the_worktree_script(repo: Path, cfg: RunnerConfig) -> None:
    wt = fake_worktree(repo, cfg)
    spec = launch.build_gate_plan(cfg, launch.GateForm(ref="astra", task="demo-task", tag=TAG))
    assert launch.run_usage_preflight(cfg, spec)["status"] == "not_applicable"
    spec = launch.build_gate_plan(cfg, launch.GateForm(ref="opus", task="demo-task", tag=TAG))
    # Stand in for the real script so no claude CLI is needed; keep the feature marker.
    script = wt / "scripts" / "frontier-v5" / "run_gate.sh"
    script.write_text(
        "#!/bin/bash\n# GATE_PREFLIGHT_ONLY write_status\n"
        'echo "noise"\n'
        'echo "{\\"model\\": \\"$1\\", \\"status\\": \\"allowed\\", \\"max_utilization\\": 0.4, \\"would_start\\": true}"\n'
        "exit 0\n"
    )
    out = launch.run_usage_preflight(cfg, spec)
    assert out["status"] == "allowed" and out["would_start"] is True and out["exit_code"] == 0
    assert out["model"] == "opus" and out["script"] == str(script)
    script.write_text("#!/bin/bash\n# GATE_PREFLIGHT_ONLY\necho nope >&2\nexit 2\n")
    out = launch.run_usage_preflight(cfg, spec)
    assert out["status"] == "error" and out["would_start"] is False and out["note"] == "nope"
    assert (
        os.environ.get("VULCANBENCH_ALLOW_DIRTY") == "1"
    )  # the test env has it; the child never does
