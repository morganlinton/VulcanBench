"""Tests for the Devin CLI adapter (``devin:<model>`` specs).

A fake ``devin`` on PATH answers ``--version``, ``auth status`` and
``models list --format json`` like the 3000.10.x CLI, and in print mode
writes the hello-world solution plus the sqlite session store the adapter
harvests receipts and tool calls from, so the whole pipeline runs offline.
"""

from __future__ import annotations

import json
import os
import stat
from pathlib import Path
from typing import Any

import pytest

from harness.agent import devin_cli
from harness.agent.cli_agents import (
    SubscriptionQuotaError,
    get_cli_agent_adapter,
    is_cli_agent_spec,
)
from harness.agent.devin_cli import (
    build_run_config,
    devin_preflight,
    effort_from_uid,
    resolve_model_uid,
    run_devin_task,
)
from harness.agent.loop import run_agent
from harness.agent.providers import ProviderError
from harness.effort import effort_config
from harness.pricing import is_priced

FAKE_DEVIN = """#!/usr/bin/env python3
import json, os, sqlite3, sys, time

args = sys.argv[1:]
mode = "__MODE__"
version = "__VERSION__"
if "--version" in args or "-V" in args:
    print(f"devin {version} (b98cc431)")
    sys.exit(0)
if args[:2] == ["auth", "status"]:
    print("Logged in (via Devin).")
    print("User: Test Person")
    sys.exit(0)
if args[:2] == ["models", "list"]:
    print(json.dumps({"families": [
        {"family_label": "SWE-2", "family_uid": "swe-2", "aliases": ["swe"], "variants": [
            {"model_uid": "swe-2-high", "label": "SWE-2 High", "cost_tier": "Free"},
            {"model_uid": "swe-2-medium", "label": "SWE-2 Medium", "cost_tier": "Free"},
            {"model_uid": "swe-2-max", "label": "SWE-2 Max", "cost_tier": "Free"},
        ]},
        {"family_label": "Claude Fable 5.1", "family_uid": "claude-fable-5-1", "aliases": [], "variants": [
            {"model_uid": "claude-fable-5-1-xhigh", "label": "Claude Fable 5.1 XHigh",
             "cost_tier": "High cost", "cost_summary": "$5 / 1M Input"},
        ]},
    ]}))
    sys.exit(0)
if "-p" not in args:
    print("unsupported", file=sys.stderr)
    sys.exit(1)
for forbidden in ("DEVIN_MODEL", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"):
    if forbidden in os.environ:
        print(f"leaked env {forbidden}", file=sys.stderr)
        sys.exit(9)
config_arg = next(a for a in args if a.startswith("--config="))
config = json.load(open(config_arg.split("=", 1)[1]))
model = args[args.index("--model") + 1]
assert config["agent"]["model"] == model, "config/model mismatch"
assert config["auto_update"] is False
assert config["respect_workspace_trust"] is False
assert "--respect-workspace-trust=false" in args
assert args[args.index("--permission-mode") + 1] == "dangerous"
prompt = open(args[args.index("--prompt-file") + 1]).read()
assert prompt.startswith("# Issue"), prompt[:40]
assert os.environ["XDG_CONFIG_HOME"] in config_arg, "config must live in the run's config home"
cwd = os.getcwd()
if mode == "limit":
    print("Usage limit reached. Request more usage at https://app.devin.ai/settings", file=sys.stderr)
    sys.exit(1)
if mode == "auth":
    print("Not authorized: please log in with `devin auth login`", file=sys.stderr)
    sys.exit(1)
served = "swe-2-medium" if mode == "wrong-model" else model
with open(os.path.join(cwd, "hello.py"), "w") as f:
    f.write('print("hello from vulcanbench")\\n')
export_arg = next((a for a in args if a.startswith("--export=")), None)
if export_arg:
    tools = ["edit", "exec", "grep", "read", "write"]
    if mode == "web-leak":
        tools.append("webfetch")
    json.dump({"schema_version": "ATIF-v1.7", "session_id": "brisk-otter",
               "agent": {"name": "devin", "version": version, "model_name": model,
                         "tool_definitions": [{"type": "function", "function": {"name": n}} for n in tools]},
               "steps": [], "final_metrics": {}}, open(export_arg.split("=", 1)[1], "w"))
db_dir = os.path.join(os.environ["XDG_DATA_HOME"], "devin", "cli")
os.makedirs(db_dir, exist_ok=True)
con = sqlite3.connect(os.path.join(db_dir, "sessions.db"))
con.executescript('''
create table if not exists sessions (id text primary key, working_directory text, backend_type text,
  model text, agent_mode text, created_at integer, last_activity_at integer, title text,
  main_chain_id integer, shell_last_seen_index integer, cogs_json text, workspace_dirs text,
  hidden integer default 0, metadata text);
create table if not exists message_nodes (row_id integer primary key autoincrement, session_id text,
  node_id integer, parent_node_id integer, chat_message text, created_at integer, metadata text);
create table if not exists tool_call_state (session_id text, tool_call_id text, tool_call_json text,
  tool_call_update_json text, primary key (session_id, tool_call_id));
''')
now = int(time.time())
sid = "brisk-otter"
con.execute("insert into sessions values (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (sid, cwd, "windsurf", model, "bypass", now, now, "fix", 1, 0, "[]", None, 0,
             json.dumps({"total_credit_cost": 3, "total_acu_cost": 0.25})))
def node(i, parent, msg):
    con.execute("insert into message_nodes (session_id, node_id, parent_node_id, chat_message, created_at, metadata) values (?,?,?,?,?,?)",
                (sid, i, parent, json.dumps(msg), now, None))
def receipt(rid, inp, out, cached):
    return {"request_id": rid, "generation_model": served,
            "metrics": {"input_tokens": inp, "output_tokens": out, "cache_read_tokens": cached,
                        "cache_creation_tokens": None}}
node(0, None, {"message_id": "u1", "role": "user", "content": prompt, "metadata": {"is_user_input": True}})
a1 = {"message_id": "a1", "role": "assistant", "content": "", "metadata": receipt("r1", 1000, 200, 500),
      "tool_calls": [{"id": "write_0", "name": "write", "kind": "function",
                      "arguments": {"file_path": os.path.join(cwd, "hello.py"), "content": "x"}}]}
node(1, 0, a1)
node(2, 1, a1)  # the CLI stores each assistant message twice with identical receipts
node(3, 2, {"message_id": "t1", "role": "tool", "tool_call_id": "write_0", "content": "Wrote hello.py",
            "metadata": {"metrics": None}})
a2 = {"message_id": "a2", "role": "assistant", "content": "", "metadata": receipt("r2", 100, 50, 0),
      "tool_calls": [{"id": "exec_1", "name": "exec", "kind": "function",
                      "arguments": {"command": "cd " + cwd + " && python hello.py"}}]}
node(4, 3, a2)
node(5, 4, a2)
node(6, 5, {"message_id": "t2", "role": "tool", "tool_call_id": "exec_1",
            "content": "hello from vulcanbench", "metadata": {"metrics": None}})
node(7, 6, {"message_id": "a3", "role": "assistant", "content": "Implemented and tested.",
            "metadata": receipt("r3", 10, 5, 0), "tool_calls": []})
con.execute("insert into tool_call_state values (?,?,?,?)",
            (sid, "write_0", json.dumps({"toolCallId": "write_0", "kind": "edit"}),
             json.dumps({"toolCallId": "write_0", "status": "completed"})))
con.commit()
con.close()
print("Implemented and tested.")
"""


class _Collector:
    def __init__(self) -> None:
        self.events: list[tuple[str, dict[str, Any]]] = []

    def record(self, event_type: str, data: dict[str, Any]) -> None:
        self.events.append((event_type, data))


def _write_fake(bin_dir: Path, *, mode: str = "success", version: str = "3000.10.31") -> Path:
    script = bin_dir / "devin"
    script.write_text(
        FAKE_DEVIN.replace("__MODE__", mode).replace("__VERSION__", version), encoding="utf-8"
    )
    script.chmod(script.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return script


@pytest.fixture
def fake_devin(tmp_path_factory: pytest.TempPathFactory, monkeypatch: pytest.MonkeyPatch) -> Path:
    bin_dir = tmp_path_factory.mktemp("fake-devin-bin")
    script = _write_fake(bin_dir)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    data_home = tmp_path_factory.mktemp("devin-data")
    monkeypatch.setenv("XDG_DATA_HOME", str(data_home))
    creds = data_home / "devin" / "credentials.toml"
    creds.parent.mkdir(parents=True)
    creds.write_text('windsurf_api_key = "ws-secret-never-copied"\n', encoding="utf-8")
    config_home = tmp_path_factory.mktemp("devin-config")
    monkeypatch.setenv("XDG_CONFIG_HOME", str(config_home))
    user_cfg = config_home / "devin" / "config.json"
    user_cfg.parent.mkdir(parents=True)
    user_cfg.write_text(
        json.dumps(
            {"version": 1, "devin": {"org_id": "org-test"}, "agent": {"model": "gpt-5-5-xhigh"}}
        ),
        encoding="utf-8",
    )
    # Must never reach the CLI: model selection travels through the adapter only.
    monkeypatch.setenv("DEVIN_MODEL", "claude-fable-5-1-xhigh")
    monkeypatch.setenv("OPENAI_API_KEY", "sk-test-should-not-reach-devin")
    devin_cli.reset_catalog_cache()
    yield script
    devin_cli.reset_catalog_cache()


def test_spec_detection_and_unpriced() -> None:
    assert is_cli_agent_spec("devin:swe-2")
    assert get_cli_agent_adapter("devin:swe-2").harness_id == "devin"
    # No public per-token price: cost stays None rather than an invented number.
    assert not is_priced("devin:swe-2")


def test_effort_map_composes_catalog_tokens() -> None:
    for requested, sent in {
        "medium": "medium",
        "high": "high",
        "extra-high": "xhigh",
        "max": "max",
        "minimal": "minimal",
    }.items():
        cfg = effort_config("devin", requested)
        assert cfg is not None and cfg.provider_value == sent and cfg.supported


def test_effort_from_uid() -> None:
    assert effort_from_uid("swe-2-high") == "high"
    assert effort_from_uid("claude-opus-5-xhigh-fast") == "xhigh"
    assert effort_from_uid("swe-1-7-lightning") is None


def test_resolve_model_uid_refuses_missing_variants(fake_devin: Path) -> None:
    catalog = devin_cli.devin_model_catalog()
    assert resolve_model_uid("swe-2", "high", catalog) == "swe-2-high"
    assert resolve_model_uid("swe-2-max", None, catalog) == "swe-2-max"
    with pytest.raises(ProviderError, match=r"available: swe-2-high, swe-2-max, swe-2-medium"):
        resolve_model_uid("swe-2", "low", catalog)
    with pytest.raises(ProviderError, match="pass --effort"):
        resolve_model_uid("swe-2", None, catalog)


def test_run_config_pins_model_web_and_updates() -> None:
    cfg = build_run_config(
        {"devin": {"org_id": "org-1"}, "agent": {"model": "other"}}, "swe-2-high", False
    )
    assert cfg["agent"] == {"model": "swe-2-high"}
    assert cfg["devin"] == {"org_id": "org-1"}
    assert cfg["auto_update"] is False
    assert cfg["respect_workspace_trust"] is False
    assert cfg["disabled_tools"] == [
        "webfetch",
        "web_search",
        "browser_preview",
        "close_browser_preview",
    ]
    assert cfg["shell"] == {"setup_complete": True}
    assert "Fetch(*)" in cfg["permissions"]["deny"]
    open_cfg = build_run_config(None, "swe-2-high", True)
    assert "disabled_tools" not in open_cfg and "permissions" not in open_cfg


def test_preflight_ready(fake_devin: Path) -> None:
    pre = devin_preflight()
    assert pre.ready
    assert pre.version == "devin 3000.10.31 (b98cc431)"
    assert pre.auth_mode == "subscription"
    assert "ws-secret" not in json.dumps(pre.as_summary())


def test_preflight_rejects_old_cli(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, fake_devin: Path
) -> None:
    old = _write_fake(tmp_path, version="2026.5.6-1")
    pre = devin_preflight(str(old))
    assert not pre.ready
    assert pre.detail is not None and "older than 3000.10.21" in pre.detail


def test_run_agent_via_devin_subscription(tmp_path: Path, fake_devin: Path) -> None:
    res = run_agent(
        task_id="hello-world",
        model="devin:swe-2",
        output_dir=tmp_path,
        tasks_root=Path("tasks/v1"),
        judges=False,
        sandbox="local",
        effort="high",
    )
    summary = res["summary"]
    assert summary["scores"]["functional"] == 1.0
    assert summary["finished"] is True
    # Three distinct requests (each stored twice): 1000+500 cache + 100 + 10 prompt.
    assert summary["tokens"]["prompt"] == 1610
    assert summary["tokens"]["completion"] == 255
    assert summary["tokens"]["cached_input"] == 500
    assert summary["cost_usd"] is None
    assert summary["economics"]["billing_mode"] == "subscription-included"
    assert summary["economics"]["measurement_quality"]["api_equivalent_cost_usd"] == "unavailable"
    cli = summary["cli_agent"]
    assert cli["harness"] == "devin"
    assert cli["harness_version"] == "devin 3000.10.31 (b98cc431)"
    assert cli["auth_method"] == "subscription"
    assert cli["session_id"] == "brisk-otter"
    assert cli["requested_model"] == "swe-2-high"
    assert cli["reported_model"] == "swe-2-high"
    assert cli["model_identity_confidence"] == "cli-reported"
    assert cli["reported_effort"] == "high"
    assert cli["num_turns"] == 3
    assert summary["effort"]["provider_value"] == "high"
    run_dir = tmp_path / res["run_id"]
    session_dir = run_dir / "devin-session"
    assert (session_dir / "messages.jsonl").exists()
    assert (session_dir / "session.json").exists()
    assert (session_dir / "export.json").exists()
    stream = (run_dir / "cli-agent-stream.jsonl").read_text()
    assert "ws-secret-never-copied" not in stream
    assert '"toolCallId": "write_0"' in stream
    assert stream.count('"toolCallId": "exec_1"') >= 1
    assert ".devin" not in (run_dir / "final.patch").read_text()
    audit = summary["integrity_audit"]
    assert audit["web"]["verdict"] == "no_web"
    assert audit["contaminated"] is False
    trace = (run_dir / "trace.jsonl").read_text()
    assert '"devin_usage"' in trace
    assert '"total_credit_cost": 3' in trace
    assert '"tool_names": ["edit", "exec", "grep", "read", "write"]' in trace


def test_direct_run_records_boundary_and_start_event(tmp_path: Path, fake_devin: Path) -> None:
    collector = _Collector()
    outcome = run_devin_task(
        workspace=tmp_path,
        prompt="# Issue\n\nfix",
        model="swe-2",
        priced_spec="devin:swe-2",
        max_turns=10,
        collector=collector,
        effort="max",
        stream_log_path=tmp_path / "cli-agent-stream.jsonl",
    )
    assert outcome.finished and outcome.requested_model == "swe-2-max"
    assert "web-tools-disabled" in (outcome.execution_boundary or "")
    start = next(d for e, d in collector.events if e == "cli_agent_start")
    assert start["model_uid"] == "swe-2-max"
    assert start["run_config"]["disabled_tools"][:2] == ["webfetch", "web_search"]
    assert start["run_config"]["devin"] == "<org binding>"
    assert "org-test" not in json.dumps(collector.events)


def test_network_flag_keeps_web(tmp_path: Path, fake_devin: Path) -> None:
    outcome = run_devin_task(
        workspace=tmp_path,
        prompt="# Issue\n\nfix",
        model="swe-2-high",
        priced_spec="devin:swe-2-high",
        max_turns=10,
        collector=_Collector(),
        network=True,
        stream_log_path=tmp_path / "cli-agent-stream.jsonl",
    )
    assert "web-allowed" in (outcome.execution_boundary or "")


def test_usage_limit_raises_quota_error(tmp_path: Path, fake_devin: Path) -> None:
    _write_fake(fake_devin.parent, mode="limit")
    with pytest.raises(SubscriptionQuotaError, match="plan limit"):
        run_devin_task(
            workspace=tmp_path,
            prompt="# Issue\n\nfix",
            model="swe-2",
            priced_spec="devin:swe-2",
            max_turns=10,
            collector=_Collector(),
            effort="high",
            stream_log_path=tmp_path / "cli-agent-stream.jsonl",
        )


def test_auth_failure_is_a_provider_error(tmp_path: Path, fake_devin: Path) -> None:
    _write_fake(fake_devin.parent, mode="auth")
    with pytest.raises(ProviderError, match="auth login"):
        run_devin_task(
            workspace=tmp_path,
            prompt="# Issue\n\nfix",
            model="swe-2",
            priced_spec="devin:swe-2",
            max_turns=10,
            collector=_Collector(),
            effort="high",
            stream_log_path=tmp_path / "cli-agent-stream.jsonl",
        )


def test_served_model_mismatch_fails_closed(tmp_path: Path, fake_devin: Path) -> None:
    _write_fake(fake_devin.parent, mode="wrong-model")
    with pytest.raises(ProviderError, match="served 'swe-2-medium'"):
        run_devin_task(
            workspace=tmp_path,
            prompt="# Issue\n\nfix",
            model="swe-2",
            priced_spec="devin:swe-2",
            max_turns=10,
            collector=_Collector(),
            effort="high",
            stream_log_path=tmp_path / "cli-agent-stream.jsonl",
        )


def test_web_tool_still_exposed_fails_closed(tmp_path: Path, fake_devin: Path) -> None:
    _write_fake(fake_devin.parent, mode="web-leak")
    with pytest.raises(ProviderError, match="still exposed web tools \\['webfetch'\\]"):
        run_devin_task(
            workspace=tmp_path,
            prompt="# Issue\n\nfix",
            model="swe-2",
            priced_spec="devin:swe-2",
            max_turns=10,
            collector=_Collector(),
            effort="high",
            stream_log_path=tmp_path / "cli-agent-stream.jsonl",
        )


def test_web_tool_exposed_is_fine_with_network(tmp_path: Path, fake_devin: Path) -> None:
    _write_fake(fake_devin.parent, mode="web-leak")
    outcome = run_devin_task(
        workspace=tmp_path,
        prompt="# Issue\n\nfix",
        model="swe-2",
        priced_spec="devin:swe-2",
        max_turns=10,
        collector=_Collector(),
        effort="high",
        network=True,
        stream_log_path=tmp_path / "cli-agent-stream.jsonl",
    )
    assert outcome.finished


def test_rejects_unenforceable_live_cost_cap(tmp_path: Path, fake_devin: Path) -> None:
    with pytest.raises(ProviderError, match="max-run-cost"):
        run_devin_task(
            workspace=tmp_path,
            prompt="fix",
            model="swe-2",
            priced_spec="devin:swe-2",
            max_turns=10,
            collector=_Collector(),
            effort="high",
            max_run_cost=1.0,
        )
