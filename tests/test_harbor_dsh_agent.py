"""scripts/frontier-v5/harbor_dsh_agent.py: options, commands, usage and infrastructure markers.

The agent runs inside harbor's own environment, which this repo's venv does not
have, so two layers are tested:

- the pure helpers (settings overlay, usage receipts, infrastructure findings)
  with small stand-ins for the harbor modules the file imports;
- the real class against the installed harbor (``~/.local/share/uv/tools/harbor``,
  the version run_gate.sh pins), in a subprocess with a fake environment that
  records every command. Skipped where harbor is not installed (CI).

No Docker, no network, no model calls.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import textwrap
import types
from pathlib import Path

import pytest
from pydantic import BaseModel, ConfigDict

from harness.agent import deepseek_harness

REPO = Path(__file__).resolve().parents[1]
AGENT = REPO / "scripts" / "frontier-v5" / "harbor_dsh_agent.py"
HARBOR_PY = Path.home() / ".local/share/uv/tools/harbor/bin/python"


def _load_with_stubs() -> types.ModuleType:
    """Import the agent module with minimal stand-ins for harbor."""

    class InstalledAgentOptions(BaseModel):
        model_config = ConfigDict(extra="forbid")
        version: str | None = None

    stubs = {
        "harbor": {},
        "harbor.agents": {},
        "harbor.agents.installed": {},
        "harbor.agents.installed.base": {
            "BaseInstalledAgent": type("BaseInstalledAgent", (), {}),
            "with_prompt_template": lambda fn: fn,
        },
        "harbor.agents.installed.node_install": {"nvm_node_install_snippet": lambda: "nvm"},
        "harbor.agents.options": {"InstalledAgentOptions": InstalledAgentOptions},
        "harbor.environments": {},
        "harbor.environments.base": {"BaseEnvironment": object},
        "harbor.models": {},
        "harbor.models.agent": {},
        "harbor.models.agent.context": {"AgentContext": object},
    }
    saved = {k: sys.modules.get(k) for k in stubs}
    try:
        for name, attrs in stubs.items():
            mod = types.ModuleType(name)
            mod.__dict__.update(attrs)
            sys.modules[name] = mod
        spec = importlib.util.spec_from_file_location("harbor_dsh_agent_stubbed", AGENT)
        assert spec and spec.loader
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        for name, old in saved.items():
            if old is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = old


dsh = _load_with_stubs()


def test_pins_match_the_frontier_v4_adapter() -> None:
    assert dsh.VERSION == deepseek_harness.VERSION == "0.2.1-rc.2"
    assert dsh.MODELS == deepseek_harness.MODELS
    assert set(dsh.EFFORTS) == set(deepseek_harness.EFFORTS)
    assert dsh.INFRA_CODES == deepseek_harness.INFRA_CODES
    # Same settings overlay as Frontier v4, apart from the header comment.
    body = lambda p: p.split("\n", 1)[1]  # noqa: E731
    assert body(dsh.benchmark_patch("deepseek-flash", "max")) == body(
        deepseek_harness.benchmark_patch("deepseek-flash", "max")
    )


def test_overlay_turns_web_and_uploads_off() -> None:
    patch = dsh.benchmark_patch("deepseek-flash", "high")
    assert "- id: tool-web\n  disabled: true" in patch
    assert "- id: session-log-deepseek\n  config:\n    enabled: false" in patch
    assert "reasoningEffort: high" in patch and "compression: none" in patch


def _msg(seq: int, usage: dict | None, kind: str = "assistant/message") -> dict:
    if kind == "assistant/attempt":
        stream = [{"type": "chunk", "chunk": {"type": "usage", "usage": usage}}] if usage else []
        return {"type": kind, "seq": seq, "data": {"stream": stream}}
    return {"type": kind, "seq": seq, "data": {"usage": usage}}


def test_usage_sums_messages_and_retried_attempts() -> None:
    rows = [
        {"type": "session/start"},
        _msg(1, {"inputTokens": 100, "outputTokens": 10, "cacheReadTokens": 400}),
        _msg(2, {"inputTokens": 50, "outputTokens": 5, "cacheWriteTokens": 7}),
        _msg(3, {"inputTokens": 30, "outputTokens": 3}, kind="assistant/attempt"),
        _msg(4, None, kind="assistant/attempt"),
    ]
    assert dsh._usage(rows) == {
        "input": 180,
        "output": 18,
        "cache_read": 400,
        "cache_write": 7,
        "calls": 2,
        "retried": 1,
    }


def test_infra_findings() -> None:
    ok = {"calls": 3}
    none = {"calls": 0}
    quota = [{"type": "error", "message": "QUOTA: insufficient balance"}]
    assert dsh.infra_findings(quota, "", ok, False) == ["dsh error event QUOTA"]
    auth = [{"type": "error", "code": "AUTH", "message": "bad key"}]
    assert dsh.infra_findings(auth, "", ok, False) == ["dsh error event AUTH"]
    # A model-side failure is the model's work.
    other = [{"type": "error", "message": "tool loop aborted"}]
    assert dsh.infra_findings(other, "", ok, False) == []
    assert dsh.infra_findings([], "plugin x did not activate", ok, True)
    assert dsh.infra_findings([{"type": "final"}], "", none, True) == [
        "dsh finished with no usage receipt"
    ]
    assert dsh.infra_findings([], "", none, False) == []


HARBOR_SCRIPT = textwrap.dedent(
    r"""
    import asyncio, json, os, sys
    from pathlib import Path
    from harbor.agents.factory import AgentFactory
    from harbor.models.agent.context import AgentContext
    import harbor_dsh_agent as m

    logs = Path(sys.argv[1])
    out = {}

    class R:
        return_code, stdout, stderr = 0, "/usr/bin/curl\n", ""

    class Env:
        default_user = None
        def __init__(self): self.cmds, self.up = [], {}
        async def exec(self, command, user=None, env=None, cwd=None, timeout_sec=None):
            self.cmds.append({"command": command, "env_keys": sorted(env or {}),
                              "dsh_home": (env or {}).get("DSH_HOME")})
            return R()
        async def upload_file(self, local, remote): self.up[remote] = Path(local).read_text()

    cls = AgentFactory.get_agent_class_from_config(
        type("C", (), {"name": "harbor_dsh_agent:DeepSeekHarness", "import_path": None})())
    out["class"] = cls.__name__
    kw = dict(version="0.2.1-rc.2", disable_web_search=True)
    refused = []
    for bad in ({"reasoning_effort": "medium"}, {"reasoning_effort": "max", "version": "0.2.0"},
                {"reasoning_effort": "max", "disable_web_search": False}):
        try:
            m.DeepSeekHarness(logs_dir=logs, model_name="deepseek-flash", **{**kw, **bad})._check_options()
        except ValueError:
            refused.append(True)
    out["refused"] = refused
    try:
        m.DeepSeekHarness(logs_dir=logs, model_name="deepseek-flash", bogus=1)
        out["extra_kwarg"] = "accepted"
    except Exception:
        out["extra_kwarg"] = "refused"

    os.environ["DEEPSEEK_API_KEY"] = "sk-not-real"
    a = m.DeepSeekHarness(logs_dir=logs, model_name="deepseek-flash", reasoning_effort="max", **kw)
    env = Env()
    asyncio.run(a.install(env))
    asyncio.run(a.run("Fix the bug.", env, AgentContext()))
    out["cmds"], out["uploads"] = env.cmds, env.up

    (logs / "dsh.txt").write_text('{"type":"error","message":"QUOTA: insufficient balance"}\n')
    s = logs / "dsh-session-logs" / "sessions" / "abc"
    s.mkdir(parents=True)
    (s / "session.v4.jsonl").write_text(json.dumps({"type": "assistant/message", "seq": 1,
        "data": {"usage": {"inputTokens": 100, "outputTokens": 10, "cacheReadTokens": 50}}}) + "\n")
    ctx = AgentContext()
    a.populate_context_post_run(ctx)
    out["ctx"] = [ctx.n_input_tokens, ctx.n_cache_tokens, ctx.n_output_tokens]
    out["infra"] = (logs / "vb-infra.txt").read_text()
    print(json.dumps(out))
    """
)


@pytest.mark.skipif(not HARBOR_PY.exists(), reason="harbor is not installed on this host")
def test_agent_against_the_installed_harbor(tmp_path: Path) -> None:
    logs = tmp_path / "agent"
    logs.mkdir()
    env = dict(os.environ, PYTHONPATH=str(AGENT.parent))
    env.pop("DEEPSEEK_API_KEY", None)
    r = subprocess.run(
        [str(HARBOR_PY), "-c", HARBOR_SCRIPT, str(logs)],
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    assert r.returncode == 0, r.stderr[-3000:]
    out = json.loads(r.stdout.strip().splitlines()[-1])
    assert out["class"] == "DeepSeekHarness"
    assert out["refused"] == [True, True, True] and out["extra_kwarg"] == "refused"
    install = next(c["command"] for c in out["cmds"] if "npm install -g" in c["command"])
    assert "npm install -g @deepseek-ai/dsh@0.2.1-rc.2" in install
    assert "= 0.2.1-rc.2" in install and "dsh-install.json" in install
    run = out["cmds"][-1]
    assert "--profile headless" in run["command"] and "--json -" in run["command"]
    assert "< /tmp/vb-dsh/prompt.txt" in run["command"]
    assert "tee /logs/agent/dsh.txt" in run["command"]
    assert run["dsh_home"] == "/logs/agent/dsh-session-logs"
    assert run["env_keys"] == [
        "DEEPSEEK_API_KEY",
        "DSH_HOME",
        "DSH_PERMISSION_MODE",
        "DSH_TELEMETRY_MODE",
    ]
    assert "sk-not-real" not in json.dumps(out["cmds"])  # the key is env, never argv
    assert out["uploads"]["/tmp/vb-dsh/prompt.txt"] == "Fix the bug."
    assert "reasoningEffort: max" in out["uploads"]["/tmp/vb-dsh/vulcanbench.patch.yml"]
    assert out["ctx"] == [150, 50, 10]
    assert out["infra"] == "VULCANBENCH_INFRA: dsh error event QUOTA\n"
