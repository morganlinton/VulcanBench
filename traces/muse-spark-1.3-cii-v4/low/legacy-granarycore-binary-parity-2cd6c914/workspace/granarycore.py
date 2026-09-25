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


def _seas_bonus(s: int) -> int:
    if s >= 0:
        return (s + 5) // 15
    return s // 15


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.shares: dict[str, int] = {}
        self.mseq: dict[str, int] = {}
        self.morder: list[str] = []
        self.mdemerit: dict[str, int] = {}
        self.bins: dict[str, dict] = {}
        self.dead: set[str] = set()
        self.next_mseq = 0
        self.next_bseq = 0
        self.c_j = 0
        self.c_o = 0
        self.c_p = 0
        self.c_w = 0
        self.c_m = 0
        self.c_t = 0
        self.c_y = 0
        self.t_active = False
        self.score = {}

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, code: str) -> None:
        self._emit(f"R {code}")

    def _drop_if_empty(self, b: dict, name: str) -> None:
        if b["holds"].get(name, 0) == 0:
            b["holds"].pop(name, None)
            if name in b["holders"]:
                b["holders"].remove(name)

    def _held_total(self, name: str) -> int:
        return sum(b["holds"].get(name, 0) for b in self.bins.values())

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
            self.mdemerit[parts[1]] = 0
            self.morder.append(parts[1])
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
            if parts[1] in self.bins or parts[1] in self.dead:
                self._reject("DUP")
                return
            self.next_bseq += 1
            self.bins[parts[1]] = {
                "cap": cap,
                "seq": self.next_bseq,
                "total": 0,
                "holders": [],
                "holds": {},
                "dem": 0,
                "levy": 0,
                "drawn": 0,
                "forgiven": 0,
                "season": 0,
                "dirty": True,
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
            b["season"] += count
            b["dirty"] = True
            b["touched"] = True
            self.c_p += 1
            self.t_active = True
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
            b["dem"] += count // 6
            b["drawn"] += count
            b["season"] -= count
            b["dirty"] = True
            b["touched"] = True
            self.mdemerit[parts[1]] += count // 4
            self.c_w += 1
            self.t_active = True
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
            room = dst["total"] + count <= dst["cap"]
            if room:
                src["dirty"] = True
                dst["dirty"] = True
            if src["holds"].get(parts[1], 0) < count:
                self._reject("LOW")
                return
            if not room:
                self._reject("FULL")
                return
            src["total"] -= count
            src["holds"][parts[1]] -= count
            self._drop_if_empty(src, parts[1])
            src["season"] -= count
            dst["total"] += count
            dst["season"] += count
            dst["holds"][parts[1]] = dst["holds"].get(parts[1], 0) + count
            if parts[1] not in dst["holders"]:
                dst["holders"].append(parts[1])
            dst["dem"] += (count + 1) // 11
            src["dirty"] = True
            dst["dirty"] = True
            src["touched"] = True
            dst["touched"] = True
            self.mdemerit[parts[1]] += 1
            self.c_m += 1
            self.t_active = True
            self._emit("OK")
            return

        if kind == "T":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_t += 1
            for n, b in self.bins.items():
                if n not in self.score or b.get("dirty"):
                    self.score[n] = (
                        b["total"] - b["dem"] - b["levy"]
                        + _seas_bonus(b["season"])
                    )
                    b["dirty"] = False
                if not b.get("touched"):
                    b["levy"] += b["total"] // 16
                b["touched"] = False
            score = self.score
            order = sorted(
                self.bins.items(),
                key=lambda kv: (
                    -score[kv[0]],
                    len(kv[1]["holders"]),
                    kv[1]["seq"],
                ),
            )
            self._emit(f"TURN {len(order)}")
            for name, _ in order:
                self._emit(f"B {name}")
            folders = {
                n
                for n, b in self.bins.items()
                if b["total"] > 0
                and 6 * b["dem"] + 3 * b["levy"]
                >= 2 * b["total"] + (1 if b["forgiven"] > 0 else 0)
            }
            for name, b in order:
                if name not in self.bins:
                    continue
                if b["total"] == 0:
                    self._emit(f"SHUT {name}")
                    del self.bins[name]
                elif name in folders:
                    recv = None
                    for cn, cb in order:
                        if cn == name or cn in folders or cn not in self.bins:
                            continue
                        if cb["total"] + b["total"] > cb["cap"]:
                            continue
                        if recv is None or score[cn] > score[recv]:
                            recv = cn
                    if recv is None:
                        continue
                    dst = self.bins[recv]
                    for holder in b["holders"]:
                        amt = b["holds"].get(holder, 0)
                        if amt == 0:
                            continue
                        dst["holds"][holder] = dst["holds"].get(holder, 0) + amt
                        if holder not in dst["holders"]:
                            dst["holders"].append(holder)
                    dst["total"] += b["total"]
                    self.c_m += 1
                    self.score[recv] = self.score.get(recv, 0) + b["total"]
                    self._emit(f"FOLD {name} {recv}")
                    del self.bins[name]
                    self.dead.add(name)
            for b in self.bins.values():
                b["season"] = 0
            return

        if kind == "Y":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_y += 1
            for b in self.bins.values():
                b["dem"] //= 2
                b["levy"] = 0
                cap = 3 * (b["total"] ^ 1)
                b["forgiven"] += min(b["drawn"], cap)
                if b["forgiven"] > cap:
                    b["forgiven"] += cap
                b["season"] = 0
            ranked = sorted(
                self.morder,
                key=lambda n: (
                    -(self._held_total(n) - self.mdemerit[n]),
                    self.mseq[n],
                ),
            )
            self._emit(f"RECK {len(ranked)}")
            for name in ranked:
                self._emit(f"S {name}")
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
