"""SchedCore dependency scheduler, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import re
import sys

_ID = re.compile(r"[A-Za-z0-9]{1,8}\Z")
_PRIO = re.compile(r"[0-9]{1,2}\Z")


class Job:
    __slots__ = (
        "name",
        "key",
        "prio",
        "deps",
        "defseq",
        "seq",
        "fails",
        "executed",
        "poisoned",
    )

    def __init__(self, name: str, prio: int, deps: list[str], defseq: int):
        self.name = name
        self.key = name.lower()
        self.prio = prio
        self.deps = deps
        self.defseq = defseq
        self.seq = defseq
        self.fails = 0
        self.executed = False
        self.poisoned = False


class Engine:
    def __init__(self, out=None):
        self.jobs: list[Job] = []
        self.by_name: dict[str, Job] = {}
        self.def_next = 1
        self.reseq = 0
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
        if j.fails == 0:
            base = j.prio
            if len(set(j.deps)) >= 2 and j.prio % 10 == 0:
                base += 1
            return base
        return max(1, j.prio - 7 * (j.fails - 1))

    def _deps_ready(self, j: Job) -> bool:
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
        job = parts[1] if len(parts) > 1 else ""
        job_ok = _ID.match(job) is not None
        echo = job if job_ok else "????????"

        if kind == "J":
            if len(parts) < 4 or not job_ok:
                self._reject(echo, "FMT")
                return
            ptok = parts[2]
            if _PRIO.match(ptok) is None:
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
                key = job.lower()
                for part in dtok.split(","):
                    if _ID.match(part) is None:
                        bad = True
                        break
                    if part.lower() == key:  # self-dependency
                        bad = True
                        break
                    deps.append(part.lower())
            if bad:
                self._reject(job, "DEPS")
                return
            if job.lower() in self.by_name:
                self._reject(self.by_name[job.lower()].name, "DUP")
                return
            j = Job(job, prio, deps, self.def_next)
            self.def_next += 1
            self.jobs.append(j)
            self.by_name[j.key] = j
            self.c_def += 1
            self._emit(f"OK {self.c_def}")
            return

        if kind == "G":
            for j in self.jobs:
                if j.executed or j.poisoned:
                    continue
                for d in j.deps:
                    if d not in self.by_name:
                        j.poisoned = True
                        break
            executed_this = 0
            while True:
                best: Job | None = None
                for j in self.jobs:
                    if j.executed or j.poisoned:
                        continue
                    if not self._deps_ready(j):
                        continue
                    if (
                        best is None
                        or self._eff(j) > self._eff(best)
                        or (
                            self._eff(j) == self._eff(best)
                            and j.seq < best.seq
                        )
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
            if len(parts) < 2 or not job_ok:
                self._reject(echo, "FMT")
                return
            j = self.by_name.get(job.lower())
            if j is None:
                self._reject(job, "STATE")
                return
            if not j.executed:
                self._reject(j.name, "STATE")
                return
            self.reseq -= 1
            j.seq = self.reseq
            j.executed = False
            j.fails += 1
            invalidated = {j.key}
            count = 1
            changed = True
            while changed:
                changed = False
                for k in reversed(self.jobs):
                    if not k.executed:
                        continue
                    if any(d in invalidated for d in k.deps):
                        self.reseq -= 1
                        k.seq = self.reseq
                        k.executed = False
                        k.fails += 1
                        invalidated.add(k.key)
                        count += 1
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
