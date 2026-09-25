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
        self.bins: dict[str, dict] = {}
        self.mwops: dict[str, list[int]] = {}
        self.mmin: dict[str, int] = {}
        self.folded: set[str] = set()
        self.t_done = False
        self.next_mseq = 0
        self.next_bseq = 0
        self.c_j = 0
        self.c_o = 0
        self.c_p = 0
        self.c_w = 0
        self.c_m = 0
        self.c_t = 0
        self.c_y = 0
        self.f_post = 0
        self.f_pre_t = 0
        self.f_last_t = 0

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
            self.morder.append(parts[1])
            self.mwops[parts[1]] = []
            self.mmin[parts[1]] = 0
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
                "wops": [],
                "foldw": [],
                "foldbase": 0,
                "pb": 0,
                "old": not self.t_done,
                "mout": 0,
                "min": False,
                "rec": 0,
                "rec_lag": 0,
                "ahist": [],
                "preYW": False,
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
            b["foldbase"] += count
            b["pb"] += count
            b["holds"][parts[1]] = b["holds"].get(parts[1], 0) + count
            if parts[1] not in b["holders"]:
                b["holders"].append(parts[1])
            if self.t_done:
                b["rec"] += count
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
            b["wops"].append(count)
            b["foldw"].append([self.c_t, count])
            if self.f_post == 0:
                b["preYW"] = True
            self.mwops[parts[1]].append(count)
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
            src["foldbase"] -= count
            src["pb"] -= count
            src["holds"][parts[1]] -= count
            self._drop_if_empty(src, parts[1])
            src["mout"] += 1
            dst["min"] = True
            dst["total"] += count
            dst["foldbase"] += count
            dst["pb"] += count
            dst["holds"][parts[1]] = dst["holds"].get(parts[1], 0) + count
            if parts[1] not in dst["holders"]:
                dst["holders"].append(parts[1])
            self.mmin[parts[1]] += 1
            self.c_m += 1
            self._emit("OK")
            return

        if kind == "T":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_t += 1
            scored = {}
            for name, b in self.bins.items():
                pen = sum(w // 5 for w in b["wops"])
                if b["min"]:
                    pen += 1
                _t = b["total"]
                _m = _t % 32
                _e1 = _m - 4
                if _e1 < 0:
                    _e1 = 0
                elif _e1 > 11:
                    _e1 = 11
                _e2 = _m - 26
                if _e2 < 0:
                    _e2 = 0
                elif _e2 > 6:
                    _e2 = 6
                bonus = (
                    17 * (_t // 32) + _e1 + _e2
                    - 2 * (_t // 32)
                    - (1 if _m > 16 else 0)
                    - (1 if _m > 17 else 0)
                )
                _r = b["rec"]
                bonus += (_r + 26) // 32 + (1 if _r >= 1 else 0)
                scored[name] = b["total"] - pen + bonus
            _base = {
                name: b["total"] - (
                    sum(w // 6 for w in b["wops"]) + (1 if b["min"] else 0)
                )
                for name, b in self.bins.items()
            }
            order = sorted(
                self.bins.items(),
                key=lambda kv: (
                    -scored[kv[0]],
                    kv[1]["old"],
                    _base[kv[0]],
                    (0 if kv[1]["holders"] else 1),
                    len(kv[1]["holders"]),
                    sum(_a for _, _a in kv[1]["foldw"]),
                    kv[1]["seq"],
                ),
            )
            self._emit(f"TURN {len(order)}")
            for name, _ in order:
                self._emit(f"B {name}")
            for name, b in self.bins.items():
                b["_snap"] = b["total"]
            _FA = [74, 66, 57, 48, 40, 33, 26, 20, 14, 8, 3]
            _FE = [3, 1, 3, 4, 5, 6, 7, 7, 7, 9, 9]
            _FC = [
                (100, 74, 300, 10),
                (100, 66, 100, 10),
                (11, 6, 135, 2),
                (100, 48, 400, 10),
                (100, 40, 500, 10),
                (100, 33, 600, 10),
                (100, 26, 700, 10),
                (100, 20, 700, 10),
                (100, 14, 700, 10),
                (100, 8, 900, 10),
                (100, 3, 900, 10),
            ]
            _n = self.c_t
            eligible = set()
            for name, b in order:
                _elig = False
                if b["total"] > 0:
                    _sw = sum(_a for _, _a in b["foldw"])
                    _fb = b["foldbase"]
                    if self.f_post:
                        _ne = min(
                            _n,
                            _n - self.f_pre_t + (1 if b["preYW"] else 0),
                            _n - self.f_last_t + self.f_post,
                        )
                    else:
                        _ne = _n
                    if _ne < 1:
                        _ne = 1
                    _pw = sum((_ne - _w - 1) * _a for _w, _a in b["foldw"])
                    if _ne == 1:
                        _elig = (
                            _sw >= _fb - _fb // 4 + 1 + (
                                1 if _fb % 40 == 20 else 0
                            )
                            and b["total"] >= _fb // 7 + 1
                        )
                    elif _ne <= 11:
                        _ca, _cb, _cc, _cd = _FC[_ne - 1]
                        _elig = _ca * _sw >= \
                            _cb * _fb + _cc + _cd * _pw
                        if _elig and self.f_post and b["rec"] >= 1:
                            _elig = False
                    elif _ne == 12:
                        _elig = 100 * _sw >= 650
                    elif _ne == 13:
                        if _fb < 60:
                            _elig = 100 * _sw >= 1150 - 15 * _fb
                        elif _fb <= 100:
                            _elig = 100 * _sw >= 250
                        else:
                            _elig = 100 * _sw >= max(0, 1150 - 15 * _fb)
                    elif _ne == 14:
                        _elig = 100 * _sw >= max(0, 1250 - 20 * _fb)
                    else:
                        _elig = 100 * _sw >= max(0, 1290 - 22 * _fb)
                if _elig:
                    eligible.add(name)
            _bord = [name for name, _ in order]
            for name, b in order:
                if name in self.bins and self.bins[name]["total"] == 0:
                    self._emit(f"SHUT {name}")
                    del self.bins[name]
                elif name in eligible or (
                    name in self.bins
                    and self.bins[name]["wops"]
                    and 0 < self.bins[name]["total"] * 5 <= self.bins[name]["pb"]
                    and sum(_a for _, _a in self.bins[name]["foldw"])
                    >= 4 * self.bins[name]["total"]
                ):
                    _dust = name not in eligible
                    for rname, rb in order:
                        if (
                            rname != name
                            and rname in self.bins
                            and rb["total"] + b["total"] <= rb["cap"]
                            and (
                                _ne == 1
                                or rb["total"] > 0
                            )
                            and (
                                rb["total"] > 0
                                or (
                                    _ne == 1
                                    and not _dust
                                    and rb["old"]
                                    and _bord.index(rname)
                                    > _bord.index(name)
                                )
                            )
                        ):
                            self._emit(f"FOLD {name} {rname}")
                            for holder in b["holders"]:
                                amt = b["holds"].get(holder, 0)
                                if amt > 0:
                                    rb["holds"][holder] = (
                                        rb["holds"].get(holder, 0) + amt
                                    )
                                    if holder not in rb["holders"]:
                                        rb["holders"].append(holder)
                            rb["total"] += b["total"]
                            rb["foldbase"] += b["total"]
                            rb["pb"] += b["total"]
                            del self.bins[name]
                            self.folded.add(name)
                            self.c_m += 1
                            break
            for name, b in list(self.bins.items()):
                snap = b.pop("_snap")
                b["rec_lag"] = b["rec"]
                if self.t_done:
                    b["ahist"].append(snap)
                else:
                    b["ahist"] = [snap]
            self.t_done = True
            return

        if kind == "Y":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_y += 1
            scored_m = {}
            for n in self.morder:
                scored_m[n] = self._held_total(n) - sum(
                    w // 4 for w in self.mwops[n]
                ) - self.mmin[n]
            ranked = sorted(
                self.morder, key=lambda n: (-scored_m[n], self.mseq[n])
            )
            self._emit(f"RECK {len(ranked)}")
            for name in ranked:
                self._emit(f"S {name}")
            if self.t_done:
                self.f_post += 1
                if self.f_post == 1:
                    self.f_pre_t = self.c_t
                self.f_last_t = self.c_t
            for b in self.bins.values():
                b["mout"] = 0
                b["min"] = False
                b["ahist"] = []
                if self.t_done:
                    b["foldw"] = []
                else:
                    b["wops"] = []
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
