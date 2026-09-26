"""SchedCore dependency scheduler, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


def _fold(name: str) -> str:
    """Case-insensitive job-id key (engine folds ASCII case)."""
    return "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c for c in name)


def _is_job_id(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and all(c.isascii() and c.isalnum() for c in tok)


class Job:
    __slots__ = ("name", "key", "prio", "deps", "defseq", "executed",
                 "fails", "poisoned")

    def __init__(self, name: str, prio: int, deps: list[str], defseq: int):
        self.name = name
        self.key = _fold(name)
        self.prio = prio
        self.deps = deps  # folded dep keys
        self.defseq = defseq
        self.executed = False
        self.fails = 0
        self.poisoned = False

    def eff_prio(self) -> int:
        if self.fails <= 0:
            bonus = 1 if (len(set(self.deps)) >= 2
                          and self.prio % 10 == 0) else 0
            return self.prio + bonus
        return max(1, self.prio - 7 * (self.fails - 1))


class Engine:
    def __init__(self, out=None):
        self.jobs: list[Job] = []
        self.by_key: dict[str, Job] = {}
        self.def_next = 1
        self.neg_next = -1
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

    def _deps_ready(self, j: Job) -> bool:
        for d in j.deps:
            k = self.by_key.get(d)
            if k is None or not k.executed:
                return False
        return True

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]
        job = parts[1] if len(parts) > 1 else ""
        job_ok = _is_job_id(job)
        echo = job if job_ok else "????????"

        if kind == "J":
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
                jkey = _fold(job)
                for part in dtok.split(","):
                    if not _is_job_id(part):
                        bad = True
                        break
                    if _fold(part) == jkey:  # self-dependency
                        bad = True
                        break
                    deps.append(_fold(part))
            if bad:
                self._reject(job, "DEPS")
                return
            if _fold(job) in self.by_key:
                self._reject(self.by_key[_fold(job)].name, "DUP")
                return
            j = Job(job, prio, deps, self.def_next)
            self.def_next += 1
            self.jobs.append(j)
            self.by_key[j.key] = j
            self.c_def += 1
            self._emit(f"OK {self.c_def}")
            return

        if kind == "G":
            # Poison: deps missing at a G never resolve later.
            for j in self.jobs:
                if j.executed or j.poisoned:
                    continue
                for d in j.deps:
                    if d not in self.by_key:
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
                        or j.eff_prio() > best.eff_prio()
                        or (j.eff_prio() == best.eff_prio()
                            and j.defseq < best.defseq)
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
            j = self.by_key.get(_fold(job))
            if j is None or not j.executed:
                self._reject(j.name if j is not None else job, "STATE")
                return
            j.executed = False
            j.fails += 1
            j.defseq = self.neg_next
            self.neg_next -= 1
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
                        k.fails += 1
                        k.defseq = self.neg_next
                        self.neg_next -= 1
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
