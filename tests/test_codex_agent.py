"""Tests for the Codex agent-CLI runner (``codex:`` specs).

A fake ``codex`` binary on PATH emits canned JSONL events (and writes the
hello-world solution), so the full run_agent pipeline — workspace, diff,
verifier, scoring, hypothetical-API pricing — is exercised offline.
"""

from __future__ import annotations

import json
import os
import stat
from pathlib import Path
from typing import Any

import pytest

from harness.agent.cli_agents import is_cli_agent_spec, run_codex_task
from harness.agent.loop import run_agent
from harness.agent.providers import ProviderError, get_provider
from harness.pricing import cost_usd
from harness.sandbox.docker_executor import SandboxError

# turn.completed usage: input 200 with 100 cached folds to
# round((200-100) + 100*0.1) = 110 effective prompt tokens; output 40.
FAKE_CODEX = """#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
mode = os.environ.get("FAKE_CODEX_MODE", "success")
api_key_present = "OPENAI_API_KEY" in os.environ
usage = {"input_tokens": 200, "cached_input_tokens": 100, "output_tokens": 40}

print(json.dumps({"type": "thread.started", "thread_id": "th1",
                  "api_key_present": api_key_present}))
if mode == "success":
    with open("hello.py", "w") as f:
        f.write('print("hello from vulcanbench")\\n')
print(json.dumps({"type": "item.completed", "item": {
    "id": "i1", "item_type": "command_execution",
    "command": "echo hi", "aggregated_output": "hi", "exit_code": 0}}))
print(json.dumps({"type": "item.completed", "item": {
    "id": "i2", "item_type": "agent_message", "text": "Wrote the file"}}))
if mode == "limit":
    print(json.dumps({"type": "error",
                      "message": "You've hit your usage limit. Try again later."}))
    sys.exit(1)
if mode == "fail":
    print(json.dumps({"type": "turn.failed", "error": {"message": "model exploded"}}))
    sys.exit(1)
print(json.dumps({"type": "turn.completed", "usage": usage}))
"""


class _Collector:
    def __init__(self) -> None:
        self.events: list[tuple[str, dict[str, Any]]] = []

    def record(self, event_type: str, data: dict[str, Any]) -> None:
        self.events.append((event_type, data))


@pytest.fixture()
def fake_codex(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fakebin")
    script = bin_dir / "codex"
    script.write_text(FAKE_CODEX, encoding="utf-8")
    script.chmod(script.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    # Must be stripped from the CLI subprocess env (subscription auth only).
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test-should-be-stripped")
    monkeypatch.delenv("FAKE_CODEX_MODE", raising=False)
    return script


def test_spec_detection() -> None:
    assert is_cli_agent_spec("codex:gpt-5.6-sol")
    assert not is_cli_agent_spec("openai:gpt-5.6-sol")


def test_pricing_alias_maps_to_openai_rates() -> None:
    api = cost_usd("openai:gpt-4o", 1000, 1000)
    cli = cost_usd("codex:gpt-4o", 1000, 1000)
    assert api is not None
    assert cli == api


def test_run_codex_task_success(tmp_path: Path, fake_codex: Path) -> None:
    collector = _Collector()
    ws = tmp_path / "ws"
    ws.mkdir()
    outcome = run_codex_task(
        workspace=ws,
        prompt="do the thing",
        model="gpt-5.6-sol",
        priced_spec="codex:gpt-5.6-sol",
        collector=collector,
        stream_log_path=tmp_path / "stream.jsonl",
    )
    assert outcome.finished
    assert outcome.harness == "codex"
    assert outcome.session_id == "th1"
    assert outcome.num_turns == 1
    assert (outcome.prompt_tokens, outcome.completion_tokens) == (110, 40)
    assert (ws / "hello.py").exists()
    kinds = [k for k, _ in collector.events]
    assert "cli_agent_start" in kinds
    assert "llm_response" in kinds
    assert "tool_observation" in kinds
    # The stream log captured the raw events.
    assert (tmp_path / "stream.jsonl").read_text().count("\n") >= 4


def test_openai_key_stripped_from_subprocess(tmp_path: Path, fake_codex: Path) -> None:
    collector = _Collector()
    ws = tmp_path / "ws"
    ws.mkdir()
    run_codex_task(
        workspace=ws,
        prompt="p",
        model="m",
        priced_spec="codex:m",
        collector=collector,
        stream_log_path=tmp_path / "s.jsonl",
    )
    first = json.loads((tmp_path / "s.jsonl").read_text().splitlines()[0])
    assert first["api_key_present"] is False


def test_usage_limit_raises_provider_error(
    tmp_path: Path, fake_codex: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FAKE_CODEX_MODE", "limit")
    ws = tmp_path / "ws"
    ws.mkdir()
    with pytest.raises(ProviderError, match="subscription limit"):
        run_codex_task(
            workspace=ws,
            prompt="p",
            model="m",
            priced_spec="codex:m",
            collector=_Collector(),
        )


def test_turn_failure_raises_provider_error(
    tmp_path: Path, fake_codex: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FAKE_CODEX_MODE", "fail")
    ws = tmp_path / "ws"
    ws.mkdir()
    with pytest.raises(ProviderError):
        run_codex_task(
            workspace=ws,
            prompt="p",
            model="m",
            priced_spec="codex:m",
            collector=_Collector(),
        )


def test_run_agent_via_codex(tmp_path: Path, fake_codex: Path) -> None:
    result = run_agent(
        task_id="hello-world",
        model="codex:gpt-5.6-sol",
        output_dir=tmp_path / "runs",
        sandbox="local",
        judges=False,
    )
    summary = result["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert summary["cli_agent"]["harness"] == "codex"
    assert summary["cli_agent"]["billing"] == "subscription"


def test_codex_requires_local_sandbox(tmp_path: Path, fake_codex: Path) -> None:
    with pytest.raises(SandboxError, match="--sandbox local"):
        run_agent(
            task_id="hello-world",
            model="codex:gpt-5.6-sol",
            output_dir=tmp_path / "runs",
            sandbox="docker",
            judges=False,
        )


def test_codex_judge_provider_single_shot(fake_codex: Path) -> None:
    provider = get_provider("codex:gpt-5.6-sol")
    response = provider.complete(
        [{"role": "user", "content": "rate this"}],
        tools=[],
    )
    assert response.content == "Wrote the file"
    assert response.usage.prompt_tokens == 110
    assert response.usage.completion_tokens == 40
