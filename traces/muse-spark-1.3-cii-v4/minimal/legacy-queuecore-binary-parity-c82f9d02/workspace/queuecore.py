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
        self.display: dict[str, str] = {}
        self.fails: dict[str, int] = {}
        self.live: set[str] = set()
        self.dead: list[str] = []
        self.queue: list[tuple[str, int, int]] = []  # (canon, prio, seq)
        self.seq_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        # last 2 dequeues; accepted F marks (not removes) the entry
        self.recent: list[list] = []  # [canon, prio, seq, consumed]
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    @staticmethod
    def _canon(item: str) -> str:
        return item.upper()

    def _show(self, canon: str, given: str) -> str:
        return self.display.get(canon, given)

    def _push(self, canon: str, prio: int, seq: int | None = None) -> None:
        if seq is None:
            seq = self.seq_next
            self.seq_next += 1
        self.queue.append((canon, prio, seq))
        self.live.add(canon)

    def _pop_best(self) -> tuple[str, int, int]:
        best = 0
        for i in range(1, len(self.queue)):
            if self.queue[i][1] > self.queue[best][1] or (
                self.queue[i][1] == self.queue[best][1]
                and self.queue[i][2] < self.queue[best][2]
            ):
                best = i
        entry = self.queue.pop(best)
        self.live.discard(entry[0])
        return entry

    def handle(self, line: str) -> None:
        stripped = line.rstrip("\r\n")
        parts = stripped.split()
        if not parts:
            if stripped == "":
                return
            self._emit("R ???????? FMT")
            return
        kind = parts[0]
        item = parts[1] if len(parts) > 1 else ""
        item_ok = 1 <= len(item) <= 8 and item.isalnum()
        echo = item if item_ok else "????????"

        if kind == "N":
            if len(parts) < 3 or not item_ok:
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
            canon = self._canon(item)
            if canon in self.live or canon in self.dead:
                self._emit(f"R {self.display[canon]} STATE")
                return
            if canon not in self.orig:
                self.fails[canon] = 0
                self.display[canon] = item
            self.orig[canon] = prio
            self._push(canon, prio)
            self.c_enq += 1
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "D":
            if not self.queue:
                self._emit("EMPTY")
                return
            canon, prio, seq = self._pop_best()
            self.recent.append([canon, prio, seq, False])
            del self.recent[:-2]
            self.c_deq += 1
            self._emit(f"I {self.display[canon]}")
            return

        if kind == "F":
            if len(parts) < 2 or not item_ok:
                self._emit(f"R {echo} FMT")
                return
            canon = self._canon(item)
            target = None
            for entry in reversed(self.recent):
                if not entry[3]:
                    target = entry
                    break
            if target is None or target[0] != canon or canon in self.live:
                self._emit(f"R {self._show(canon, item)} STATE")
                return
            for entry in self.recent:
                if entry[0] == canon:
                    entry[3] = True
            _, prio, seq, _ = target
            self.fails[canon] += 1
            self.c_fail += 1
            if self.fails[canon] >= 3:
                self.dead.append(canon)
                self.c_dead += 1
                self._emit(f"DLQ {self.display[canon]}")
                return
            retry_prio = self.orig[canon] - 10 * self.fails[canon]
            if retry_prio < 1:
                retry_prio = 1
            self._push(canon, retry_prio, seq)
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "K":
            for canon in self.dead:
                self.fails[canon] = 0
                self._push(canon, self.orig[canon])
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
