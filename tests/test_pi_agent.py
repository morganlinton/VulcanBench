"""Pi adapter: pinning, usage receipts, cost cap and metered-billing summaries."""

import hashlib
import json

import pytest

from harness.agent import pi_agent
from harness.agent.cli_agents import get_cli_agent_adapter, is_cli_agent_spec
from harness.agent.pi_agent import PiAdapter, add_usage
from harness.agent.providers import ProviderError
from harness.effort import effort_config
from harness.pricing import cost_usd

MODEL = "meta/muse-wonder-1.0"


class _Collector:
    def __init__(self):
        self.events = []

    def record(self, name, payload):
        self.events.append((name, payload))


class _FakeProc:
    def __init__(self, cmd, stdout_lines, returncode, killed):
        self.cmd, self.pid, self.returncode = cmd, 4242, returncode
        self._lines, self._killed, self._done = stdout_lines, killed, False
        self.stdout = self._stream()
        self.stderr = iter([])

    def _stream(self):
        for line in self._lines:
            if self._killed:
                return
            yield line

    def wait(self):
        self._done = True
        return self.returncode

    def poll(self):
        return self.returncode if self._done else None


def _assistant(stop="stop", input_=1000, output=100, cache_read=0, model="muse-wonder-1.0"):
    usage = {
        "input": input_,
        "output": output,
        "cacheRead": cache_read,
        "cacheWrite": 0,
        "reasoning": 10,
        "totalTokens": input_ + output + cache_read,
        "cost": {"total": 0.01},
    }
    message = {
        "role": "assistant",
        "provider": "meta",
        "model": model,
        "stopReason": stop,
        "usage": usage,
        "content": [{"type": "text", "text": "working"}],
    }
    return json.dumps({"type": "message_end", "message": message}) + "\n"


SETTLED = json.dumps({"type": "agent_settled"}) + "\n"


def _pin(tmp_path, monkeypatch):
    node = tmp_path / "node"
    node.write_text("#!/bin/sh\n")
    node.chmod(0o700)
    cli = tmp_path / "cli.js"
    cli.write_text("// fake pi bundle, never executed")
    config = tmp_path / "pi-config"
    config.mkdir()
    (config / "models.json").write_text('{"providers": {}}')
    monkeypatch.setenv("VULCANBENCH_PI_NODE", str(node))
    monkeypatch.setenv("VULCANBENCH_PI_CLI", str(cli))
    monkeypatch.setenv("VULCANBENCH_PI_SHA256", hashlib.sha256(cli.read_bytes()).hexdigest())
    monkeypatch.setenv("VULCANBENCH_PI_CONFIG", str(config))
    monkeypatch.setenv("MUSE_API_KEY", "meta-test-key")
    monkeypatch.setattr(pi_agent, "_version", lambda node, cli: "Pi 9.9.9")
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    return workspace


def _run(tmp_path, monkeypatch, lines, returncode=0, **kwargs):
    workspace = _pin(tmp_path, monkeypatch)
    launched, killed = {}, []

    def fake_popen(cmd, **popen_kwargs):
        launched["cmd"], launched["env"] = cmd, popen_kwargs["env"]
        launched["stdin"] = popen_kwargs.get("stdin")
        return _FakeProc(cmd, lines, returncode, killed)

    monkeypatch.setattr(pi_agent.subprocess, "Popen", fake_popen)
    monkeypatch.setattr(pi_agent.os, "killpg", lambda pid, sig: killed.append(sig))
    collector = _Collector()
    stream = tmp_path / "run/stream.jsonl"
    stream.parent.mkdir()
    outcome = PiAdapter().run_task(
        workspace=workspace,
        prompt="Fix the engine.",
        model=MODEL,
        priced_spec=f"pi:{MODEL}",
        max_turns=40,
        collector=collector,
        stream_log_path=stream,
        timeout_s=30,
        effort="low",
        **kwargs,
    )
    return outcome, collector, launched, killed


def test_registry_effort_and_pricing():
    assert is_cli_agent_spec("pi:meta/muse-wonder-1.0")
    assert get_cli_agent_adapter("pi:meta/muse-wonder-1.0").harness_id == "pi"
    assert effort_config("pi", "low").provider_value == "low"
    assert effort_config("pi", "extra-high").provider_value == "xhigh"
    assert effort_config("pi", "max").provider_value == "max"
    # 1M uncached input + 1M cached input + 1M output at Wonder 1.0 list rates.
    assert (
        cost_usd("pi:meta/muse-wonder-1.0", 2_000_000, 1_000_000, cached_input_tokens=1_000_000)
        == 24.2
    )


def test_run_counts_usage_and_records_api_billing(tmp_path, monkeypatch):
    lines = [
        "not json\n",
        _assistant("toolUse", input_=1800, output=120),
        _assistant("stop", input_=200, output=20, cache_read=1800),
        SETTLED,
    ]
    outcome, collector, launched, _ = _run(tmp_path, monkeypatch, lines)
    assert outcome.finished and outcome.subtype == "stop"
    assert (outcome.prompt_tokens, outcome.cached_input_tokens, outcome.completion_tokens) == (
        3800,
        1800,
        140,
    )
    assert outcome.num_turns == 2 and outcome.reported_model == MODEL
    assert outcome.billing == "api" and outcome.auth_method == "api-key"
    assert outcome.cli_reported_cost_usd == 0.02
    cmd = launched["cmd"]
    assert cmd[0] == "/usr/bin/sandbox-exec" and "--no-context-files" in cmd and "--offline" in cmd
    assert cmd[cmd.index("--thinking") + 1] == "low"
    assert cmd[cmd.index("--model") + 1] == "muse-wonder-1.0"
    assert launched["env"]["MUSE_API_KEY"] == "meta-test-key"
    assert launched["env"]["PI_OFFLINE"] == "1"
    assert launched["stdin"] is pi_agent.subprocess.DEVNULL
    start = collector.events[0][1]
    assert start["argv"][-1] == "<task prompt>" and start["billing"] == "api"
    assert collector.events[-1][0] == "cli_agent_result"


def test_model_mismatch_and_bad_receipts_fail_closed():
    totals = dict(
        input=0, output=0, cacheRead=0, cacheWrite=0, reasoning=0, calls=0, pi_cost_usd=0.0
    )
    message = json.loads(_assistant(model="other-model"))["message"]
    with pytest.raises(ProviderError, match="model mismatch"):
        add_usage(message, "meta", "muse-wonder-1.0", totals)
    message = json.loads(_assistant())["message"]
    message["usage"]["output"] = -1
    with pytest.raises(ProviderError, match="Invalid Pi token count"):
        add_usage(message, "meta", "muse-wonder-1.0", totals)


def test_cost_cap_kills_the_run(tmp_path, monkeypatch):
    big = _assistant("toolUse", input_=2_000_000, output=10)
    lines = [big, big, SETTLED]
    outcome, _, _, killed = _run(tmp_path, monkeypatch, lines, max_run_cost=5.0)
    assert outcome.cost_capped and killed and not outcome.finished


def test_error_stop_raises(tmp_path, monkeypatch):
    lines = [_assistant("error"), SETTLED]
    with pytest.raises(ProviderError, match="Pi failed"):
        _run(tmp_path, monkeypatch, lines, returncode=1)


def test_requires_key_pin_and_effort(tmp_path, monkeypatch):
    workspace = _pin(tmp_path, monkeypatch)
    kwargs = dict(
        workspace=workspace,
        prompt="x",
        model=MODEL,
        priced_spec=f"pi:{MODEL}",
        max_turns=1,
        collector=_Collector(),
        timeout_s=10,
    )
    with pytest.raises(ProviderError, match="thinking level"):
        PiAdapter().run_task(**kwargs)
    monkeypatch.setenv("VULCANBENCH_PI_SHA256", "0" * 64)
    with pytest.raises(ProviderError, match="SHA256"):
        PiAdapter().run_task(**kwargs, effort="low")
    with pytest.raises(ProviderError, match="not configured"):
        PiAdapter().run_task(**{**kwargs, "model": "meta/other"}, effort="low")
