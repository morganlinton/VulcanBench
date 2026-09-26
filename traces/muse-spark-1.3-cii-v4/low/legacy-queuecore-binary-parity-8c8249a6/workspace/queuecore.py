"""QueueCore work-queue engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: commands on stdin, result lines on stdout, trailer at end
of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


def _item_ok(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and tok.isascii() and tok.isalnum()


def _echo(tok: str) -> str:
    return tok if _item_ok(tok) else "????????"


class Engine:
    def __init__(self, out=None):
        self.orig: dict[str, int] = {}
        self.fails: dict[str, int] = {}
        self.gen: dict[str, int] = {}
        self.seq: dict[str, int] = {}
        self.stored: dict[str, str] = {}
        self.live: set[str] = set()
        self.dead: list[str] = []
        self.deadset: set[str] = set()
        self.queue: list[tuple[str, int, int]] = []  # (key, prio, seq)
        self.pending: list[tuple[str, int, int]] = []  # last <= 2 dequeues
        self.lastd: tuple[str, int] | None = None
        self.seq_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _push_new(self, key: str, prio: int) -> None:
        self.queue.append((key, prio, self.seq_next))
        self.seq_next += 1
        self.live.add(key)

    def _pop_best(self) -> str:
        best = 0
        for i in range(1, len(self.queue)):
            if self.queue[i][1] > self.queue[best][1] or (
                self.queue[i][1] == self.queue[best][1]
                and self.queue[i][2] < self.queue[best][2]
            ):
                best = i
        key = self.queue.pop(best)[0]
        self.live.discard(key)
        return key

    def handle(self, line: str) -> None:
        stripped = line.rstrip("\r\n")
        if stripped == "":
            return
        parts = stripped.split()
        if not parts:
            self._emit("R ???????? FMT")
            return
        kind = parts[0]

        if kind == "N":
            item = parts[1] if len(parts) > 1 else ""
            if len(parts) < 3 or not _item_ok(item):
                self._emit(f"R {_echo(item)} FMT")
                return
            ptok = parts[2]
            if not (
                ptok.isascii()
                and ptok.isdigit()
                and 1 <= len(ptok) <= 3
                and 1 <= int(ptok) <= 999
            ):
                self._emit(f"R {item} PRIO")
                return
            prio = int(ptok)
            key = item.lower()
            if key in self.live or key in self.deadset:
                self._emit(f"R {self.stored[key]} STATE")
                return
            if key not in self.stored:
                self.stored[key] = item
            self.gen[key] = self.gen.get(key, 0) + 1
            if key not in self.fails:
                self.fails[key] = 0
            self.orig[key] = prio
            self.seq[key] = self.seq_next
            self._push_new(key, prio)
            self.c_enq += 1
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "D":
            if not self.queue:
                self._emit("EMPTY")
                return
            key = self._pop_best()
            self.pending.append((key, self.gen[key], self.c_fail))
            del self.pending[:-2]
            self.lastd = (key, self.gen[key])
            self.c_deq += 1
            self._emit(f"I {self.stored[key]}")
            return

        if kind == "F":
            item = parts[1] if len(parts) > 1 else ""
            if len(parts) < 2 or not _item_ok(item):
                self._emit(f"R {_echo(item)} FMT")
                return
            key = item.lower()
            if (
                not self.pending
                or self.pending[-1][:2] != (key, self.gen.get(key, 0))
                or (
                    (key, self.gen.get(key, 0)) != self.lastd
                    and self.c_fail - self.pending[-1][2] > 1
                )
            ):
                self._emit(f"R {self.stored.get(key, item)} STATE")
                return
            self.pending.pop()
            self.fails[key] += 1
            self.c_fail += 1
            if self.fails[key] >= 3:
                self.dead.append(key)
                self.deadset.add(key)
                self.c_dead += 1
                self._emit(f"DLQ {self.stored[key]}")
                return
            cand = self.orig[key] - 10 * self.fails[key]
            if cand < 1:
                cand = 1
            if cand % 100 == 0:
                self.dead.append(key)
                self.deadset.add(key)
                self.c_dead += 1
                self._emit(f"DLQ {self.stored[key]}")
                return
            self.queue.append((key, cand, self.seq[key]))
            self.live.add(key)
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "K":
            for key in self.dead:
                self.fails[key] = 0
                self._push_new(key, self.orig[key])
            self.dead = []
            self.deadset = set()
            self._emit(f"OK {len(self.queue)}")
            return

        self._emit(f"R {_echo(parts[1] if len(parts) > 1 else '')} FMT")

    def finish(self) -> None:
        self._emit(f"X {self.c_enq} {self.c_deq} {self.c_fail} {self.c_dead}")


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
