"""scripts/frontier-v5/import_harbor.py on synthetic Harbor jobs.

One job per agent (Codex, Claude Code, dsh) with the files each Harbor agent
leaves (``codex.txt`` plus the rollout, ``claude-code.txt`` plus session logs,
dsh's session logs), a tree-sitter verifier record graded from the task's real
families.json, and an artifact tree for the patch. No Docker, no model calls.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts" / "frontier-v5" / "import_harbor.py"
TASK = "treesitter-query-anchor-matching"

_spec = importlib.util.spec_from_file_location("import_harbor", SCRIPT)
assert _spec and _spec.loader
ih = importlib.util.module_from_spec(_spec)
sys.modules["import_harbor"] = ih
_spec.loader.exec_module(ih)

FAM = json.loads((REPO / "tasks" / "frontier-v5" / TASK / "tests" / "families.json").read_text())


def _jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(r) + "\n" for r in rows))


def make_job(
    root: Path,
    name: str,
    agent: str,
    model: str,
    *,
    effort: str = "max",
    failing: tuple[str, ...] = (),
    exception: str | None = None,
    verifier: bool = True,
    trials: int = 1,
    tokens: tuple[int, int, int] = (1_000_000, 800_000, 20_000),
) -> Path:
    job = root / name
    job.mkdir(parents=True)
    (job / "config.json").write_text(json.dumps({"job_name": name, "n_concurrent_trials": 1}))
    for i in range(trials):
        tr = job / f"{TASK[:32]}__T{i}"
        (tr / "agent").mkdir(parents=True)
        (tr / "config.json").write_text(
            json.dumps(
                {
                    "task": {"path": f"/wt/tasks/frontier-v5/{TASK}"},
                    "agent": {
                        "name": agent if agent != "dsh" else None,
                        "import_path": "harbor_dsh_agent:DeepSeekHarness"
                        if agent == "dsh"
                        else None,
                        "model_name": model,
                        "kwargs": {"version": "9.9.9", "reasoning_effort": effort},
                    },
                }
            )
        )
        result = {
            "task_checksum": "abc",
            "agent_info": {"name": agent, "version": "9.9.9"},
            "agent_result": {
                "n_input_tokens": tokens[0],
                "n_cache_tokens": tokens[1],
                "n_output_tokens": tokens[2],
                "cost_usd": 1.23,
            },
            "started_at": f"2026-11-20T0{i}:00:00Z",
            "finished_at": f"2026-11-20T0{i}:40:00Z",
            "agent_execution": {
                "started_at": f"2026-11-20T0{i}:05:00Z",
                "finished_at": f"2026-11-20T0{i}:35:30Z",
            },
            "exception_info": {"exception_type": exception} if exception else None,
        }
        (tr / "result.json").write_text(json.dumps(result))
        if verifier:
            v = tr / "verifier"
            v.mkdir()
            bad = set(failing)
            lines = [
                f"{'failed' if t in bad else 'ok'} {t.split('::', 1)[1]}"
                for t in FAM["fail_to_pass"] + FAM["pass_to_pass"]
            ]
            (v / "cli.log").write_text("\n".join(lines) + "\n")
            reward = 0 if bad else 1
            (v / "reward.json").write_text(
                json.dumps(
                    {
                        "reward": reward,
                        "tamper_clean": 1,
                        "artifact_present": 1,
                        "build_ok": 1,
                        "fail_to_pass": int(not bad & set(FAM["fail_to_pass"])),
                        "pass_to_pass": int(not bad & set(FAM["pass_to_pass"])),
                    }
                )
            )
        art = tr / "artifacts"
        (art / "app" / "lib" / "src").mkdir(parents=True)
        (art / "app" / "lib" / "src" / "query.c").write_text("int anchor(void) { return 1; }\n")
        (art / "app" / "lib" / "src" / "new.h").write_text("#define NEW 1\n")
        (art / "manifest.json").write_text(
            json.dumps(
                [
                    {
                        "source": "/logs/artifacts",
                        "destination": "artifacts/logs/artifacts",
                        "status": "empty",
                    },
                    {
                        "source": "/app/lib/src",
                        "destination": "artifacts/app/lib/src",
                        "status": "ok",
                    },
                ]
            )
        )
        AGENT_LOGS[agent](tr / "agent")
    return job


def codex_logs(d: Path) -> None:
    _jsonl(
        d / "codex.txt",
        [
            {"type": "thread.started"},
            {
                "type": "item.started",
                "item": {"id": "item_1", "type": "command_execution", "command": "ls"},
            },
            {
                "type": "item.completed",
                "item": {
                    "id": "item_1",
                    "type": "command_execution",
                    "command": "ls",
                    "exit_code": 0,
                    "aggregated_output": "a\nb\n",
                    "status": "completed",
                },
            },
            {
                "type": "item.completed",
                "item": {
                    "id": "item_2",
                    "type": "file_change",
                    "changes": [],
                    "status": "completed",
                },
            },
            {
                "type": "item.completed",
                "item": {"id": "item_3", "type": "agent_message", "text": "done"},
            },
        ],
    )
    _jsonl(
        d / "sessions" / "2026" / "11" / "20" / "rollout-2026-11-20T00-05-00-x.jsonl",
        [
            {
                "type": "event_msg",
                "payload": {
                    "type": "item_completed",
                    "item": {"id": "r1", "type": "Reasoning"},
                    "started_at_ms": 1795132000000,
                    "completed_at_ms": 1795132000500,
                },
            },
            {
                "type": "event_msg",
                "payload": {
                    "type": "item_completed",
                    "item": {"id": "e1", "type": "CommandExecution"},
                    "started_at_ms": 1795132001000,
                    "completed_at_ms": 1795132003000,
                },
            },
            {
                "type": "event_msg",
                "payload": {
                    "type": "item_completed",
                    "item": {"id": "f1", "type": "FileChange"},
                    "started_at_ms": 1795132004000,
                    "completed_at_ms": 1795132004100,
                },
            },
        ],
    )


def claude_logs(d: Path) -> None:
    _jsonl(
        d / "claude-code.txt",
        [
            {"type": "system", "subtype": "init"},
            {
                "type": "assistant",
                "uuid": "u1",
                "message": {
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "Bash",
                            "input": {"command": "make"},
                        }
                    ]
                },
            },
            {
                "type": "user",
                "uuid": "u2",
                "message": {
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": "ok",
                            "is_error": False,
                        }
                    ]
                },
            },
            {
                "type": "assistant",
                "uuid": "u3",
                "parent_tool_use_id": "toolu_0",
                "message": {
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_2",
                            "name": "Read",
                            "input": {"file_path": "x"},
                        }
                    ]
                },
            },
            {
                "type": "result",
                "is_error": False,
                "total_cost_usd": 9.99,
                "modelUsage": {
                    "claude-fable-5-2": {
                        "inputTokens": 1000,
                        "outputTokens": 2000,
                        "cacheReadInputTokens": 100000,
                        "cacheCreationInputTokens": 50000,
                    }
                },
            },
        ],
    )
    _jsonl(
        d / "sessions" / "projects" / "-app" / "s.jsonl",
        [
            {"uuid": "u1", "timestamp": "2026-11-20T00:06:00.000Z", "type": "assistant"},
            {"uuid": "u2", "timestamp": "2026-11-20T00:06:02.500Z", "type": "user"},
        ],
    )


def dsh_logs(d: Path) -> None:
    _jsonl(d / "dsh.txt", [{"type": "session", "sessionId": "s"}, {"type": "final"}])
    _jsonl(
        d / "dsh-session-logs" / "sessions" / "s" / "session.v4.jsonl",
        [
            {
                "type": "tool/call",
                "time": 1795132001000,
                "data": {"callId": "c1", "name": "shell", "arguments": '{"cmd": "ls"}'},
            },
            {
                "type": "tool/result",
                "time": 1795132002000,
                "data": {"message": {"toolCallId": "c1", "isError": True, "content": "boom"}},
            },
        ],
    )


AGENT_LOGS = {"codex": codex_logs, "claude-code": claude_logs, "dsh": dsh_logs}


def bases(root: Path) -> Path:
    b = root / "bases"
    src = b / TASK / "app" / "lib" / "src"
    src.mkdir(parents=True)
    (src / "query.c").write_text("int anchor(void) { return 0; }\n")
    (src / "gone.c").write_text("int gone;\n")
    return b


def run_json(out: Path, job: Path, trial: str = f"{TASK[:32]}__T0") -> dict:
    return json.loads((out / job.name / trial / "run.json").read_text())


def test_codex_trial_record(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"sol62-max-{TASK}", "codex", "gpt-6.2-sol")
    out = tmp_path / "out"
    assert (
        ih.main([str(job), "--out", str(out), "--repo", str(REPO), "--bases", str(bases(tmp_path))])
        == 0
    )
    r = run_json(out, job)
    assert r["schema"] == "vulcanbench-frontier-v5-run/1"
    assert (r["ref"], r["agent"], r["model"], r["effort"], r["attempt"]) == (
        "sol62",
        "codex",
        "gpt-6.2-sol",
        "max",
        1,
    )
    assert r["scored"] and r["reward"] == 1 and r["progress"] == 1.0 and r["checks"]["consistent"]
    assert r["agent_minutes"] == 30.5 and r["concurrency"] == 1
    assert r["tokens"] == {"input": 1_000_000, "cached_input": 800_000, "output": 20_000}
    # gpt-6.2-sol list price: $2 input, $0.10 cached, $10 output per MTok.
    assert r["cost"]["api_equivalent_usd"] == pytest.approx(
        (200_000 * 2 + 800_000 * 0.1 + 20_000 * 10) / 1e6
    )
    assert r["cost"]["harness_reported_usd"] == 1.23
    calls = [
        json.loads(x)
        for x in (out / job.name / r["trial"] / "tool-calls.jsonl").read_text().splitlines()
    ]
    assert [c["tool"] for c in calls] == ["command_execution", "file_change"]
    assert calls[0]["duration_ms"] == 2000 and calls[0]["started_at"].startswith("2026-")
    assert r["tool_calls"]["timed"] == {"start": 2, "end": 2}
    public = (out / job.name / r["trial"] / "public-tool-calls.jsonl").read_text()
    assert '"input"' not in public and '"event": "tool_call"' in public
    patch = (out / job.name / r["trial"] / "patch.diff").read_text()
    assert "a/app/lib/src/query.c" in patch and "-int anchor(void) { return 0; }" in patch
    assert "b/app/lib/src/new.h" in patch and "a/app/lib/src/gone.c" in patch
    assert r["patch"] == {
        "file": "patch.diff",
        "files_changed": 3,
        "lines_added": 2,
        "lines_removed": 2,
    }
    checks = json.loads((out / job.name / r["trial"] / "checks.json").read_text())
    assert len(checks) == len(FAM["fail_to_pass"]) + len(FAM["pass_to_pass"])


def test_codex_timing_needs_matching_item_kinds(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"terra-max-{TASK}", "codex", "gpt-6.1-terra")
    rollout = next((job / f"{TASK[:32]}__T0" / "agent").glob("sessions/**/rollout-*.jsonl"))
    rows = [json.loads(x) for x in rollout.read_text().splitlines()]
    rollout.write_text("".join(json.dumps(r) + "\n" for r in rows[:2]))  # FileChange missing
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert r["tool_calls"]["count"] == 2 and r["tool_calls"]["timed"] == {"start": 0, "end": 0}
    assert r["patch"] == {"missing": "no base trees given (--bases)"}


def test_claude_code_trial_record(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"fable52-max-{TASK}", "claude-code", "claude-fable-5-2")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert r["ref"] == "fable52" and r["agent"] == "claude-code"
    # Fable 5.2 list: $8 input, $0.20 cache read, $10 5m write, $40 output per MTok.
    assert r["cost"]["api_equivalent_usd"] == pytest.approx(
        (1000 * 8 + 100000 * 0.2 + 50000 * 10 + 2000 * 40) / 1e6
    )
    assert r["tokens"]["cache_write"] == 50000
    calls = [
        json.loads(x)
        for x in (out / job.name / r["trial"] / "tool-calls.jsonl").read_text().splitlines()
    ]
    assert [(c["tool"], c["kind"], c["agent"]) for c in calls] == [
        ("Bash", "shell", None),
        ("Read", "read", "subagent"),
    ]
    assert calls[0]["duration_ms"] == 2500 and calls[0]["status"] == "ok"
    assert calls[1]["started_at"] is None  # no session timestamp for that event


def test_dsh_trial_record(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"dsflash-max-{TASK}", "dsh", "deepseek-flash")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert r["ref"] == "dsflash" and r["agent"] == "dsh"
    assert r["cost"]["api_equivalent_usd"] == pytest.approx(
        (200_000 * 0.15 + 800_000 * 0.003 + 20_000 * 0.6) / 1e6
    )
    calls = [
        json.loads(x)
        for x in (out / job.name / r["trial"] / "tool-calls.jsonl").read_text().splitlines()
    ]
    assert (
        calls[0]["tool"] == "shell"
        and calls[0]["status"] == "error"
        and calls[0]["duration_ms"] == 1000
    )


def test_dsh_infra_marker_makes_the_trial_unscored(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"dsflash-max-{TASK}", "dsh", "deepseek-flash")
    (job / f"{TASK[:32]}__T0" / "agent" / "vb-infra.txt").write_text(
        "VULCANBENCH_INFRA: dsh error event QUOTA\n"
    )
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert not r["scored"] and r["infra"] == ["dsh error event QUOTA"]


def test_exceptions_partial_progress_and_attempt_order(tmp_path: Path) -> None:
    job = make_job(
        tmp_path / "jobs",
        f"sol62-max-{TASK}",
        "codex",
        "gpt-6.2-sol",
        failing=(FAM["fail_to_pass"][0],),
        exception="AgentSafetyRefusalError",
        trials=2,
    )
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r0, r1 = run_json(out, job), run_json(out, job, f"{TASK[:32]}__T1")
    assert (r0["attempt"], r1["attempt"]) == (1, 2)
    assert (
        r0["scored"]
        and r0["flags"] == ["safety-refusal"]
        and r0["exception"] == "AgentSafetyRefusalError"
    )
    n = len(FAM["fail_to_pass"]) + 2  # rule C: 11 fail_to_pass units + 2 held-out guards
    assert r0["reward"] == 0 and r0["progress"] == pytest.approx((n - 1) / n, abs=1e-6)


def test_infra_exception_and_missing_verifier_are_unscored(tmp_path: Path) -> None:
    j1 = make_job(
        tmp_path / "jobs",
        f"terra-max-{TASK}",
        "codex",
        "gpt-6.1-terra",
        exception="ApiUsageLimitError",
    )
    j2 = make_job(tmp_path / "jobs2", f"terra-max-{TASK}", "codex", "gpt-6.1-terra", verifier=False)
    out1, out2 = tmp_path / "o1", tmp_path / "o2"
    ih.main([str(j1), "--out", str(out1), "--repo", str(REPO)])
    ih.main([str(j2), "--out", str(out2), "--repo", str(REPO)])
    assert run_json(out1, j1)["infra"] == ["exception ApiUsageLimitError"]
    r2 = run_json(out2, j2)
    assert not r2["scored"] and r2["reward"] is None and r2["infra"] == ["the verifier did not run"]


def test_infra_jobs_are_skipped_unless_asked(tmp_path: Path, capsys) -> None:
    job = make_job(tmp_path / "jobs", f"INFRA-2026-11-19-sol62-max-{TASK}", "codex", "gpt-6.2-sol")
    out = tmp_path / "out"
    assert ih.main([str(job), "--out", str(out), "--repo", str(REPO)]) == 0
    assert not out.exists() and "set aside as INFRA" in capsys.readouterr().err
    ih.main([str(job), "--out", str(out), "--repo", str(REPO), "--include-infra"])
    assert "job set aside as INFRA" in run_json(out, job)["infra"]


def test_ref_of() -> None:
    assert ih.ref_of(f"sol62-max-{TASK}", TASK, "max") == "sol62"
    assert ih.ref_of(f"astra-{TASK}", TASK, None) == "astra"
    assert ih.ref_of(f"terra-max-{TASK}-topup", TASK, "max") == "terra"


def test_codex_cyber_flag_is_a_recorded_safety_refusal(tmp_path: Path) -> None:
    job = make_job(
        tmp_path / "jobs",
        f"sol62-max-{TASK}",
        "codex",
        "gpt-6.2-sol",
        exception="AgentSafetyRefusalError",
    )
    log = job / f"{TASK[:32]}__T0" / "agent" / "codex.txt"
    msg = "This content was flagged for possible cybersecurity risk. If this seems wrong, try rephrasing."
    log.write_text(log.read_text() + json.dumps({"type": "error", "message": msg}) + "\n")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert r["scored"] and r["reward"] == 1 and r["infra"] == []
    assert r["safety_refusal"] and r["safety_message"] == msg


def test_claude_refusal_fallback_is_a_recorded_safety_refusal(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"fable52-max-{TASK}", "claude-code", "claude-fable-5-2")
    log = job / f"{TASK[:32]}__T0" / "agent" / "claude-code.txt"
    ev = {
        "type": "system",
        "subtype": "model_refusal_fallback",
        "fallback_model": "claude-opus-4-8",
    }
    log.write_text(json.dumps(ev) + "\n" + log.read_text())
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert r["scored"] and r["flags"] == ["safety-refusal"]
    assert r["safety_message"] == "Claude Code model_refusal_fallback to claude-opus-4-8"


def test_refusal_text_in_tool_output_is_not_a_refusal(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", f"terra-max-{TASK}", "codex", "gpt-6.1-terra")
    log = job / f"{TASK[:32]}__T0" / "agent" / "codex.txt"
    item = {
        "type": "item.completed",
        "item": {
            "id": "item_9",
            "type": "agent_message",
            "text": "flagged for possible cybersecurity risk",
        },
    }
    log.write_text(log.read_text() + json.dumps(item) + "\n")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    r = run_json(out, job)
    assert not r["safety_refusal"] and r["flags"] == []


def test_running_trials_are_skipped(tmp_path: Path, capsys) -> None:
    job = make_job(tmp_path / "jobs", f"sol62-max-{TASK}", "codex", "gpt-6.2-sol", trials=2)
    res = job / f"{TASK[:32]}__T1" / "result.json"
    d = json.loads(res.read_text())
    d.pop("finished_at")
    res.write_text(json.dumps(d))
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out), "--repo", str(REPO)])
    assert (out / job.name / f"{TASK[:32]}__T0" / "run.json").is_file()
    assert not (out / job.name / f"{TASK[:32]}__T1").exists()
    assert "still running" in capsys.readouterr().err
