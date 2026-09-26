"""SchedCore dependency scheduler, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


def _valid_id(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and tok.isascii() and tok.isalnum()


class Job:
    __slots__ = (
        "name",
        "fold",
        "prio",
        "deps",
        "defseq",
        "executed",
        "fails",
        "ord",
        "poisoned",
    )

    def __init__(
        self,
        name: str,
        prio: int,
        deps: list[str],
        defseq: int,
    ):
        self.name = name
        self.fold = name.lower()
        self.prio = prio
        self.deps = deps
        self.defseq = defseq
        self.executed = False
        self.fails = 0
        self.ord = defseq
        self.poisoned = False


class Engine:
    def __init__(self, out=None):
        self.jobs: list[Job] = []
        self.by_fold: dict[str, Job] = {}
        self.def_next = 1
        self.stamp_next = 1
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

    def _eff(self, j: Job) -> int:
        if j.fails == 0:
            return j.prio
        return max(1, j.prio - 7 * (j.fails - 1))

    def _deps_ready(self, j: Job) -> bool:
        for d in j.deps:
            k = self.by_fold.get(d)
            if k is None or not k.executed:
                return False
        return True

    def handle(self, line: str) -> None:
        line = line.rstrip("\r\n")
        if line == "":
            return
        parts = line.split()
        if not parts:
            self._reject("????????", "FMT")
            return
        kind = parts[0]
        job = parts[1] if len(parts) > 1 else ""
        job_ok = _valid_id(job)
        echo = job if job_ok else "????????"

        if kind == "J":
            if len(parts) < 4 or not job_ok:
                self._reject(echo, "FMT")
                return
            ptok = parts[2]
            if not (1 <= len(ptok) <= 2 and ptok.isascii() and ptok.isdigit()):
                self._reject(job, "PRIO")
                return
            prio = int(ptok)
            if not 1 <= prio <= 99:
                self._reject(job, "PRIO")
                return
            dtok = parts[3]
            deps: list[str] = []
            fold = job.lower()
            if dtok != "0":
                seen: set[str] = set()
                for part in dtok.split(","):
                    if not _valid_id(part):
                        self._reject(job, "DEPS")
                        return
                    if part.lower() == fold:  # self-dependency
                        self._reject(job, "DEPS")
                        return
                    if part.lower() not in seen:
                        seen.add(part.lower())
                        deps.append(part.lower())
            if fold in self.by_fold:
                self._reject(self.by_fold[fold].name, "DUP")
                return
            j = Job(job, prio, deps, self.def_next)
            self.def_next += 1
            self.jobs.append(j)
            self.by_fold[fold] = j
            self.c_def += 1
            self._emit(f"OK {self.c_def}")
            return

        if kind == "G":
            for j in self.jobs:
                if not j.executed:
                    for d in j.deps:
                        if d not in self.by_fold:
                            j.poisoned = True
                            break
            executed_this = 0
            while True:
                best: Job | None = None
                best_key: tuple | None = None
                for j in self.jobs:
                    if j.executed or j.poisoned:
                        continue
                    if not self._deps_ready(j):
                        continue
                    fresh = j.ord > 0
                    fan = (
                        0
                        if (
                            fresh
                            and len(j.deps) >= 2
                            and j.prio % 10 == 0
                        )
                        else 1
                    )
                    key = (
                        -self._eff(j),
                        0 if not fresh else 1,
                        fan,
                        j.ord,
                    )
                    if best is None or key < best_key:  # type: ignore[operator]
                        best = j
                        best_key = key
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
            fold = job.lower()
            j = self.by_fold.get(fold)
            if j is None:
                self._reject(job, "STATE")
                return
            if not j.executed:
                self._reject(j.name, "STATE")
                return
            invalidated = {fold}
            changed = True
            while changed:
                changed = False
                for k in self.jobs:
                    if not k.executed:
                        continue
                    if k.fold in invalidated:
                        continue
                    if any(d in invalidated for d in k.deps):
                        invalidated.add(k.fold)
                        changed = True
            for k in self.jobs:
                if k.fold in invalidated:
                    k.executed = False
                    k.fails += 1
                    k.ord = -self.stamp_next
                    self.stamp_next += 1
            self.c_fail += 1
            self._emit(f"OK {len(invalidated)}")
            return

        self._reject(echo, "FMT")

    def finish(self) -> None:
        self._emit(f"X {self.c_def} {self.c_exec} {self.c_fail} {self.c_rej}")


def main() -> None:
    engine = Engine()
    data = sys.stdin.buffer.read().split(b"\n")
    for chunk in data:
        try:
            line = chunk.decode("utf-8")
        except UnicodeDecodeError:
            line = chunk.decode("utf-8", errors="replace")
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
