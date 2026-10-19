"""DeepSeek Harness adapter: pinning, settings overlay, usage receipts and peak pricing."""

import io
import json
from datetime import UTC, datetime
from pathlib import Path

import pytest

from harness.agent import deepseek_harness
from harness.agent.cli_agents import get_cli_agent_adapter, is_cli_agent_spec
from harness.agent.deepseek_harness import (
    DeepSeekHarnessAdapter,
    benchmark_patch,
    collect_usage,
    tree_sha256,
)
from harness.agent.providers import ProviderError
from harness.effort import effort_config
from harness.pricing import cost_usd, cost_usd_at, peak_multiplier

MODEL = "deepseek-flash"
SPEC = f"dsh:{MODEL}"
# Sunday 2026-10-18 12:00 UTC: off-peak. Monday 2026-10-19 02:00 UTC: peak.
OFF_PEAK_MS = int(datetime(2026, 10, 18, 12, tzinfo=UTC).timestamp() * 1000)
PEAK_MS = int(datetime(2026, 10, 19, 2, tzinfo=UTC).timestamp() * 1000)


class _Collector:
    def __init__(self):
        self.events = []

    def record(self, name, payload):
        self.events.append((name, payload))


class _FakeProc:
    def __init__(self, stdout_lines, returncode):
        self.pid, self.returncode, self._done = 4242, returncode, False
        self.stdin = io.StringIO()
        self.stdin.close = lambda: None
        self.stdout = iter(stdout_lines)
        self.stderr = iter([])

    def wait(self):
        self._done = True
        return self.returncode

    def poll(self):
        return self.returncode if self._done else None


def _message(seq, time_ms=OFF_PEAK_MS, model=MODEL, usage=None, **data):
    usage = usage if usage is not None else {"inputTokens": 1000, "outputTokens": 100}
    payload = {
        "turn": 1,
        "step": seq,
        "message": {"source": {"provider": "deepseek-official", "model": model}},
        "usage": usage,
        **data,
    }
    return {"type": "assistant/message", "seq": seq, "time": time_ms, "data": payload}


def _attempt(seq, usage, time_ms=OFF_PEAK_MS):
    stream = [{"type": "chunk", "chunk": {"type": "text-delta"}}]
    if usage is not None:
        stream.append({"type": "chunk", "chunk": {"type": "usage", "usage": usage}})
    return {"type": "assistant/attempt", "seq": seq, "time": time_ms, "data": {"stream": stream}}


def _write_log(path: Path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    header = {"version": 1, "id": path.parent.name, "createdAt": OFF_PEAK_MS}
    path.write_text("".join(json.dumps(r) + "\n" for r in [header, *rows]))
    return path


def _pin(tmp_path, monkeypatch):
    node = tmp_path / "node"
    node.write_text("#!/bin/sh\n")
    node.chmod(0o700)
    prefix = tmp_path / "dsh-prefix"
    package = prefix / "node_modules/@deepseek-ai/dsh"
    (package / "lib").mkdir(parents=True, exist_ok=True)
    (package / "package.json").write_text(json.dumps({"version": deepseek_harness.VERSION}))
    (package / "lib/bin.js").write_text("// fake dsh, never executed")
    monkeypatch.setenv("VULCANBENCH_DSH_NODE", str(node))
    monkeypatch.setenv("VULCANBENCH_DSH_PREFIX", str(prefix))
    monkeypatch.setenv("VULCANBENCH_DSH_SHA256", tree_sha256(prefix / "node_modules"))
    monkeypatch.setenv("DEEPSEEK_API_KEY", "sk-test")
    workspace = tmp_path / "workspace"
    workspace.mkdir(exist_ok=True)
    return workspace, prefix


def _run(tmp_path, monkeypatch, stdout, rows, returncode=0, **kwargs):
    workspace, _ = _pin(tmp_path, monkeypatch)
    launched = {}

    def fake_popen(cmd, **popen_kwargs):
        env = popen_kwargs["env"]
        launched.update(cmd=cmd, env=env, cwd=popen_kwargs["cwd"])
        _write_log(Path(env["DSH_HOME"]) / "sessions/session-abc/events.jsonl", rows)
        proc = _FakeProc(stdout, returncode)
        launched["proc"] = proc
        return proc

    monkeypatch.setattr(deepseek_harness.subprocess, "Popen", fake_popen)
    monkeypatch.setattr(deepseek_harness.os, "killpg", lambda pid, sig: None)
    collector = _Collector()
    stream = tmp_path / "run/stream.jsonl"
    stream.parent.mkdir(exist_ok=True)
    options = dict(
        workspace=workspace,
        prompt="Fix the engine.",
        model=MODEL,
        priced_spec=SPEC,
        max_turns=40,
        collector=collector,
        stream_log_path=stream,
        timeout_s=30,
        effort="high",
    )
    options.update(kwargs)
    outcome = DeepSeekHarnessAdapter().run_task(**options)
    return outcome, collector, launched, stream


STDOUT = [
    json.dumps({"type": "session", "sessionId": "session-abc", "cwd": "/w"}) + "\n",
    json.dumps({"type": "text", "text": "done"}) + "\n",
    json.dumps({"type": "final", "text": "done"}) + "\n",
]


def test_registry_effort_and_pricing():
    assert is_cli_agent_spec(SPEC)
    assert get_cli_agent_adapter(SPEC).harness_id == "dsh"
    assert effort_config("dsh", "low").provider_value == "low"
    assert effort_config("dsh", "max").provider_value == "max"
    medium = effort_config("dsh", "medium")
    assert medium is not None and not medium.supported
    # 1M uncached + 1M cached input + 1M output at V4.1 Flash off-peak list rates.
    assert cost_usd(SPEC, 2_000_000, 1_000_000, cached_input_tokens=1_000_000) == 0.753
    assert cost_usd("dsh:deepseek-v4-pro", 1_000_000, 1_000_000) == 2.64


def test_peak_windows():
    monday = datetime(2026, 10, 19, tzinfo=UTC)
    for hour, expected in ((0, 1.0), (1, 2.0), (3, 2.0), (4, 1.0), (6, 2.0), (9, 2.0), (10, 1.0)):
        assert peak_multiplier(SPEC, monday.replace(hour=hour)) == expected
    assert peak_multiplier(SPEC, datetime(2026, 10, 18, 2, tzinfo=UTC)) == 1.0  # Sunday
    assert peak_multiplier("openai:gpt-6.1-sol", monday.replace(hour=2)) == 1.0
    assert cost_usd_at(SPEC, 1_000_000, 0, monday.replace(hour=2)) == 0.30
    with pytest.raises(ValueError, match="timezone-aware"):
        peak_multiplier(SPEC, datetime(2026, 10, 19, 2))


def test_patch_turns_off_uploads_and_web_tools():
    patch = benchmark_patch("deepseek-v4-pro", "max")
    assert "model: deepseek-v4-pro" in patch and "reasoningEffort: max" in patch
    assert "id: session-log-deepseek\n  config:\n    enabled: false" in patch
    assert "mode: DISABLED" in patch
    assert "id: tool-web\n  disabled: true" in patch
    assert "compression: none" in patch


def test_usage_counts_messages_retries_and_peak_calls(tmp_path):
    log = _write_log(
        tmp_path / "s/events.jsonl",
        [
            {"type": "turn/start", "seq": 1, "time": OFF_PEAK_MS, "data": {}},
            _message(2, usage={"inputTokens": 200, "outputTokens": 50, "cacheReadTokens": 800}),
            _attempt(3, {"inputTokens": 1000, "outputTokens": 10}),
            _attempt(4, None),
            _message(5, time_ms=PEAK_MS, usage={"inputTokens": 1000, "outputTokens": 100}),
        ],
    )
    usage = collect_usage([log], MODEL, SPEC)
    assert (usage["calls"], usage["retried_attempts"], usage["attempts_without_usage"]) == (2, 1, 1)
    assert (usage["input"], usage["cacheRead"], usage["output"]) == (2200, 800, 160)
    assert usage["peak_calls"] == 1
    off_peak = cost_usd(SPEC, 1000, 50, cached_input_tokens=800) + cost_usd(SPEC, 1000, 10)
    assert usage["metered_usd"] == round(off_peak + 2 * cost_usd(SPEC, 1000, 100), 6)


def test_receipts_fail_closed(tmp_path):
    with pytest.raises(ProviderError, match="model mismatch"):
        collect_usage(
            [_write_log(tmp_path / "a/e.jsonl", [_message(1, model="other")])], MODEL, SPEC
        )
    bad = _message(1, usage={"inputTokens": -1, "outputTokens": 1})
    with pytest.raises(ProviderError, match="Invalid dsh token count"):
        collect_usage([_write_log(tmp_path / "b/e.jsonl", [bad])], MODEL, SPEC)
    missing = _message(1)
    del missing["data"]["usage"]
    with pytest.raises(ProviderError, match="no usage receipt"):
        collect_usage([_write_log(tmp_path / "c/e.jsonl", [missing])], MODEL, SPEC)
    dup = [_message(1), _message(1)]
    with pytest.raises(ProviderError, match="unique sequence"):
        collect_usage([_write_log(tmp_path / "d/e.jsonl", dup)], MODEL, SPEC)


def test_run_sets_settings_records_usage_and_archives(tmp_path, monkeypatch):
    rows = [_message(1), _message(2, usage={"inputTokens": 100, "outputTokens": 10})]
    outcome, collector, launched, stream = _run(tmp_path, monkeypatch, STDOUT, rows)
    assert outcome.finished and outcome.session_id == "session-abc"
    assert (outcome.prompt_tokens, outcome.completion_tokens, outcome.num_turns) == (1100, 110, 2)
    assert outcome.billing == "api" and outcome.cost_basis == "metered-api-pricing"
    assert outcome.cli_reported_cost_usd == round(
        cost_usd(SPEC, 1000, 100) + cost_usd(SPEC, 100, 10), 6
    )
    cmd, env = launched["cmd"], launched["env"]
    assert cmd[0] == "/usr/bin/sandbox-exec" and cmd[cmd.index("--profile") + 1] == "headless"
    assert cmd[-2:] == ["--json", "-"]
    assert "Fix the engine." in launched["proc"].stdin.getvalue()
    assert not any("Fix the engine." in part for part in cmd)
    assert env["DSH_PERMISSION_MODE"] == "danger-full-access"
    assert env["DSH_TELEMETRY_MODE"] == "DISABLED" and env["DEEPSEEK_API_KEY"] == "sk-test"
    patch = Path(cmd[cmd.index("--patch") + 1]).read_text()
    assert "model: deepseek-flash" in patch and "reasoningEffort: high" in patch
    archive = stream.parent / "dsh-session-logs"
    assert (archive / "session-abc/events.jsonl").is_file()
    assert (archive / "vulcanbench.patch.yml").read_text() == patch
    start = collector.events[0][1]
    assert start["harness_version"] == deepseek_harness.VERSION and start["billing"] == "api"
    assert collector.events[-1][0] == "cli_agent_result"


def test_failed_run_raises_and_flags_api_failures(tmp_path, monkeypatch):
    stdout = [json.dumps({"type": "error", "message": "QUOTA: insufficient balance"}) + "\n"]
    with pytest.raises(ProviderError, match="dsh API failure"):
        _run(tmp_path, monkeypatch, stdout, [], returncode=1)


def test_refusals(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match="low, high or max"):
        _run(tmp_path, monkeypatch, STDOUT, [_message(1)], effort="medium")
    with pytest.raises(ProviderError, match="web tools off"):
        _run(tmp_path, monkeypatch, STDOUT, [_message(1)], network=True)
    with pytest.raises(ProviderError, match="not configured"):
        _run(tmp_path, monkeypatch, STDOUT, [_message(1)], model="deepseek-v4-flash")


def test_pin_rejects_changed_install(tmp_path, monkeypatch):
    _, prefix = _pin(tmp_path, monkeypatch)
    deepseek_harness.pinned_install()
    (prefix / "node_modules/@deepseek-ai/dsh/lib/bin.js").write_text("// tampered")
    with pytest.raises(ProviderError, match="tree SHA256 mismatch"):
        deepseek_harness.pinned_install()
    monkeypatch.setenv("VULCANBENCH_DSH_SHA256", tree_sha256(prefix / "node_modules"))
    (prefix / "node_modules/@deepseek-ai/dsh/package.json").write_text('{"version": "0.2.0"}')
    with pytest.raises(ProviderError, match="adapter pins"):
        deepseek_harness.pinned_install()
