"""Runner discovery over harbor gate jobs and harness run dirs (no processes, no Docker)."""

from __future__ import annotations

from datetime import timedelta
from pathlib import Path

from backend.runner.discovery import Discovery, parse_dt, tail_text
from tests.runner_fixtures import (
    T0,
    claude_stream,
    codex_stream,
    gate_job,
    harbor_proc,
    harness_run,
    make_config,
    trial_containers,
)

NO_SOURCE = (None, None)


def _source(_: Path) -> tuple[str | None, str | None]:
    return ("bench/2026-10-28-wave1", "4bdd2330")


def test_finished_gate_job_counts_attempts_solves_and_source(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    gate_job(
        cfg,
        "opus-javaparser-member-resolution-scope",
        trials=[
            {"id": "aaa1111", "reward": {"reward": 0, "tests_seen": 10}},
            {"id": "bbb2222", "reward": {"reward": 1, "tests_seen": 10}},
            {"id": "ccc3333", "reward": {"reward": 0, "tests_seen": 10}},
        ],
    )
    runs = Discovery(
        cfg, procs=[], containers=[], now=T0 + timedelta(hours=2), source_info=_source
    ).all_runs()
    assert len(runs) == 1
    r = runs[0]
    assert r.id == "gate:opus-javaparser-member-resolution-scope"
    assert r.kind == "gate" and r.status == "finished" and r.owned is False
    assert r.model == "claude-opus-5-5" and r.harness == "claude-code" and r.effort == "medium"
    assert r.suite_or_task == "javaparser-member-resolution-scope"
    assert (r.attempts_done, r.attempts_total, r.solves) == (3, 3, 1)
    assert (r.source_tag, r.source_commit) == ("bench/2026-10-28-wave1", "4bdd2330")
    assert r.elapsed_s == 3600.0 and r.cost_usd == 4.5
    assert [t.reward["reward"] for t in r.trials if t.reward] == [0, 1, 0]
    assert all(t.status == "finished" for t in r.trials)
    assert all(t.first_check and t.first_check.status == "ok" for t in r.trials)
    assert "job.log" in r.log_sources


def test_running_gate_job_needs_a_live_harbor_process(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    job = "opus-javaparser-member-resolution-scope"
    gate_job(
        cfg,
        job,
        finished=False,
        trials=[
            {"id": "aaa1111", "reward": {"reward": 0}},
            {"id": "bbb2222", "running": True, "stream": claude_stream(tool_calls=0, turns=0)},
        ],
    )
    trial_dir = next(
        d for d in (cfg.gate_out / "jobs" / job).iterdir() if d.name.endswith("bbb2222")
    )
    now = T0 + timedelta(minutes=30)

    alive = Discovery(
        cfg,
        procs=[harbor_proc(job)],
        containers=trial_containers(trial_dir.name),
        now=now,
        source_info=lambda _: NO_SOURCE,
    ).get("gate:" + job)
    assert alive is not None
    assert alive.status == "external_running" and alive.pid == 4242
    assert alive.finished_at is None and alive.elapsed_s == 1800.0
    assert (alive.attempts_done, alive.attempts_total) == (1, 3)
    running = alive.trials[1]
    assert running.status == "running"
    assert [c.name for c in running.containers] == [trial_dir.name.lower() + "__env-main-1"]
    assert alive.first_check is running.first_check

    # Same tree, no harbor process: a null finished_at is "stopped", never "running".
    dead = Discovery(cfg, procs=[], containers=[], now=now, source_info=lambda _: NO_SOURCE).get(
        "gate:" + job
    )
    assert dead is not None
    assert dead.status == "stopped" and dead.pid is None
    assert dead.trials[1].status == "interrupted"
    assert any("no harbor process" in n for n in dead.notes)


def test_early_exit_marks_the_third_trial_cancelled(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    gate_job(
        cfg,
        "astra-boosturl-path-compare-normalize",
        agent="codex",
        model="gpt-6-astra",
        task="boosturl-path-compare-normalize",
        trials=[
            {"id": "s1", "reward": {"reward": 1}, "stream": codex_stream()},
            {"id": "s2", "reward": {"reward": 1}, "stream": codex_stream()},
            {
                "id": "x3",
                "reward": None,
                "exception": "RuntimeError",
                "stream": codex_stream(commands=1),
            },
        ],
    )
    r = Discovery(
        cfg, procs=[], containers=[], now=T0 + timedelta(days=1), source_info=lambda _: NO_SOURCE
    ).all_runs()[0]
    assert r.solves == 2 and r.status == "finished"
    assert [t.status for t in r.trials] == ["finished", "finished", "cancelled"]
    assert r.trials[2].exception is not None and r.trials[2].exception.startswith("RuntimeError")
    assert r.trials[0].first_check is not None
    assert r.trials[0].first_check.status == "ok"
    assert r.trials[0].first_check.models == {"gpt-6-astra": 0}
    assert r.trials[0].first_check.tool_calls == 3


def test_status_file_from_run_gate_wins_over_guesses(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    job = "fable-x-task"
    gate_job(
        cfg, job, task="x-task", finished=False, trials=[{"id": "a1", "reward": {"reward": 0}}]
    )
    (cfg.gate_out / f"{job}.status.json").write_text(
        '{"phase": "infra_failed", "stop_reason": "infra", "repo": "deadbee"}'
    )
    r = Discovery(cfg, procs=[], containers=[], now=T0, source_info=lambda _: NO_SOURCE).get(
        "gate:" + job
    )
    assert r is not None
    assert r.status == "infra_failed"
    assert r.source_commit == "deadbee"
    assert f"../../{job}.status.json" in r.log_sources


def test_job_dir_prefixes_and_missing_config_are_handled(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    gate_job(
        cfg,
        "STOPPED-usage-pause-fable-t",
        task="t",
        finished=False,
        trials=[{"id": "a1", "reward": {"reward": 0}}],
    )
    (cfg.gate_out / "jobs" / "not-a-job").mkdir()
    runs = Discovery(
        cfg, procs=[], containers=[], now=T0, source_info=lambda _: NO_SOURCE
    ).all_runs()
    assert [r.status for r in runs] == ["stopped"]


def test_harness_runs_finished_and_live(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    root = cfg.harness_runs_dir
    harness_run(
        root / "high", "hello-world-0a1b2c3d", functional=1.0, suite="coding-intelligence-index-v4"
    )
    harness_run(root / "high", "other-task-11112222", functional=0.0)
    harness_run(
        root / "high", "live-task-33334444", finished=False, started=T0 - timedelta(minutes=5)
    )
    now = T0
    runs = Discovery(
        cfg, procs=[], containers=[], now=now, source_info=lambda _: NO_SOURCE
    ).all_runs()
    by_name = {r.name: r for r in runs}
    assert set(by_name) == {"hello-world-0a1b2c3d", "other-task-11112222", "live-task-33334444"}
    done = by_name["hello-world-0a1b2c3d"]
    assert done.id == "harness:0:high/hello-world-0a1b2c3d"
    assert done.kind == "suite" and done.suite_or_task == "coding-intelligence-index-v4"
    assert done.status == "finished" and done.solves == 1 and done.model == "claude-opus-5-5"
    assert done.harness == "claude-code" and done.effort == "high"
    assert (done.source_tag, done.source_commit) == ("bench/2026-10-05-x", "abc1234")
    assert done.elapsed_s == 540.0 and done.cost_usd == 0.42
    assert by_name["other-task-11112222"].solves == 0
    live = by_name["live-task-33334444"]
    assert live.status == "running"  # trace still fresh, no process known
    assert runs[0] is live  # active runs sort first

    stale = Discovery(
        cfg, procs=[], containers=[], now=now + timedelta(hours=1), source_info=lambda _: NO_SOURCE
    )
    assert stale.get(live.id) is not None
    assert stale.get(live.id).status == "stopped"  # type: ignore[union-attr]


def test_get_refuses_ids_that_escape_the_roots(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    d = Discovery(cfg, procs=[], containers=[], now=T0, source_info=lambda _: NO_SOURCE)
    assert d.get("gate:../../etc") is None
    assert d.get("harness:0:../../etc") is None
    assert d.get("harness:9:x") is None
    assert d.get("bogus") is None


def test_resolve_log_allowlist(tmp_path: Path) -> None:
    cfg = make_config(tmp_path)
    job = "opus-t"
    gate_job(cfg, job, task="t", trials=[{"id": "a1", "reward": {"reward": 0}}])
    (cfg.gate_out / f"{job}.log").write_text("harbor table\n")
    (cfg.gate_out / "other.log").write_text("not yours\n")
    sessions = cfg.gate_out / "jobs" / job / "t__a1" / "agent" / "sessions"
    sessions.mkdir(parents=True)
    (sessions / "secret.jsonl").write_text("x")
    d = Discovery(cfg, procs=[], containers=[], now=T0, source_info=lambda _: NO_SOURCE)
    run = d.get("gate:" + job)
    assert run is not None
    assert d.resolve_log(run, "trial.log", "a1") is not None
    assert d.resolve_log(run, "agent/claude-code.txt", "a1") is not None
    assert d.resolve_log(run, "agent/sessions/secret.jsonl", "a1") is None
    assert d.resolve_log(run, "../config.json", "a1") is None
    assert d.resolve_log(run, "job.log", None) is not None
    assert d.resolve_log(run, f"../../{job}.log", None) is not None
    assert d.resolve_log(run, "../../other.log", None) is None
    assert d.resolve_log(run, "trial.log", "nope") is None


def test_parse_dt_and_tail_text(tmp_path: Path) -> None:
    assert parse_dt("2026-10-28T13:37:43.669496Z") is not None
    naive = parse_dt("2026-10-28T06:37:43.432861")
    assert naive is not None and naive.tzinfo is not None
    assert parse_dt("nonsense") is None and parse_dt(None) is None
    f = tmp_path / "big.log"
    f.write_text("\n".join(f"line {i}" for i in range(1000)) + "\n")
    text, truncated = tail_text(f, max_bytes=2000, max_lines=50)
    assert truncated and text.splitlines()[-1] == "line 999" and len(text.splitlines()) == 50
    text, truncated = tail_text(f, max_lines=5000)
    assert not truncated and text.splitlines()[0] == "line 0"
