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
        # per-member reckoning state
        self.m_wpen: dict[str, int] = {}
        self.m_mpen: dict[str, int] = {}
        self.m_draw_flag: dict[str, bool] = {}
        self.m_draw_streak: dict[str, int] = {}

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

    def _bin_key(self, b: dict) -> int:
        if b.get("bonus"):
            return b["total"] + 7
        return (
            b["total"]
            - b.get("w_drag", 0)
            - b.get("m_drag", 0)
            + b.get("p_recent", 0) // 14
        )

    def _member_key(self, name: str) -> int:
        total = self._held_total(name)
        pen = self.m_wpen.get(name, 0) + self.m_mpen.get(name, 0)
        if self.m_draw_streak.get(name, 0) >= 4:
            pen += 20
        return total - pen

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
            self.m_wpen[parts[1]] = 0
            self.m_mpen[parts[1]] = 0
            self.m_draw_flag[parts[1]] = False
            self.m_draw_streak[parts[1]] = 0
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
                "w_drag": 0,
                "m_drag": 0,
                "p_recent": 0,
                "first_t": False,
                "bonus": False,
                "prev_last": False,
                "prev_total": None,
                "w_flag": False,
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
            if b.get("first_t"):
                b["p_recent"] += count
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
            b["w_drag"] += count // 6
            b["w_flag"] = True
            self.m_wpen[parts[1]] += count // 4
            self.m_draw_flag[parts[1]] = True
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
            self._drop_if_empty(src, parts[1])
            dst["total"] += count
            dst["holds"][parts[1]] = dst["holds"].get(parts[1], 0) + count
            if parts[1] not in dst["holders"]:
                dst["holders"].append(parts[1])
            dst["m_drag"] += count // 10
            self.m_mpen[parts[1]] += 1
            self.c_m += 1
            self._emit("OK")
            return

        if kind == "T":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_t += 1
            # update member draw streaks (consecutive Ts with draws)
            for name in self.morder:
                if self.m_draw_flag.get(name, False):
                    self.m_draw_streak[name] = self.m_draw_streak.get(name, 0) + 1
                else:
                    self.m_draw_streak[name] = 0
                self.m_draw_flag[name] = False
            order = sorted(
                self.bins.items(),
                key=lambda kv: (
                    -self._bin_key(kv[1]),
                    len(kv[1]["holders"]),
                    kv[1]["seq"],
                ),
            )
            # snapshot totals/keys for trailing + fold processing
            order_totals = {name: b["total"] for name, b in order}
            n = len(order)
            self._emit(f"TURN {n}")
            for name, _ in order:
                self._emit(f"B {name}")
            alive: dict[str, dict] = {name: b for name, b in order}
            # process folds/shuts in listed order
            for idx, (name, b) in enumerate(order):
                if name not in alive:
                    continue
                cur = alive[name]
                if cur["total"] == 0:
                    self._emit(f"SHUT {name}")
                    del alive[name]
                    del self.bins[name]
                elif cur["total"] > 0 and cur["total"] < 2 * (
                    cur.get("w_drag", 0) + cur.get("m_drag", 0)
                ):
                    # find earliest earlier bin with cap space
                    dest = None
                    for j in range(idx):
                        cand_name, _ = order[j]
                        if cand_name not in alive:
                            continue
                        cand = alive[cand_name]
                        if cand["total"] + cur["total"] <= cand["cap"]:
                            dest = cand_name
                            break
                    if dest is not None:
                        self._emit(f"FOLD {name} {dest}")
                        dst = alive[dest]
                        # pool sacks and roster (positive holders only)
                        for holder in list(cur["holders"]):
                            amt = cur["holds"].get(holder, 0)
                            if amt > 0:
                                dst["holds"][holder] = (
                                    dst["holds"].get(holder, 0) + amt
                                )
                                if holder not in dst["holders"]:
                                    dst["holders"].append(holder)
                        dst["total"] += cur["total"]
                        self.folded.add(name)
                        del alive[name]
                        del self.bins[name]
                        self.c_m += 1
            # update trailing + first-T state for survivors
            for idx, (name, b) in enumerate(order):
                if name not in self.bins:
                    continue
                cur = self.bins[name]
                is_last = n > 0 and idx == n - 1
                if (
                    cur.get("prev_last")
                    and is_last
                    and cur.get("prev_total") == order_totals[name]
                    and cur.get("w_flag")
                ):
                    cur["bonus"] = True
                cur["prev_last"] = is_last
                cur["prev_total"] = order_totals[name]
                cur["w_flag"] = False
                cur["first_t"] = True
            return

        if kind == "Y":
            if len(parts) != 1:
                self._reject("FMT")
                return
            self.c_y += 1
            # clear per-bin transfer drag and deposit bonus
            for b in self.bins.values():
                b["m_drag"] = 0
                b["p_recent"] = 0
                b["first_t"] = False
            ranked = sorted(
                self.morder, key=lambda n: (-self._member_key(n), self.mseq[n])
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
