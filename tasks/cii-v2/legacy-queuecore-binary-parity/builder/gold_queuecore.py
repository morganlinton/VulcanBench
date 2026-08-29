"""QueueCore work-queue engine, Python implementation (engine-faithful).

Replaces the retired legacy binary. Every deviation from docs/SPEC.md is
marked ``engine:`` - the engine's behavior is the contract.
"""

from __future__ import annotations

import sys

U_EMPTY, U_OPEN, U_FAILED, U_CLOSED = 0, 1, 2, 3


def _requeue_prio(orig: int, fails: int) -> int:
    # engine: the requeue path computes from an internal 1000
    # representation for priority 999, then applies 10 per accumulated
    # failure, clamped to 1..999. A 999 item's first requeue lands at
    # 990, not 989.
    base = 1000 if orig == 999 else orig
    return max(1, min(999, base - 10 * fails))


class Engine:
    def __init__(self, out=None):
        # engine: item ids fold case for identity; the first-seen
        # spelling is what output lines echo.
        self.seen: dict[str, str] = {}
        self.orig: dict[str, int] = {}
        self.fails: dict[str, int] = {}
        self.seq: dict[str, int] = {}
        self.live: set[str] = set()
        self.dead: list[str] = []
        self.queue: list[tuple[str, int, int]] = []  # (fold, prio, seq)
        self.seq_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        self.u1: str | None = None
        self.u2: str | None = None
        self.u1_state = U_EMPTY
        self.u2_state = U_EMPTY
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _push(self, fold: str, prio: int, seq: int) -> None:
        self.queue.append((fold, prio, seq))
        self.live.add(fold)

    def _pop_best(self) -> str:
        best = 0
        for i in range(1, len(self.queue)):
            if self.queue[i][1] > self.queue[best][1] or (
                self.queue[i][1] == self.queue[best][1]
                and self.queue[i][2] < self.queue[best][2]
            ):
                best = i
        fold = self.queue.pop(best)[0]
        self.live.discard(fold)
        return fold

    def _deadletter(self, fold: str) -> None:
        self.dead.append(fold)
        self.c_dead += 1
        self._emit(f"DLQ {self.seen[fold]}")

    def _do_fail(self, fold: str, slot: int) -> None:
        self.fails[fold] += 1
        self.c_fail += 1
        if slot == 1:
            self.u1_state = U_FAILED
        else:
            self.u2_state = U_FAILED
        if self.fails[fold] >= 3:
            self._deadletter(fold)
            return
        p = _requeue_prio(self.orig[fold], self.fails[fold])
        if p % 100 == 0:
            # engine: a penalty landing exactly on a multiple of 100
            # collides with an internal sentinel and dead-letters the
            # item immediately, regardless of fail count.
            self._deadletter(fold)
            return
        # engine: requeued items keep their ORIGINAL arrival sequence.
        self._push(fold, p, self.seq[fold])
        self._emit(f"OK {len(self.queue)}")

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]
        item = parts[1] if len(parts) > 1 else ""
        item_ok = 1 <= len(item) <= 8 and item.isalnum()
        echo = item if item_ok else "????????"

        if kind == "N":
            # engine: extra trailing tokens are tolerated.
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
            fold = item.lower()
            if fold in self.seen and (fold in self.live or fold in self.dead):
                self._emit(f"R {self.seen[fold]} STATE")
                return
            if fold not in self.seen:
                self.seen[fold] = item
                self.fails[fold] = 0
            self.orig[fold] = prio
            self.seq[fold] = self.seq_next
            self.seq_next += 1
            self._push(fold, prio, self.seq[fold])
            if self.u1 == fold and self.u1_state == U_OPEN:
                self.u1_state = U_CLOSED
            if self.u2 == fold and self.u2_state == U_OPEN:
                self.u2_state = U_CLOSED
            self.c_enq += 1
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "D":
            # engine: extra trailing tokens are tolerated.
            if not self.queue:
                self._emit("EMPTY")
                return
            fold = self._pop_best()
            self.u2, self.u2_state = self.u1, self.u1_state
            self.u1, self.u1_state = fold, U_OPEN
            self.c_deq += 1
            self._emit(f"I {self.seen[fold]}")
            return

        if kind == "F":
            if len(parts) < 2 or not item_ok:
                self._emit(f"R {echo} FMT")
                return
            fold = item.lower()
            known = fold in self.seen
            if known and self.u1 == fold and self.u1_state == U_OPEN:
                self._do_fail(fold, 1)
                return
            # engine: two-deep undo buffer; the second-most-recent
            # dequeue is failable once the most recent was failed.
            if (
                known
                and self.u1_state == U_FAILED
                and self.u2 == fold
                and self.u2_state == U_OPEN
            ):
                self._do_fail(fold, 2)
                return
            self._emit(f"R {self.seen[fold] if known else item} STATE")
            return

        if kind == "K":
            # engine: extra trailing tokens are tolerated. Drained items
            # go through the requeue path with fails reset and FRESH
            # sequence numbers.
            for fold in self.dead:
                self.fails[fold] = 0
                self.seq[fold] = self.seq_next
                self.seq_next += 1
                self._push(fold, _requeue_prio(self.orig[fold], 0), self.seq[fold])
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
