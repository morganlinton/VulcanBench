"""Sandboxed, cached execution for the generated Software families.

Everything the code-generation builders execute goes through here: a fresh
temp directory per run, a subprocess with a timeout, a scrubbed environment
with proxies pointed at a dead port, and (for Python) a guard that disables
socket connects and DNS before the target runs. Results are cached as JSON
under ``<repo>/verdict-v2-items/cache/<namespace>`` keyed by a content hash of
everything that can change the outcome, so rebuilds are fast and identical.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

from harness.verdict.v2.registry import BuildContext

DEFAULT_TIMEOUT = 5.0
DEFAULT_WORKERS = 4
CACHE_VERSION = "1"

# Placed next to every Python target. It blocks outbound connections and name
# lookups, then runs the target as __main__ (or a module, for pytest).
PY_GUARD = """\
import runpy
import socket
import sys


def _blocked(*args, **kwargs):
    raise OSError("network disabled in the Verdict v2 sandbox")


socket.socket.connect = _blocked
socket.socket.connect_ex = _blocked
socket.getaddrinfo = _blocked
socket.create_connection = _blocked
sys.path.insert(0, ".")
_mode, _target, *_rest = sys.argv[1:]
sys.argv = [_target, *_rest]
if _mode == "module":
    runpy.run_module(_target, run_name="__main__", alter_sys=True)
else:
    runpy.run_path(_target, run_name="__main__")
"""
GUARD_NAME = "_verdict_guard.py"


@dataclass(frozen=True)
class Job:
    """One execution: files to write, then ``argv`` run inside that directory.

    ``argv[0]`` may be ``"python"`` (the running interpreter, wrapped in the
    network guard), ``"python-module"`` (``python -m argv[1]`` under the
    guard), or any executable on PATH such as ``node``. ``base`` is a
    directory tree hard-linked into the run directory first (used for copied
    library packages); ``base_key`` must identify its content for the cache.
    ``collect`` names files to read back after the run.
    """

    files: dict[str, str]
    argv: tuple[str, ...]
    timeout: float = DEFAULT_TIMEOUT
    base: Path | None = None
    base_key: str = ""
    collect: tuple[str, ...] = ()
    tag: str = ""

    def key(self) -> str:
        payload = json.dumps(
            {
                "v": CACHE_VERSION,
                "files": sorted(self.files.items()),
                "argv": self.argv,
                "timeout": self.timeout,
                "base": self.base_key,
                "collect": self.collect,
                "tag": self.tag,
                "python": sys.version if self.argv[0].startswith("python") else "",
            },
            sort_keys=True,
        )
        return hashlib.sha256(payload.encode()).hexdigest()


@dataclass(frozen=True)
class RunResult:
    stdout: str
    stderr: str
    returncode: int
    timed_out: bool
    collected: dict[str, str] = field(default_factory=dict)

    @property
    def ok(self) -> bool:
        return self.returncode == 0 and not self.timed_out


def _env(home: Path) -> dict[str, str]:
    return {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": str(home),
        "TMPDIR": str(home),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONHASHSEED": "0",
        "PYTHONIOENCODING": "utf-8",
        "PYTHONNOUSERSITE": "1",
        "NO_COLOR": "1",
        "http_proxy": "http://127.0.0.1:9",
        "https_proxy": "http://127.0.0.1:9",
        "HTTP_PROXY": "http://127.0.0.1:9",
        "HTTPS_PROXY": "http://127.0.0.1:9",
        "ALL_PROXY": "http://127.0.0.1:9",
        "GOFLAGS": "-mod=mod",
        "GOPROXY": "off",
        "GOCACHE": str(home / "gocache"),
    }


def _argv(job: Job) -> list[str]:
    head, *rest = job.argv
    if head == "python":
        return [sys.executable, "-B", "-s", GUARD_NAME, "path", *rest]
    if head == "python-module":
        return [sys.executable, "-B", "-s", GUARD_NAME, "module", *rest]
    if head == "python-raw":
        return [sys.executable, "-B", "-s", *rest]
    return [head, *rest]


def _decode(data: bytes | str | None) -> str:
    if data is None:
        return ""
    if isinstance(data, str):
        return data
    return data.decode("utf-8", errors="replace")


def execute(job: Job) -> RunResult:
    """Run one job uncached."""
    with tempfile.TemporaryDirectory(prefix="verdict-v2-") as tmp:
        root = Path(tmp)
        work = root / "work"
        home = root / "home"
        home.mkdir()
        if job.base is not None:
            shutil.copytree(job.base, work, copy_function=os.link)
        else:
            work.mkdir()
        for rel, text in job.files.items():
            target = work / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                target.unlink()  # never write through a hard link into the base
            target.write_text(text)
        if job.argv[0].startswith("python"):
            (work / GUARD_NAME).write_text(PY_GUARD)
        stdout: bytes | str | None
        stderr: bytes | str | None
        try:
            proc = subprocess.run(
                _argv(job),
                cwd=work,
                env=_env(home),
                capture_output=True,
                timeout=job.timeout,
                stdin=subprocess.DEVNULL,
                check=False,
            )
            stdout, stderr, code, timed_out = proc.stdout, proc.stderr, proc.returncode, False
        except subprocess.TimeoutExpired as error:
            stdout, stderr, code, timed_out = error.stdout, error.stderr, -9, True
        collected = {}
        for rel in job.collect:
            path = work / rel
            if path.exists():
                collected[rel] = path.read_text(errors="replace")
        return RunResult(_decode(stdout), _decode(stderr), code, timed_out, collected)


class Runner:
    """Cached, parallel execution. Results come back in job order."""

    def __init__(self, cache_dir: Path, workers: int = DEFAULT_WORKERS) -> None:
        self.cache_dir = cache_dir
        self.workers = max(1, workers)
        self.hits = 0
        self.misses = 0

    def _path(self, key: str) -> Path:
        return self.cache_dir / key[:2] / f"{key}.json"

    def _load(self, key: str) -> RunResult | None:
        path = self._path(key)
        if not path.exists():
            return None
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError):
            return None
        return RunResult(
            data["stdout"],
            data["stderr"],
            data["returncode"],
            data["timed_out"],
            data.get("collected", {}),
        )

    def _store(self, key: str, result: RunResult) -> None:
        path = self._path(key)
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(".tmp")
        tmp.write_text(
            json.dumps(
                {
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "returncode": result.returncode,
                    "timed_out": result.timed_out,
                    "collected": result.collected,
                }
            )
        )
        tmp.replace(path)

    def run_many(self, jobs: Sequence[Job]) -> list[RunResult]:
        keys = [job.key() for job in jobs]
        results: list[RunResult | None] = [self._load(k) for k in keys]
        todo = [i for i, r in enumerate(results) if r is None]
        self.hits += len(jobs) - len(todo)
        self.misses += len(todo)
        if todo:
            with ThreadPoolExecutor(max_workers=self.workers) as pool:
                for i, result in zip(todo, pool.map(execute, [jobs[i] for i in todo]), strict=True):
                    self._store(keys[i], result)
                    results[i] = result
        return [r for r in results if r is not None]

    def run(self, job: Job) -> RunResult:
        return self.run_many([job])[0]


def cached_json(cache_dir: Path, key: str, compute: Callable[[], object]) -> object:
    """Memoise a JSON-serialisable computation (used for batched tool runs)."""
    path = cache_dir / key[:2] / f"{key}.json"
    if path.exists():
        try:
            return json.loads(path.read_text())
        except ValueError:
            pass
    value = compute()
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(value))
    tmp.replace(path)
    return value


def content_hash(*parts: str) -> str:
    digest = hashlib.sha256()
    for part in parts:
        digest.update(part.encode())
        digest.update(b"\0")
    return digest.hexdigest()


def tree_hash(root: Path, suffixes: tuple[str, ...] = (".py",)) -> str:
    """Hash every file with these suffixes under ``root`` (stable order)."""
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.suffix in suffixes and "__pycache__" not in path.parts:
            digest.update(str(path.relative_to(root)).encode())
            digest.update(path.read_bytes())
    return digest.hexdigest()


def cache_root(ctx: BuildContext) -> Path:
    return ctx.repo / "verdict-v2-items" / "cache"


def runner_for(ctx: BuildContext, namespace: str) -> Runner:
    workers = int((ctx.options or {}).get("workers", DEFAULT_WORKERS))
    return Runner(cache_root(ctx) / namespace, workers=workers)
