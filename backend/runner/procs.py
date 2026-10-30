"""Process listing on macOS and Linux without /proc, via ``ps``.

The Runs page needs to know whether a gate or harness run is still alive when
the queue does not own it (hand launches), and the supervisor will later use
the same identity fields (pid, start time, argv token) to adopt jobs after a
backend restart.
"""

from __future__ import annotations

import re
import subprocess
import time
from dataclasses import dataclass

CACHE_TTL_S = 2.0
_TIMEOUT_S = 8

# pid ppid pgid stat lstart command; lstart is "Thu Oct 29 08:04:33 2026".
_LINE = re.compile(
    r"^\s*(\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s+"
    r"(\w{3}\s+\w{3}\s+\d{1,2}\s+\d\d:\d\d:\d\d\s+\d{4})\s+(.*)$"
)


@dataclass(frozen=True)
class Proc:
    pid: int
    ppid: int
    pgid: int
    stat: str
    lstart: str
    command: str

    @property
    def zombie(self) -> bool:
        return self.stat.startswith("Z")


class _Cache:
    stamp: float = 0.0
    items: list[Proc] | None = None


_cache = _Cache()


def list_processes(*, now: float | None = None) -> list[Proc]:
    t = time.monotonic() if now is None else now
    cached = _cache.items
    if cached is not None and t - _cache.stamp < CACHE_TTL_S:
        return cached
    try:
        proc = subprocess.run(
            # -ww: never truncate the command column (procps and macOS cut it at the
            # terminal width otherwise, which drops the argv token the identity check needs).
            ["ps", "-ax", "-ww", "-o", "pid=,ppid=,pgid=,stat=,lstart=,command="],
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_S,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return cached or []
    out: list[Proc] = []
    for line in proc.stdout.splitlines():
        m = _LINE.match(line)
        if not m:
            continue
        out.append(
            Proc(
                pid=int(m.group(1)),
                ppid=int(m.group(2)),
                pgid=int(m.group(3)),
                stat=m.group(4),
                lstart=re.sub(r"\s+", " ", m.group(5)),
                command=m.group(6),
            )
        )
    _cache.stamp, _cache.items = t, out
    return out


def reset_cache() -> None:
    _cache.stamp, _cache.items = 0.0, None


_JOB_NAME = re.compile(r"--job-name[ =](\S+)")


def find_harbor_job(job_name: str, procs: list[Proc] | None = None) -> Proc | None:
    """The ``harbor run`` process for a gate job, if it is alive."""
    for p in procs if procs is not None else list_processes():
        if "harbor run" not in p.command or p.zombie:
            continue
        m = _JOB_NAME.search(p.command)
        if m and m.group(1) == job_name:
            return p
    return None


def find_vulcanbench_runs(procs: list[Proc] | None = None) -> list[Proc]:
    """Every live ``vulcanbench run`` (or effort-sweep) process."""
    out: list[Proc] = []
    for p in procs if procs is not None else list_processes():
        cmd = p.command
        if ("vulcanbench run " in cmd or "vulcanbench effort-sweep " in cmd) and not p.zombie:
            out.append(p)
    return out


def descendants(pid: int, procs: list[Proc]) -> list[Proc]:
    by_parent: dict[int, list[Proc]] = {}
    for p in procs:
        by_parent.setdefault(p.ppid, []).append(p)
    out: list[Proc] = []
    stack = [pid]
    while stack:
        cur = stack.pop()
        for child in by_parent.get(cur, []):
            out.append(child)
            stack.append(child.pid)
    return out


# ------------------------------------------------------------ identity --
# The supervisor records (pid, lstart, boot time, an argv token) when it
# starts a child and checks all four when it re-adopts the job after a
# backend restart, so a recycled pid is never mistaken for the run.


def boot_time() -> str | None:
    """A string that changes on every reboot (macOS kern.boottime or Linux btime)."""
    try:
        proc = subprocess.run(
            ["sysctl", "-n", "kern.boottime"],
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_S,
            check=False,
        )
        if proc.returncode == 0 and proc.stdout.strip():
            m = re.search(r"sec\s*=\s*(\d+)", proc.stdout)
            return m.group(1) if m else proc.stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        pass
    try:
        with open("/proc/stat", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("btime "):
                    return line.split()[1]
    except OSError:
        pass
    return None


def find_pid(pid: int, procs: list[Proc] | None = None) -> Proc | None:
    for p in procs if procs is not None else list_processes():
        if p.pid == pid:
            return p
    return None


def same_process(
    pid: int,
    lstart: str | None,
    token: str | None,
    procs: list[Proc] | None = None,
) -> bool:
    """True when ``pid`` is alive, not a zombie, started at ``lstart`` and runs ``token``."""
    p = find_pid(pid, procs)
    if p is None or p.zombie:
        return False
    if lstart and p.lstart != re.sub(r"\s+", " ", lstart.strip()):
        return False
    return not token or token in p.command
