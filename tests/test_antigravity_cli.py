"""Antigravity CLI adapter: pinning, custom agent, private HOME, usage and tool calls."""

import hashlib
import json
from pathlib import Path

import pytest

from harness.agent import antigravity_cli as agy
from harness.agent import tool_calls as tc
from harness.agent.antigravity_cli import AntigravityAdapter, agent_file, model_slug
from harness.agent.cli_agents import get_cli_agent_adapter, is_cli_agent_spec
from harness.agent.providers import ProviderError
from harness.agent.run_audit import audit_stream
from harness.effort import effort_config
from harness.pricing import cost_usd

MODEL = "gemini-4-neon"
SPEC = f"agy:{MODEL}"
CID = "13410691-8b01-4d8c-a94f-625da3be3104"
ALL_TOOLS = ["read_url_content", "search_web", "view_file", "run_command"]


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


def _usage(inp, out, thinking=0, cache=0):
    return {
        "input_tokens": inp,
        "output_tokens": out,
        "thinking_tokens": thinking,
        "cache_read_tokens": cache,
        "total_tokens": inp + out,
    }


def _step(index, step_type, state, **fields):
    return {
        "event": "step_update",
        "step_update": {
            "conversation_id": CID,
            "step_index": index,
            "state": state,
            "step_type": step_type,
            **fields,
        },
    }


def _events(slug="gemini-4-neon-high", agent="vulcanbench", tool="run_command", result=None):
    command = {"name": tool, "parameters": {"CommandLine": "python3 hello.py"}}
    done = {**command, "output": "Created At: x\nCommand exited with code 0.\nhello\n"}
    rows = [
        {
            "event": "init",
            "conversation_id": CID,
            "init": {"model": slug, "cwd": "/w", "agent": agent, "tools": ALL_TOOLS},
        },
        _step(0, "user_input", "DONE"),
        _step(1, "agent_response", "DONE", usage=_usage(8000, 200, 120)),
        _step(2, "tool", "ACTIVE", tool_name=tool, tool_info=command),
        _step(2, "tool", "DONE", tool_name=tool, tool_info=done, duration_seconds=1.25),
        _step(3, "agent_response", "ACTIVE", text_delta="DONE"),
        _step(3, "agent_response", "DONE", text_delta="\n", usage=_usage(500, 20, 10, 7000)),
    ]
    totals = _usage(8500, 220, 130, 7000)
    rows.append(
        {
            "event": "result",
            "result": result
            or {"conversation_id": CID, "status": "SUCCESS", "response": "DONE\n", "usage": totals},
        }
    )
    return [json.dumps(r) + "\n" for r in rows]


def _pin(tmp_path, monkeypatch):
    tmp_path.mkdir(parents=True, exist_ok=True)
    binary = tmp_path / "agy"
    binary.write_text("#!/bin/sh\n# fake agy, never executed\n")
    binary.chmod(0o700)
    monkeypatch.setenv("VULCANBENCH_AGY_BIN", str(binary))
    monkeypatch.setattr(agy, "BINARY_SHA256", hashlib.sha256(binary.read_bytes()).hexdigest())
    workspace = tmp_path / "workspace"
    workspace.mkdir(exist_ok=True)
    return workspace


def _run(tmp_path, monkeypatch, stdout, returncode=0, **kwargs):
    workspace = _pin(tmp_path, monkeypatch)
    launched = {}

    def fake_popen(cmd, **popen_kwargs):
        env = popen_kwargs["env"]
        home = Path(env["HOME"])
        launched.update(
            cmd=cmd,
            env=env,
            cwd=popen_kwargs["cwd"],
            agent=(home / ".gemini/config/agents/vulcanbench.md").read_text(),
            keychains=home / "Library/Keychains",
        )
        launched["keychain_target"] = launched["keychains"].readlink()
        state = home / ".gemini/antigravity-cli"
        (state / "conversations").mkdir(parents=True)
        (state / "conversations" / f"{CID}.db").write_bytes(b"SQLite format 3\0")
        (state / "brain" / CID).mkdir(parents=True)
        (state / "brain" / CID / "task.md").write_text("- [x] hello\n")
        (state / "log").mkdir()
        (state / "log/cli-1.log").write_text("I1119 started\n")
        (state / "secrets.json").write_text('{"token": "must-not-be-archived"}')
        return _FakeProc(stdout, returncode)

    monkeypatch.setattr(agy.subprocess, "Popen", fake_popen)
    monkeypatch.setattr(agy.os, "killpg", lambda pid, sig: None)
    collector = _Collector()
    run_dir = tmp_path / "run"
    run_dir.mkdir(exist_ok=True)
    stream = run_dir / tc.STREAM
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
    outcome = AntigravityAdapter().run_task(**options)
    return outcome, collector, launched, stream


def test_registry_effort_and_pricing():
    assert is_cli_agent_spec(SPEC)
    assert get_cli_agent_adapter(SPEC).harness_id == "agy"
    for level, value in (("low", "low"), ("medium", "medium"), ("high", "high")):
        assert effort_config("agy", level).provider_value == value
    assert effort_config("agy", "extra-high").provider_value == "xhigh"
    for level in ("max", "minimal"):
        config = effort_config("agy", level)
        assert config is not None and not config.supported
    assert model_slug(MODEL, "xhigh") == "gemini-4-neon-xhigh"
    # 1M fresh + 1M cached input + 1M output at Neon list rates: 3.00 + 0.15 + 15.00.
    assert cost_usd(SPEC, 2_000_000, 1_000_000, cached_input_tokens=1_000_000) == 18.15


def test_agent_file_keeps_agy_identity_and_only_file_and_shell_tools():
    text = agent_file()
    front, _, body = text.removeprefix("---\n").partition("---\n")
    assert "tools: [read_file, write_file, edit_file, run_command]" in front
    assert "mainAgent: true" in front and "subagent: false" in front
    assert "search_web" not in text
    assert body.startswith(
        "You are Antigravity, a powerful agentic AI assistant designed by the Google Deepmind team."
    )
    assert "<USER_REQUEST>" in body


def test_binary_must_match_the_pin(tmp_path, monkeypatch):
    binary = tmp_path / "agy"
    binary.write_text("#!/bin/sh\n")
    binary.chmod(0o700)
    monkeypatch.setenv("VULCANBENCH_AGY_BIN", str(binary))
    with pytest.raises(ProviderError, match="SHA256 mismatch"):
        agy.pinned_binary()


def test_run_settings_home_usage_and_archive(tmp_path, monkeypatch):
    outcome, collector, launched, stream = _run(tmp_path, monkeypatch, _events())
    cmd = launched["cmd"]
    assert cmd[:2] == ["/usr/bin/sandbox-exec", "-f"]
    assert cmd[cmd.index("--model") + 1] == "gemini-4-neon-high"
    assert cmd[cmd.index("--agent") + 1] == "vulcanbench"
    assert "--dangerously-skip-permissions" in cmd and "--sandbox" not in cmd
    settings = json.loads(cmd[cmd.index("--settings") + 1])
    assert settings["useG1Credits"] is False
    assert set(settings["permissions"]["deny"]) == {"read_url(*)", "execute_url(*)", "mcp(*)"}
    assert cmd[-1].endswith("Fix the engine.")
    env = launched["env"]
    assert env["HOME"] != str(Path.home()) and env["AGY_CLI_DISABLE_AUTO_UPDATE"] == "1"
    assert launched["keychain_target"] == Path.home() / "Library/Keychains"
    assert launched["agent"] == agent_file()
    assert "GEMINI_API_KEY" not in env
    assert env["TMPPREFIX"] == str(Path(env["TMPDIR"]) / "zsh")
    profile = (stream.parent / "agy-session/boundary.sb").read_text()
    assert str((Path.home() / ".gemini").resolve()) in profile
    assert "(allow pseudo-tty)" in profile and "/dev/ptmx" in profile

    assert outcome.finished and outcome.subtype == "completed"
    assert outcome.session_id == CID
    assert outcome.billing == "subscription" and outcome.plan_name == "Google AI Pro"
    assert outcome.prompt_tokens == 8500 + 7000 and outcome.cached_input_tokens == 7000
    assert outcome.completion_tokens == 220 and outcome.reasoning_output_tokens == 130
    assert outcome.num_turns == 2 and outcome.reported_model == MODEL
    assert ("assistant_text", {"text": "DONE\n"}) in collector.events

    archive = stream.parent / "agy-session"
    assert (archive / f"{CID}.db").read_bytes().startswith(b"SQLite")
    assert (archive / "brain/task.md").exists() and (archive / "log/cli-1.log").exists()
    assert not list(archive.rglob("secrets.json"))
    assert json.loads((archive / "init-tools.json").read_text()) == ALL_TOOLS
    assert all("vb_received_at" in json.loads(line) for line in stream.read_text().splitlines())


def test_tool_outside_the_allowlist_voids_the_run(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match=r"outside the allowlist.*search_web"):
        _run(tmp_path, monkeypatch, _events(tool="search_web"))


def test_wrong_model_or_agent_is_refused(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match="served"):
        _run(tmp_path, monkeypatch, _events(slug="gemini-4-neon-low"))
    with pytest.raises(ProviderError, match="ran agent"):
        _run(tmp_path / "b", monkeypatch, _events(agent=None))


def test_usage_must_match_the_result(tmp_path, monkeypatch):
    bad = {"status": "SUCCESS", "response": "DONE", "usage": _usage(1, 1)}
    with pytest.raises(ProviderError, match="usage mismatch"):
        _run(tmp_path, monkeypatch, _events(result=bad))


def test_quota_failure_is_infrastructure(tmp_path, monkeypatch):
    failed = {
        "status": "ERROR",
        "error": "RESOURCE_EXHAUSTED: quota exceeded",
        "usage": _usage(0, 0),
    }
    lines = [_events()[0], json.dumps({"event": "result", "result": failed}) + "\n"]
    with pytest.raises(ProviderError, match="service or quota failure"):
        _run(tmp_path, monkeypatch, lines, returncode=1)


def test_unsupported_effort_and_network_are_refused(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match="explicit effort"):
        _run(tmp_path, monkeypatch, _events(), effort="max")
    with pytest.raises(ProviderError, match="web tools off"):
        _run(tmp_path, monkeypatch, _events(), network=True)


def test_tool_calls_come_from_the_stream(tmp_path, monkeypatch):
    _, _, _, stream = _run(tmp_path, monkeypatch, _events())
    calls = tc.EXTRACTORS["agy"](stream.parent)
    assert len(calls) == 1
    call = calls[0]
    assert call["call_id"] == "2" and call["tool"] == "run_command"
    assert call["input"] == {"CommandLine": "python3 hello.py"}
    assert call["status"] == "ok" and call["exit_code"] == 0 and call["duration_ms"] == 1250
    assert call["started_at"] and call["ended_at"]


def test_web_audit_ignores_the_init_tool_list(tmp_path, monkeypatch):
    _, _, _, stream = _run(tmp_path, monkeypatch, _events())
    assert audit_stream(stream, {})["verdict"] == "no_web"
    used = tmp_path / "used.jsonl"
    step = _step(4, "tool", "DONE", tool_name="search_web", tool_info={"name": "search_web"})
    used.write_text(json.dumps(step) + "\n")
    assert audit_stream(used, {})["verdict"] != "no_web"
