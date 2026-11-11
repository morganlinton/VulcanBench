"""scripts/computer-use/summarize_gate.py on synthetic gate jobs for both subscription rows."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "summarize_gate", REPO / "scripts" / "computer-use" / "summarize_gate.py"
)
assert SPEC and SPEC.loader
sg = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sg)

TASK = "demo-task"
WALL = {"started_at": "2026-11-10T12:00:00Z", "finished_at": "2026-11-10T12:02:00Z"}


def make_job(root: Path, ref: str, model: str, agent_files: dict[str, str]) -> Path:
    jobs = root / "jobs"
    job = f"cu-{ref}-{TASK}-m5"
    trial = jobs / job / f"{TASK}__abc"
    (trial / "verifier").mkdir(parents=True)
    (trial / "agent").mkdir()
    (trial / "verifier" / "reward.json").write_text(json.dumps({"reward": 1, "dense": 1.0}))
    (trial / "result.json").write_text(json.dumps({**WALL, "agent_execution": WALL}))
    for name, text in agent_files.items():
        (trial / "agent" / name).write_text(text)
    status = {"job": job, "task": TASK, "model": model, "effort": "medium", "condition": "pixels"}
    (root / f"{job}.status.json").write_text(json.dumps(status))
    return jobs


def test_codex_trial_numbers_come_from_the_trajectory_and_list_prices(tmp_path: Path) -> None:
    traj = {
        "steps": [
            {"step_id": 1, "source": "user"},
            {"step_id": 2, "source": "agent"},
            {"step_id": 3, "source": "agent"},
        ],
        "final_metrics": {
            "total_prompt_tokens": 1_000_000,
            "total_completion_tokens": 100_000,
            "total_cached_tokens": 800_000,
        },
    }
    log = "\n".join(
        [
            "Reading prompt",
            json.dumps(
                {"type": "item.completed", "item": {"type": "mcp_tool_call", "tool": "screenshot"}}
            ),
            json.dumps(
                {"type": "item.completed", "item": {"type": "mcp_tool_call", "tool": "left_click"}}
            ),
            json.dumps(
                {"type": "item.completed", "item": {"type": "agent_message", "text": "done"}}
            ),
        ]
    )
    jobs = make_job(
        tmp_path, "codex", "gpt-6.1-sol", {"trajectory.json": json.dumps(traj), "codex.txt": log}
    )
    entry = sg.summarise(jobs, tmp_path, "-m5", TASK, "codex")
    trial = entry["trials"][0]
    assert entry["ref"] == "codex" and entry["solved"] == 1 and entry["condition"] == "pixels"
    assert trial["num_turns"] == 2 and trial["tool_calls"] == 2 and trial["duration_api_s"] is None
    assert trial["tokens"] == {
        "input": 200_000,
        "output": 100_000,
        "cache_read": 800_000,
        "cache_creation": None,
    }
    # gpt-6.1-sol at 2.00 input, 0.10 cached input, 10.00 output per 1M tokens
    assert trial["cost_usd"] == pytest.approx(0.4 + 0.08 + 1.0)
    assert trial["agent_wall_s"] == 120
    assert "| demo-task [pixels] | 1 of 1 | 2 |" in sg.table([entry], {})


def test_codex_trial_without_a_trajectory_reports_unknowns(tmp_path: Path) -> None:
    jobs = make_job(tmp_path, "codex", "gpt-6.1-sol", {})
    trial = sg.summarise(jobs, tmp_path, "-m5", TASK, "codex")["trials"][0]
    assert trial["num_turns"] is None and trial["cost_usd"] is None and trial["tool_calls"] == 0
    assert "| ? |" in sg.table([sg.summarise(jobs, tmp_path, "-m5", TASK, "codex")], {})


def test_claude_code_trial_numbers_still_come_from_the_result_record(tmp_path: Path) -> None:
    result = {
        "type": "result",
        "num_turns": 31,
        "duration_api_ms": 61_400,
        "total_cost_usd": 0.734,
        "usage": {
            "input_tokens": 10,
            "output_tokens": 2000,
            "cache_read_input_tokens": 5,
            "cache_creation_input_tokens": 7,
        },
    }
    jobs = make_job(
        tmp_path, "claude-code", "claude-opus-5-5", {"claude-code.txt": json.dumps(result) + "\n"}
    )
    trial = sg.summarise(jobs, tmp_path, "-m5", TASK)["trials"][0]
    assert trial["num_turns"] == 31 and trial["duration_api_s"] == 61 and trial["cost_usd"] == 0.73
    assert trial["tokens"]["output"] == 2000 and "tool_calls" not in trial
