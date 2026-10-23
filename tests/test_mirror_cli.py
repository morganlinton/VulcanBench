"""Mirror CLI adapter: pinning, private home with consent, usage receipts and run lifecycle."""

import io
import json
from pathlib import Path

import pytest

from harness.agent import mirror_cli
from harness.agent.cli_agents import get_cli_agent_adapter, is_cli_agent_spec
from harness.agent.mirror_cli import MirrorAdapter, collect_usage, mirror_home, tree_sha256
from harness.agent.providers import ProviderError
from harness.effort import effort_config

MODEL = "Beam-1.0-501B-A23B"
SPEC = f"mirror:{MODEL}"


class _Collector:
    def __init__(self):
        self.events = []

    def record(self, name, payload):
        self.events.append((name, payload))


class _FakeProc:
    def __init__(self, returncode):
        self.pid, self.returncode, self._done = 4242, returncode, False
        self.stdin = io.StringIO()
        self.stdin.close = lambda: None
        self.stdout = iter(["Done.\n"])
        self.stderr = iter(["mirror: session abc123\n"])

    def wait(self):
        self._done = True
        return self.returncode

    def poll(self):
        return self.returncode if self._done else None


def _rows(*usages, model=MODEL):
    rows = [
        {"sequence": 1, "id": "a", "time": 1.0, "model": model, "effort": "low", "role": "user"}
    ]
    for i, (prompt, completion, cached) in enumerate(usages):
        rows.append(
            {
                "sequence": i + 2,
                "id": f"r{i}",
                "time": 2.0 + i,
                "role": "assistant",
                "text": "working",
                "usage": {
                    "prompt": prompt,
                    "completion": completion,
                    "cached": cached,
                    "cache_write": 0,
                },
            }
        )
    return rows


def _pin(tmp_path, monkeypatch):
    prefix = tmp_path / "mirror-prefix"
    packages = prefix / "lib/python3.12/site-packages"
    (packages / "mirror").mkdir(parents=True, exist_ok=True)
    (packages / "mirror/__init__.py").write_text("# fake mirror, never executed")
    (packages / f"reflection_mirror-{mirror_cli.VERSION}.dist-info").mkdir(exist_ok=True)
    (prefix / "bin").mkdir(exist_ok=True)
    (prefix / "bin/mirror").write_text("#!/bin/sh\n")
    monkeypatch.setenv("VULCANBENCH_MIRROR_PREFIX", str(prefix))
    monkeypatch.setenv("VULCANBENCH_MIRROR_SHA256", tree_sha256(packages / "mirror"))
    monkeypatch.setenv("REFLECTION_API_KEY", "rf-test")
    workspace = tmp_path / "workspace"
    workspace.mkdir(exist_ok=True)
    return workspace, packages


def _run(tmp_path, monkeypatch, rows, returncode=0, **kwargs):
    workspace, _ = _pin(tmp_path, monkeypatch)
    launched = {}

    def fake_popen(cmd, **popen_kwargs):
        launched.update(cmd=cmd, env=popen_kwargs["env"])
        Path(cmd[cmd.index("--database") + 1]).write_text("db")
        proc = _FakeProc(returncode)
        launched["proc"] = proc
        return proc

    monkeypatch.setattr(mirror_cli.subprocess, "Popen", fake_popen)
    monkeypatch.setattr(mirror_cli, "decode_database", lambda python, packages, db: rows)
    monkeypatch.setattr(mirror_cli.os, "killpg", lambda pid, sig: None)
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
        effort="xhigh",
    )
    options.update(kwargs)
    outcome = MirrorAdapter().run_task(**options)
    return outcome, collector, launched, stream


def test_registry_and_effort():
    assert is_cli_agent_spec(SPEC)
    assert get_cli_agent_adapter(SPEC).harness_id == "mirror"
    assert effort_config("mirror", "extra-high").provider_value == "xhigh"
    assert effort_config("mirror", "max").provider_value == "max"
    assert effort_config("mirror", "medium").provider_value == "medium"


def test_private_home_acknowledges_notices_for_the_run_workspace_only(tmp_path):
    workspace = tmp_path / "ws"
    workspace.mkdir()
    home = mirror_home(tmp_path / "scratch", workspace)
    settings = json.loads((home / ".mirror/config.json").read_text())
    assert settings == {
        "usage_warning_acknowledged": True,
        "trusted_directories": [str(workspace.resolve())],
        "analytics": False,
    }


def test_usage_sums_assistant_records_and_fails_closed():
    usage = collect_usage(_rows((1000, 100, 800), (200, 20, 0)), MODEL)
    assert (usage["prompt"], usage["completion"], usage["cached"], usage["calls"]) == (
        1200,
        120,
        800,
        2,
    )
    with pytest.raises(ProviderError, match="model mismatch"):
        collect_usage(_rows((1, 1, 0), model="other"), MODEL)
    with pytest.raises(ProviderError, match="Invalid Mirror token count"):
        collect_usage(_rows((-1, 1, 0)), MODEL)
    with pytest.raises(ProviderError, match="Inconsistent"):
        collect_usage(_rows((10, 1, 11)), MODEL)


def test_run_settings_usage_and_archive(tmp_path, monkeypatch):
    outcome, collector, launched, stream = _run(tmp_path, monkeypatch, _rows((1000, 100, 800)))
    assert outcome.finished and outcome.session_id == "abc123"
    assert (outcome.prompt_tokens, outcome.completion_tokens, outcome.cached_input_tokens) == (
        1000,
        100,
        800,
    )
    assert outcome.billing == "api" and outcome.num_turns == 1
    cmd, env = launched["cmd"], launched["env"]
    assert cmd[0] == "/usr/bin/sandbox-exec" and cmd[-2:] == ["exec", "-"]
    assert cmd[cmd.index("--reasoning-effort") + 1] == "xhigh"
    assert cmd[cmd.index("--approval-mode") + 1] == "auto" and "--no-update" in cmd
    assert env["MIRROR_SKIP_UPDATE"] == "1" and env["REFLECTION_API_KEY"] == "rf-test"
    assert Path(env["HOME"]).name == "mirror-home" and Path(env["HOME"]) != Path.home()
    assert "Fix the engine." in launched["proc"].stdin.getvalue()
    assert not any("Fix the engine." in part for part in cmd)
    assert len(stream.read_text().splitlines()) == 2
    assert (stream.parent / "mirror-session/stderr.txt").is_file()
    assert collector.events[0][1]["mirror_settings"]["trusted_directories"] == ["<run workspace>"]
    assert collector.events[-1][0] == "cli_agent_result"


def test_failures_fail_closed(tmp_path, monkeypatch):
    with pytest.raises(ProviderError, match="no auditable usage"):
        _run(tmp_path, monkeypatch, _rows())
    with pytest.raises(ProviderError, match="Mirror failed"):
        _run(tmp_path, monkeypatch, _rows((1, 1, 0)), returncode=1)
    with pytest.raises(ProviderError, match="xhigh or max"):
        _run(tmp_path, monkeypatch, _rows((1, 1, 0)), effort="extra-high")
    with pytest.raises(ProviderError, match="web tools off"):
        _run(tmp_path, monkeypatch, _rows((1, 1, 0)), network=True)


def test_pin_rejects_changed_install(tmp_path, monkeypatch):
    _, packages = _pin(tmp_path, monkeypatch)
    mirror_cli.pinned_install()
    (packages / "mirror/__init__.py").write_text("# tampered")
    with pytest.raises(ProviderError, match="tree SHA256 mismatch"):
        mirror_cli.pinned_install()
    (packages / "mirror/__pycache__").mkdir()
    (packages / "mirror/__pycache__/x.pyc").write_bytes(b"cache")
    monkeypatch.setenv("VULCANBENCH_MIRROR_SHA256", tree_sha256(packages / "mirror"))
    (packages / "mirror/__pycache__/x.pyc").write_bytes(b"other cache")
    mirror_cli.pinned_install()
