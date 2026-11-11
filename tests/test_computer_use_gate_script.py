"""scripts/computer-use/run_gate.sh against a fake harbor.

Mirrors tests/test_run_gate_script.py: fake harbor, docker and git live in a
throwaway HOME's ``.local/bin``, so nothing touches real tools, credentials or
Docker. Checked here: the composed ``harbor run`` argv (computer-1, the model
and kwargs from suite.json pins, three attempts, one at a time, absolute
jobs dir, job name), that the API key reaches harbor's environment and never
its argv, the preflight-only mode, the refusals, the status file's key set
and phases, that every attempt runs (no early exit), and the infrastructure
guard.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts" / "computer-use" / "run_gate.sh"

pytestmark = pytest.mark.skipif(shutil.which("bash") is None, reason="needs bash")

EXPECTED_STATUS_KEYS = {
    "job",
    "task",
    "ref",
    "agent",
    "model",
    "effort",
    "pid",
    "harbor_pid",
    "phase",
    "solves",
    "started_at",
    "updated_at",
    "exit_code",
    "stop_reason",
    "repo",
    "job_dir",
    "condition",
    "task_dir",
}

FAKE_HARBOR = r"""#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
if args == ["--version"]:
    print("0.24.0"); sys.exit(0)
job = args[args.index("--job-name") + 1]
out = args[args.index("-o") + 1]
task = args[args.index("-p") + 1]
mode = os.environ.get("FAKE_HARBOR_MODE", "solve3")
jd = os.path.join(out, job)
os.makedirs(jd)
with open(os.path.join(jd, "argv.json"), "w") as fh:
    json.dump({"argv": args, "has_key": "ANTHROPIC_API_KEY" in os.environ,
               "env_names": sorted(k for k in os.environ if k.startswith(("CLAUDE_", "ANTHROPIC_", "CODEX_"))),
               "codex_auth": os.environ.get("CODEX_AUTH_JSON_PATH")}, fh)
with open(os.path.join(jd, "config.json"), "w") as fh:
    json.dump({"job_name": job, "n_attempts": int(args[args.index("-k") + 1]),
               "agents": [{"name": args[args.index("-a") + 1], "model_name": args[args.index("-m") + 1]}],
               "tasks": [{"path": task}]}, fh)
name = os.path.basename(task)[:32]

def trial(i, reward, log='{"turn": 1}\n', logname="computer-1.txt"):
    td = os.path.join(jd, "%s__t%d" % (name, i))
    os.makedirs(os.path.join(td, "verifier")); os.makedirs(os.path.join(td, "agent"))
    with open(os.path.join(td, "verifier", "reward.json"), "w") as fh:
        json.dump({"reward": reward, "guards": 1}, fh)
    with open(os.path.join(td, "agent", logname), "w") as fh:
        fh.write(log)
    with open(os.path.join(td, "result.json"), "w") as fh:
        json.dump({"started_at": "2026-11-06T12:00:00", "finished_at": "2026-11-06T12:01:00"}, fh)

if mode == "solve3":
    for i in range(int(args[args.index("-k") + 1])):
        trial(i, 1)
elif mode == "infra":
    trial(0, 0, log='{"error": {"type": "authentication_error"}}\n')
elif mode == "codex-mcp":
    trial(0, 1, log='{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","server":"computer","tool":"screenshot"}}\n', logname="codex.txt")
elif mode == "codex-shell":
    trial(0, 1, log='{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"ls"}}\n', logname="codex.txt")
with open(os.path.join(jd, "result.json"), "w") as fh:
    json.dump({"finished_at": "2026-11-06T12:03:00"}, fh)
sys.exit(0)
"""

FAKE_DOCKER = """#!/bin/sh
echo "$*" >> "$FAKE_LOG/docker.log"
exit 0
"""

FAKE_GIT = """#!/bin/sh
echo abc1234
"""

# What `codex debug models` prints: a served catalog with the tool keys set.
CATALOG = {
    "models": [
        {
            "slug": "gpt-test-model",
            "apply_patch_tool_type": "freeform",
            "multi_agent_version": "v2",
            "tool_mode": "code_mode_only",
            "shell_type": "unified_exec",
            "model_messages": {"persistent_instructions": "be careful"},
        },
        {"slug": "gpt-other", "apply_patch_tool_type": "freeform"},
    ]
}

FAKE_CODEX = r"""#!/usr/bin/env python3
import json, os, sys
if sys.argv[1:] == ["debug", "models"]:
    print(open(os.environ["FAKE_CATALOG"]).read()); sys.exit(0)
sys.exit(1)
"""

# What the one-turn usage probe prints: a rate_limit_event line among others.
FAKE_CLAUDE = r"""#!/usr/bin/env python3
import json, os
u = float(os.environ.get("FAKE_UTIL", "0.5"))
print(json.dumps({"type": "system", "subtype": "init"}))
print(json.dumps({"type": "rate_limit_event", "rate_limit_info": {
    "status": os.environ.get("FAKE_RL_STATUS", "allowed"),
    "unifiedWindows": {"five_hour": {"utilization": u / 2}, "seven_day": {"utilization": u}}}}))
print(json.dumps({"type": "result", "result": "ok"}))
"""

SUITE = {
    "id": "computer-use-v1",
    "tasks": ["demo-computer-task-with-a-fairly-long-name"],
    "pins": {
        "agent": "computer-1",
        "agent_kwargs": {
            "max_turns": 7,
            "desktop_width": 1024,
            "start_url": "http://127.0.0.1:8000/",
        },
        "references": [
            {
                "name": "claude-code",
                "agent": "claude-code",
                "model": "claude-test-model",
                "version": "9.9.9",
                "disallowed_tools": "Bash,Read,Edit",
            },
            {"name": "codex", "agent": "codex", "model": "gpt-test-model", "version": "8.8.8"},
            {"name": "claude", "agent": "computer-1", "model": "anthropic/test-model"},
        ],
    },
}


class Gate:
    def __init__(self, tmp_path: Path) -> None:
        self.home = tmp_path / "home"
        self.bin = self.home / ".local" / "bin"
        self.bin.mkdir(parents=True)
        self.root = tmp_path / "repo"
        (self.root / "scripts" / "computer-use").mkdir(parents=True)
        self.script = self.root / "scripts" / "computer-use" / "run_gate.sh"
        shutil.copy(SCRIPT, self.script)
        for helper in ("codex-gate.toml", "codex_catalog.py"):
            shutil.copy(SCRIPT.parent / helper, self.script.parent / helper)
        self.task = SUITE["tasks"][0]
        suite_dir = self.root / "tasks" / "computer-use-v1"
        (suite_dir / self.task).mkdir(parents=True)
        (suite_dir / self.task / "task.toml").write_text(
            '[metadata]\nvulcanbench_suite = "computer-use-v1"\n\n'
            '[[environment.mcp_servers]]\nname = "computer"\ntransport = "stdio"\n'
            'command = "python3"\n'
            'args = ["/opt/vbdesk/vb_computer_mcp.py", "--start-url", "http://127.0.0.1:8000/"]\n'
        )
        (suite_dir / self.task / "environment").mkdir()
        (suite_dir / self.task / "environment" / "Dockerfile").write_text(
            'FROM vulcanbench/sandbox:cu-base\nCMD ["sleep", "infinity"]\n'
        )
        (suite_dir / "suite.json").write_text(json.dumps(SUITE))
        self.out = tmp_path / "gate"
        self.log = tmp_path / "fakelog"
        self.log.mkdir()
        for name, body in (
            ("harbor", FAKE_HARBOR),
            ("docker", FAKE_DOCKER),
            ("git", FAKE_GIT),
            ("claude", FAKE_CLAUDE),
            ("codex", FAKE_CODEX),
        ):
            p = self.bin / name
            p.write_text(body)
            p.chmod(0o755)
        self.envd = self.home / ".config" / "vulcanbench"
        self.envd.mkdir(parents=True)
        (self.envd / "computer-use-gate.env").write_text("ANTHROPIC_API_KEY=not-a-real-key\n")
        (self.envd / "claude-gate.env").write_text("CLAUDE_CODE_OAUTH_TOKEN=not-a-real-token\n")
        (self.home / ".codex").mkdir()
        (self.home / ".codex" / "auth.json").write_text(
            '{"tokens": {"access_token": "not-a-real-token"}}'
        )
        self.catalog = tmp_path / "catalog.json"
        self.catalog.write_text(json.dumps(CATALOG))

    def env(self, **extra: str) -> dict[str, str]:
        env = {
            "HOME": str(self.home),
            "PATH": os.pathsep.join([str(Path(sys.executable).parent), "/usr/bin", "/bin"]),
            "GATE_OUT": str(self.out),
            "GATE_POLL_S": "0.2",
            "FAKE_LOG": str(self.log),
            "FAKE_CATALOG": str(self.catalog),
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

    def status(self, job: str) -> dict[str, object]:
        data: dict[str, object] = json.loads((self.out / f"{job}.status.json").read_text())
        return data

    def argv(self, job: str) -> dict[str, object]:
        data: dict[str, object] = json.loads((self.out / "jobs" / job / "argv.json").read_text())
        return data


@pytest.fixture
def gate(tmp_path: Path) -> Gate:
    return Gate(tmp_path)


def test_refs_prints_the_three_references(gate: Gate) -> None:
    r = gate.run("--refs")
    assert r.returncode == 0, r.stderr
    refs = [json.loads(line) for line in r.stdout.splitlines()]
    assert [x["name"] for x in refs] == ["claude-code", "codex", "claude"]
    assert [x["agent"] for x in refs] == ["claude-code", "codex", "computer-1"]


def test_preflight_only_reports_and_launches_nothing(gate: Gate) -> None:
    r = gate.run("claude", gate.task, GATE_PREFLIGHT_ONLY="1")
    assert r.returncode == 0, r.stdout + r.stderr
    line = json.loads(r.stdout.strip().splitlines()[-1])
    assert line["model"] == "anthropic/test-model" and line["would_start"] is True
    assert line["agent_kwargs"] == [
        "max_turns=7",
        "desktop_width=1024",
        "start_url=http://127.0.0.1:8000/",
    ]
    assert not (gate.out / "jobs").exists() or not list((gate.out / "jobs").iterdir())


def test_missing_env_file_is_refused(gate: Gate) -> None:
    (gate.envd / "computer-use-gate.env").unlink()
    r = gate.run("claude", gate.task)
    assert r.returncode == 2 and "ANTHROPIC_API_KEY" in r.stderr
    assert not list((gate.out / "jobs").glob("cu-*"))


def test_unknown_task_is_refused_before_launch(gate: Gate) -> None:
    r = gate.run("claude", "no-such-task")
    assert r.returncode == 2 and "no task" in r.stderr


def test_normal_run_composes_harbor_argv_and_status(gate: Gate) -> None:
    r = gate.run("claude", gate.task)
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"cu-claude-{gate.task}"
    rec = gate.argv(job)
    argv = rec["argv"]
    assert argv[:2] == ["run", "-p"] and argv[2] == str(
        gate.root / "tasks" / "computer-use-v1" / gate.task
    )
    assert argv[argv.index("-a") + 1] == "computer-1"
    assert argv[argv.index("-m") + 1] == "anthropic/test-model"
    aks = [argv[i + 1] for i, a in enumerate(argv) if a == "--ak"]
    assert aks == ["max_turns=7", "desktop_width=1024", "start_url=http://127.0.0.1:8000/"]
    assert argv[argv.index("-k") + 1] == "3" and argv[argv.index("-n") + 1] == "1"
    assert Path(argv[argv.index("-o") + 1]).is_absolute()
    assert argv[argv.index("--job-name") + 1] == job
    assert "--effort" not in argv and "--ae" not in argv and "not-a-real-key" not in " ".join(argv)
    assert rec["has_key"] is True
    st = gate.status(job)
    assert set(st) == EXPECTED_STATUS_KEYS
    assert st["phase"] == "done" and st["exit_code"] == 0 and st["stop_reason"] is None
    assert st["solves"] == 3 and st["agent"] == "computer-1" and st["effort"] == "n/a"
    assert st["condition"] == "pixels+zoom" and st["task_dir"] == argv[2]
    assert st["repo"] == "abc1234" and isinstance(st["harbor_pid"], int)
    lines = r.stdout.splitlines()
    assert lines[0].startswith(f"=== {gate.task} ") and "concurrency=1" in lines[0]
    assert sum(1 for line in lines if line.startswith("  ")) == 3
    assert any(line.startswith(f"--- {gate.task}: 3 solve(s) of 3") for line in lines)
    assert lines[-1].startswith("=== DONE")


def test_model_and_kwargs_overrides(gate: Gate) -> None:
    r = gate.run(
        "claude",
        gate.task,
        CU_GATE_MODEL="anthropic/other",
        CU_GATE_AGENT_KWARGS="max_turns=2",
        GATE_ATTEMPTS="1",
    )
    assert r.returncode == 0, r.stdout + r.stderr
    argv = gate.argv(f"cu-claude-{gate.task}")["argv"]
    assert argv[argv.index("-m") + 1] == "anthropic/other"
    assert [argv[i + 1] for i, a in enumerate(argv) if a == "--ak"] == ["max_turns=2"]
    assert argv[argv.index("-k") + 1] == "1"


def test_infrastructure_failure_stops_the_run(gate: Gate) -> None:
    r = gate.run("claude", gate.task, FAKE_HARBOR_MODE="infra")
    assert r.returncode == 1, r.stdout + r.stderr
    st = gate.status(f"cu-claude-{gate.task}")
    assert st["phase"] == "infra_failed" and str(st["stop_reason"]).startswith("infra:")


def test_claude_code_run_uses_subscription_auth_and_mcp_only(gate: Gate) -> None:
    r = gate.run("claude-code", gate.task)
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"cu-claude-code-{gate.task}"
    rec = gate.argv(job)
    argv = rec["argv"]
    assert argv[argv.index("-a") + 1] == "claude-code"
    assert argv[argv.index("-m") + 1] == "claude-test-model"
    assert argv[argv.index("--effort") + 1] == "medium"
    aks = [argv[i + 1] for i, a in enumerate(argv) if a == "--ak"]
    assert aks == ["version=9.9.9", "disallowed_tools=Bash,Read,Edit", "disable_web_search=true"]
    assert argv[argv.index("--env-file") + 1].endswith("claude-gate.env")
    assert "--ae" not in argv and "not-a-real-token" not in " ".join(argv)
    assert "CLAUDE_FORCE_OAUTH" in rec["env_names"] and rec["has_key"] is False
    st = gate.status(job)
    assert set(st) == EXPECTED_STATUS_KEYS
    assert st["phase"] == "done" and st["effort"] == "medium" and st["agent"] == "claude-code"
    assert any(
        line.startswith("=== usage preflight claude-test-model") for line in r.stdout.splitlines()
    )


def test_claude_code_preflight_only_reports_usage(gate: Gate) -> None:
    r = gate.run("claude-code", gate.task, GATE_PREFLIGHT_ONLY="1")
    assert r.returncode == 0, r.stdout + r.stderr
    line = json.loads(r.stdout.strip().splitlines()[0])
    assert line["model"] == "claude-test-model" and line["would_start"] is True
    assert line["max_utilization"] == 0.5
    assert not (gate.out / "jobs").exists() or not list((gate.out / "jobs").iterdir())


def test_claude_code_refuses_above_usage_max(gate: Gate) -> None:
    r = gate.run("claude-code", gate.task, FAKE_UTIL="0.9")
    assert r.returncode == 2, r.stdout + r.stderr
    st = gate.status(f"cu-claude-code-{gate.task}")
    assert st["phase"] == "infra_failed" and st["stop_reason"] == "usage"


def test_claude_code_missing_oauth_env_is_refused(gate: Gate) -> None:
    (gate.envd / "claude-gate.env").unlink()
    r = gate.run("claude-code", gate.task)
    assert r.returncode == 2 and "claude-gate.env" in r.stderr


def test_condition_runs_a_patched_task_copy_for_claude_code(gate: Gate) -> None:
    r = gate.run("claude-code", gate.task, CU_GATE_CONDITION="pixels-lowres", GATE_JOB_SUFFIX="-m4")
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"cu-claude-code-{gate.task}-pixels-lowres-m4"
    argv = gate.argv(job)["argv"]
    copy = Path(argv[argv.index("-p") + 1])
    assert copy == gate.out / "task-copies" / job / gate.task
    original = gate.root / "tasks" / "computer-use-v1" / gate.task / "task.toml"
    assert "--condition" not in original.read_text()
    patched = (copy / "task.toml").read_text()
    assert (
        '"/opt/vbdesk/vb_computer_mcp.py", "--condition", "pixels-lowres", "--start-url"' in patched
    )
    st = gate.status(job)
    assert st["condition"] == "pixels-lowres" and st["task_dir"] == str(copy)
    assert "condition=pixels-lowres" in r.stdout.splitlines()[1]


def test_condition_is_refused_for_computer_1_and_unknown_names(gate: Gate) -> None:
    r = gate.run("claude", gate.task, CU_GATE_CONDITION="pixels")
    assert r.returncode == 2 and "claude-code and codex references only" in r.stderr
    r = gate.run("claude-code", gate.task, CU_GATE_CONDITION="tree")
    assert r.returncode == 2 and "CU_GATE_CONDITION must be" in r.stderr
    assert not (gate.out / "jobs").exists() or not list((gate.out / "jobs").iterdir())


def test_default_condition_runs_the_task_in_place(gate: Gate) -> None:
    r = gate.run("claude-code", gate.task)
    assert r.returncode == 0, r.stdout + r.stderr
    argv = gate.argv(f"cu-claude-code-{gate.task}")["argv"]
    assert argv[argv.index("-p") + 1] == str(gate.root / "tasks" / "computer-use-v1" / gate.task)
    assert not (gate.out / "task-copies").exists()


def test_codex_run_uses_subscription_auth_config_and_a_catalog_copy(gate: Gate) -> None:
    r = gate.run("codex", gate.task, FAKE_HARBOR_MODE="codex-mcp")
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"cu-codex-{gate.task}"
    rec = gate.argv(job)
    argv = rec["argv"]
    assert argv[argv.index("-a") + 1] == "codex"
    assert argv[argv.index("-m") + 1] == "gpt-test-model"
    assert argv[argv.index("--effort") + 1] == "medium"
    aks = [argv[i + 1] for i, a in enumerate(argv) if a == "--ak"]
    assert aks == [
        "version=8.8.8",
        "config=" + str(gate.root / "scripts" / "computer-use" / "codex-gate.toml"),
    ]
    hosts = [argv[i + 1] for i, a in enumerate(argv) if a == "--allow-agent-host"]
    assert hosts == ["chatgpt.com", "auth.openai.com", "api.openai.com"]
    assert rec["codex_auth"] == str(gate.home / ".codex" / "auth.json")
    assert "--ae" not in argv and "not-a-real-token" not in " ".join(argv)
    assert "CLAUDE_FORCE_OAUTH" not in rec["env_names"]
    # harbor runs a copy whose image carries the locked catalog
    copy = Path(argv[argv.index("-p") + 1])
    assert copy == gate.out / "task-copies" / job / gate.task
    dockerfile = (copy / "environment" / "Dockerfile").read_text()
    assert dockerfile.rstrip().endswith("COPY codex-catalog.json /opt/vbdesk/codex-catalog.json")
    original = gate.root / "tasks" / "computer-use-v1" / gate.task / "environment" / "Dockerfile"
    assert "codex-catalog" not in original.read_text()
    assert "--condition" not in (copy / "task.toml").read_text()
    locked = json.loads((copy / "environment" / "codex-catalog.json").read_text())
    assert [m["slug"] for m in locked["models"]] == ["gpt-test-model"]
    entry = locked["models"][0]
    assert entry["apply_patch_tool_type"] is None and entry["multi_agent_version"] is None
    assert entry["tool_mode"] == "code_mode_only" and entry["model_messages"]
    assert json.loads((gate.out / f"{job}.codex-catalog.json").read_text()) == locked
    st = gate.status(job)
    assert set(st) == EXPECTED_STATUS_KEYS
    assert st["phase"] == "done" and st["agent"] == "codex" and st["task_dir"] == str(copy)
    assert any(line.startswith("=== codex catalog: ") for line in r.stdout.splitlines())


def test_codex_condition_and_catalog_share_one_copy(gate: Gate) -> None:
    r = gate.run(
        "codex",
        gate.task,
        FAKE_HARBOR_MODE="codex-mcp",
        CU_GATE_CONDITION="pixels",
        CU_GATE_CODEX_CATALOG_SOURCE=str(gate.catalog),
    )
    assert r.returncode == 0, r.stdout + r.stderr
    job = f"cu-codex-{gate.task}-pixels"
    copy = Path(gate.argv(job)["argv"][gate.argv(job)["argv"].index("-p") + 1])
    assert '"--condition", "pixels"' in (copy / "task.toml").read_text()
    assert (copy / "environment" / "codex-catalog.json").is_file()


def test_codex_shell_event_stops_the_run_as_a_tool_surface_breach(gate: Gate) -> None:
    r = gate.run("codex", gate.task, FAKE_HARBOR_MODE="codex-shell")
    assert r.returncode == 1, r.stdout + r.stderr
    st = gate.status(f"cu-codex-{gate.task}")
    assert st["phase"] == "infra_failed" and str(st["stop_reason"]).startswith("tool-surface:")


def test_codex_unknown_model_and_missing_auth_are_refused(gate: Gate) -> None:
    r = gate.run("codex", gate.task, CU_GATE_MODEL="gpt-not-served")
    assert r.returncode == 2 and "codex catalog" in r.stderr
    (gate.home / ".codex" / "auth.json").unlink()
    r = gate.run("codex", gate.task)
    assert r.returncode == 2 and "codex login" in r.stderr
    assert not (gate.out / "jobs").exists() or not list((gate.out / "jobs").iterdir())
