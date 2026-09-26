"""SchedCore dependency scheduler, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).

Engine behaviors that differ from the spec and are reproduced here:

- Job ids are ASCII alphanumerics (1-8 chars) matched
  case-insensitively; the first spelling is kept for display.
- Duplicate dependency entries are accepted.
- Commands tolerate trailing tokens (only the leading tokens matter);
  a truly empty line is ignored while a whitespace-only line is
  rejected ``FMT``.
- A job seen by a run while one of its dependencies is still undefined
  never becomes ready, even if that dependency is defined later.
- Reruns order equal-priority jobs by reverse teardown order (each
  invalidated job takes the next sequence number: target first, then
  dependents in definition order); jobs never torn down keep
  definition order among themselves.
- Each invalidation lowers that job's effective priority by 7
  (floored at 1); the first failure leaves it unchanged.
- A never-failed job with a round-ten priority and at least two
  distinct dependencies runs with effective priority +1.
"""

from __future__ import annotations

import sys


def _is_job_id(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and all(
        "0" <= c <= "9" or "A" <= c <= "Z" or "a" <= c <= "z" for c in tok
    )


def _canon(tok: str) -> str:
    """Canonical job-id form: ASCII uppercase (engine is case-insensitive)."""
    return "".join(chr(ord(c) - 32) if "a" <= c <= "z" else c for c in tok)


def _is_prio(tok: str) -> bool:
    return 1 <= len(tok) <= 2 and all("0" <= c <= "9" for c in tok)


class Job:
    __slots__ = (
        "name",
        "key",
        "prio",
        "deps",
        "defseq",
        "executed",
        "failcount",
        "stamp",
        "seen",
        "dead",
    )

    def __init__(self, name: str, prio: int, deps: list[str], defseq: int):
        self.name = name
        self.key = _canon(name)
        self.prio = prio
        self.deps = deps  # canonical dep keys, as listed
        self.defseq = defseq
        self.executed = False
        self.failcount = 0
        self.stamp = 0
        self.seen = False
        self.dead = False

    def eff(self) -> int:
        if self.failcount == 0:
            if self.prio % 10 == 0 and len(set(self.deps)) >= 2:
                return self.prio + 1
            return self.prio
        return max(1, self.prio - 7 * (self.failcount - 1))


class Engine:
    def __init__(self, out=None):
        self.jobs: list[Job] = []
        self.by_name: dict[str, Job] = {}
        self.def_next = 1
        self.tear_next = 1
        self.c_def = 0
        self.c_exec = 0
        self.c_fail = 0
        self.c_rej = 0
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, echo: str, code: str) -> None:
        self._emit(f"R {echo} {code}")
        self.c_rej += 1

    def _ready(self, j: Job) -> bool:
        if j.dead:
            return False
        for d in j.deps:
            k = self.by_name.get(d)
            if k is None or not k.executed:
                return False
        return True

    def handle(self, line: str) -> None:
        text = line.rstrip("\r\n")
        if text == "":
            return
        parts = text.split()
        if not parts:
            self._reject("????????", "FMT")
            return
        kind = parts[0]
        raw_job = parts[1] if len(parts) > 1 else ""
        echo = raw_job if _is_job_id(raw_job) else "????????"

        if kind == "J":
            if len(parts) < 4 or not _is_job_id(raw_job):
                self._reject(echo, "FMT")
                return
            ptok = parts[2]
            if not _is_prio(ptok):
                self._reject(raw_job, "PRIO")
                return
            prio = int(ptok)
            if not 1 <= prio <= 99:
                self._reject(raw_job, "PRIO")
                return
            dtok = parts[3]
            deps: list[str] = []
            bad = False
            if dtok != "0":
                for part in dtok.split(","):
                    if not _is_job_id(part):
                        bad = True
                        break
                    if _canon(part) == _canon(raw_job):  # self-dependency
                        bad = True
                        break
                    deps.append(_canon(part))
            if bad:
                self._reject(raw_job, "DEPS")
                return
            if _canon(raw_job) in self.by_name:
                self._reject(self.by_name[_canon(raw_job)].name, "DUP")
                return
            j = Job(raw_job, prio, deps, self.def_next)
            self.def_next += 1
            self.jobs.append(j)
            self.by_name[j.key] = j
            self.c_def += 1
            self._emit(f"OK {self.c_def}")
            return

        if kind == "G":
            for j in self.jobs:
                if not j.seen:
                    j.seen = True
                    if any(d not in self.by_name for d in j.deps):
                        j.dead = True
            eff = {id(j): j.eff() for j in self.jobs}
            executed_this = 0
            while True:
                best: Job | None = None
                for j in self.jobs:
                    if j.executed:
                        continue
                    if not self._ready(j):
                        continue
                    if (
                        best is None
                        or (eff[id(j)], j.stamp, -j.defseq)
                        > (eff[id(best)], best.stamp, -best.defseq)
                    ):
                        best = j
                if best is None:
                    break
                best.executed = True
                self.c_exec += 1
                executed_this += 1
                self._emit(f"E {best.name}")
            blocked = sum(1 for j in self.jobs if not j.executed)
            self._emit(f"GEND {executed_this} {blocked}")
            return

        if kind == "F":
            if len(parts) < 2 or not _is_job_id(raw_job):
                self._reject(echo, "FMT")
                return
            j = self.by_name.get(_canon(raw_job))
            if j is None:
                self._reject(raw_job, "STATE")
                return
            if not j.executed:
                self._reject(j.name, "STATE")
                return
            self.c_fail += 1
            j.executed = False
            j.failcount += 1
            j.stamp = self.tear_next
            self.tear_next += 1
            invalidated = {j.key}
            count = 1
            changed = True
            while changed:
                changed = False
                for k in self.jobs:
                    if not k.executed:
                        continue
                    if any(d in invalidated for d in k.deps):
                        k.executed = False
                        k.failcount += 1
                        k.stamp = self.tear_next
                        self.tear_next += 1
                        invalidated.add(k.key)
                        count += 1
                        changed = True
            self._emit(f"OK {count}")
            return

        self._reject(echo, "FMT")

    def finish(self) -> None:
        self._emit(f"X {self.c_def} {self.c_exec} {self.c_fail} {self.c_rej}")


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
