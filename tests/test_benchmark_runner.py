from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

from scripts import benchmark_runner as runner


def git(path: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(path), *args], text=True).strip()


@pytest.fixture
def configured(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[dict[str, str], str]:
    source = tmp_path / "author"
    source.mkdir()
    git(source, "init", "-q")
    git(source, "config", "user.email", "test@example.test")
    git(source, "config", "user.name", "Test")
    git(source, "remote", "add", "origin", str(source))
    files = {
        ".gitignore": ".venv/\n__pycache__/\n",
        "vulcanbench.toml": '[effort]\nblocked=["ultra"]\n',
        "uv.lock": "test dependency lock\n",
        "harness/__init__.py": "",
        "harness/tasks.py": """from pathlib import Path
from types import SimpleNamespace
def load_task(name, root):
    return SimpleNamespace(task_id=name, root=Path(root)/name, snapshot=None, metadata={"timeout_s":10800})
def task_hash(task): return "fixture-hash"
""",
        "harness/suite.py": """from pathlib import Path
from types import SimpleNamespace
def load_suite(name):
    return SimpleNamespace(task_ids=["example"], tasks_root=Path("tasks")/name)
""",
        "harness/cli.py": """import argparse,json,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("action");p.add_argument("--output-dir");args,_=p.parse_known_args()
assert "PYTHONPATH" not in os.environ
out=Path(args.output_dir);out.mkdir(parents=True)
(out/"result.json").write_text(json.dumps({"cwd":os.getcwd()}))
""",
        "tasks/example/example/metadata.json": "{}",
    }
    for name, contents in files.items():
        path = source / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)
    git(source, "add", ".")
    git(source, "commit", "-qm", "fixture")
    sha = git(source, "rev-parse", "HEAD")
    config_path = tmp_path / "config.json"
    args = argparse.Namespace(
        source=source, runner=tmp_path / "runner", results=tmp_path / "results", config=config_path
    )
    runner.initialize(args)
    config = runner.load_config(config_path)
    checkout = Path(config["runner"])
    (checkout / ".venv/bin").mkdir(parents=True)
    (checkout / ".venv/bin/python").symlink_to(sys.executable)
    runner.write_json(
        checkout / ".git/PREPARED.json",
        {
            "revision": sha,
            "environment": runner.environment_inventory(
                checkout, runner.child_environment(checkout)
            ),
            "uv_lock_sha256": hashlib.sha256((checkout / "uv.lock").read_bytes()).hexdigest(),
        },
    )
    monkeypatch.setattr(runner, "active_benchmarks", lambda repo=None: [])
    monkeypatch.setattr(runner, "tool_versions", lambda args, env: {})
    return config, sha


def args(sha: str, *extra: str) -> argparse.Namespace:
    return runner.parser().parse_args(
        [
            "run",
            "--revision",
            sha,
            "--suite",
            "example",
            "--model",
            "mock:synthetic",
            "--sandbox",
            "local",
            *extra,
        ]
    )


def test_run_uses_runner_and_external_results(configured, tmp_path, monkeypatch):
    config, sha = configured
    monkeypatch.setenv("PYTHONPATH", config["source"])
    (Path(config["source"]) / "harness/cli.py").write_text(
        'raise RuntimeError("authoring changed")'
    )
    with runner.execution_lock(tmp_path / "lock") as lock:
        assert runner.launch(args(sha), config, lock) == 0
    receipt_path = next(Path(config["results"]).glob("*/experiment.json"))
    receipt = json.loads(receipt_path.read_text())
    assert receipt["revision"] == sha and receipt["status"] == "completed"
    assert receipt["inputs"]["tasks"][0]["task_hash"] == "fixture-hash"
    assert receipt["inputs"]["harness_path"].startswith(config["runner"])
    result = json.loads((receipt_path.parent / "runs/result.json").read_text())
    assert result["cwd"] == config["runner"]
    assert git(Path(config["runner"]), "status", "--porcelain") == ""
    assert (
        git(Path(config["runner"]), "remote", "get-url", "--push", "origin") == runner.PUSH_DISABLED
    )


def test_dirty_runner_and_wrong_revision_stop_before_execution(configured, tmp_path):
    config, sha = configured
    checkout = Path(config["runner"])
    with runner.execution_lock(tmp_path / "lock") as lock:
        with pytest.raises(ValueError, match="revision differs"):
            runner.launch(args("a" * 40), config, lock)
        (checkout / "harness/cli.py").write_text("modified")
        with pytest.raises(ValueError, match="local changes"):
            runner.launch(args(sha), config, lock)
    assert not list(Path(config["results"]).iterdir())


def test_legacy_benchmark_blocks_execution_but_preflight_is_safe(configured, tmp_path, monkeypatch):
    config, sha = configured
    monkeypatch.setattr(runner, "active_benchmarks", lambda repo=None: [123])
    with runner.execution_lock(tmp_path / "lock") as lock:
        with pytest.raises(ValueError, match="Existing benchmark"):
            runner.launch(args(sha), config, lock)
        assert runner.launch(args(sha, "--check-only"), config, lock) == 0
    receipt_path = next(Path(config["results"]).glob("*/experiment.json"))
    receipt = json.loads(receipt_path.read_text())
    assert receipt["status"] == "preflight"
    assert receipt["active_legacy_benchmark_pids"] == [123]
    assert not (receipt_path.parent / "console.log").exists()
    assert not (receipt_path.parent / "runs").exists()


def test_failure_and_checkout_mutation_are_preserved(configured, tmp_path, monkeypatch):
    config, sha = configured
    monkeypatch.setattr(runner, "execute", lambda *a: 7)
    with runner.execution_lock(tmp_path / "lock") as lock:
        assert runner.launch(args(sha), config, lock) == 7

        def modify(*_):
            (Path(config["runner"]) / "unexpected.txt").write_text("evidence")
            return 0

        monkeypatch.setattr(runner, "execute", modify)
        with pytest.raises(ValueError, match="local changes"):
            runner.launch(args(sha), config, lock)
    receipts = [
        json.loads(p.read_text()) for p in Path(config["results"]).glob("*/experiment.json")
    ]
    assert len(receipts) == 2 and all(
        r["status"] == "failed" and r["finished_at"] for r in receipts
    )
    assert any(r.get("exit_code") == 7 for r in receipts)
    assert any("local changes" in r.get("error", "") for r in receipts)


def test_environment_drift_requires_preparation(configured, tmp_path):
    config, sha = configured
    prepared = Path(config["runner"]) / ".git/PREPARED.json"
    data = json.loads(prepared.read_text())
    data["environment"]["packages"] = []
    runner.write_json(prepared, data)
    with (
        runner.execution_lock(tmp_path / "lock") as lock,
        pytest.raises(ValueError, match="dependencies differ"),
    ):
        runner.launch(args(sha), config, lock)


def test_lock_rejects_overlap_and_releases_after_error(tmp_path):
    path = tmp_path / "lock"
    with pytest.raises(RuntimeError), runner.execution_lock(path):
        with pytest.raises(ValueError, match="holds"), runner.execution_lock(path):
            pytest.fail("overlap accepted")
        raise RuntimeError("interrupted")
    with runner.execution_lock(path):
        pass


@pytest.mark.parametrize(
    "extra", [("--suite", "../outside"), ("--tasks-root", "/tmp"), ("--repeat", "0")]
)
def test_selection_rejects_uncommitted_paths_or_invalid_repeat(extra):
    with pytest.raises(ValueError):
        runner.selection(args("a" * 40, *extra))


def test_nested_or_symlinked_outputs_rejected(tmp_path):
    source = tmp_path / "source"
    source.mkdir()
    alias = tmp_path / "alias"
    alias.symlink_to(source, target_is_directory=True)
    with pytest.raises(ValueError, match="separate"):
        runner.disjoint(source, tmp_path / "runner", alias / "results")


def test_legacy_process_scan_ignores_ancestors(monkeypatch):
    listing = f"{os.getpid()} {os.getppid()} python launcher\n{os.getppid()} 1 shell vulcanbench run\n123 1 /repo/.venv/bin/vulcanbench run --suite v4\n124 1 /runner/.venv/bin/python -m harness.cli run --task foo\n125 1 unrelated\n"
    monkeypatch.setattr(runner, "command", lambda *a, **k: listing)
    assert runner.active_benchmarks() == [123, 124]
    assert runner.active_benchmarks(Path("/runner")) == [124]


def test_prepare_records_locked_environment_and_refuses_active_runner(configured, monkeypatch):
    config, sha = configured
    original_git = runner.git
    original_run = subprocess.run
    calls = []

    def fake_git(repo, *arguments):
        if arguments == ("lfs", "pull"):
            return ""
        return original_git(repo, *arguments)

    def fake_run(cmd, **kwargs):
        if cmd[:2] == ["uv", "sync"]:
            calls.append(cmd)
            return subprocess.CompletedProcess(cmd, 0)
        return original_run(cmd, **kwargs)

    monkeypatch.setattr(runner, "git", fake_git)
    monkeypatch.setattr(runner.subprocess, "run", fake_run)
    runner.prepare(config, sha)
    assert "--locked" in calls[0]
    prepared = json.loads((Path(config["runner"]) / ".git/PREPARED.json").read_text())
    assert prepared["revision"] == sha and prepared["environment"]["packages"]
    monkeypatch.setattr(runner, "active_benchmarks", lambda repo=None: [123])
    with pytest.raises(ValueError, match="unmanaged benchmark"):
        runner.prepare(config, sha)
    assert len(calls) == 1


def test_signal_forwarding_preserves_child_exit_and_releases_lock(tmp_path):
    ready = tmp_path / "ready"
    script = """import sys
from pathlib import Path
from scripts.benchmark_runner import execute,execution_lock
folder=Path(sys.argv[1])
with execution_lock(folder/'lock') as lock:
    grand = "from pathlib import Path; import time; Path("+repr(str(folder/'ready'))+").touch(); time.sleep(30)"
    nested = "import subprocess,sys; from pathlib import Path; p=subprocess.Popen([sys.executable,'-c',"+repr(grand)+"],start_new_session=True); Path("+repr(str(folder/'nested_pid'))+").write_text(str(p.pid)); p.wait()"
    code=execute([sys.executable,'-c',nested],Path.cwd(),folder,dict(__import__('os').environ),lock)
    (folder/'exit').write_text(str(code))
"""
    child = subprocess.Popen([sys.executable, "-c", script, str(tmp_path)])
    try:
        deadline = time.monotonic() + 10
        while not ready.exists() and child.poll() is None and time.monotonic() < deadline:
            time.sleep(0.02)
        assert ready.exists()
        child.send_signal(signal.SIGTERM)
        assert child.wait(timeout=5) == 0
        assert int((tmp_path / "exit").read_text()) == -signal.SIGTERM
        assert runner.live_pids({int((tmp_path / "nested_pid").read_text())}) == []
        with runner.execution_lock(tmp_path / "lock"):
            pass
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()


def test_unresolved_cleanup_marker_blocks_next_operation(tmp_path):
    lock = tmp_path / "execution.lock"
    runner.write_json(lock.with_suffix(".review.json"), {"pids": [123]})
    with pytest.raises(ValueError, match="cleanup needs review"), runner.execution_lock(lock):
        pytest.fail("unresolved child ignored")
