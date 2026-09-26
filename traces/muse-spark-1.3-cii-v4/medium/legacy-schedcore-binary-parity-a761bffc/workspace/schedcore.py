"""SchedCore dependency scheduler, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


def _canon(name: str) -> str:
    # Job ids are case-insensitive in the engine; ids are ASCII
    # alphanumerics so upper() is a 1-1 canonicalization.
    return name.upper()


def _is_job_id(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and tok.isascii() and tok.isalnum()


class Job:
    __slots__ = (
        "name",
        "prio",
        "deps",
        "defseq",
        "executed",
        "fails",
        "torn",
        "poisoned",
    )

    def __init__(self, name: str, prio: int, deps: list[str], defseq: int):
        self.name = name  # first-seen spelling, used for output/echo
        self.prio = prio
        self.deps = deps  # canonical keys, de-duplicated
        self.defseq = defseq
        self.executed = False
        self.fails = 0  # times un-executed via F (directly or transitively)
        self.torn = 0  # teardown recency stamp (0 = never torn down)
        self.poisoned = False  # dep missing at a G: never unblocks


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

    @staticmethod
    def _eff(j: Job) -> int:
        # Each failure after the first costs 7 priority points, floored at 1.
        if j.fails == 0:
            return j.prio
        return max(1, j.prio - 7 * (j.fails - 1))

    @staticmethod
    def _boost(j: Job) -> int:
        # Never-torn-down fan-in jobs (2+ deps) jump the queue among
        # equal-priority ready jobs when the priority is a multiple of 10.
        if j.fails == 0 and len(j.deps) >= 2 and j.prio % 10 == 0:
            return 1
        return 0

    def _deps_ready(self, j: Job) -> bool:
        for d in j.deps:
            k = self.by_name.get(d)
            if k is None or not k.executed:
                return False
        return True

    def _is_ready(self, j: Job) -> bool:
        return not j.executed and not j.poisoned and self._deps_ready(j)

    def _better(self, j: Job, best: Job) -> bool:
        ej, eb = self._eff(j), self._eff(best)
        if ej != eb:
            return ej > eb
        bj, bb = self._boost(j), self._boost(best)
        if bj != bb:
            return bj > bb
        if j.torn != best.torn:
            return j.torn > best.torn
        return j.defseq < best.defseq

    def _tear_down(self, j: Job, invalidated: set[str]) -> int:
        j.executed = False
        j.fails += 1
        j.torn = self.tear_next
        self.tear_next += 1
        invalidated.add(_canon(j.name))
        return 1

    def handle(self, line: str) -> None:
        raw = line.rstrip("\r\n")
        if raw == "":
            return
        parts = raw.split()
        if not parts:
            self._reject("????????", "FMT")
            return
        kind = parts[0]
        job = parts[1] if len(parts) > 1 else ""
        job_ok = _is_job_id(job)
        echo = job if job_ok else "????????"

        if kind == "J":
            # Extra trailing tokens are ignored; only the first four
            # tokens are significant.
            if len(parts) < 4 or not job_ok:
                self._reject(echo, "FMT")
                return
            ptok = parts[2]
            if not (ptok.isascii() and ptok.isdigit() and 1 <= len(ptok) <= 2):
                self._reject(job, "PRIO")
                return
            prio = int(ptok)
            if not 1 <= prio <= 99:
                self._reject(job, "PRIO")
                return
            dtok = parts[3]
            deps: list[str] = []
            bad = False
            if dtok != "0":
                seen: set[str] = set()
                for part in dtok.split(","):
                    if not _is_job_id(part):
                        bad = True
                        break
                    cpart = _canon(part)
                    if cpart == _canon(job):  # self-dependency
                        bad = True
                        break
                    if cpart not in seen:
                        seen.add(cpart)
                        deps.append(cpart)
            if bad:
                self._reject(job, "DEPS")
                return
            if _canon(job) in self.by_name:
                self._reject(self.by_name[_canon(job)].name, "DUP")
                return
            j = Job(job, prio, deps, self.def_next)
            self.def_next += 1
            self.jobs.append(j)
            self.by_name[_canon(job)] = j
            self.c_def += 1
            self._emit(f"OK {self.c_def}")
            return

        if kind == "G":
            # Extra trailing tokens are ignored.
            for j in self.jobs:
                if j.executed or j.poisoned:
                    continue
                if any(d not in self.by_name for d in j.deps):
                    j.poisoned = True
            executed_this = 0
            while True:
                best: Job | None = None
                for j in self.jobs:
                    if not self._is_ready(j):
                        continue
                    if best is None or self._better(j, best):
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
            # Extra trailing tokens are ignored.
            if len(parts) < 2 or not job_ok:
                self._reject(echo, "FMT")
                return
            j = self.by_name.get(_canon(job))
            if j is None:
                self._reject(job, "STATE")
                return
            if not j.executed:
                self._reject(j.name, "STATE")
                return
            invalidated: set[str] = set()
            count = self._tear_down(j, invalidated)
            changed = True
            while changed:
                changed = False
                for k in self.jobs:
                    if not k.executed:
                        continue
                    if any(d in invalidated for d in k.deps):
                        count += self._tear_down(k, invalidated)
                        changed = True
            self.c_fail += 1
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
