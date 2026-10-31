"""Benchmark-a-model plans: options at a tag, one job per effort level, quota-free checks, progress."""

from __future__ import annotations

import json
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from backend.runner import launch, sweep
from backend.runner.config import RunnerConfig
from harness.agent.cli_agents import CLI_AGENT_PROVIDERS
from tests.runner_fixtures import (
    SUITE_ID,
    SUITE_TASKS,
    fake_repo,
    fake_worktree,
    git,
    harness_run,
    make_config,
)

TAG = "bench/2026-10-29-test"
NOW = datetime(2026, 10, 29, 13, 0, tzinfo=UTC)


@pytest.fixture
def repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    r = fake_repo(tmp_path)
    monkeypatch.setattr(launch, "REPO_ROOT", r)
    monkeypatch.setattr(sweep, "REPO_ROOT", r)
    return r


@pytest.fixture
def cfg(tmp_path: Path) -> RunnerConfig:
    return make_config(tmp_path)


def form(**over: object) -> sweep.SuiteForm:
    base: dict[str, object] = {
        "model": "gpt-6.1-terra",
        "harness": "codex",
        "suite": SUITE_ID,
        "tag": TAG,
        "efforts": ("low", "high"),
        "billing": "subscription",
    }
    base.update(over)
    return sweep.SuiteForm(**base)  # type: ignore[arg-type]


def test_harness_list_matches_the_harness_package() -> None:
    assert set(sweep.CLI_HARNESSES) == set(CLI_AGENT_PROVIDERS)
    assert "ultra" not in sweep.efforts_offered() and "max" in sweep.efforts_offered()
    assert (
        sweep.SANDBOX_DEFAULTS["codex"] == "local" and sweep.SANDBOX_DEFAULTS["vulcan"] == "docker"
    )


def test_suites_at_tag_from_git_and_from_the_worktree(repo: Path, cfg: RunnerConfig) -> None:
    from_git = sweep.suites_at(cfg, TAG)
    assert [s["id"] for s in from_git] == [SUITE_ID]
    assert (
        from_git[0]["task_ids"] == sorted(SUITE_TASKS)
        and from_git[0]["name"] == "VulcanBench Demo v1"
    )
    fake_worktree(repo, cfg)
    assert sweep.suites_at(cfg, TAG) == from_git
    opts = sweep.suite_options(cfg, TAG)
    assert opts["harnesses"][-1] == "vulcan" and opts["sandbox_defaults"]["claude-code"] == "local"


def test_plan_has_one_job_per_level_with_the_exact_argv(repo: Path, cfg: RunnerConfig) -> None:
    wt = fake_worktree(repo, cfg)
    spec = sweep.build_suite_plan(cfg, form(), today="2026-10-30")
    assert spec["kind"] == "sweep" and spec["name"] == "gpt-6.1-terra-demo-suite-2026-10-30"
    assert [j["effort"] for j in spec["jobs"]] == ["low", "high"]
    low = spec["jobs"][0]
    assert low["argv"] == [
        str(wt / ".venv" / "bin" / "vulcanbench"),
        "run",
        "--suite",
        SUITE_ID,
        "--model",
        "gpt-6.1-terra",
        "--harness",
        "codex",
        "--billing",
        "subscription",
        "--effort",
        "low",
        "--repeat",
        "1",
        "--sandbox",
        "local",
        "--no-judges",
        "--max-concurrency",
        "1",
        "--max-infra-retries",
        "0",
        "--only-missing",
        "-o",
        str(cfg.harness_runs_dir / spec["name"] / "low"),
    ]
    assert "--allow-dirty" not in low["argv"] and low["env"] == {} and low["cwd"] == str(wt)
    assert low["argv_token"] == f"-o {low['out_dir']}" and low["task_ids"] == sorted(SUITE_TASKS)
    assert low["quota_continue"] == {"max": 0, "wait_s": sweep.QUOTA_WAIT_S, "used": 0}
    # Quota-free checks ran against the worktree's own CLI.
    assert spec["doctor"]["status"] == "ready" and spec["doctor"]["version"] == "9.9.9"
    est = spec["estimate"]
    assert est["status"] == "ok" and est["per_level_usd"] == 4.5 and est["total_usd"] == 9.0
    assert est["total_runs"] == 4 and est["max_wall_s"] == 4 * 3 * 3600
    assert "--max-infra-retries 0" in est["infra_retries"]
    assert spec["usage_check_required"] is False
    # The hash covers every level's argv.
    h = launch.plan_hash(spec)
    spec["jobs"][1]["argv"][-1] = "/elsewhere"
    assert launch.plan_hash(spec) != h
    assert sweep.validate_suite_launch(cfg, low) == []


def test_plan_warnings_and_knobs(repo: Path, cfg: RunnerConfig) -> None:
    old_tag = "bench/2026-10-01-old"
    (repo / "harness" / "cli.py").write_text("# older\ndef harness_doctor(): ...\n")
    git(repo, "commit", "-qam", "older cli")
    git(repo, "tag", old_tag)
    wt = fake_worktree(repo, cfg, old_tag)
    spec = sweep.build_suite_plan(
        cfg,
        form(
            model="gpt-7-nova",
            tag=old_tag,
            efforts=("max",),
            quota_continue=3,
            only_missing=False,
            label="trial",
        ),
        today="2026-10-30",
    )
    warn = " ".join(spec["warnings"])
    assert "no list price" in warn and "not onboarded" in warn
    assert "no --max-infra-retries" in warn and "--max-infra-retries" not in spec["jobs"][0]["argv"]
    assert "--only-missing" not in spec["jobs"][0]["argv"]
    assert "at most 3 time(s)" in warn and spec["estimate"]["total_usd"] is None
    assert spec["name"] == "trial" and spec["estimate"]["status"] == "no_pricing"
    (wt / ".fake-doctor-not-ready").write_text("")
    spec = sweep.build_suite_plan(cfg, form(tag=old_tag), today="2026-10-30")
    assert spec["doctor"]["status"] == "not_ready" and "not ready" in " ".join(spec["warnings"])


def test_plan_refusals(repo: Path, cfg: RunnerConfig) -> None:
    with pytest.raises(launch.LaunchError) as missing:
        sweep.build_suite_plan(cfg, form(), today="2026-10-30")
    assert missing.value.status == 409 and "prepare the worktree first" in missing.value.detail
    fake_worktree(repo, cfg)
    cases = [
        (form(efforts=("ultra",)), 422, "blocked"),
        (form(efforts=("ludicrous",)), 422, "unknown effort"),
        (form(efforts=()), 422, "at least one"),
        (form(harness="vulcan", model="gpt-6.1-terra"), 422, "provider:model"),
        (form(harness="codex", model="openai:gpt-6.1-terra"), 422, "bare model"),
        (form(harness="typewriter"), 422, "unknown harness"),
        (form(billing="subscription", harness="vulcan", model="openai:x"), 422, "CLI harness"),
        (form(suite="nope"), 422, "no frozen suite"),
        (form(repeat=99), 422, "repeat"),
        (form(tag="bench/none"), 422, "no tag"),
        (form(label="../x"), 422, "label"),
        (form(cli_bin_dir="/definitely/not/here"), 422, "CLI bin dir"),
    ]
    for f, status, text in cases:
        with pytest.raises(launch.LaunchError) as exc:
            sweep.build_suite_plan(cfg, f, today="2026-10-30")
        assert exc.value.status == status and text in exc.value.detail, (f, exc.value.detail)


def test_validate_suite_launch_catches_tampering(repo: Path, cfg: RunnerConfig) -> None:
    wt = fake_worktree(repo, cfg)
    spec = sweep.build_suite_plan(cfg, form(efforts=("low",)), today="2026-10-30")
    job = spec["jobs"][0]
    assert sweep.validate_any(cfg, job) == []
    bad = {**job, "argv": [*job["argv"], "--allow-dirty"]}
    assert "argv contains --allow-dirty" in sweep.validate_any(cfg, bad)
    argv = list(job["argv"])
    argv[argv.index("--effort") + 1] = "ultra"
    assert any("blocked" in p for p in sweep.validate_any(cfg, {**job, "argv": argv}))
    argv = list(job["argv"])
    argv[argv.index("-o") + 1] = "/tmp/elsewhere"
    assert any("outside the runner" in p for p in sweep.validate_any(cfg, {**job, "argv": argv}))
    assert any("env override" in p for p in sweep.validate_any(cfg, {**job, "env": {"X": "1"}}))
    (wt / "harness" / "cli.py").write_text("dirty\n")
    assert any("uncommitted" in p for p in sweep.validate_any(cfg, job))


def test_progress_reads_the_output_directory(tmp_path: Path) -> None:
    out = tmp_path / "low"
    harness_run(out, "alpha-task-aaaaaaaa", task_id="alpha-task", functional=1.0, suite=SUITE_ID)
    harness_run(out, "beta-task-bbbbbbbb", task_id="beta-task", functional=0.0, suite=SUITE_ID)
    harness_run(out, "gamma-task-cccccccc", task_id="gamma-task", finished=False)
    spec = {"out_dir": str(out), "task_ids": ["alpha-task", "beta-task", "gamma-task"], "repeat": 1}
    now = datetime.now(UTC)
    p = sweep.progress(spec, now)
    assert p["runs_done"] == 2 and p["passed"] == 1 and p["pass_rate"] == 0.5
    assert p["cost_usd"] == 0.84 and p["avg_run_s"] == 540 and p["eta_s"] is not None
    assert p["current"]["task_id"] == "gamma-task" and p["missing"] == ["gamma-task"]
    assert p["complete"] is False and p["total_runs"] == 3
    harness_run(out, "gamma-task-dddddddd", task_id="gamma-task", functional=1.0)
    assert sweep.progress(spec, now)["complete"] is True
    assert (
        sweep.progress({"out_dir": str(tmp_path / "none"), "task_ids": ["a"]}, now)["runs_done"]
        == 0
    )


def test_classify_exit_sees_quota_and_auth(tmp_path: Path) -> None:
    out = tmp_path / "low"
    out.mkdir()
    log = tmp_path / "console.log"
    log.write_text("running suite\n1 run(s) errored\n")
    spec = {"out_dir": str(out)}
    assert sweep.classify_exit(spec, log, NOW) is None
    d = out / "beta-task-bbbbbbbb"
    d.mkdir()
    (d / "cli-agent-stream.jsonl").write_text('{"message":"Usage limit reached for 5 hours"}\n')
    assert sweep.classify_exit(spec, log, NOW) == "quota"
    log.write_text("provider error refresh token has expired\n")
    assert sweep.classify_exit(spec, log, NOW) == "auth"
    (d / "summary.json").write_text(json.dumps({"task_id": "beta-task"}))
    log.write_text("fine\n")
    assert sweep.classify_exit(spec, log, NOW) is None


def test_child_env_prepends_a_cli_dir_and_keeps_login_homes(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("PATH", "/usr/bin:/bin")
    monkeypatch.setenv("CODEX_HOME", str(tmp_path / "codex"))
    monkeypatch.setenv("CODEX_API_KEY", "sk-no")
    env = launch.child_env(path_prepend=str(tmp_path))
    assert env["PATH"].split(":")[0] == str(tmp_path)
    assert env["CODEX_HOME"] == str(tmp_path / "codex") and "CODEX_API_KEY" not in env
    assert (NOW + timedelta(seconds=1)) > NOW  # keep the import honest for the helper above
