"""QueueCore work-queue engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: commands on stdin, result lines on stdout, trailer at end
of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


class Engine:
    def __init__(self, out=None):
        self.orig: dict[str, int] = {}
        self.fails: dict[str, int] = {}
        self.live: set[str] = set()
        self.dead: list[str] = []
        self.queue: list[tuple[str, int, int]] = []  # (item, prio, seq)
        self.seq_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        self.last: str | None = None
        self.last_open = False
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _push(self, item: str, prio: int) -> None:
        self.queue.append((item, prio, self.seq_next))
        self.seq_next += 1
        self.live.add(item)

    def _pop_best(self) -> str:
        best = 0
        for i in range(1, len(self.queue)):
            if self.queue[i][1] > self.queue[best][1] or (
                self.queue[i][1] == self.queue[best][1]
                and self.queue[i][2] < self.queue[best][2]
            ):
                best = i
        item = self.queue.pop(best)[0]
        self.live.discard(item)
        return item

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]
        item = parts[1] if len(parts) > 1 else ""
        item_ok = 1 <= len(item) <= 8 and item.isalnum()
        echo = item if item_ok else "????????"

        if kind == "N":
            if len(parts) != 3 or not item_ok:
                self._emit(f"R {echo} FMT")
                return
            ptok = parts[2]
            if not (ptok.isdigit() and 1 <= len(ptok) <= 3):
                self._emit(f"R {item} PRIO")
                return
            prio = int(ptok)
            if not 1 <= prio <= 999:
                self._emit(f"R {item} PRIO")
                return
            if item in self.live or item in self.dead:
                self._emit(f"R {item} STATE")
                return
            if item not in self.orig:
                self.fails[item] = 0
            self.orig[item] = prio
            self._push(item, prio)
            if self.last == item:
                self.last_open = False
            self.c_enq += 1
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "D":
            if len(parts) != 1:
                self._emit(f"R {echo} FMT")
                return
            if not self.queue:
                self._emit("EMPTY")
                return
            item = self._pop_best()
            self.last = item
            self.last_open = True
            self.c_deq += 1
            self._emit(f"I {item}")
            return

        if kind == "F":
            if len(parts) != 2 or not item_ok:
                self._emit(f"R {echo} FMT")
                return
            if not (self.last_open and self.last == item):
                self._emit(f"R {item} STATE")
                return
            self.last_open = False
            self.fails[item] += 1
            self.c_fail += 1
            if self.fails[item] >= 3:
                self.dead.append(item)
                self.c_dead += 1
                self._emit(f"DLQ {item}")
                return
            self._push(item, self.orig[item])
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "K":
            if len(parts) != 1:
                self._emit(f"R {echo} FMT")
                return
            for item in self.dead:
                self.fails[item] = 0
                self._push(item, self.orig[item])
            self.dead = []
            self._emit(f"OK {len(self.queue)}")
            return

        self._emit(f"R {echo} FMT")

    def finish(self) -> None:
        self._emit(f"X {self.c_enq} {self.c_deq} {self.c_fail} {self.c_dead}")


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
