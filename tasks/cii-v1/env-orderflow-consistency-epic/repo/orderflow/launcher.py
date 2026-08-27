"""Boot a full orderflow deployment: five services, one process each.

Used by ``scripts/dev_up.py`` and by any test harness. Requires the
infrastructure stack (postgres + redis) to be up and resolvable through
``.vb_services.json``. Each service prints ``READY <port>`` once serving;
the launcher collects the ports and writes ``.of_topology.json``.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

from orderflow.ofkit import topology

SERVICES = ("orders", "inventory", "billing", "worker", "gateway")
_READY_TIMEOUT_S = 60


class Deployment:
    def __init__(self, procs: dict[str, subprocess.Popen], urls: dict[str, str]):
        self.procs = procs
        self.urls = urls

    def stop(self) -> None:
        for proc in self.procs.values():
            if proc.poll() is None:
                proc.terminate()
        deadline = time.monotonic() + 10
        for proc in self.procs.values():
            remaining = max(0.1, deadline - time.monotonic())
            try:
                proc.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                proc.kill()

    def __enter__(self) -> Deployment:
        return self

    def __exit__(self, *exc: object) -> None:
        self.stop()


def _spawn(service: str, repo_root: Path, env: dict[str, str]) -> subprocess.Popen:
    return subprocess.Popen(
        [sys.executable, "-m", f"orderflow.{service}", "--port", "0"],
        cwd=repo_root,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )


def _await_ready(service: str, proc: subprocess.Popen) -> int:
    deadline = time.monotonic() + _READY_TIMEOUT_S
    assert proc.stdout is not None
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"{service} exited with {proc.returncode} before READY")
        line = proc.stdout.readline().strip()
        if line.startswith("READY "):
            return int(line.split(" ", 1)[1])
    raise RuntimeError(f"{service} did not print READY within {_READY_TIMEOUT_S}s")


def start(repo_root: Path | None = None, extra_env: dict[str, str] | None = None) -> Deployment:
    root = (repo_root or Path.cwd()).resolve()
    env = dict(os.environ)
    env["PYTHONPATH"] = str(root) + os.pathsep + env.get("PYTHONPATH", "")
    env.setdefault("OF_MANIFEST_DIR", str(topology.app_manifest_path().parent))
    env.update(extra_env or {})

    procs: dict[str, subprocess.Popen] = {}
    urls: dict[str, str] = {}
    try:
        for service in SERVICES:
            procs[service] = _spawn(service, root, env)
        for service, proc in procs.items():
            port = _await_ready(service, proc)
            urls[service] = f"http://127.0.0.1:{port}"
    except Exception:
        Deployment(procs, urls).stop()
        raise

    manifest = topology.app_manifest_path()
    manifest.write_text(
        json.dumps({"services": urls}, indent=2) + "\n", encoding="utf-8"
    )
    topology.reset_cache()
    return Deployment(procs, urls)
