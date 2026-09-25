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


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.shares: dict[str, int] = {}
        self.mseq: dict[str, int] = {}
        self.morder: list[str] = []
        # per-member activity (for reckonings)
        self.mwsq: dict[str, int] = {}
        self.mweven: dict[str, int] = {}
        self.mminvol: dict[str, int] = {}
        self.mminsq: dict[str, int] = {}
        self.bins: dict[str, dict] = {}
        self.next_mseq = 0
        self.next_bseq = 0
        # persistent name sets (survive shut; folds never reopen)
        self.latched: set[str] = set()
        self.folded: set[str] = set()
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

    @staticmethod
    def _decay(total: int, iters: int) -> int:
        s = total
        for _ in range(iters):
            s -= s // 16
        return s

    def _score(self, b: dict, maxfundt: int) -> int:
        fundt = b["fundt"] if b["fundt"] is not None else maxfundt
        return self._decay(b["total"], maxfundt - fundt)

    def _tkey(self, name: str, b: dict, folder: bool, maxfundt: int):
        forfeit = 1 if (6 * b["wsq"] > b["total"] and name not in self.latched) else 0
        eff = self._score(b, maxfundt) + (1 if name in self.latched else 0)
        return (
            1 if folder else 0,
            -eff,
            forfeit,
            b["seq"],
        )

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
            self.mwsq[parts[1]] = 0
            self.mweven[parts[1]] = 0
            self.mminvol[parts[1]] = 0
            self.mminsq[parts[1]] = 0
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
                "fundt": None,
                "total": 0,
                "holders": [],
                "holds": {},
                "wsq": 0,
                "weven": 0,
                "minsq": 0,
                "mincount": 0,
                "minmax": None,
                "minmin": None,
                "minrange": 0,
                "prange": 0,
                "pmax": None,
                "pmin": None,
                "pcount": 0,
                "pvol": 0,
                "moutvol": 0,
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
            if b["fundt"] is None:
                b["fundt"] = self.c_t
            b["holds"][parts[1]] = b["holds"].get(parts[1], 0) + count
            if parts[1] not in b["holders"]:
                b["holders"].append(parts[1])
            b["pcount"] += 1
            b["pvol"] += count
            if b["pmax"] is None or count > b["pmax"]:
                b["pmax"] = count
            if b["pmin"] is None or count < b["pmin"]:
                b["pmin"] = count
            b["prange"] = b["pmax"] - b["pmin"]
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
            # a drawn-down holding stays on the roster (keeps its place)
            b["wsq"] += count * count
            if count % 2 == 0:
                b["weven"] += 1
            self.mwsq[parts[1]] += count * count
            if count % 2 == 0:
                self.mweven[parts[1]] += 1
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
                src["holds"].pop(parts[1], None)
                if parts[1] in src["holders"]:
                    src["holders"].remove(parts[1])
            src["moutvol"] += count
            dst["total"] += count
            if dst["fundt"] is None:
                dst["fundt"] = self.c_t
            dst["holds"][parts[1]] = dst["holds"].get(parts[1], 0) + count
            if parts[1] not in dst["holders"]:
                dst["holders"].append(parts[1])
            dst["minsq"] += count * count
            dst["mincount"] += 1
            if dst["minmax"] is None or count > dst["minmax"]:
                dst["minmax"] = count
            if dst["minmin"] is None or count < dst["minmin"]:
                dst["minmin"] = count
            dst["minrange"] = dst["minmax"] - dst["minmin"]
            self.mminvol[parts[1]] += count
            self.mminsq[parts[1]] += count * count
            self.c_m += 1
            self._emit("OK")
            return

        if kind == "T":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_t += 1
            maxfundt = 0
            for b in self.bins.values():
                if b["fundt"] is not None and b["fundt"] > maxfundt:
                    maxfundt = b["fundt"]
            if self.bins:
                folders = {
                    name
                    for name, b in self.bins.items()
                    if b["total"] > 0 and 9 * b["total"] < 2 * (b["pvol"] - b["moutvol"])
                }
            else:
                folders = set()
            scores = {
                name: self._score(b, maxfundt) for name, b in self.bins.items()
            }
            order = sorted(
                self.bins.items(),
                key=lambda kv: self._tkey(kv[0], kv[1], kv[0] in folders, maxfundt),
            )
            # latch bins trailing below the top score for future turnings
            if scores:
                topscore = max(scores.values())
                for name, s in scores.items():
                    if s < topscore:
                        self.latched.add(name)
            self._emit(f"TURN {len(order)}")
            for name, _ in order:
                self._emit(f"B {name}")
            for name, b in order:
                if b["total"] == 0:
                    self._emit(f"SHUT {name}")
                    del self.bins[name]
            # receiver: first listed healthy bin with room
            for name in [n for n, _ in order if n in folders and n in self.bins]:
                b = self.bins[name]
                recv = None
                for rn, rb in order:
                    if rn in folders or rn not in self.bins:
                        continue
                    if rb["total"] == 0:
                        continue
                    if rb["total"] + b["total"] > rb["cap"]:
                        continue
                    recv = rn
                    break
                if recv is None:
                    continue
                rb = self.bins[recv]
                self._emit(f"FOLD {name} {recv}")
                for m in list(b["holders"]):
                    amt = b["holds"].get(m, 0)
                    if amt <= 0:
                        continue
                    rb["total"] += amt
                    rb["holds"][m] = rb["holds"].get(m, 0) + amt
                    if m not in rb["holders"]:
                        rb["holders"].append(m)
                    rb["minsq"] += amt * amt
                    rb["mincount"] += 1
                    if rb["minmax"] is None or amt > rb["minmax"]:
                        rb["minmax"] = amt
                    if rb["minmin"] is None or amt < rb["minmin"]:
                        rb["minmin"] = amt
                    rb["minrange"] = rb["minmax"] - rb["minmin"]
                    self.mminvol[m] += amt
                    self.mminsq[m] += amt * amt
                rb["pvol"] += b["total"]
                del self.bins[name]
                self.folded.add(name)
                self.c_m += 1
            return

        if kind == "Y":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_y += 1
            ranked = sorted(
                self.morder,
                key=lambda n: (
                    -self._held_total(n),
                    self.mwsq[n],
                    self.mweven[n],
                    -self.mminvol[n],
                    self.mminsq[n],
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
