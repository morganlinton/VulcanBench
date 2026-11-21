"""scripts/frontier-v5/import_harbor.py on synthetic Harbor grid jobs.

One job per agent (Codex, Claude Code, dsh) with the files each Harbor agent
leaves (``codex.txt`` plus the rollout, ``claude-code.txt`` plus session logs,
dsh's session logs), a grid verifier record (``items.json`` and
``reward.json``, the grid brief's schema), and an artifact tree for the
patch. No Docker, no model calls.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts" / "frontier-v5" / "import_harbor.py"
TASK = "treesitter-grid"
JOB = "sol62-max-grid-b1"

_spec = importlib.util.spec_from_file_location("import_harbor", SCRIPT)
assert _spec and _spec.loader
ih = importlib.util.module_from_spec(_spec)
sys.modules["import_harbor"] = ih
_spec.loader.exec_module(ih)

ITEMS = [
    "anchor-first-child",
    "anchor-last-child",
    "quantifier-anchor",
    "predicate-cache",
    "field-negation",
    "range-limit",
    "add-match-limit-flag",
    "diagnose-slow-query",
]
KIND = {"add-match-limit-flag": "feature", "diagnose-slow-query": "diagnosis"}


def items_doc(failing: tuple[str, ...], tamper_clean: bool = True) -> dict:
    return {
        "schema": 1,
        "tamper_clean": tamper_clean,
        "items": {
            i: {
                "kind": KIND.get(i, "defect"),
                "done": i not in failing,
                "checks_passed": 2 if i in failing else 3,
                "checks_total": 3,
            }
            for i in ITEMS
        },
        "guards": {"total": 120, "failed": ["guard::one"] if failing else []},
    }


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
    tamper_clean: bool = True,
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
                    "agent_timeout_multiplier": 0.111,
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
            doc = items_doc(failing, tamper_clean)
            (v / "items.json").write_text(json.dumps(doc))
            done = sum(1 for it in doc["items"].values() if it["done"]) if tamper_clean else 0
            (v / "reward.json").write_text(
                json.dumps(
                    {"reward": done / len(ITEMS), "items_done": done, "items_total": len(ITEMS)}
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
    job = make_job(tmp_path / "jobs", JOB, "codex", "gpt-6.2-sol")
    out = tmp_path / "out"
    assert ih.main([str(job), "--out", str(out), "--bases", str(bases(tmp_path))]) == 0
    r = run_json(out, job)
    assert r["schema"] == "vulcanbench-frontier-v5-run/2"
    assert (r["ref"], r["batch"], r["agent"], r["model"], r["effort"], r["task"]) == (
        "sol62",
        1,
        "codex",
        "gpt-6.2-sol",
        "max",
        TASK,
    )
    assert r["scored"] and r["reward"] == 1 and r["problems"] == []
    assert (r["items_done"], r["items_total"], r["tamper_clean"]) == (8, 8, True)
    assert r["items_by_kind"] == {"defect": [6, 6], "diagnosis": [1, 1], "feature": [1, 1]}
    assert r["guards_total"] == 120 and r["guards_failed"] == 0
    assert not r["budget_reached"] and r["agent_timeout_multiplier"] == 0.111
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
    items = json.loads((out / job.name / r["trial"] / "items.json").read_text())
    assert sorted(items["items"]) == sorted(ITEMS) and items["tamper_clean"] is True


def test_codex_timing_needs_matching_item_kinds(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", "terra-max-grid-b1", "codex", "gpt-6.1-terra")
    rollout = next((job / f"{TASK[:32]}__T0" / "agent").glob("sessions/**/rollout-*.jsonl"))
    rows = [json.loads(x) for x in rollout.read_text().splitlines()]
    rollout.write_text("".join(json.dumps(r) + "\n" for r in rows[:2]))  # FileChange missing
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    r = run_json(out, job)
    assert r["tool_calls"]["count"] == 2 and r["tool_calls"]["timed"] == {"start": 0, "end": 0}
    assert r["patch"] == {"missing": "no base trees given (--bases)"}


def test_claude_code_trial_record(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", "fable52-max-grid-b2", "claude-code", "claude-fable-5-2")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
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
    job = make_job(tmp_path / "jobs", "dsflash-max-grid-b3", "dsh", "deepseek-flash")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
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
    job = make_job(tmp_path / "jobs", "dsflash-max-grid-b3", "dsh", "deepseek-flash")
    (job / f"{TASK[:32]}__T0" / "agent" / "vb-infra.txt").write_text(
        "VULCANBENCH_INFRA: dsh error event QUOTA\n"
    )
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    r = run_json(out, job)
    assert not r["scored"] and r["infra"] == ["dsh error event QUOTA"]


def test_budget_stop_partial_items_and_safety(tmp_path: Path) -> None:
    job = make_job(
        tmp_path / "jobs",
        JOB,
        "codex",
        "gpt-6.2-sol",
        failing=("range-limit", "add-match-limit-flag"),
        exception="AgentTimeoutError",
        trials=2,
    )
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    r0 = run_json(out, job)
    assert r0["scored"] and r0["budget_reached"] and r0["exception"] == "AgentTimeoutError"
    assert r0["infra"] == [] and r0["flags"] == []
    assert (r0["items_done"], r0["items_total"]) == (6, 8) and r0["reward"] == 0.75
    assert r0["items_by_kind"]["feature"] == [0, 1] and r0["guards_failed"] == 1
    job2 = make_job(
        tmp_path / "jobs2", JOB, "codex", "gpt-6.2-sol", exception="AgentSafetyRefusalError"
    )
    ih.main([str(job2), "--out", str(tmp_path / "o2")])
    r = run_json(tmp_path / "o2", job2)
    assert r["scored"] and r["flags"] == ["safety-refusal"] and not r["budget_reached"]


def test_tamper_flag_scores_zero_items(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", JOB, "codex", "gpt-6.2-sol", tamper_clean=False)
    out = tmp_path / "out"
    assert ih.main([str(job), "--out", str(out)]) == 0
    r = run_json(out, job)
    assert r["scored"] and r["items_done"] == 0 and r["tamper_clean"] is False


def test_verifier_problems_are_unscored_and_fail_the_import(tmp_path: Path, capsys) -> None:
    job = make_job(tmp_path / "jobs", JOB, "codex", "gpt-6.2-sol")
    v = job / f"{TASK[:32]}__T0" / "verifier"
    (v / "reward.json").write_text(json.dumps({"reward": 1, "items_done": 7, "items_total": 8}))
    out = tmp_path / "out"
    assert ih.main([str(job), "--out", str(out)]) == 1
    r = run_json(out, job)
    assert not r["scored"] and r["items_done"] is None and r["infra"] == []
    assert any("items_done=7" in p for p in r["problems"])
    (v / "items.json").unlink()
    assert ih.main([str(job), "--out", str(tmp_path / "o2")]) == 1
    assert run_json(tmp_path / "o2", job)["problems"] == [
        "the verifier wrote reward.json but no items.json"
    ]
    assert "verifier problems" in capsys.readouterr().err


def test_a_verifier_output_harbor_rejected_is_unscored(tmp_path: Path) -> None:
    job = make_job(
        tmp_path / "jobs", JOB, "codex", "gpt-6.2-sol", exception="VerifierOutputParseError"
    )
    out = tmp_path / "out"
    assert ih.main([str(job), "--out", str(out)]) == 1
    r = run_json(out, job)
    assert not r["scored"] and r["items_done"] is None
    assert r["problems"] == ["Harbor VerifierOutputParseError: the verifier's output was rejected"]


def test_claude_session_stopped_at_the_budget_is_priced_from_its_messages(tmp_path: Path) -> None:
    job = make_job(
        tmp_path / "jobs",
        "fable52-max-grid-b2",
        "claude-code",
        "claude-fable-5-2",
        exception="AgentTimeoutError",
    )
    ar = job / f"{TASK[:32]}__T0"
    res = json.loads((ar / "result.json").read_text())
    res["agent_result"] = {"n_input_tokens": None, "n_cache_tokens": None, "n_output_tokens": None}
    (ar / "result.json").write_text(json.dumps(res))
    usage = {
        "input_tokens": 10,
        "output_tokens": 500,
        "cache_read_input_tokens": 9000,
        "cache_creation_input_tokens": 1000,
    }
    msg = {"id": "msg_1", "model": "claude-fable-5-2", "usage": usage, "content": []}
    rows = [
        {"type": "system", "subtype": "init"},
        {"type": "assistant", "message": {**msg, "content": [{"type": "text", "text": "a"}]}},
        {"type": "assistant", "message": {**msg, "content": [{"type": "text", "text": "b"}]}},
        {"type": "assistant", "message": {**msg, "id": "msg_2"}},
        {"type": "assistant", "message": {"id": "x", "model": "<synthetic>", "usage": usage}},
    ]
    (ar / "agent" / "claude-code.txt").write_text("".join(json.dumps(x) + "\n" for x in rows))
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    r = run_json(out, job)
    assert r["budget_reached"] and r["scored"]
    # two distinct messages: 20 input, 18000 cache read, 2000 cache write, 1000 output
    assert r["cost"]["api_equivalent_usd"] == pytest.approx(
        (20 * 8 + 18000 * 0.2 + 2000 * 10 + 1000 * 40) / 1e6
    )
    assert "stopped before its result event" in r["cost"]["basis"]
    assert r["tokens"] == {
        "input": 20020,
        "cached_input": 18000,
        "output": 1000,
        "cache_write": 2000,
    }


def test_infra_exception_and_missing_verifier_are_unscored(tmp_path: Path) -> None:
    j1 = make_job(
        tmp_path / "jobs",
        "terra-max-grid-b1",
        "codex",
        "gpt-6.1-terra",
        exception="ApiUsageLimitError",
    )
    j2 = make_job(tmp_path / "jobs2", "terra-max-grid-b1", "codex", "gpt-6.1-terra", verifier=False)
    out1, out2 = tmp_path / "o1", tmp_path / "o2"
    ih.main([str(j1), "--out", str(out1)])
    ih.main([str(j2), "--out", str(out2)])
    assert run_json(out1, j1)["infra"] == ["exception ApiUsageLimitError"]
    r2 = run_json(out2, j2)
    assert not r2["scored"] and r2["reward"] is None and r2["infra"] == ["the verifier did not run"]


def test_infra_jobs_are_skipped_unless_asked(tmp_path: Path, capsys) -> None:
    job = make_job(tmp_path / "jobs", "INFRA-2026-11-21-sol62-max-grid-b1", "codex", "gpt-6.2-sol")
    out = tmp_path / "out"
    assert ih.main([str(job), "--out", str(out)]) == 0
    assert not out.exists() and "set aside as INFRA" in capsys.readouterr().err
    ih.main([str(job), "--out", str(out), "--include-infra"])
    assert "job set aside as INFRA" in run_json(out, job)["infra"]


def test_ref_effort_and_batch_of() -> None:
    assert ih.ref_and_effort(JOB, "max") == ("sol62", "max") and ih.batch_of(JOB) == 1
    assert ih.ref_and_effort("astra-medium-grid-b12", "medium") == ("astra", "medium")
    assert ih.ref_and_effort("terra-max-grid-b3-rerun", "max") == ("terra", "max")
    # Harbor's oracle and nop have no reasoning_effort: the job name gives it.
    assert ih.ref_and_effort("oracle-max-grid-b1", None) == ("oracle", "max")
    assert ih.ref_and_effort("odd", None) == ("odd", "medium")
    assert ih.batch_of("terra-max-grid-b3-rerun") == 3 and ih.batch_of("odd") is None


def test_codex_cyber_flag_is_a_recorded_safety_refusal(tmp_path: Path) -> None:
    job = make_job(
        tmp_path / "jobs",
        JOB,
        "codex",
        "gpt-6.2-sol",
        exception="AgentSafetyRefusalError",
    )
    log = job / f"{TASK[:32]}__T0" / "agent" / "codex.txt"
    msg = "This content was flagged for possible cybersecurity risk. If this seems wrong, try rephrasing."
    log.write_text(log.read_text() + json.dumps({"type": "error", "message": msg}) + "\n")
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    r = run_json(out, job)
    assert r["scored"] and r["reward"] == 1 and r["infra"] == []
    assert r["safety_refusal"] and r["safety_message"] == msg


def test_claude_refusal_fallback_is_a_recorded_safety_refusal(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", "fable52-max-grid-b2", "claude-code", "claude-fable-5-2")
    log = job / f"{TASK[:32]}__T0" / "agent" / "claude-code.txt"
    ev = {
        "type": "system",
        "subtype": "model_refusal_fallback",
        "fallback_model": "claude-opus-4-8",
    }
    log.write_text(json.dumps(ev) + "\n" + log.read_text())
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    r = run_json(out, job)
    assert r["scored"] and r["flags"] == ["safety-refusal"]
    assert r["safety_message"] == "Claude Code model_refusal_fallback to claude-opus-4-8"


def test_refusal_text_in_tool_output_is_not_a_refusal(tmp_path: Path) -> None:
    job = make_job(tmp_path / "jobs", "terra-max-grid-b1", "codex", "gpt-6.1-terra")
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
    ih.main([str(job), "--out", str(out)])
    r = run_json(out, job)
    assert not r["safety_refusal"] and r["flags"] == []


def test_running_trials_are_skipped(tmp_path: Path, capsys) -> None:
    job = make_job(tmp_path / "jobs", JOB, "codex", "gpt-6.2-sol", trials=2)
    res = job / f"{TASK[:32]}__T1" / "result.json"
    d = json.loads(res.read_text())
    d.pop("finished_at")
    res.write_text(json.dumps(d))
    out = tmp_path / "out"
    ih.main([str(job), "--out", str(out)])
    assert (out / job.name / f"{TASK[:32]}__T0" / "run.json").is_file()
    assert not (out / job.name / f"{TASK[:32]}__T1").exists()
    assert "still running" in capsys.readouterr().err
