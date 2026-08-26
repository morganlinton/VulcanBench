"""Tests for the vendor agent-CLI runner (``claude-code:`` specs).

A fake ``claude`` binary on PATH emits canned ``stream-json`` output (and
writes the hello-world solution), so the full run_agent pipeline, workspace,
diff, verifier, scoring, hypothetical-API pricing, is exercised offline.
"""

from __future__ import annotations

import json
import os
import sqlite3
import stat
from pathlib import Path
from typing import Any

import pytest

from harness.agent import loop as loop_mod
from harness.agent.cli_agents import (
    _ZCODE_LIMIT_PATTERN,
    SubscriptionQuotaError,
    _subscription_env,
    _zcode_preflight,
    _zcode_session_limit_error,
    is_cli_agent_spec,
    run_claude_code_task,
    run_codex_task,
    run_cursor_task,
    run_pi_task,
    run_zcode_task,
)
from harness.agent.loop import _resolve_run_engine, run_agent
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

if "--version" in args:
    print("2.1.198 (Claude Code)")
elif args[:3] == ["auth", "status", "--json"]:
    print(json.dumps({"loggedIn": True, "authMethod": "claude.ai",
                      "subscriptionType": "max"}))
elif "stream-json" in args:
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

FAKE_CODEX = """#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
if "--version" in args:
    print("codex-cli 0.139.0")
elif args[:2] == ["login", "status"]:
    print("Logged in using ChatGPT")
elif args and args[0] == "exec":
    mode = "__CODEX_MODE__"
    prompt = sys.stdin.read()
    with open("hello.py", "w") as f:
        f.write('print("hello from vulcanbench")\\n')
    print(json.dumps({"type": "thread.started", "thread_id": "thread-1",
                      "api_key_present": "OPENAI_API_KEY" in os.environ or
                                         "CODEX_API_KEY" in os.environ}))
    print(json.dumps({"type": "item.completed", "item": {
        "id": "item-1", "type": "agent_message", "text": "Implemented and tested"}}))
    if mode == "limit":
        print(json.dumps({"type": "error",
                          "message": "You've hit your usage limit. Try again later."}))
        sys.exit(1)
    if mode == "fail":
        print(json.dumps({"type": "turn.failed", "error": {"message": "model exploded"}}))
        sys.exit(1)
    print(json.dumps({"type": "turn.completed", "usage": {
        "input_tokens": 120, "cached_input_tokens": 80,
        "output_tokens": 30, "reasoning_output_tokens": 10}}))
else:
    print("unsupported", file=sys.stderr)
    sys.exit(1)
"""


FAKE_CURSOR = """#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
mode = os.environ.get("FAKE_CURSOR_MODE", "success")
if "--version" in args or "-v" in args:
    print("2026.06.19-fake")
elif args[:1] == ["status"]:
    if mode == "logged_out":
        print("Not logged in")
    else:
        print("Logged in as morgan@example.com")
        print("Plan: Pro")
elif "-p" in args:
    model = args[args.index("--model") + 1]
    if mode == "limit":
        print(json.dumps({"type": "result", "subtype": "error",
                          "is_error": True, "result": "Usage limit reached for your plan",
                          "session_id": "cur-1"}))
        sys.exit(0)
    with open("hello.py", "w") as f:
        f.write('print("hello from vulcanbench")\\n')
    print(json.dumps({"type": "system", "subtype": "init", "session_id": "cur-1",
                      "model": model,
                      "leaked_key": os.environ.get("XAI_API_KEY", "")}))
    print(json.dumps({"type": "assistant", "session_id": "cur-1", "message": {
        "role": "assistant", "content": [{"type": "text", "text": "Implemented."}]}}))
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                      "duration_ms": 1200, "duration_api_ms": 800,
                      "result": "done", "session_id": "cur-1"}))
else:
    print("unsupported", file=sys.stderr)
    sys.exit(1)
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


@pytest.fixture()
def fake_codex(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fake-codex-bin")
    script = bin_dir / "codex"
    script.write_text(FAKE_CODEX.replace("__CODEX_MODE__", "success"), encoding="utf-8")
    script.chmod(script.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test-should-not-reach-codex")
    monkeypatch.setenv("CODEX_API_KEY", "sk-test-should-not-reach-codex")
    return script


@pytest.fixture
def fake_cursor(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fake-cursor-bin")
    script = bin_dir / "cursor-agent"
    script.write_text(FAKE_CURSOR, encoding="utf-8")
    script.chmod(script.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("XAI_API_KEY", "xai-secret-should-not-reach-cursor")
    monkeypatch.delenv("CURSOR_API_KEY", raising=False)
    monkeypatch.delenv("FAKE_CURSOR_MODE", raising=False)
    return script


def test_spec_detection() -> None:
    assert is_cli_agent_spec("claude-code:claude-opus-4-8")
    assert is_cli_agent_spec("codex:gpt-5.6-sol")
    assert is_cli_agent_spec("cursor:grok-4.6")
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
    assert cli["cost_basis"] == "api-equivalent"
    assert cli["cli_reported_cost_usd"] == 0.0123
    assert cli["session_id"] == "s1"
    assert cli["num_turns"] == 2
    economics = summary["economics"]
    assert economics["billing_mode"] == "subscription-included"
    assert economics["marginal_cash_usd"] is None
    assert economics["api_equivalent_cost_usd"] == summary["cost_usd"]
    assert economics["plan_name"] == "max"

    # Raw stream persisted for audit, and the API key never reached the CLI.
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
            env_overrides={"FAKE_CLAUDE_MODE": "limit"},
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
        env_overrides={"FAKE_CLAUDE_MODE": "max_turns"},
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


def test_run_agent_via_cursor_subscription(tmp_path: Path, fake_cursor: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="cursor:grok-4.6",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
        effort="high",
    )
    summary = res["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert summary["finished"] is True
    # Cursor's stream reports no usage: token counts are honestly zero and the
    # API-equivalent value is unavailable rather than a fabricated $0.
    assert summary["tokens"]["total"] == 0
    assert summary["cost_usd"] is None
    assert summary["economics"]["billing_mode"] == "subscription-included"
    assert summary["economics"]["measurement_quality"]["api_equivalent_cost_usd"] == "unavailable"
    cli = summary["cli_agent"]
    assert cli["harness"] == "cursor"
    assert cli["auth_method"] == "subscription"
    assert cli["plan_name"] == "Pro"
    assert cli["session_id"] == "cur-1"
    assert cli["requested_model"] == "grok-4.6"
    # The loop resolved effort=high and the bracket syntax carried it.
    assert cli["reported_model"] == "grok-4.6[effort=high]"
    stream_path = tmp_path / res["run_id"] / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in stream_path.read_text().splitlines()]
    assert events[0]["leaked_key"] == ""  # provider keys never reach the CLI


def test_cursor_writes_web_deny_permissions(tmp_path: Path, fake_cursor: Path) -> None:
    # Default (network=False): a workspace permissions file denies Cursor's web
    # tools. Denies survive --force. Without this, v3's post-cutoff
    # decontamination is defeated at runtime (Harness Study No. 01).
    run_cursor_task(
        workspace=tmp_path,
        prompt="fix",
        model="grok-4.6",
        priced_spec="cursor:grok-4.6",
        max_turns=10,
        collector=_Collector(),
    )
    cfg = json.loads((tmp_path / ".cursor" / "cli.json").read_text())
    assert "WebFetch(*)" in cfg["permissions"]["deny"]
    assert any(d.startswith("WebSearch") for d in cfg["permissions"]["deny"])


def test_cursor_network_flag_skips_web_deny(tmp_path: Path, fake_cursor: Path) -> None:
    outcome = run_cursor_task(
        workspace=tmp_path,
        prompt="fix",
        model="grok-4.6",
        priced_spec="cursor:grok-4.6",
        max_turns=10,
        collector=_Collector(),
        network=True,
    )
    assert not (tmp_path / ".cursor" / "cli.json").exists()
    assert "web-allowed" in (outcome.execution_boundary or "")


def test_cli_harness_workspace_is_outside_the_repo(tmp_path: Path, fake_cursor: Path) -> None:
    # Containment, not permissions: CLI harnesses run on the host, so the
    # workspace must sit where no gold_patch.diff exists anywhere above it.
    # Observed before this: 46 runs read their own task's answer key.
    seen: dict[str, str] = {}
    real_prepare = loop_mod.prepare_workspace

    def spy(task, workspace):  # type: ignore[no-untyped-def]
        seen["workspace"] = str(workspace)
        return real_prepare(task, workspace)

    loop_mod.prepare_workspace = spy  # type: ignore[assignment]
    try:
        res = run_agent(
            task_id="hello-world",
            model="cursor:grok-4.6",
            output_dir=tmp_path,
            tasks_root=Path("tasks/v1"),
            judges=False,
            sandbox="local",
        )
    finally:
        loop_mod.prepare_workspace = real_prepare  # type: ignore[assignment]

    repo_root = Path(__file__).resolve().parents[1]
    assert not seen["workspace"].startswith(str(repo_root)), seen["workspace"]
    # ...and the tree is reclaimed into the run dir afterwards.
    assert (tmp_path / res["run_id"] / "workspace").exists()


def test_cursor_run_records_web_audit(tmp_path: Path, fake_cursor: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="cursor:grok-4.6",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
    )
    summary = res["summary"]
    audit = summary["integrity_audit"]
    assert audit["web"]["verdict"] == "no_web"
    assert audit["filesystem"]["verdict"] in ("clean", "out_of_workspace")
    assert audit["contaminated"] is False
    # The deny file must not leak into the captured patch.
    patch = (tmp_path / res["run_id"] / "final.patch").read_text()
    assert ".cursor" not in patch


def test_cursor_allows_docker_verifier(fake_cursor: Path) -> None:
    # Cursor brings its own agent sandbox (like Codex), so --sandbox docker is
    # legal: the agent works the host workspace while setup/verification run in
    # Docker. Forcing local also forced the verifier onto the host, where
    # toolchains don't match the sandbox image.
    adapter, provider, _ = _resolve_run_engine(
        model="cursor:grok-4.6", provider=None, effort=None, sandbox="docker"
    )
    assert adapter is not None and adapter.harness_id == "cursor"
    assert provider is None


def test_cursor_logged_out_fails_closed(tmp_path: Path, fake_cursor: Path) -> None:
    # The preflight subprocess gets the minimal allowlisted env, so the mode
    # must be baked into the fake script rather than passed via os.environ.
    fake_cursor.write_text(
        FAKE_CURSOR.replace('os.environ.get("FAKE_CURSOR_MODE", "success")', '"logged_out"'),
        encoding="utf-8",
    )
    with pytest.raises(ProviderError, match=r"signed out|subscription"):
        run_cursor_task(
            workspace=tmp_path,
            prompt="fix",
            model="grok-4.6",
            priced_spec="cursor:grok-4.6",
            max_turns=10,
            collector=_Collector(),
        )


def test_cursor_api_key_auth_fails_closed(
    tmp_path: Path, fake_cursor: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("CURSOR_API_KEY", "key-1")
    with pytest.raises(ProviderError, match="subscription"):
        run_cursor_task(
            workspace=tmp_path,
            prompt="fix",
            model="grok-4.6",
            priced_spec="cursor:grok-4.6",
            max_turns=10,
            collector=_Collector(),
        )


def test_cursor_usage_limit_raises_quota_error(
    tmp_path: Path, fake_cursor: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FAKE_CURSOR_MODE", "limit")
    with pytest.raises(SubscriptionQuotaError, match="topping up credits"):
        run_cursor_task(
            workspace=tmp_path,
            prompt="fix",
            model="grok-4.6",
            priced_spec="cursor:grok-4.6",
            max_turns=10,
            collector=_Collector(),
            env_overrides={"FAKE_CURSOR_MODE": "limit"},
        )


def test_cursor_rejects_unenforceable_live_cost_cap(tmp_path: Path, fake_cursor: Path) -> None:
    with pytest.raises(ProviderError, match="max-run-cost"):
        run_cursor_task(
            workspace=tmp_path,
            prompt="fix",
            model="grok-4.6",
            priced_spec="cursor:grok-4.6",
            max_turns=10,
            collector=_Collector(),
            max_run_cost=1.0,
        )


def test_run_agent_via_codex_subscription(tmp_path: Path, fake_codex: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="codex:gpt-5.6-sol",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
        effort="high",
    )
    summary = res["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert summary["finished"] is True
    assert summary["tokens"] == {
        "prompt": 120,
        "completion": 30,
        "total": 150,
        "cached_input": 80,
        "reasoning_output": 10,
    }
    cli = summary["cli_agent"]
    assert cli["harness"] == "codex"
    assert cli["auth_method"] == "subscription"
    assert cli["session_id"] == "thread-1"
    assert cli["requested_model"] == "gpt-5.6-sol"
    assert cli["reported_model"] is None
    assert cli["execution_boundary"] == "host-workspace; sandbox=workspace-write"
    assert summary["economics"]["billing_mode"] == "subscription-included"
    stream_path = tmp_path / res["run_id"] / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in stream_path.read_text().splitlines()]
    assert events[0]["api_key_present"] is False


def test_codex_rejects_unenforceable_live_cost_cap(tmp_path: Path, fake_codex: Path) -> None:
    with pytest.raises(ProviderError, match="cannot be enforced live"):
        run_codex_task(
            workspace=tmp_path,
            prompt="p",
            model="gpt-5.6-sol",
            priced_spec="codex:gpt-5.6-sol",
            max_turns=5,
            collector=_Collector(),
            max_run_cost=1.0,
        )


def test_codex_resolves_relative_workspace_before_passing_cd(
    tmp_path: Path,
    fake_codex: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.chdir(tmp_path)
    Path("workspace").mkdir()
    collector = _Collector()
    run_codex_task(
        workspace=Path("workspace"),
        prompt="p",
        model="gpt-5.6-sol",
        priced_spec="codex:gpt-5.6-sol",
        max_turns=5,
        collector=collector,
    )
    start = next(data for event, data in collector.events if event == "cli_agent_start")
    cd_value = start["argv"][start["argv"].index("--cd") + 1]
    assert Path(cd_value).is_absolute()


# ---------------------------------------------------------------------------
# ZCode (Z.ai's GLM harness): fake `zcode` launcher that writes the sqlite
# session store the adapter harvests tool calls and usage from.
# ---------------------------------------------------------------------------

FAKE_ZCODE = """#!/usr/bin/env python3
import json, os, sqlite3, sys, time

args = sys.argv[1:]
mode = "__MODE__"
if "--version" in args or "-v" in args:
    print("zcode-app-cli 3.8.1-15")
    print("zcode-runtime 0.16.3")
    sys.exit(0)
if "--prompt" not in args:
    print("unsupported", file=sys.stderr)
    sys.exit(1)

cwd = os.path.realpath(args[args.index("--cwd") + 1])
if mode == "limit":
    print("Error: Turn execution failed (traceId: t-1)", file=sys.stderr)
    print("Cause: AiSdkModelAdapterError: 429 Too Many Requests: quota exceeded", file=sys.stderr)
    sys.exit(1)
if mode == "crash":
    print("Error: Turn execution failed (traceId: t-2)", file=sys.stderr)
    print("Cause: AiSdkModelAdapterError: Model provider is missing an API key: zai", file=sys.stderr)
    sys.exit(1)

with open(os.path.join(cwd, "hello.py"), "w") as f:
    f.write('print("hello from vulcanbench")\\n')

root = os.environ["ZCODE_STORAGE_DIR"]
db_dir = os.path.join(root, "cli", "db")
os.makedirs(db_dir, exist_ok=True)
con = sqlite3.connect(os.path.join(db_dir, "db.sqlite"))
con.executescript('''
create table if not exists session (id text primary key, project_id text, parent_id text,
  slug text, directory text, title text, version text, time_created integer, time_updated integer);
create table if not exists message (id text primary key, session_id text, sequence integer,
  time_created integer, time_updated integer, data text);
create table if not exists part (id text primary key, message_id text, session_id text,
  sequence integer, time_created integer, time_updated integer, data text);
create table if not exists model_usage (id text primary key, session_id text, query_source text,
  provider_id text, model_id text, variant text, status text, started_at integer,
  input_tokens integer, output_tokens integer, reasoning_tokens integer,
  cache_creation_input_tokens integer, cache_read_input_tokens integer,
  provider_total_tokens integer, finish_reason text, raw_usage_json text);
create table if not exists turn_usage (session_id text, turn_id text, status text,
  input_tokens integer, output_tokens integer);
create table if not exists tool_usage (id text primary key, session_id text, tool_name text,
  tool_call_id text, status text);
''')
now = int(time.time() * 1000)
sid = "sess_fake-1"
con.execute("insert into session values (?,?,?,?,?,?,?,?,?)",
            (sid, "proj", None, sid, cwd, "fix", "0.16.3", now, now))
con.execute("insert into message values (?,?,?,?,?,?)",
            ("msg_u1", sid, 0, now, now, json.dumps({"role": "user", "id": "msg_u1"})))
con.execute("insert into message values (?,?,?,?,?,?)",
            ("msg_a1", sid, 1, now, now, json.dumps({"role": "assistant", "id": "msg_a1",
             "modelID": "glm-5.3", "providerID": "zai"})))
con.execute("insert into part values (?,?,?,?,?,?,?)",
            ("prt_1", "msg_a1", sid, 0, now, now, json.dumps({"type": "tool", "callID": "call_1",
             "tool": "Write", "state": {"status": "completed",
             "input": {"filePath": os.path.join(cwd, "hello.py")}}})))
con.execute("insert into part values (?,?,?,?,?,?,?)",
            ("prt_2", "msg_a1", sid, 1, now, now, json.dumps({"type": "text",
             "text": "Implemented and tested."})))
con.execute("insert into model_usage values (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("mu_1", sid, "main_turn", "zai", "glm-5.3", "__VARIANT__", "completed", now,
             1000, 200, 0, 0, 500, None, "stop", "{}"))
con.execute("insert into model_usage values (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            ("mu_2", sid, "main_turn", "zai", "glm-5.3", "__VARIANT__", "completed", now + 1,
             100, 50, 0, 0, 0, None, "stop", "{}"))
con.execute("insert into turn_usage values (?,?,?,?,?)", (sid, "turn_1", "completed", 1100, 250))
con.execute("insert into tool_usage values (?,?,?,?,?)", ("tu_1", sid, "Write", "call_1", "completed"))
con.commit()
con.close()
print("Done. Wrote hello.py and ran the tests.")
"""


def _write_fake_zcode(bin_dir: Path, *, mode: str = "success", variant: str = "max") -> Path:
    script = bin_dir / "zcode"
    script.write_text(
        FAKE_ZCODE.replace("__MODE__", mode).replace("__VARIANT__", variant), encoding="utf-8"
    )
    script.chmod(script.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return script


@pytest.fixture
def fake_zcode(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fake-zcode-bin")
    script = _write_fake_zcode(bin_dir)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    # ZCode's own state roots: the fake writes its session DB under the storage
    # dir and the preflight reads (presence only) the OAuth credential store.
    state = tmp_path_factory.mktemp("zcode-state")
    monkeypatch.setenv("ZCODE_STORAGE_DIR", str(state / "storage"))
    monkeypatch.setenv("ZCODE_DATA_BASE_DIR", str(state / "data"))
    creds = state / "data" / ".zcode" / "v2" / "credentials.json"
    creds.parent.mkdir(parents=True)
    creds.write_text(
        json.dumps(
            {
                "oauth:active_provider": "zai",
                "oauth:zai:access_token": "tok-should-never-be-copied",
                "oauth:zai:user_info": {"plan": "Pro"},
            }
        ),
        encoding="utf-8",
    )
    monkeypatch.setenv("ZAI_API_KEY", "zai-secret-should-not-reach-zcode")
    # ZCode user config: the provider block (with baseURL + Coding Plan key) the
    # adapter must copy into the per-run project config so `model.main` resolves.
    user_cfg = state / "storage" / "cli" / "config.json"
    user_cfg.parent.mkdir(parents=True)
    user_cfg.write_text(
        json.dumps(
            {
                "provider": {
                    "zai": {
                        "kind": "anthropic",
                        "options": {
                            "apiKeyRequired": True,
                            "baseURL": "https://api.z.ai/api/anthropic",
                            "apiKey": "codingplan-key-copied-not-logged",
                        },
                        "models": {"glm-5.3": {"name": "GLM-5.3"}, "glm-5.2": {"name": "GLM-5.2"}},
                    }
                },
                "model": {"main": "zai/glm-5.2", "lite": "zai/glm-5-turbo"},
            }
        ),
        encoding="utf-8",
    )
    return script


def test_zcode_spec_detection_and_pricing_alias() -> None:
    assert is_cli_agent_spec("zcode:glm-5.3")
    assert is_priced("zcode:glm-5.3")
    assert cost_usd("zcode:glm-5.3", 1_000_000, 1_000_000) == cost_usd(
        "zai:glm-5.3", 1_000_000, 1_000_000
    )


def test_run_agent_via_zcode_subscription(tmp_path: Path, fake_zcode: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="zcode:glm-5.3",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
        effort="extra-high",
    )
    summary = res["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert summary["finished"] is True
    # Usage harvested from the session store: two completed requests,
    # 1000+500 (cache read) + 100 prompt, 250 completion.
    assert summary["tokens"]["prompt"] == 1600
    assert summary["tokens"]["completion"] == 250
    assert summary["tokens"]["cached_input"] == 500
    assert summary["cost_usd"] is not None and summary["cost_usd"] > 0
    assert summary["economics"]["billing_mode"] == "subscription-included"
    assert (
        summary["economics"]["measurement_quality"]["api_equivalent_cost_usd"]
        == "estimated-from-reported-tokens-with-cache-pricing"
    )
    cli = summary["cli_agent"]
    assert cli["harness"] == "zcode"
    assert cli["harness_version"] == "zcode-app-cli 3.8.1-15; zcode-runtime 0.16.3"
    assert cli["auth_method"] == "subscription"
    assert cli["plan_name"] == "Pro"
    assert cli["session_id"] == "sess_fake-1"
    assert cli["requested_model"] == "glm-5.3"
    assert cli["reported_model"] == "zai/glm-5.3"
    assert cli["model_identity_confidence"] == "cli-reported"
    # extra-high -> ZCode "max", read back from the usage ledger's variant.
    assert summary["effort"]["provider_value"] == "max"
    assert cli["reported_effort"] == "max"
    assert cli["num_turns"] == 2
    run_dir = tmp_path / res["run_id"]
    # Harvested session artifacts land beside the trace.
    assert (run_dir / "zcode-session" / "messages.jsonl").exists()
    assert (run_dir / "zcode-session" / "model_usage.jsonl").exists()
    stream = (run_dir / "cli-agent-stream.jsonl").read_text()
    assert "tok-should-never-be-copied" not in stream
    assert '"toolCallId": "call_1"' in stream
    # The per-run project config must not leak into the captured patch.
    assert ".zcode" not in (run_dir / "final.patch").read_text()
    audit = summary["integrity_audit"]
    assert audit["web"]["verdict"] == "no_web"
    assert audit["contaminated"] is False


def test_zcode_project_config_pins_model_effort_memory_and_web(
    tmp_path: Path, fake_zcode: Path
) -> None:
    collector = _Collector()
    outcome = run_zcode_task(
        workspace=tmp_path,
        prompt="fix",
        model="glm-5.3",
        priced_spec="zcode:glm-5.3",
        max_turns=10,
        collector=collector,
        effort="low",
    )
    cfg = json.loads((tmp_path / ".zcode" / "config.json").read_text())
    assert cfg["model"]["main"] == "zai/glm-5.3"
    assert cfg["permission"]["mode"] == "yolo"
    assert cfg["features"]["memory"] is False
    assert cfg["memory"] == {"use": False, "write": False, "autoConsolidate": False}
    assert cfg["modelCatalog"]["overrides"]["zai/glm-5.3"]["reasoning"]["defaultLevel"] == "low"
    assert set(cfg["permission"]["disallowedTools"]) == {"WebFetch", "WebSearch", "web_search"}
    assert cfg["plugins"]["enabledPlugins"] == {"browser-use@zcode-plugins-official": False}
    assert "web-tools-removed" in (outcome.execution_boundary or "")
    assert outcome.finished is True
    # The provider block is copied from the user config so `model.main`
    # resolves (baseURL) with the same credential ZCode is authed against.
    assert cfg["provider"]["zai"]["options"]["baseURL"] == "https://api.z.ai/api/anthropic"
    assert cfg["provider"]["zai"]["options"]["apiKey"] == "codingplan-key-copied-not-logged"
    # ...but that credential must never reach the trace.
    start = next(d for e, d in collector.events if e == "cli_agent_start")
    assert start["project_config"]["provider"]["zai"]["options"]["apiKey"] == "***"
    assert "codingplan-key-copied-not-logged" not in json.dumps(collector.events)


def test_zcode_network_flag_keeps_web(tmp_path: Path, fake_zcode: Path) -> None:
    outcome = run_zcode_task(
        workspace=tmp_path,
        prompt="fix",
        model="glm-5.3",
        priced_spec="zcode:glm-5.3",
        max_turns=10,
        collector=_Collector(),
        network=True,
    )
    cfg = json.loads((tmp_path / ".zcode" / "config.json").read_text())
    assert "disallowedTools" not in cfg["permission"]
    assert "plugins" not in cfg
    assert "modelCatalog" not in cfg  # no effort requested -> ZCode default level
    assert "web-allowed" in (outcome.execution_boundary or "")


def test_zcode_rejects_unenforceable_live_cost_cap(tmp_path: Path, fake_zcode: Path) -> None:
    with pytest.raises(ProviderError, match="max-run-cost"):
        run_zcode_task(
            workspace=tmp_path,
            prompt="fix",
            model="glm-5.3",
            priced_spec="zcode:glm-5.3",
            max_turns=10,
            collector=_Collector(),
            max_run_cost=1.0,
        )


def test_zcode_allows_docker_verifier() -> None:
    adapter, provider, effort = _resolve_run_engine(
        model="zcode:glm-5.3", provider=None, effort="high", sandbox="docker"
    )
    assert adapter is not None and adapter.harness_id == "zcode"
    assert provider is None
    assert effort is not None and effort.provider_value == "high" and effort.supported
    # medium has no ZCode level: recorded-but-not-sent.
    _, _, medium = _resolve_run_engine(
        model="zcode:glm-5.3", provider=None, effort="medium", sandbox="docker"
    )
    assert medium is not None and medium.supported is False


def test_zcode_signed_out_fails_closed(
    tmp_path: Path, fake_zcode: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Truly signed out: no OAuth credentials AND no configured Coding Plan key.
    creds = Path(os.environ["ZCODE_DATA_BASE_DIR"]) / ".zcode" / "v2" / "credentials.json"
    creds.unlink()
    (Path(os.environ["ZCODE_STORAGE_DIR"]) / "cli" / "config.json").unlink()
    with pytest.raises(ProviderError, match="signed out"):
        run_zcode_task(
            workspace=tmp_path,
            prompt="fix",
            model="glm-5.3",
            priced_spec="zcode:glm-5.3",
            max_turns=10,
            collector=_Collector(),
        )


def test_zcode_metered_api_key_fails_closed(
    tmp_path: Path, fake_zcode: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # No OAuth login, and the user config carries a key against the general
    # pay-as-you-go endpoint: that bills metered usage, not the Coding Plan.
    creds = Path(os.environ["ZCODE_DATA_BASE_DIR"]) / ".zcode" / "v2" / "credentials.json"
    creds.unlink()
    cfg_path = Path(os.environ["ZCODE_STORAGE_DIR"]) / "cli" / "config.json"
    cfg_path.parent.mkdir(parents=True, exist_ok=True)
    cfg_path.write_text(
        json.dumps(
            {
                "provider": {
                    "zai": {
                        "kind": "openai-compatible",
                        "options": {
                            "apiKey": "sk-metered",
                            "baseURL": "https://api.z.ai/api/paas/v4",
                        },
                    }
                },
                "model": {"main": "zai/glm-5.3"},
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ProviderError, match="metered billing"):
        run_zcode_task(
            workspace=tmp_path,
            prompt="fix",
            model="glm-5.3",
            priced_spec="zcode:glm-5.3",
            max_turns=10,
            collector=_Collector(),
        )
    # ...whereas the same key against the Coding Plan endpoint is a plan run.
    cfg = json.loads(cfg_path.read_text())
    cfg["provider"]["zai"]["options"]["baseURL"] = "https://api.z.ai/api/anthropic"
    cfg_path.write_text(json.dumps(cfg), encoding="utf-8")
    receipt = _zcode_preflight().as_summary()
    assert receipt["ready"] is True
    assert receipt["auth_mode"] == "subscription"
    assert "sk-metered" not in json.dumps(receipt)


def test_zcode_quota_exhaustion_raises_quota_error(tmp_path: Path, fake_zcode: Path) -> None:
    _write_fake_zcode(fake_zcode.parent, mode="limit")
    with pytest.raises(SubscriptionQuotaError, match="Coding Plan limit"):
        run_zcode_task(
            workspace=tmp_path,
            prompt="fix",
            model="glm-5.3",
            priced_spec="zcode:glm-5.3",
            max_turns=10,
            collector=_Collector(),
        )


def test_zcode_runtime_error_is_provider_error(tmp_path: Path, fake_zcode: Path) -> None:
    _write_fake_zcode(fake_zcode.parent, mode="crash")
    with pytest.raises(ProviderError, match="missing an API key"):
        run_zcode_task(
            workspace=tmp_path,
            prompt="fix",
            model="glm-5.3",
            priced_spec="zcode:glm-5.3",
            max_turns=10,
            collector=_Collector(),
        )


def test_zcode_session_limit_error_detects_buried_1308(tmp_path: Path) -> None:
    # The 5-hour window limit (code 1308) lands in model_usage.error_message,
    # not on stderr; the helper must surface it so the suite pauses cleanly.
    db = tmp_path / "db.sqlite"
    con = sqlite3.connect(db)
    con.execute(
        "create table model_usage(id text, session_id text, started_at integer, "
        "error_message text, status text)"
    )
    con.execute(
        "insert into model_usage values (?,?,?,?,?)",
        (
            "m1",
            "sess-x",
            1,
            "[1308][Usage limit reached for 5 hour. Your limit will reset at 2026-08-22 16:58:09]",
            "error",
        ),
    )
    con.commit()
    con.close()
    assert _ZCODE_LIMIT_PATTERN.search("[1308] whatever")
    assert _ZCODE_LIMIT_PATTERN.search("[1302][Rate limit reached]")
    assert "1308" in (_zcode_session_limit_error(db, "sess-x") or "")
    # A different session, or a missing db, is not a limit.
    assert _zcode_session_limit_error(db, "sess-other") is None
    assert _zcode_session_limit_error(tmp_path / "missing.sqlite", "sess-x") is None


def _rewrite_fake_codex(script: Path, mode: str) -> None:
    script.write_text(FAKE_CODEX.replace("__CODEX_MODE__", mode), encoding="utf-8")


def test_codex_usage_limit_raises_quota_error(tmp_path: Path, fake_codex: Path) -> None:
    _rewrite_fake_codex(fake_codex, "limit")
    with pytest.raises(SubscriptionQuotaError, match="limit"):
        run_codex_task(
            workspace=tmp_path,
            prompt="p",
            model="gpt-5.6-sol",
            priced_spec="codex:gpt-5.6-sol",
            max_turns=10,
            collector=_Collector(),
        )


def test_codex_turn_failure_raises_provider_error(tmp_path: Path, fake_codex: Path) -> None:
    _rewrite_fake_codex(fake_codex, "fail")
    with pytest.raises(ProviderError, match="codex exec failed"):
        run_codex_task(
            workspace=tmp_path,
            prompt="p",
            model="gpt-5.6-sol",
            priced_spec="codex:gpt-5.6-sol",
            max_turns=10,
            collector=_Collector(),
        )


def test_codex_judge_provider_single_shot(
    tmp_path: Path, fake_codex: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The fake writes hello.py into its cwd; keep that inside the tmp dir.
    monkeypatch.chdir(tmp_path)
    provider = get_provider("codex:gpt-5.6-sol")
    response = provider.complete([{"role": "user", "content": "rate this"}], tools=[])
    assert response.content == "Implemented and tested"
    # 120 input with 80 cached folds to (120-80) + 80*0.1 = 48 effective.
    assert response.usage.prompt_tokens == 48
    assert response.usage.completion_tokens == 30


def test_subscription_env_blocks_system_pip_installs() -> None:
    """Host-run agents must not be able to pip-install into the system Python.

    A live ZCode run on oss-aiohttp-upgrade-deferred installed an editable
    aiohttp into Homebrew's python3.14 and broke the host pytest.
    """
    env = _subscription_env()
    assert env["PIP_REQUIRE_VIRTUALENV"] == "1"
    assert env["PYTHONNOUSERSITE"] == "1"
    # Test adapters may still override explicitly.
    assert _subscription_env({"PIP_REQUIRE_VIRTUALENV": "0"})["PIP_REQUIRE_VIRTUALENV"] == "0"


FAKE_PI = """#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
mode = os.environ.get("FAKE_PI_MODE", "success")
if "--version" in args:
    print("pi 0.52.0")
    sys.exit(0)
if mode == "crash":
    print("boom", file=sys.stderr)
    sys.exit(2)
if "--thinking" in args:
    thinking = args[args.index("--thinking") + 1]
else:
    thinking = None
model = args[args.index("--model") + 1] if "--model" in args else None
if mode == "success":
    with open("hello.py", "w") as f:
        f.write('print("hello from vulcanbench")\\n')
print(json.dumps({"type": "session", "version": 3, "id": "pi-s1"}))
print(json.dumps({"type": "agent_start"}))
print(json.dumps({"type": "turn_start"}))
print(json.dumps({
    "type": "message_end",
    "message": {
        "role": "assistant",
        "usage": {"input": 200, "output": 40, "cacheRead": 50, "cost": {"total": 0.0042}},
    },
}))
print(json.dumps({"type": "turn_end"}))
print(json.dumps({"type": "agent_end", "messages": []}))
open(os.path.join(os.environ.get("HOME", "."), "pi-argv.json"), "w").write(
    json.dumps({"thinking": thinking, "model": model, "api_key_present": "META_MUSE_SPARK_API" in os.environ})
)
"""


def _write_fake_pi(bin_dir: Path) -> Path:
    script = bin_dir / "pi"
    script.write_text(FAKE_PI)
    script.chmod(script.stat().st_mode | stat.S_IEXEC)
    return script


@pytest.fixture
def fake_pi(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fake-pi-bin")
    script = _write_fake_pi(bin_dir)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")
    monkeypatch.setenv("META_MUSE_SPARK_API", "meta-test-key")
    return script


def test_pi_spec_detection_and_pricing_alias() -> None:
    assert is_cli_agent_spec("pi:meta:muse-spark-1.2")
    assert is_priced("pi:meta:muse-spark-1.2")
    assert cost_usd("pi:meta:muse-spark-1.2", 1_000_000, 1_000_000) == cost_usd(
        "meta:muse-spark-1.2", 1_000_000, 1_000_000
    )


def test_run_agent_via_pi_api_harness(tmp_path: Path, fake_pi: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="pi:meta:muse-spark-1.2",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
        effort="low",
    )
    summary = res["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert summary["finished"] is True
    cli = summary["cli_agent"]
    assert cli["harness"] == "pi"
    assert cli["billing"] == "api"
    assert cli["requested_model"] == "meta:muse-spark-1.2"
    assert cli["reported_model"] == "vulcan-meta/muse-spark-1.2"
    assert cli["reported_effort"] == "low"
    assert summary["tokens"]["prompt"] == 200
    assert summary["tokens"]["completion"] == 40
    assert summary["tokens"]["cached_input"] == 50
    economics = summary["economics"]
    assert economics["billing_mode"] == "api-metered"
    assert economics["marginal_cash_usd"] == summary["cost_usd"]
    stream_path = tmp_path / res["run_id"] / "cli-agent-stream.jsonl"
    events = [json.loads(line) for line in stream_path.read_text().splitlines()]
    assert any(e.get("type") == "session" for e in events)
    start = json.loads(
        next(
            line
            for line in (tmp_path / res["run_id"] / "trace.jsonl").read_text().splitlines()
            if '"cli_agent_start"' in line
        )
    )
    argv = start["data"]["argv"]
    assert "--thinking" in argv and argv[argv.index("--thinking") + 1] == "low"
    assert "-p" in argv
    assert "--no-session" in argv
    assert "--model" in argv and "vulcan-meta/" in argv[argv.index("--model") + 1]


def test_pi_writes_meta_models_json_without_secret(tmp_path: Path, fake_pi: Path) -> None:
    (tmp_path / "ws").mkdir()
    outcome = run_pi_task(
        workspace=tmp_path / "ws",
        prompt="fix",
        model="meta:muse-spark-1.2",
        priced_spec="pi:meta:muse-spark-1.2",
        max_turns=10,
        collector=_Collector(),
        effort="high",
    )
    assert outcome.finished is True
    models = json.loads((tmp_path / "pi-home" / ".pi" / "agent" / "models.json").read_text())
    assert models["providers"]["vulcan-meta"]["api"] == "openai-responses"
    assert models["providers"]["vulcan-meta"]["apiKey"] == "$META_MUSE_SPARK_API"
    secret = "meta-test-key"
    assert secret not in (tmp_path / "pi-home" / ".pi" / "agent" / "models.json").read_text()


def test_pi_rejects_unenforceable_live_cost_cap(tmp_path: Path, fake_pi: Path) -> None:
    (tmp_path / "ws").mkdir()
    with pytest.raises(ProviderError, match="max-run-cost"):
        run_pi_task(
            workspace=tmp_path / "ws",
            prompt="fix",
            model="meta:muse-spark-1.2",
            priced_spec="pi:meta:muse-spark-1.2",
            max_turns=10,
            collector=_Collector(),
            max_run_cost=1.0,
        )


def test_pi_allows_docker_verifier() -> None:
    adapter, provider, effort = _resolve_run_engine(
        model="pi:meta:muse-spark-1.2", provider=None, effort="extra-high", sandbox="docker"
    )
    assert adapter is not None and adapter.harness_id == "pi"
    assert provider is None
    assert effort is not None and effort.provider_value == "xhigh" and effort.supported


def test_pi_missing_api_key_fails_closed(
    tmp_path: Path, fake_pi: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("META_MUSE_SPARK_API", raising=False)
    monkeypatch.delenv("MODEL_API_KEY", raising=False)
    monkeypatch.delenv("OPENROUTER_API_KEY", raising=False)
    (tmp_path / "ws").mkdir()
    with pytest.raises(ProviderError, match=r"API-metered|is not set|preflight"):
        run_pi_task(
            workspace=tmp_path / "ws",
            prompt="fix",
            model="meta:muse-spark-1.2",
            priced_spec="pi:meta:muse-spark-1.2",
            max_turns=10,
            collector=_Collector(),
        )
