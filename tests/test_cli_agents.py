"""Tests for the vendor agent-CLI runner (``claude-code:`` specs).

A fake ``claude`` binary on PATH emits canned ``stream-json`` output (and
writes the hello-world solution), so the full run_agent pipeline — workspace,
diff, verifier, scoring, hypothetical-API pricing — is exercised offline.
"""

from __future__ import annotations

import json
import os
import stat
from pathlib import Path
from typing import Any

import pytest

from harness.agent.cli_agents import is_cli_agent_spec, run_claude_code_task
from harness.agent.loop import run_agent
from harness.agent.providers import ProviderError, get_provider
from harness.pricing import cost_usd, is_priced
from harness.sandbox.docker_executor import SandboxError

# Result usage: 150 uncached + 50 cache-read (0.1x) + 10 cache-write (1.25x)
# folds to round(167.5) = 168 effective prompt tokens.
FAKE_CLAUDE = """#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
mode = os.environ.get("FAKE_CLAUDE_MODE", "success")
api_key_present = "ANTHROPIC_API_KEY" in os.environ
usage = {"input_tokens": 150, "output_tokens": 30,
         "cache_read_input_tokens": 50, "cache_creation_input_tokens": 10}

if "stream-json" in args:
    if mode == "success":
        with open("hello.py", "w") as f:
            f.write('print("hello from vulcanbench")\\n')
    print(json.dumps({"type": "system", "subtype": "init", "session_id": "s1",
                      "model": "claude-opus-4-8", "api_key_present": api_key_present}))
    print(json.dumps({"type": "assistant", "message": {
        "id": "m1",
        "content": [{"type": "text", "text": "Writing the file"},
                    {"type": "tool_use", "id": "t1", "name": "Write",
                     "input": {"file_path": "hello.py"}}],
        "usage": {"input_tokens": 100, "output_tokens": 20,
                  "cache_read_input_tokens": 50, "cache_creation_input_tokens": 10}}}))
    print(json.dumps({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]}}))
    if mode == "limit":
        print(json.dumps({"type": "result", "subtype": "error_during_execution",
                          "is_error": True, "result": "Claude AI usage limit reached|123",
                          "session_id": "s1", "num_turns": 1, "usage": usage}))
    elif mode == "max_turns":
        print(json.dumps({"type": "result", "subtype": "error_max_turns",
                          "is_error": True, "result": "", "session_id": "s1",
                          "num_turns": 2, "total_cost_usd": 0.01, "usage": usage}))
    else:
        print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                          "result": "Done", "session_id": "s1", "num_turns": 2,
                          "total_cost_usd": 0.0123, "usage": usage}))
else:
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                      "result": json.dumps({"score": 80, "rationale": "fake judge"}),
                      "session_id": "s2", "num_turns": 1, "total_cost_usd": 0.001,
                      "usage": usage}))
"""


class _Collector:
    """Minimal TraceCollector stand-in for direct runner tests."""

    def __init__(self) -> None:
        self.events: list[tuple[str, dict[str, Any]]] = []

    def record(self, event_type: str, data: dict[str, Any]) -> None:
        self.events.append((event_type, data))


@pytest.fixture()
def fake_claude(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fakebin")
    script = bin_dir / "claude"
    script.write_text(FAKE_CLAUDE, encoding="utf-8")
    script.chmod(script.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    # Must be stripped from the CLI subprocess env (subscription auth only).
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-test-should-be-stripped")
    monkeypatch.delenv("FAKE_CLAUDE_MODE", raising=False)
    return script


def test_spec_detection() -> None:
    assert is_cli_agent_spec("claude-code:claude-opus-4-8")
    assert not is_cli_agent_spec("anthropic:claude-opus-4-8")
    assert not is_cli_agent_spec("mock:synthetic")


def test_pricing_alias_maps_to_api_rates() -> None:
    assert is_priced("claude-code:claude-opus-4-8")
    assert cost_usd("claude-code:claude-opus-4-8", 1000, 100) == cost_usd(
        "anthropic:claude-opus-4-8", 1000, 100
    )


def test_run_agent_via_claude_code(tmp_path: Path, fake_claude: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="claude-code:claude-opus-4-8",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
    )
    summary = res["summary"]

    # The CLI's edits go through the same diff/verify/score pipeline.
    assert summary["scores"]["functional"] == 1.0
    assert summary["finished"] is True

    # Usage from the CLI's final result, cache-folded (150 + 5 + 12.5 -> 168).
    assert summary["tokens"]["prompt"] == 168
    assert summary["tokens"]["completion"] == 30

    # cost_usd is the hypothetical API cost at anthropic rates.
    assert summary["cost_usd"] == pytest.approx((168 * 5.00 + 30 * 25.00) / 1_000_000)

    cli = summary["cli_agent"]
    assert cli["harness"] == "claude-code"
    assert cli["billing"] == "subscription"
    assert cli["cost_basis"] == "hypothetical-api-pricing"
    assert cli["cli_reported_cost_usd"] == 0.0123
    assert cli["session_id"] == "s1"
    assert cli["num_turns"] == 2

    # Raw stream persisted for audit — and the API key never reached the CLI.
    stream_path = tmp_path / res["run_id"] / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in stream_path.read_text().splitlines()]
    init = next(e for e in events if e.get("type") == "system")
    assert init["api_key_present"] is False


def test_claude_code_requires_local_sandbox(tmp_path: Path, fake_claude: Path) -> None:
    with pytest.raises(SandboxError, match="--sandbox local"):
        run_agent(
            task_id="hello-world",
            model="claude-code:claude-opus-4-8",
            output_dir=tmp_path,
            tasks_root=Path("tasks/v1"),
            judges=False,
            sandbox="docker",
        )


def test_usage_limit_raises_provider_error(
    tmp_path: Path, fake_claude: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FAKE_CLAUDE_MODE", "limit")
    with pytest.raises(ProviderError, match="subscription limit"):
        run_claude_code_task(
            workspace=tmp_path,
            prompt="p",
            model="claude-opus-4-8",
            priced_spec="claude-code:claude-opus-4-8",
            max_turns=5,
            collector=_Collector(),
        )


def test_max_turns_is_a_scored_outcome_not_an_error(
    tmp_path: Path, fake_claude: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FAKE_CLAUDE_MODE", "max_turns")
    out = run_claude_code_task(
        workspace=tmp_path,
        prompt="p",
        model="claude-opus-4-8",
        priced_spec="claude-code:claude-opus-4-8",
        max_turns=5,
        collector=_Collector(),
    )
    assert out.finished is False
    assert out.subtype == "error_max_turns"
    assert out.prompt_tokens == 168
    assert out.completion_tokens == 30


def test_cost_cap_kills_run_and_keeps_partial_usage(tmp_path: Path, fake_claude: Path) -> None:
    collector = _Collector()
    out = run_claude_code_task(
        workspace=tmp_path,
        prompt="p",
        model="claude-opus-4-8",
        priced_spec="claude-code:claude-opus-4-8",
        max_turns=5,
        collector=collector,
        max_run_cost=0.0005,  # below the first assistant message's cost
    )
    assert out.cost_capped is True
    assert out.finished is False
    # Partial usage from the streamed assistant message (100 + 5 + 12.5 -> 118).
    assert out.prompt_tokens == 118
    assert out.completion_tokens == 20
    assert any(etype == "cost_cap_exceeded" for etype, _ in collector.events)


def test_claude_code_judge_provider_single_shot(fake_claude: Path) -> None:
    provider = get_provider("claude-code:claude-opus-4-8")
    assert provider.name == "claude-code"
    resp = provider.complete(
        [
            {"role": "system", "content": "You are a strict judge."},
            {"role": "user", "content": "Score this patch."},
        ],
        [],
    )
    assert resp.content is not None
    assert json.loads(resp.content) == {"score": 80, "rationale": "fake judge"}
    assert resp.usage.prompt_tokens == 168
    assert resp.usage.completion_tokens == 30
