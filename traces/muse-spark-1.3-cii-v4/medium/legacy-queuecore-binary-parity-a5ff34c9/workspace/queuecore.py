"""QueueCore work-queue engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: commands on stdin, result lines on stdout, trailer at end
of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).

Input and output are raw bytes; the engine treats command text as opaque
bytes outside a small ASCII subset, so this module does the same.
"""

from __future__ import annotations

import sys


def _is_alnum(b: int) -> bool:
    return 48 <= b <= 57 or 65 <= b <= 90 or 97 <= b <= 122


def _valid_item(tok: bytes) -> bool:
    return 1 <= len(tok) <= 8 and all(_is_alnum(c) for c in tok)


def _valid_prio(tok: bytes) -> bool:
    return (
        1 <= len(tok) <= 3
        and all(48 <= c <= 57 for c in tok)
        and 1 <= int(tok) <= 999
    )


_Q = b"????????"


class Engine:
    def __init__(self, out=None):
        self.orig: dict[bytes, int] = {}
        self.fails: dict[bytes, int] = {}
        self.shown: dict[bytes, bytes] = {}
        self.live: set[bytes] = set()
        self.dead: list[bytes] = []
        self.deadset: set[bytes] = set()
        self.queue: list[tuple[bytes, int, int]] = []  # (key, prio, seq)
        self.slots: list[list] = []  # [key, seq, open] of last 2 dequeues
        self.seq_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        self.out = out if out is not None else sys.stdout.buffer

    def _emit(self, line: bytes) -> None:
        self.out.write(line + b"\n")

    def _push(self, key: bytes, prio: int, seq: int) -> None:
        self.queue.append((key, prio, seq))
        self.live.add(key)

    def _pop_best(self) -> tuple[bytes, int]:
        best = 0
        for i in range(1, len(self.queue)):
            if self.queue[i][1] > self.queue[best][1] or (
                self.queue[i][1] == self.queue[best][1]
                and self.queue[i][2] < self.queue[best][2]
            ):
                best = i
        key, _, seq = self.queue.pop(best)
        self.live.discard(key)
        return key, seq

    @staticmethod
    def _echo(parts: list[bytes]) -> bytes:
        tok = parts[1] if len(parts) > 1 else b""
        return tok if _valid_item(tok) else _Q

    def handle(self, line: bytes) -> None:
        text = line.rstrip(b"\r\n")
        if text == b"":
            return
        parts = text.split()
        if not parts:
            self._emit(b"R " + _Q + b" FMT")
            return
        kind = parts[0]

        if kind == b"N":
            item = parts[1] if len(parts) > 1 else b""
            if len(parts) < 3 or not _valid_item(item):
                self._emit(b"R " + self._echo(parts) + b" FMT")
                return
            ptok = parts[2]
            if not _valid_prio(ptok):
                self._emit(b"R " + item + b" PRIO")
                return
            prio = int(ptok)
            key = item.lower()
            if key in self.live or key in self.deadset:
                self._emit(b"R " + self.shown[key] + b" STATE")
                return
            if key not in self.orig:
                self.fails[key] = 0
                self.shown[key] = item
            self.orig[key] = prio
            self._push(key, prio, self.seq_next)
            self.seq_next += 1
            for i in range(len(self.slots) - 1, -1, -1):
                if self.slots[i][0] == key:
                    for j in range(i + 1):
                        self.slots[j][2] = False
                    break
            self.c_enq += 1
            self._emit(b"OK " + str(len(self.queue)).encode())
            return

        if kind == b"D":
            if not self.queue:
                self._emit(b"EMPTY")
                return
            key, seq = self._pop_best()
            self.slots.append([key, seq, True])
            if len(self.slots) > 2:
                del self.slots[0]
            self.c_deq += 1
            self._emit(b"I " + self.shown[key])
            return

        if kind == b"F":
            item = parts[1] if len(parts) > 1 else b""
            if len(parts) < 2 or not _valid_item(item):
                self._emit(b"R " + self._echo(parts) + b" FMT")
                return
            key = item.lower()
            target = -1
            for i in range(len(self.slots) - 1, -1, -1):
                if self.slots[i][2]:
                    target = i
                    break
            if target < 0 or self.slots[target][0] != key:
                self._emit(b"R " + self.shown.get(key, item) + b" STATE")
                return
            self.slots[target][2] = False
            self.fails[key] += 1
            self.c_fail += 1
            base = self.orig[key]
            if base == 999:
                base += 1
            prio = base - 10 * self.fails[key]
            if prio < 1:
                prio = 1
            if self.fails[key] >= 3 or prio % 100 == 0:
                self.dead.append(key)
                self.deadset.add(key)
                self.c_dead += 1
                self._emit(b"DLQ " + self.shown[key])
                return
            self._push(key, prio, self.slots[target][1])
            self._emit(b"OK " + str(len(self.queue)).encode())
            return

        if kind == b"K":
            for key in self.dead:
                self.fails[key] = 0
                self._push(key, self.orig[key], self.seq_next)
                self.seq_next += 1
            del self.dead[:]
            self.deadset.clear()
            self._emit(b"OK " + str(len(self.queue)).encode())
            return

        if len(kind) > 7:
            tail = kind[7:]
            echo = tail if _valid_item(tail) else _Q
        else:
            echo = self._echo(parts)
        self._emit(b"R " + echo + b" FMT")

    def finish(self) -> None:
        self._emit(
            f"X {self.c_enq} {self.c_deq} {self.c_fail} {self.c_dead}".encode()
        )


def main() -> None:
    engine = Engine()
    for line in sys.stdin.buffer:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
