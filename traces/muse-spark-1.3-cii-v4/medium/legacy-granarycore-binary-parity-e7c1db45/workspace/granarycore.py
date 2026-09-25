"""GranaryCore co-op ledger engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One
session per process: commands on stdin, result lines on stdout, a
trailer at end of input. Format reference: ``docs/SPEC.md`` (mind the
drift warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys


def _id_ok(s: str) -> bool:
    return 1 <= len(s) <= 8 and s.isalnum() and s.isascii()


def _num_ok(s: str, maxlen: int) -> bool:
    return 1 <= len(s) <= maxlen and s.isdigit() and s.isascii()


def _cdiv(a: int, b: int) -> int:
    """C-style truncation division (b > 0)."""
    q = abs(a) // b
    return q if a >= 0 else -q


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.shares: dict[str, int] = {}
        self.mseq: dict[str, int] = {}
        self.morder: list[str] = []
        self.mpen: dict[str, int] = {}
        self.bins: dict[str, dict] = {}
        self.folded: set[str] = set()
        self.next_mseq = 0
        self.next_bseq = 0
        self.c_j = 0
        self.c_o = 0
        self.c_p = 0
        self.c_w = 0
        self.c_m = 0
        self.c_t = 0
        self.c_y = 0

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, code: str) -> None:
        self._emit(f"R {code}")

    def _held_total(self, name: str) -> int:
        return sum(b["holds"].get(name, 0) for b in self.bins.values())

    def _footprint(self, name: str) -> int:
        return sum(1 for b in self.bins.values() if b["holds"].get(name, 0) > 0)

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]

        if kind == "J":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _num_ok(parts[2], 2):
                self._reject("FMT")
                return
            val = int(parts[2])
            if not 1 <= val <= 99:
                self._reject("RNG")
                return
            if parts[1] in self.shares:
                self._reject("DUP")
                return
            self.next_mseq += 1
            self.shares[parts[1]] = val
            self.mseq[parts[1]] = self.next_mseq
            self.morder.append(parts[1])
            self.mpen[parts[1]] = 0
            self.c_j += 1
            self._emit(f"OK {len(self.morder)}")
            return

        if kind == "O":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _num_ok(parts[2], 3):
                self._reject("FMT")
                return
            cap = int(parts[2])
            if not 1 <= cap <= 999:
                self._reject("RNG")
                return
            if parts[1] in self.bins or parts[1] in self.folded:
                self._reject("DUP")
                return
            self.next_bseq += 1
            self.bins[parts[1]] = {
                "cap": cap,
                "seq": self.next_bseq,
                "total": 0,
                "holders": [],
                "holds": {},
                "pen": 0,
                "last": 15,
                "flast": 0,
                "first": 0,
                "turns": 0,
            }
            self.c_o += 1
            self._emit(f"OK {len(self.bins)}")
            return

        if kind == "P":
            if len(parts) != 4 or not _id_ok(parts[1]) or not _id_ok(parts[2]) \
                    or not _num_ok(parts[3], 3):
                self._reject("FMT")
                return
            count = int(parts[3])
            if not 1 <= count <= 999:
                self._reject("RNG")
                return
            if parts[1] not in self.shares:
                self._reject("WHO")
                return
            if parts[2] not in self.bins:
                self._reject("LOC")
                return
            b = self.bins[parts[2]]
            if b["total"] + count > b["cap"]:
                self._reject("FULL")
                return
            b["total"] += count
            b["holds"][parts[1]] = b["holds"].get(parts[1], 0) + count
            if parts[1] not in b["holders"]:
                b["holders"].append(parts[1])
            self.c_p += 1
            self._emit("OK")
            return

        if kind == "W":
            if len(parts) != 4 or not _id_ok(parts[1]) or not _id_ok(parts[2]) \
                    or not _num_ok(parts[3], 3):
                self._reject("FMT")
                return
            count = int(parts[3])
            if not 1 <= count <= 999:
                self._reject("RNG")
                return
            if parts[1] not in self.shares:
                self._reject("WHO")
                return
            if parts[2] not in self.bins:
                self._reject("LOC")
                return
            b = self.bins[parts[2]]
            if b["holds"].get(parts[1], 0) < count:
                self._reject("LOW")
                return
            b["total"] -= count
            b["holds"][parts[1]] -= count
            b["pen"] += count // 6
            self.mpen[parts[1]] = self.mpen.get(parts[1], 0) + count // 4
            self.c_w += 1
            self._emit("OK")
            return

        if kind == "M":
            if len(parts) != 5 or not _id_ok(parts[1]) or not _id_ok(parts[2]) \
                    or not _id_ok(parts[3]) or not _num_ok(parts[4], 3):
                self._reject("FMT")
                return
            count = int(parts[4])
            if not 1 <= count <= 999:
                self._reject("RNG")
                return
            if parts[1] not in self.shares:
                self._reject("WHO")
                return
            if parts[2] not in self.bins:
                self._reject("LOC")
                return
            if parts[3] not in self.bins:
                self._reject("LOC")
                return
            if parts[2] == parts[3]:
                self._reject("DUP")
                return
            src = self.bins[parts[2]]
            dst = self.bins[parts[3]]
            if src["holds"].get(parts[1], 0) < count:
                self._reject("LOW")
                return
            if dst["total"] + count > dst["cap"]:
                self._reject("FULL")
                return
            src["total"] -= count
            src["holds"][parts[1]] -= count
            if src["holds"][parts[1]] == 0:
                del src["holds"][parts[1]]
                if parts[1] in src["holders"]:
                    src["holders"].remove(parts[1])
            dst["total"] += count
            dst["holds"][parts[1]] = dst["holds"].get(parts[1], 0) + count
            if parts[1] not in dst["holders"]:
                dst["holders"].append(parts[1])
            dst["pen"] += count // 10
            self.mpen[parts[1]] = self.mpen.get(parts[1], 0) + 1
            self.c_m += 1
            self._emit("OK")
            return

        if kind == "T":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_t += 1
            order = sorted(
                self.bins.items(),
                key=lambda kv: (
                    -(kv[1]["total"] - kv[1]["pen"]
                      - (1 if kv[1]["turns"] > 0 else 0)
                      - kv[1]["pen"] // 2 - kv[1]["first"] // 17),
                    len(kv[1]["holders"]),
                    kv[1]["last"],
                    -kv[1]["total"],
                    kv[1]["seq"],
                ),
            )
            for _, b in order:
                b["last"] = b["total"] - b["pen"]
            self._emit(f"TURN {len(order)}")
            for name, _ in order:
                self._emit(f"B {name}")
            alive = set(self.bins)
            for name, b in order:
                if name not in alive:
                    continue
                if b["total"] == 0:
                    self._emit(f"SHUT {name}")
                    del self.bins[name]
                    alive.discard(name)
                elif b["total"] < 2 * b["pen"] + 2 * _cdiv(b["first"], 16) \
                        + 2 * _cdiv(b["flast"], 17) \
                        + max(0, b["turns"] - 2) * 2 * _cdiv(b["flast"] - 4, 16):
                    recv = None
                    for rname, rb in order:
                        if rname not in alive or rname == name:
                            continue
                        if rb["total"] >= 2 * (rb["pen"] + _cdiv(rb["flast"], 16)) \
                                and rb["total"] + b["total"] <= rb["cap"]:
                            recv = rname
                            break
                    if recv is not None:
                        rb = self.bins[recv]
                        self._emit(f"FOLD {name} {recv}")
                        for holder in b["holders"]:
                            amt = b["holds"].get(holder, 0)
                            if amt > 0:
                                rb["holds"][holder] = rb["holds"].get(holder, 0) + amt
                                if holder not in rb["holders"]:
                                    rb["holders"].append(holder)
                        rb["total"] += b["total"]
                        del self.bins[name]
                        alive.discard(name)
                        self.folded.add(name)
                        self.c_m += 1
            for b in self.bins.values():
                b["flast"] = b["first"]
                if b["first"] == 0:
                    b["first"] = b["total"] - b["pen"]
                b["turns"] += 1
            return

        if kind == "Y":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_y += 1
            self.morder.sort(
                key=lambda n: (
                    -(self._held_total(n) - self.mpen.get(n, 0)),
                    self.mseq[n],
                )
            )
            self._emit(f"RECK {len(self.morder)}")
            for name in self.morder:
                self._emit(f"S {name}")
            for b in self.bins.values():
                b["pen"] = _cdiv(b["pen"] + 1, 2)
                b["turns"] = 0
                b["flast"] = 0
            return

        if kind == "L":
            if len(parts) != 2 or not _id_ok(parts[1]):
                self._reject("FMT")
                return
            if parts[1] not in self.bins:
                self._reject("LOC")
                return
            b = self.bins[parts[1]]
            self._emit(f"HELD {parts[1]} {len(b['holders'])}")
            for name in b["holders"]:
                self._emit(f"H {name}")
            return

        self._reject("FMT")

    def finish(self) -> None:
        self._emit(
            f"Z {self.c_j} {self.c_o} {self.c_p} {self.c_w} "
            f"{self.c_m} {self.c_t} {self.c_y}"
        )


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
