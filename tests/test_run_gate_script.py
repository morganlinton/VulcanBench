"""scripts/frontier-v5/run_gate.sh against a fake harbor.

The fake harbor, docker, git and claude live in a throwaway HOME's
``.local/bin`` (the script puts that first on PATH), so nothing here touches
the real tools, credentials or Docker daemon. What is checked: the status
file's phases and exit codes, that one SIGINT to the script reaches harbor
and ends the run within seconds, the early reject, the infrastructure guard,
``--refs`` and the pre-flight-only mode.
"""

from __future__ import annotations

import json
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts" / "frontier-v5" / "run_gate.sh"

pytestmark = pytest.mark.skipif(shutil.which("bash") is None, reason="needs bash")

FAKE_HARBOR = r"""#!/usr/bin/env python3
import json, os, signal, sys, time
args = sys.argv[1:]
if args == ["--version"]:
    print("fake-harbor"); sys.exit(0)
job = args[args.index("--job-name") + 1]
out = args[args.index("-o") + 1]
task = args[args.index("-p") + 1]
mode = os.environ.get("FAKE_HARBOR_MODE", "finish")
jd = os.path.join(out, job)
os.makedirs(jd)
with open(os.path.join(jd, "argv.json"), "w") as fh:
    json.dump({"argv": args, "env": {k: v for k, v in os.environ.items() if k.startswith(("CODEX_", "CLAUDE_"))}}, fh)
with open(os.path.join(jd, "config.json"), "w") as fh:
    json.dump({"job_name": job, "n_attempts": int(args[args.index("-k") + 1]),
               "agents": [{"name": args[args.index("-a") + 1], "model_name": args[args.index("-m") + 1]}],
               "tasks": [{"path": task}]}, fh)

def on_int(*_):
    with open(os.path.join(jd, "cancelled"), "w") as fh:
        fh.write("sigint")
    sys.exit(130)

signal.signal(signal.SIGINT, on_int)
name = os.path.basename(task)[:32]

def trial(i, reward, infra=False, agent_log=None):
    td = os.path.join(jd, "%s__t%d" % (name, i))
    os.makedirs(os.path.join(td, "verifier")); os.makedirs(os.path.join(td, "agent"))
    with open(os.path.join(td, "verifier", "reward.json"), "w") as fh:
        json.dump({"reward": reward}, fh)
    with open(os.path.join(td, "agent", "codex.txt"), "w") as fh:
        fh.write(agent_log or ("hit your usage limit\n" if infra else '{"type":"item.completed"}\n'))
    with open(os.path.join(td, "result.json"), "w") as fh:
        json.dump({"started_at": "2026-10-29T12:00:00", "finished_at": "2026-10-29T12:01:00"}, fh)

if mode == "finish":
    for i in range(3):
        trial(i, 0)
    with open(os.path.join(jd, "result.json"), "w") as fh:
        json.dump({"finished_at": "2026-10-29T12:03:00"}, fh)
    sys.exit(0)
if mode == "solve2":
    trial(0, 1); trial(1, 1)
elif mode == "infra":
    trial(0, 0, infra=True)
elif mode == "apps":
    trial(0, 1, agent_log='{"type":"item_completed","item":{"type":"McpToolCall","server":"codex_apps"}}\n')
elif mode == "ignore-int":
    signal.signal(signal.SIGINT, signal.SIG_IGN)
time.sleep(600)
"""

FAKE_DOCKER = """#!/bin/sh
echo "$*" >> "$FAKE_LOG/docker.log"
exit 0
"""

FAKE_GIT = """#!/bin/sh
echo abc1234
"""

# Prints what the one-turn probe prints: a rate_limit_event line among others.
FAKE_CLAUDE = r"""#!/usr/bin/env python3
import json, os
u = float(os.environ.get("FAKE_UTIL", "0.5"))
print(json.dumps({"type": "system", "subtype": "init"}))
print(json.dumps({"type": "rate_limit_event", "rate_limit_info": {
    "status": os.environ.get("FAKE_RL_STATUS", "allowed"),
    "unifiedWindows": {"five_hour": {"utilization": u / 2}, "seven_day": {"utilization": u}}}}))
print(json.dumps({"type": "result", "result": "ok"}))
"""


class Gate:
    def __init__(self, tmp_path: Path) -> None:
        self.home = tmp_path / "home"
        self.bin = self.home / ".local" / "bin"
        self.bin.mkdir(parents=True)
        self.root = tmp_path / "repo"
        (self.root / "scripts" / "frontier-v5").mkdir(parents=True)
        self.script = self.root / "scripts" / "frontier-v5" / "run_gate.sh"
        shutil.copy(SCRIPT, self.script)
        shutil.copy(SCRIPT.parent / "codex-gate.toml", self.script.parent / "codex-gate.toml")
        self.task = "demo-task-with-a-fairly-long-name-indeed"
        task_dir = self.root / "tasks" / "frontier-v5" / self.task
        task_dir.mkdir(parents=True)
        (task_dir / "task.toml").write_text('[metadata]\nvulcanbench_language = "python"\n')
        self.out = tmp_path / "gate"
        self.log = tmp_path / "fakelog"
        self.log.mkdir()
        for name, body in (
            ("harbor", FAKE_HARBOR),
            ("docker", FAKE_DOCKER),
            ("git", FAKE_GIT),
            ("claude", FAKE_CLAUDE),
        ):
            p = self.bin / name
            p.write_text(body)
            p.chmod(0o755)
        (self.home / ".codex").mkdir()
        (self.home / ".codex" / "auth.json").write_text("{}")
        envd = self.home / ".config" / "vulcanbench"
        envd.mkdir(parents=True)
        (envd / "claude-gate.env").write_text("CLAUDE_CODE_OAUTH_TOKEN=not-a-real-token\n")

    def env(self, **extra: str) -> dict[str, str]:
        py_dir = str(Path(sys.executable).parent)
        env = {
            "HOME": str(self.home),
            "PATH": os.pathsep.join([py_dir, "/usr/bin", "/bin"]),
            "GATE_OUT": str(self.out),
            "GATE_POLL_S": "0.2",
            "FAKE_LOG": str(self.log),
            "LANG": "C",
        }
        env.update(extra)
        return env

    def run(self, *args: str, **extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", str(self.script), *args],
            env=self.env(**extra),
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )

    def start(self, *args: str, **extra: str) -> subprocess.Popen[str]:
        return subprocess.Popen(
            ["bash", str(self.script), *args],
            env=self.env(**extra),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )

    def status(self, job: str) -> dict[str, object]:
        data: dict[str, object] = json.loads((self.out / f"{job}.status.json").read_text())
        return data

    def wait_phase(self, job: str, phase: str, timeout: float = 20.0) -> dict[str, object]:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                st = self.status(job)
            except (OSError, json.JSONDecodeError):
                st = {}
            if st.get("phase") == phase:
                return st
            time.sleep(0.05)
        raise AssertionError(f"{job} never reached phase {phase}: {st}")

    def docker_calls(self) -> list[str]:
        p = self.log / "docker.log"
        return p.read_text().splitlines() if p.exists() else []


@pytest.fixture
def gate(tmp_path: Path) -> Gate:
    return Gate(tmp_path)


def test_refs_prints_one_json_line_per_reference(gate: Gate) -> None:
    r = gate.run("--refs")
    assert r.returncode == 0, r.stderr
    refs = [json.loads(line) for line in r.stdout.splitlines()]
    assert [x["name"] for x in refs] == ["astra", "fable", "opus"]
    assert refs[2] == {
        "name": "opus",
        "agent": "claude-code",
        "model": "claude-opus-5-5",
        "version": "2.1.302",
    }


def test_normal_run_writes_phases_and_keeps_stdout_shape(gate: Gate) -> None:
    r = gate.run("astra", gate.task)
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"astra-{gate.task}"
    st = gate.status(job)
    assert st["phase"] == "done" and st["exit_code"] == 0 and st["stop_reason"] is None
    assert st["solves"] == 0 and st["agent"] == "codex" and st["model"] == "gpt-6-astra"
    assert st["repo"] == "abc1234" and isinstance(st["harbor_pid"], int)
    lines = r.stdout.splitlines()
    assert lines[0].startswith(f"=== {gate.task} ") and "concurrency=2" in lines[0]
    assert sum(1 for line in lines if line.startswith("  ")) == 3
    assert any(line.startswith(f"--- {gate.task}: 0 solve(s)") for line in lines)
    assert lines[-1].startswith("=== DONE")
    # The sweep runs after every job, and harbor got the auth env, not the script's.
    assert any(
        "ps -aq --filter name=^demo-task-with-a-fairly-long-nam__" in c for c in gate.docker_calls()
    )
    argv = json.loads((gate.out / "jobs" / job / "argv.json").read_text())
    assert argv["env"] == {"CODEX_AUTH_JSON_PATH": str(gate.home / ".codex" / "auth.json")}
    assert (
        "--effort" in argv["argv"] and argv["argv"][argv["argv"].index("--effort") + 1] == "medium"
    )
    assert argv["argv"][argv["argv"].index("-k") + 1] == "3"
    # Provider-side web tools bypass the task's network allowlist, so they are off.
    aks = [argv["argv"][i + 1] for i, a in enumerate(argv["argv"]) if a == "--ak"]
    assert "disable_web_search=true" in aks
    # Codex also gets the gate config that turns off connected apps and plugins.
    cfg = [a.split("=", 1)[1] for a in aks if a.startswith("config=")]
    assert cfg and Path(cfg[0]).name == "codex-gate.toml" and Path(cfg[0]).is_file()
    assert "apps = false" in Path(cfg[0]).read_text()


def test_sigint_to_the_script_stops_harbor_within_seconds(gate: Gate) -> None:
    job = f"astra-{gate.task}"
    proc = gate.start("astra", gate.task, FAKE_HARBOR_MODE="hang")
    try:
        st = gate.wait_phase(job, "running")
        assert st["pid"] == proc.pid
        t0 = time.monotonic()
        os.kill(proc.pid, signal.SIGINT)
        out, _ = proc.communicate(timeout=30)
        assert time.monotonic() - t0 < 15, out
    finally:
        if proc.poll() is None:
            proc.kill()
    assert proc.returncode == 130, out
    assert (gate.out / "jobs" / job / "cancelled").read_text() == "sigint"
    st = gate.status(job)
    assert st["phase"] == "stopped" and st["stop_reason"] == "signal-INT"
    assert st["exit_code"] == 130
    assert "stop requested (SIGINT)" in out
    assert gate.docker_calls(), "the sandbox sweep must run on a signal stop"


def test_sigterm_is_handled_like_sigint(gate: Gate) -> None:
    job = f"astra-{gate.task}"
    proc = gate.start("astra", gate.task, FAKE_HARBOR_MODE="hang")
    try:
        gate.wait_phase(job, "running")
        os.kill(proc.pid, signal.SIGTERM)
        out, _ = proc.communicate(timeout=30)
    finally:
        if proc.poll() is None:
            proc.kill()
    assert proc.returncode == 130, out
    assert gate.status(job)["stop_reason"] == "signal-TERM"


def test_early_reject_after_two_solves(gate: Gate) -> None:
    r = gate.run("astra", gate.task, FAKE_HARBOR_MODE="solve2")
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"astra-{gate.task}"
    st = gate.status(job)
    assert st["phase"] == "done" and st["stop_reason"] == "early-reject" and st["solves"] == 2
    assert st["exit_code"] == 130  # harbor's own exit after SIGINT
    assert "two solves, early reject" in r.stdout
    assert (gate.out / "jobs" / job / "cancelled").exists()


def test_infrastructure_failure_stops_the_run(gate: Gate) -> None:
    r = gate.run("astra", gate.task, FAKE_HARBOR_MODE="infra")
    assert r.returncode == 1, r.stdout + r.stderr
    job = f"astra-{gate.task}"
    st = gate.status(job)
    assert st["phase"] == "infra_failed"
    assert str(st["stop_reason"]).startswith("infra:demo-task-with-a-fairly-long-nam__t0")
    assert (gate.out / "jobs" / job / "cancelled").exists()


def test_provider_side_tool_use_is_an_infrastructure_failure(gate: Gate) -> None:
    # A solve that reached the account's connected apps is not a gate result.
    r = gate.run("astra", gate.task, FAKE_HARBOR_MODE="apps")
    assert r.returncode == 1, r.stdout + r.stderr
    st = gate.status(f"astra-{gate.task}")
    assert st["phase"] == "infra_failed"
    assert str(st["stop_reason"]).startswith("infra:demo-task-with-a-fairly-long-nam__t0")


def test_dedicated_gate_login_is_preferred(gate: Gate) -> None:
    gate_auth = gate.home / ".config" / "vulcanbench" / "codex-gate" / "auth.json"
    gate_auth.parent.mkdir(parents=True)
    gate_auth.write_text("{}")
    r = gate.run("astra", gate.task)
    assert r.returncode == 0, r.stdout + r.stderr
    argv = json.loads((gate.out / "jobs" / f"astra-{gate.task}" / "argv.json").read_text())
    assert argv["env"] == {"CODEX_AUTH_JSON_PATH": str(gate_auth)}


def test_refusals_before_harbor_starts(gate: Gate) -> None:
    assert gate.run("nope", gate.task).returncode == 2
    assert gate.run("astra", "missing-task").returncode == 2
    assert gate.run("astra", gate.task, GATE_ATTEMPTS="4").returncode == 2
    (gate.out / "jobs" / f"astra-{gate.task}").mkdir(parents=True)
    r = gate.run("astra", gate.task)
    assert r.returncode == 2 and "exists; move it aside" in r.stderr


def test_preflight_only_reports_usage_without_starting(gate: Gate) -> None:
    r = gate.run("opus", gate.task, GATE_PREFLIGHT_ONLY="1", FAKE_UTIL="0.5")
    assert r.returncode == 0, r.stdout + r.stderr
    d = json.loads(r.stdout.strip().splitlines()[-1])
    assert d == {
        "model": "claude-opus-5-5",
        "status": "allowed",
        "max_utilization": 0.5,
        "windows": {"five_hour": 0.25, "seven_day": 0.5},
        "max_allowed": 0.85,
        "would_start": True,
    }
    assert not (gate.out / "jobs").exists() or not list((gate.out / "jobs").iterdir())

    r = gate.run("opus", gate.task, GATE_PREFLIGHT_ONLY="1", FAKE_UTIL="0.9")
    assert r.returncode == 2
    assert json.loads(r.stdout.strip())["would_start"] is False

    r = gate.run("fable", GATE_PREFLIGHT_ONLY="1", FAKE_RL_STATUS="rejected")
    assert r.returncode == 2 and json.loads(r.stdout.strip())["status"] == "rejected"

    r = gate.run("astra", GATE_PREFLIGHT_ONLY="1")
    assert r.returncode == 0
    assert json.loads(r.stdout.strip())["status"] == "not_applicable"


def test_usage_refusal_writes_infra_failed_status(gate: Gate) -> None:
    r = gate.run("opus", gate.task, FAKE_UTIL="0.95")
    assert r.returncode == 2
    st = gate.status(f"opus-{gate.task}")
    assert st["phase"] == "infra_failed" and st["stop_reason"] == "usage" and st["exit_code"] == 2
    assert not (gate.out / "jobs" / f"opus-{gate.task}").exists()
