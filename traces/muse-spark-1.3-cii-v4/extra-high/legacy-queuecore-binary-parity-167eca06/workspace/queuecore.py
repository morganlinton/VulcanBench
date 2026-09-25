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
        self.canon: dict[str, str] = {}
        self.item_seq: dict[str, int] = {}
        self.live: set[str] = set()
        self.dead: list[str] = []
        self.dead_set: set[str] = set()
        self.queue: list[tuple[str, int, int]] = []  # (norm, prio, seq)
        self.seq_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        self.hist: list[list] = []  # [[norm, open], ...] oldest first, maxlen 2
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _push(self, norm: str, prio: int, seq: int) -> None:
        self.queue.append((norm, prio, seq))
        self.live.add(norm)

    def _pop_best(self) -> str:
        best = 0
        for i in range(1, len(self.queue)):
            a = self.queue[i]
            b = self.queue[best]
            if a[1] != b[1]:
                if a[1] > b[1]:
                    best = i
            elif a[2] < b[2]:
                best = i
        norm = self.queue.pop(best)[0]
        self.live.discard(norm)
        return norm

    def handle(self, line: str) -> None:
        raw = line.rstrip("\r\n")
        parts = raw.split()
        if not parts:
            if raw == "":
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
            norm = item.lower()
            if norm in self.live or norm in self.dead_set:
                self._emit(f"R {self.canon[norm]} STATE")
                return
            if norm not in self.orig:
                self.fails[norm] = 0
                self.canon[norm] = item
            self.orig[norm] = prio
            seq = self.seq_next
            self.seq_next += 1
            self.item_seq[norm] = seq
            self._push(norm, prio, seq)
            if self.hist:
                if self.hist[-1][0] == norm:
                    for entry in self.hist:
                        entry[1] = False
                elif len(self.hist) == 2 and self.hist[-2][0] == norm:
                    self.hist[-2][1] = False
            self.c_enq += 1
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "D":
            if not self.queue:
                self._emit("EMPTY")
                return
            norm = self._pop_best()
            self.hist.append([norm, True])
            if len(self.hist) > 2:
                del self.hist[0]
            self.c_deq += 1
            self._emit(f"I {self.canon[norm]}")
            return

        if kind == "F":
            if len(parts) < 2 or not item_ok:
                self._emit(f"R {echo} FMT")
                return
            norm = item.lower()
            ok_last = bool(self.hist) and self.hist[-1][0] == norm and self.hist[-1][1]
            ok_prev = (
                len(self.hist) == 2
                and self.hist[-2][0] == norm
                and self.hist[-2][1]
                and not self.hist[-1][1]
            )
            if not (ok_last or ok_prev):
                if norm in self.canon:
                    self._emit(f"R {self.canon[norm]} STATE")
                else:
                    self._emit(f"R {item} STATE")
                return
            if ok_last:
                self.hist[-1][1] = False
            else:
                self.hist[-2][1] = False
            self.fails[norm] += 1
            self.c_fail += 1
            orig0 = self.orig[norm]
            fails = self.fails[norm]
            if fails >= 3:
                dlq = True
            elif 100 <= orig0 <= 999 and orig0 % 100 == 10 and fails == 1:
                dlq = True
            elif 100 <= orig0 <= 999 and orig0 % 100 == 20 and fails == 2:
                dlq = True
            else:
                dlq = False
            if dlq:
                self.dead.append(norm)
                self.dead_set.add(norm)
                self.c_dead += 1
                self._emit(f"DLQ {self.canon[norm]}")
                return
            orig = self.orig[norm]
            eff = orig - 10 * self.fails[norm]
            if orig == 999:
                eff += 1
            if eff < 1:
                eff = 1
            self._push(norm, eff, self.item_seq[norm])
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "K":
            for norm in self.dead:
                self.fails[norm] = 0
                seq = self.seq_next
                self.seq_next += 1
                self.item_seq[norm] = seq
                self._push(norm, self.orig[norm], seq)
            self.dead = []
            self.dead_set = set()
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
