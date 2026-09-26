"""QueueCore work-queue engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: commands on stdin, result lines on stdout, trailer at end
of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


def _valid_item(token: str) -> bool:
    return (
        1 <= len(token) <= 8
        and token.isascii()
        and token.isalnum()
    )


def _valid_prio_token(token: str) -> bool:
    return (
        1 <= len(token) <= 3
        and token.isascii()
        and token.isdigit()
    )


class Engine:
    def __init__(self, out=None):
        self.orig: dict[str, int] = {}
        self.fails: dict[str, int] = {}
        self.eff: dict[str, int] = {}
        self.seq: dict[str, int] = {}
        self.arrival: dict[str, int] = {}
        self.canon: dict[str, str] = {}
        self.born: dict[str, int] = {}
        self.live: set[str] = set()
        self.drained: set[str] = set()
        self.renewed: set[str] = set()
        self.dead: list[str] = []
        self.queue: list[str] = []
        self.seq_next = 1
        self.arrival_next = 1
        self.c_enq = 0
        self.c_deq = 0
        self.c_fail = 0
        self.c_dead = 0
        self.last: str | None = None
        self.second_last: str | None = None
        self.last_open = False
        self.armed = False
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _push(self, key: str) -> None:
        self.queue.append(key)
        self.seq[key] = self.seq_next
        self.seq_next += 1
        self.arrival[key] = self.arrival_next
        self.arrival_next += 1
        self.live.add(key)

    def _beats(self, a: str, b: str) -> bool:
        if self.eff[a] != self.eff[b]:
            return self.eff[a] > self.eff[b]
        if self.fails[a] == 0 and self.fails[b] == 0:
            return self.seq[a] < self.seq[b]
        return self.born[a] < self.born[b]

    def _pop_best(self) -> str:
        best = 0
        for i in range(1, len(self.queue)):
            if self._beats(self.queue[i], self.queue[best]):
                best = i
        key = self.queue.pop(best)
        self.live.discard(key)
        return key

    def handle(self, line: str) -> None:
        raw = line.rstrip("\r\n")
        if raw == "":
            return
        parts = raw.split()
        if not parts:
            self._emit("R ???????? FMT")
            return
        kind = parts[0]
        token = parts[1] if len(parts) > 1 else ""
        item_ok = _valid_item(token)
        typed = token if item_ok else "????????"
        key = token.lower()

        if kind == "N":
            if len(parts) < 3 or not item_ok:
                self._emit(f"R {typed} FMT")
                return
            ptok = parts[2]
            if not _valid_prio_token(ptok):
                self._emit(f"R {token} PRIO")
                return
            prio = int(ptok)
            if not 1 <= prio <= 999:
                self._emit(f"R {token} PRIO")
                return
            if key in self.live or key in self.dead:
                self._emit(f"R {self.canon[key]} STATE")
                return
            if key not in self.orig:
                self.fails[key] = 0
                self.canon[key] = token
            else:
                self.renewed.add(key)
            self.orig[key] = prio
            self.eff[key] = prio
            self._push(key)
            self.born[key] = self.arrival[key]
            self.c_enq += 1
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "D":
            if not self.queue:
                self._emit("EMPTY")
                return
            key = self._pop_best()
            self.second_last = self.last
            self.last = key
            self.last_open = True
            self.armed = False
            self.drained.add(key)
            self.c_deq += 1
            self._emit(f"I {self.canon[key]}")
            return

        if kind == "F":
            if len(parts) < 2 or not item_ok:
                self._emit(f"R {typed} FMT")
                return
            if key in self.live or key in self.dead or key not in self.drained:
                self._emit(f"R {self.canon.get(key, token)} STATE")
                return
            via_last = self.last_open and self.last == key
            via_chain = self.armed and key == self.second_last
            if key in self.renewed and self.fails.get(key, 0) > 0 and not via_last and not via_chain:
                self._emit(f"R {self.canon.get(key, token)} STATE")
                return
            if not via_last and not via_chain:
                self._emit(f"R {self.canon.get(key, token)} STATE")
                return
            self.armed = via_last
            self.fails[key] += 1
            self.c_fail += 1
            if self.fails[key] >= 3:
                self.dead.append(key)
                self.c_dead += 1
                self._emit(f"DLQ {self.canon[key]}")
                return
            eff = self.orig[key] - 10 * self.fails[key]
            if self.orig[key] == 999:
                eff += 1
            if eff < 1:
                eff = 1
            self.eff[key] = eff
            renewed = key in self.renewed
            self._push(key)
            self.seq[key] = self.seq[key] - (2 if renewed else 1)
            self._emit(f"OK {len(self.queue)}")
            return

        if kind == "K":
            for key in self.dead:
                self.fails[key] = 0
                self.eff[key] = self.orig[key]
                self._push(key)
            self.dead = []
            self._emit(f"OK {len(self.queue)}")
            return

        self._emit(f"R {typed} FMT")

    def finish(self) -> None:
        self._emit(f"X {self.c_enq} {self.c_deq} {self.c_fail} {self.c_dead}")


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
