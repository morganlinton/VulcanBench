import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

from harness.agent import muse_code
from harness.agent.cli_agents import (
    SubscriptionQuotaError,
    _subscription_env,
    get_cli_agent_adapter,
)
from harness.agent.muse_code import (
    MuseCodeAdapter,
    boundary_profile,
    collect_usage,
    pinned_executable,
)
from harness.agent.providers import ProviderError
from harness.effort import effort_config


def test_muse_efforts_and_subscription_env(monkeypatch):
    monkeypatch.setenv("META_API_KEY", "must-not-leak")
    assert "META_API_KEY" not in _subscription_env()
    assert get_cli_agent_adapter("muse-code:muse-spark-1.3").harness_id == "muse-code"
    for effort, sent in [
        ("minimal", "minimal"),
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("extra-high", "xhigh"),
        ("max", "max"),
        ("ultra", "ultra"),
    ]:
        config = effort_config("muse-code", effort)
        assert config.supported and config.provider_value == sent


def test_usage_deduplicates_mirrors_not_children(tmp_path):
    def event(record_id, kind="model_completed", model="muse-spark-1.3"):
        return {
            "id": record_id,
            "payload": {
                "run_id": "parent",
                "source_run_record_id": record_id,
                "event": {
                    "kind": kind,
                    "model": model,
                    "usage": {
                        "input_tokens": 100,
                        "output_tokens": 20,
                        "cached_tokens": 30,
                        "reasoning_tokens": 5,
                    },
                },
            },
        }

    main = tmp_path / "main.jsonl"
    child = tmp_path / "child.jsonl"
    main.write_text(
        "\n".join(json.dumps(r) for r in [event("a"), event("b", "goal_usage_attribution")])
    )
    child.write_text("\n".join(json.dumps(r) for r in [event("a"), event("c")]))
    assert collect_usage([main, child], "muse-spark-1.3") == {
        "input_tokens": 200,
        "output_tokens": 40,
        "cached_tokens": 60,
        "reasoning_tokens": 10,
        "calls": 2,
    }
    child.write_text(json.dumps(event("c", model="muse-spark-1.3-contributor")))
    with pytest.raises(ProviderError, match="mismatch"):
        collect_usage([main, child], "muse-spark-1.3")


def receipt(run="parent", record="1", tokens=100):
    return {
        "id": record,
        "payload": {
            "run_id": run,
            "source_run_record_id": record,
            "event": {
                "kind": "model_completed",
                "model": "muse-spark-1.3-contributor",
                "usage": {
                    "input_tokens": tokens,
                    "output_tokens": 20,
                    "cached_tokens": 30,
                    "reasoning_tokens": 5,
                },
            },
        },
    }


def test_record_id_collisions_across_child_runs_and_mirrors(tmp_path):
    first, second = tmp_path / "a.jsonl", tmp_path / "b.jsonl"
    first.write_text(json.dumps(receipt()) + "\n" + json.dumps(receipt("child", tokens=200)))
    second.write_text(json.dumps(receipt("child", tokens=200)))
    expected = dict(
        input_tokens=300, output_tokens=40, cached_tokens=60, reasoning_tokens=10, calls=2
    )
    for paths in ([first, second], [second, first]):
        assert collect_usage(paths, "muse-spark-1.3-contributor") == expected
    second.write_text(json.dumps(receipt("child", tokens=201)))
    with pytest.raises(ProviderError, match="Conflicting"):
        collect_usage([first, second], "muse-spark-1.3-contributor")


def test_ambiguous_run_identity_rejected(tmp_path):
    row = receipt()
    del row["payload"]["run_id"]
    path = tmp_path / "session.jsonl"
    path.write_text(json.dumps(row))
    with pytest.raises(ProviderError, match="run identity"):
        collect_usage([path], "muse-spark-1.3-contributor")


def test_contributor_max_rejected_before_any_process():
    with pytest.raises(ProviderError, match="Standard-only"):
        MuseCodeAdapter().run_task(
            workspace="unused",
            prompt="unused",
            model="muse-spark-1.3-contributor",
            priced_spec="unused",
            max_turns=1,
            collector=None,
            effort="max",
            timeout_s=1,
        )


@pytest.mark.parametrize("value", [-1, 1.5, "100", True, None])
def test_invalid_receipts_fail_closed(tmp_path, value):
    row = receipt()
    row["payload"]["event"]["usage"]["input_tokens"] = value
    path = tmp_path / "session.jsonl"
    path.write_text(json.dumps(row))
    with pytest.raises(ProviderError, match="token count"):
        collect_usage([path], "muse-spark-1.3-contributor")


def test_retained_receipt_and_plain_mirror_count_once(tmp_path):
    path = tmp_path / "session.jsonl"
    path.write_text(
        json.dumps(
            {
                "retained_frame": "session_permission_transaction",
                "children": [{"record_json": json.dumps(receipt())}],
            }
        )
        + "\n"
        + json.dumps(receipt())
    )
    assert collect_usage([path], "muse-spark-1.3-contributor")["calls"] == 1


def test_pin_required_and_rechecked(tmp_path, monkeypatch):
    binary = tmp_path / "muse-bin-test"
    binary.write_bytes(b"fake binary, never executed")
    binary.chmod(0o700)
    monkeypatch.setenv("VULCANBENCH_MUSE_BINARY", str(binary))
    monkeypatch.setenv("VULCANBENCH_MUSE_SHA256", hashlib.sha256(binary.read_bytes()).hexdigest())
    assert pinned_executable()[0] == binary.resolve()
    binary.write_bytes(b"changed")
    with pytest.raises(ProviderError, match="SHA256 mismatch"):
        pinned_executable()
    binary.write_bytes(b"#!/bin/sh\nexit 0\n")
    with pytest.raises(ProviderError, match="launcher"):
        pinned_executable()


def test_preflight_reports_missing_pin_without_running_launcher(monkeypatch):

    monkeypatch.delenv("VULCANBENCH_MUSE_BINARY", raising=False)
    checked = MuseCodeAdapter().preflight()
    assert not checked.ready and "VULCANBENCH_MUSE_BINARY" in checked.detail


@pytest.mark.skipif(sys.platform != "darwin", reason="macOS kernel sandbox")
def test_real_sandbox_scratch_and_stale_temp_isolation(tmp_path):
    workspace, scratch = tmp_path / "workspace", tmp_path / "scratch"
    workspace.mkdir()
    scratch.mkdir()
    (scratch / "tmp").mkdir()
    profile = tmp_path / "test.sb"
    profile.write_text(boundary_profile(workspace, scratch))
    with tempfile.TemporaryDirectory(prefix="vb-muse-canary-", dir="/private/tmp") as other:
        stale = Path(other) / "stale.txt"
        stale.write_text("synthetic isolation canary")
        sibling = tmp_path / "other-run.txt"
        sibling.write_text("another synthetic canary")
        (workspace / "stale-link").symlink_to(stale)
        (workspace / "outside-link").symlink_to(sibling)
        # No provider call. Exercise real subprocesses, compiler and temp tools.
        code = """
set -eu
printf valid > "$TMPDIR/probe.txt"
test "$(cat "$TMPDIR/probe.txt")" = valid
mktemp "$TMPDIR/probe.XXXXXX" > scratch-name.txt
printf 'int main(void) { return 0; }' > main.c
clang main.c -o main
./main
if cat stale-link >/dev/null 2>&1; then exit 71; fi
if cat outside-link >/dev/null 2>&1; then exit 72; fi
if printf bad > outside-link 2>/dev/null; then exit 73; fi
if cat "$VB_PROTECTED_REPO/README.md" >/dev/null 2>&1; then exit 74; fi
"""
        proc = subprocess.run(
            ["/usr/bin/sandbox-exec", "-f", str(profile), "/bin/sh", "-c", code],
            cwd=workspace,
            env={
                **_subscription_env(),
                "TMPDIR": str((scratch / "tmp").resolve()),
                "VB_PROTECTED_REPO": str(Path(__file__).resolve().parents[1]),
            },
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        assert proc.returncode == 0, proc.stderr
        assert sibling.read_text() == "another synthetic canary"
        assert Path((workspace / "scratch-name.txt").read_text().strip()).is_relative_to(
            scratch.resolve()
        )


class _Collector:
    def __init__(self):
        self.events = []

    def record(self, name, payload):
        self.events.append((name, payload))


class _FakeProc:
    """Stand-in for the sandboxed Muse process: streams events, writes a session log."""

    def __init__(self, cmd, env, stdout_lines, stderr_text, returncode, session_rows):
        self.cmd, self.env, self.pid, self.returncode = cmd, env, 4242, returncode
        self._done = False
        session = Path(env["XDG_DATA_HOME"]) / "muse/sessions/run-1"
        session.mkdir(parents=True)
        (session / "session.jsonl").write_text("\n".join(json.dumps(r) for r in session_rows))
        self.stdout = iter(stdout_lines)
        self.stderr = iter([stderr_text] if stderr_text else [])

    def wait(self):
        self._done = True
        return self.returncode

    def poll(self):
        return self.returncode if self._done else None


def _pin_fake_binary(tmp_path, monkeypatch):
    binary = tmp_path / "muse-bin"
    binary.write_bytes(b"fake binary, never executed")
    binary.chmod(0o700)
    monkeypatch.setenv("VULCANBENCH_MUSE_BINARY", str(binary))
    monkeypatch.setenv("VULCANBENCH_MUSE_SHA256", hashlib.sha256(binary.read_bytes()).hexdigest())
    config = tmp_path / "config"
    (config / "muse").mkdir(parents=True, exist_ok=True)
    (config / "muse/auth.json").write_text("{}")
    monkeypatch.setenv("XDG_CONFIG_HOME", str(config))
    monkeypatch.setattr(muse_code, "_version", lambda executable: "9.9.9")
    workspace = tmp_path / "workspace"
    workspace.mkdir(exist_ok=True)
    return workspace


def _run(
    tmp_path,
    monkeypatch,
    *,
    stdout_lines,
    stderr_text="",
    returncode=0,
    session_rows=(),
    model="muse-spark-1.3",
    **kwargs,
):
    workspace = _pin_fake_binary(tmp_path, monkeypatch)
    launched = {}

    def fake_popen(cmd, **popen_kwargs):
        launched["cmd"], launched["env"] = cmd, popen_kwargs["env"]
        return _FakeProc(
            cmd, popen_kwargs["env"], stdout_lines, stderr_text, returncode, session_rows
        )

    monkeypatch.setattr(muse_code.subprocess, "Popen", fake_popen)
    collector = _Collector()
    stream_log = tmp_path / "run/stream.jsonl"
    stream_log.parent.mkdir(exist_ok=True)
    outcome = MuseCodeAdapter().run_task(
        workspace=workspace,
        prompt="Replace the retired engine.",
        model=model,
        priced_spec="unused",
        max_turns=40,
        collector=collector,
        stream_log_path=stream_log,
        timeout_s=30,
        effort="high",
        **kwargs,
    )
    return outcome, collector, launched, stream_log


def test_run_task_archives_session_and_counts_usage(tmp_path, monkeypatch):
    rows = [receipt(record="1"), receipt(record="2", tokens=50)]
    for row in rows:
        row["payload"]["event"]["model"] = "muse-spark-1.3"
    stdout = [
        "not json\n",
        json.dumps({"payload_type": "run.output.delta", "payload": {"text": "hello"}}) + "\n",
        json.dumps({"payload_type": "run.terminal.completed", "payload": {"terminal": "completed"}})
        + "\n",
    ]
    outcome, collector, launched, stream_log = _run(
        tmp_path, monkeypatch, stdout_lines=stdout, session_rows=rows
    )
    assert outcome.finished and not outcome.timed_out and outcome.subtype == "completed"
    assert (outcome.prompt_tokens, outcome.completion_tokens, outcome.num_turns) == (150, 40, 2)
    assert outcome.reported_model == "muse-spark-1.3"
    assert outcome.model_identity_confidence == "provider-reported"
    cmd = launched["cmd"]
    assert cmd[0] == "/usr/bin/sandbox-exec" and "--disable-web-tools" in cmd
    assert cmd[cmd.index("--reasoning-effort") + 1] == "high"
    assert cmd[-1].endswith("Replace the retired engine.")
    assert launched["env"]["MUSE_NO_AUTO_UPDATE"] == "1"
    assert "META_API_KEY" not in launched["env"]
    names = [name for name, _ in collector.events]
    assert (
        names[0] == "cli_agent_start"
        and "assistant_text" in names
        and names[-1] == "cli_agent_result"
    )
    start = collector.events[0][1]
    assert start["argv"][-1] == "<task prompt>" and start["binary_sha256"] == pinned_executable()[1]
    assert (
        json.loads(stream_log.read_text().splitlines()[-1])["payload_type"]
        == "run.terminal.completed"
    )
    archive = stream_log.parent / "muse-session-logs"
    assert (archive / "run-1/session.jsonl").exists()
    assert "(deny file-write*)" in (archive / "boundary.sb").read_text()


def test_run_task_maps_usage_limit_to_quota_error(tmp_path, monkeypatch):
    with pytest.raises(SubscriptionQuotaError, match="subscription limit"):
        _run(
            tmp_path,
            monkeypatch,
            stdout_lines=[],
            stderr_text="usage limit reached\n",
            returncode=1,
        )


def test_run_task_rejects_completion_without_receipts_and_bad_options(tmp_path, monkeypatch):
    stdout = [
        json.dumps({"payload_type": "run.terminal.completed", "payload": {"terminal": "completed"}})
    ]
    with pytest.raises(ProviderError, match="auditable usage"):
        _run(tmp_path, monkeypatch, stdout_lines=stdout)
    with pytest.raises(ProviderError, match="live cost cap"):
        _run(tmp_path, monkeypatch, stdout_lines=stdout, max_run_cost=1.0)
    with pytest.raises(ProviderError, match="wall-clock"):
        MuseCodeAdapter().run_task(
            workspace=tmp_path,
            prompt="x",
            model="muse-spark-1.3",
            priced_spec="unused",
            max_turns=1,
            collector=None,
            effort="high",
            timeout_s=0,
        )


@pytest.mark.parametrize(
    ("spec", "cli_model", "mode"),
    [
        ("muse-wonder-1.0-contributor", "muse-wonder-1.0", "on"),
        ("muse-wonder-1.0", "muse-wonder-1.0", "off"),
        ("muse-wonder-full-1.0-contributor", "muse-wonder-full-1.0", "on"),
    ],
)
def test_wonder_passes_explicit_contributor_mode(tmp_path, monkeypatch, spec, cli_model, mode):
    rows = [receipt(record="1")]
    rows[0]["payload"]["event"]["model"] = cli_model
    stdout = [
        json.dumps({"payload_type": "run.terminal.completed", "payload": {"terminal": "completed"}})
    ]
    outcome, collector, launched, _ = _run(
        tmp_path, monkeypatch, stdout_lines=stdout, session_rows=rows, model=spec
    )
    cmd = launched["cmd"]
    assert cmd[cmd.index("--model") + 1] == cli_model
    assert cmd[cmd.index("--contributor-mode") + 1] == mode
    assert outcome.finished and outcome.reported_model == spec
    start = collector.events[0][1]
    assert start["contributor_mode"] == mode and start["cli_model"] == cli_model
    assert start["pricing_tier"] == ("contributor" if mode == "on" else "standard")


def test_spark_keeps_model_id_contributor_without_flag(tmp_path, monkeypatch):
    rows = [receipt(record="1")]
    stdout = [
        json.dumps({"payload_type": "run.terminal.completed", "payload": {"terminal": "completed"}})
    ]
    _, _, launched, _ = _run(
        tmp_path,
        monkeypatch,
        stdout_lines=stdout,
        session_rows=rows,
        model="muse-spark-1.3-contributor",
    )
    assert "--contributor-mode" not in launched["cmd"]


def test_wonder_contributor_max_rejected_before_any_process():
    with pytest.raises(ProviderError, match="Standard-only"):
        MuseCodeAdapter().run_task(
            workspace="unused",
            prompt="unused",
            model="muse-wonder-1.0-contributor",
            priced_spec="unused",
            max_turns=1,
            collector=None,
            effort="max",
            timeout_s=1,
        )
