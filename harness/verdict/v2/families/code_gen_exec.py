"""Sandboxed, cached execution for the generated Software families.

Everything the code-generation builders execute goes through here: a fresh
temp directory per run, a subprocess with a timeout and a memory cap, a
scrubbed environment with proxies pointed at a dead port, and (for Python) a
guard that disables socket connects and DNS before the target runs. Results
are cached as JSON under ``<repo>/verdict-v2-items/cache/<namespace>`` keyed by
a content hash of everything that can change the outcome, so rebuilds are fast
and identical.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

from harness.verdict.v2.registry import BuildContext

DEFAULT_TIMEOUT = 5.0
DEFAULT_WORKERS = 4
CACHE_VERSION = "1"

# A run whose resident memory passes this is killed and reported exactly like
# a timeout. Mutants can turn a bounded loop into an unbounded one (for
# example ``out.append(v)`` -> ``values.append(v)`` inside ``for v in
# values``), which reached 6 GB in the 5 seconds before the timeout fired;
# with four workers that can exhaust a 16 GB CI runner, which then shut down
# mid-suite. Normal runs stay well under 200 MB. Timed-out runs were
# already dropped by every builder, so the cap only stops them sooner; a run
# that finishes under it is untouched, which is why the cap is not part of the
# cache key.
MEMORY_LIMIT_BYTES = 1024 * 1024 * 1024
_POLL_S = 0.05 if sys.platform.startswith("linux") else 0.25

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


def _rss_bytes(pid: int) -> int | None:
    """Resident memory of a live process, or None when it cannot be read."""
    try:
        with open(f"/proc/{pid}/statm") as statm:
            return int(statm.read().split()[1]) * os.sysconf("SC_PAGE_SIZE")
    except (OSError, ValueError, IndexError):
        if sys.platform.startswith("linux"):
            return None
    try:
        out = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(pid)],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        ).stdout.strip()
        return int(out) * 1024 if out else None
    except (OSError, ValueError, subprocess.SubprocessError):
        return None


def _run_capped(
    argv: list[str], cwd: Path, env: dict[str, str], timeout: float
) -> tuple[bytes, bytes, int, bool]:
    """``(stdout, stderr, returncode, timed_out)`` under the time and memory caps.

    Like ``subprocess.run(timeout=...)``: on either cap the child is killed,
    its remaining output is collected and the run reports ``-9, True``.
    """
    with subprocess.Popen(
        argv,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ) as proc:
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining > 0:
                try:
                    stdout, stderr = proc.communicate(timeout=min(_POLL_S, remaining))
                    return stdout, stderr, proc.returncode, False
                except subprocess.TimeoutExpired:
                    rss = _rss_bytes(proc.pid)
                    if rss is None or rss <= MEMORY_LIMIT_BYTES:
                        continue
                    note = f"\n[killed: resident memory {rss >> 20} MB over the cap]\n"
            else:
                note = ""
            proc.kill()
            stdout, stderr = proc.communicate()
            return stdout, stderr + note.encode(), -9, True


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
        stdout, stderr, code, timed_out = _run_capped(_argv(job), work, _env(home), job.timeout)
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
