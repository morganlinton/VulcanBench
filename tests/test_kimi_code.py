"""Kimi Code CLI adapter: pinning, run config, usage receipts and tool calls."""

import json
import tomllib
from pathlib import Path

import pytest

from harness.agent import kimi_code
from harness.agent import tool_calls as tc
from harness.agent.cli_agents import get_cli_agent_adapter, is_cli_agent_spec
from harness.agent.deepseek_harness import tree_sha256
from harness.agent.kimi_code import KimiCodeAdapter, benchmark_config, collect_usage
from harness.agent.providers import ProviderError
from harness.effort import effort_config
from harness.pricing import cost_usd

MODEL = "kimi-k3.1-pro"
SPEC = f"kimi-code:{MODEL}"
T0 = 1793650759618


class _Collector:
    def __init__(self):
        self.events = []

    def record(self, name, payload):
        self.events.append((name, payload))


class _FakeProc:
    def __init__(self, stdout_lines, returncode):
        self.pid, self.returncode, self._done = 4242, returncode, False
        self.stdout = iter(stdout_lines)
        self.stderr = iter([])

    def wait(self):
        self._done = True
        return self.returncode

    def poll(self):
        return self.returncode if self._done else None


def _usage(time_ms=T0, model=MODEL, agent="main", **usage):
    usage = usage or {"inputOther": 1000, "output": 100, "inputCacheRead": 0}
    return {
        "type": "usage.record",
        "agentId": agent,
        "model": model,
        "usage": usage,
        "usageScope": "turn",
        "time": time_ms,
    }


def _loop(event, time_ms=T0, agent="main"):
    return {"type": "context.append_loop_event", "agentId": agent, "event": event, "time": time_ms}


def _write(path: Path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(r) + "\n" for r in rows))
    return path


def _pin(tmp_path, monkeypatch):
    node = tmp_path / "node"
    node.write_text("#!/bin/sh\n")
    node.chmod(0o700)
    prefix = tmp_path / "kimi-prefix"
    package = prefix / "node_modules/@moonshot-ai/kimi-code"
    (package / "dist").mkdir(parents=True, exist_ok=True)
    (package / "package.json").write_text(json.dumps({"version": kimi_code.VERSION}))
    (package / "dist/main.mjs").write_text("// fake kimi, never executed")
    monkeypatch.setenv("VULCANBENCH_KIMI_CODE_NODE", str(node))
    monkeypatch.setenv("VULCANBENCH_KIMI_CODE_PREFIX", str(prefix))
    monkeypatch.setenv("VULCANBENCH_KIMI_CODE_SHA256", tree_sha256(prefix / "node_modules"))
    monkeypatch.setenv("MOONSHOT_API_KEY", "sk-test")
    workspace = tmp_path / "workspace"
    workspace.mkdir(exist_ok=True)
    return workspace, prefix


def _run(tmp_path, monkeypatch, stdout, rows, returncode=0, **kwargs):
    workspace, _ = _pin(tmp_path, monkeypatch)
    launched = {}

    def fake_popen(cmd, **popen_kwargs):
        env = popen_kwargs["env"]
        launched.update(cmd=cmd, env=env, cwd=popen_kwargs["cwd"])
        session = Path(env["KIMI_CODE_HOME"]) / "sessions/wd_ws/session_abc"
        _write(session / "agents/main/wire.jsonl", rows)
        return _FakeProc(stdout, returncode)

    monkeypatch.setattr(kimi_code.subprocess, "Popen", fake_popen)
    monkeypatch.setattr(kimi_code.os, "killpg", lambda pid, sig: None)
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
    outcome = KimiCodeAdapter().run_task(**options)
    return outcome, collector, launched, stream


STDOUT = [
    json.dumps({"role": "meta", "type": "system.version", "version": "2.4.1"}) + "\n",
    json.dumps({"role": "assistant", "content": "done"}) + "\n",
    json.dumps({"role": "meta", "type": "session.resume_hint", "session_id": "session_abc"}) + "\n",
]


def test_registry_effort_and_pricing():
    assert is_cli_agent_spec(SPEC)
    assert get_cli_agent_adapter(SPEC).harness_id == "kimi-code"
    for level in ("low", "high", "max"):
        assert effort_config("kimi-code", level).provider_value == level
    medium = effort_config("kimi-code", "medium")
    assert medium is not None and not medium.supported
    # 1M fresh + 1M cached input + 1M output at K3.1 Pro list rates.
    assert cost_usd(SPEC, 2_000_000, 1_000_000, cached_input_tokens=1_000_000) == 12.2


def test_config_pins_endpoint_model_effort_and_data_settings():
    config = tomllib.loads(benchmark_config(MODEL, "max"))
    assert config["telemetry"] is False and config["default_permission_mode"] == "auto"
    provider = config["providers"]["moonshot"]
    assert provider == {
        "type": "kimi",
        "base_url": "https://api.moonshot.ai/v1",
        "api_key_env": "MOONSHOT_API_KEY",
    }
    model = config["models"][MODEL]
    assert model["model"] == MODEL and model["max_context_size"] == 1_048_576
    assert model["max_output_size"] == 131_072 and "always_thinking" in model["capabilities"]
    assert config["thinking"]["effort"] == "max"
    assert config["background"]["print_background_mode"] == "exit"
    assert config["tools"]["disabled"] == ["WebSearch", "FetchURL"]


def test_usage_splits_cache_reads_and_writes_and_dedupes(tmp_path):
    main = _write(
        tmp_path / "s/agents/main/wire.jsonl",
        [
            {"type": "llm.request", "model": MODEL},
            _usage(inputOther=200, output=50, inputCacheRead=800, inputCacheCreation=100),
            _usage(time_ms=T0 + 1, inputOther=1000, output=100),
        ],
    )
    sub = _write(
        tmp_path / "s/agents/sub1/wire.jsonl",
        [_usage(time_ms=T0 + 1, inputOther=1000, output=100), _usage(T0 + 2, agent="sub1")],
    )
    usage = collect_usage([main, sub], MODEL, SPEC)
    assert usage["calls"] == 3
    assert (usage["input"], usage["cacheRead"], usage["cacheWrite"]) == (2200, 800, 100)
    assert usage["output"] == 250
    expected = cost_usd(SPEC, 1100, 50, cached_input_tokens=800) + 2 * cost_usd(SPEC, 1000, 100)
    assert usage["metered_usd"] == round(expected, 6)


def test_receipts_fail_closed(tmp_path):
    with pytest.raises(ProviderError, match="model mismatch"):
        collect_usage([_write(tmp_path / "a/wire.jsonl", [_usage(model="kimi-k3")])], MODEL, SPEC)
    bad = _usage(inputOther=-1, output=1)
    with pytest.raises(ProviderError, match="Invalid kimi-code token count"):
        collect_usage([_write(tmp_path / "b/wire.jsonl", [bad])], MODEL, SPEC)


def test_run_sets_config_records_usage_and_archives(tmp_path, monkeypatch):
    rows = [_usage(), _usage(T0 + 1, inputOther=100, output=10)]
    outcome, collector, launched, stream = _run(tmp_path, monkeypatch, STDOUT, rows)
    assert outcome.finished and outcome.session_id == "session_abc"
    assert (outcome.prompt_tokens, outcome.completion_tokens, outcome.num_turns) == (1100, 110, 2)
    assert outcome.billing == "api" and outcome.cost_basis == "metered-api-pricing"
    assert outcome.cli_reported_cost_usd == round(
        cost_usd(SPEC, 1000, 100) + cost_usd(SPEC, 100, 10), 6
    )
    cmd, env = launched["cmd"], launched["env"]
    assert cmd[0] == "/usr/bin/sandbox-exec"
    assert cmd[-4:-1] == ["--output-format", "stream-json", "-p"]
    assert cmd[-1].endswith("Fix the engine.")
    assert env["KIMI_DISABLE_TELEMETRY"] == "1" and env["KIMI_CODE_NO_AUTO_UPDATE"] == "1"
    assert env["MOONSHOT_API_KEY"] == "sk-test"
    config = (Path(env["KIMI_CODE_HOME"]) / "config.toml").read_text()
    assert tomllib.loads(config)["thinking"]["effort"] == "high"
    archive = stream.parent / "kimi-code-sessions"
    assert (archive / "wd_ws/session_abc/agents/main/wire.jsonl").is_file()
    assert (archive / "config.toml").read_text() == config
    start = collector.events[0][1]
    assert start["harness_version"] == kimi_code.VERSION and start["billing"] == "api"
    assert "Fix the engine." not in json.dumps(start["argv"])
    assert collector.events[-1][0] == "cli_agent_result"


def test_finished_run_without_receipts_fails_closed(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match=r"no wire\.jsonl receipts"):
        _run(tmp_path, monkeypatch, STDOUT, [])


def test_failed_run_raises_and_flags_api_failures(tmp_path, monkeypatch):
    workspace, _ = _pin(tmp_path, monkeypatch)

    class _Failing(_FakeProc):
        def __init__(self):
            super().__init__([], 1)
            self.stderr = iter(['{"error":{"type":"exceeded_current_quota_error"}}\n'])

    monkeypatch.setattr(kimi_code.subprocess, "Popen", lambda cmd, **kw: _Failing())
    with pytest.raises(ProviderError, match="kimi-code API failure"):
        KimiCodeAdapter().run_task(
            workspace=workspace,
            prompt="x",
            model=MODEL,
            priced_spec=SPEC,
            max_turns=1,
            collector=_Collector(),
            timeout_s=30,
            effort="low",
        )


def test_refusals(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match="low, high or max"):
        _run(tmp_path, monkeypatch, STDOUT, [_usage()], effort="medium")
    with pytest.raises(ProviderError, match="web tools off"):
        _run(tmp_path, monkeypatch, STDOUT, [_usage()], network=True)
    with pytest.raises(ProviderError, match="not configured"):
        _run(tmp_path, monkeypatch, STDOUT, [_usage()], model="kimi-for-coding")


def test_pin_rejects_changed_install(tmp_path, monkeypatch):
    _, prefix = _pin(tmp_path, monkeypatch)
    kimi_code.pinned_install()
    (prefix / "node_modules/@moonshot-ai/kimi-code/dist/main.mjs").write_text("// tampered")
    with pytest.raises(ProviderError, match="tree SHA256 mismatch"):
        kimi_code.pinned_install()
    monkeypatch.setenv("VULCANBENCH_KIMI_CODE_SHA256", tree_sha256(prefix / "node_modules"))
    (prefix / "node_modules/@moonshot-ai/kimi-code/package.json").write_text('{"version": "2.4.0"}')
    with pytest.raises(ProviderError, match="adapter pins"):
        kimi_code.pinned_install()


def test_tool_calls_from_wire_logs(tmp_path):
    session = tmp_path / "kimi-code-sessions/wd_ws/session_abc/agents"
    _write(
        session / "main/wire.jsonl",
        [
            _loop(
                {"type": "tool.call", "toolCallId": "c1", "name": "Bash", "args": {"command": "ls"}}
            ),
            _loop(
                {
                    "type": "tool.result",
                    "toolCallId": "c1",
                    "result": {"output": "a\n", "durationMs": 12},
                },
                T0 + 20,
            ),
        ],
    )
    _write(
        session / "sub1/wire.jsonl",
        [
            _loop(
                {"type": "tool.call", "toolCallId": "c2", "name": "Write", "args": {}}, agent="sub1"
            ),
            _loop(
                {
                    "type": "tool.result",
                    "toolCallId": "c2",
                    "result": {"output": "no", "isError": True},
                },
                agent="sub1",
            ),
        ],
    )
    first, second = tc.extract(tmp_path, "kimi-code")
    assert (first["tool"], first["kind"], first["agent"], first["status"]) == (
        "Bash",
        "shell",
        None,
        "ok",
    )
    assert first["duration_ms"] == 12 and first["output_chars"] == 2
    assert first["input"] == {"command": "ls"}
    assert (second["tool"], second["agent"], second["status"]) == ("Write", "sub1", "error")
