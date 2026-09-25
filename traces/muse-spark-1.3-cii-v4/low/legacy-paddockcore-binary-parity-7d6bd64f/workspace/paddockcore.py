"""PaddockCore pony field co-op engine, Python implementation.

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
        self.build: dict[str, int] = {}
        self.pseq: dict[str, int] = {}
        self.porder: list[str] = []
        self.where: dict[str, str | None] = {}
        # accepted-command counters per pony
        self.gtotal: dict[str, int] = {}
        self.ltotal: dict[str, int] = {}
        # led flag: an accepted lead before the first shift rota step
        self.ledflag: dict[str, bool] = {}
        # shift index of the last accepted lead / turnout
        self.lastL: dict[str, int] = {}
        self.lastG: dict[str, int] = {}
        # relocations by the rota (PUT or BARN lines)
        self.reloc: dict[str, int] = {}
        # affinity binding: field with 3+ arrivals, cleared by bring-in
        self.bound: dict[str, str | None] = {}
        # shift-barned flag (for the muster barn order)
        self.barned: dict[str, bool] = {}
        # arrival counts per (pony, field); leads move one unit src -> dest
        self.counts: dict[tuple[str, str], int] = {}
        self.slotmem: dict[tuple[str, str], int] = {}
        self.fields: dict[str, dict] = {}
        self.forder: list[str] = []
        self.next_pseq = 0
        self.next_fseq = 0
        self.shift_count = 0
        self.touch_tick = 0
        self.c_n = 0
        self.c_f = 0
        self.c_g = 0
        self.c_h = 0
        self.c_l = 0
        self.c_s = 0
        self.c_u = 0

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, code: str) -> None:
        self._emit(f"NAY {code}")

    def _touch(self, fname: str) -> None:
        self.touch_tick += 1
        self.fields[fname]["touch"] = self.touch_tick

    def _occ(self, fname: str) -> int:
        return sum(1 for p in self.fields[fname]["roster"] if p is not None)

    def _vacate(self, pony: str) -> None:
        cur = self.where[pony]
        if cur is not None:
            ros = self.fields[cur]["roster"]
            try:
                ros[ros.index(pony)] = None
            except ValueError:
                pass
            self.where[pony] = None

    def _arrive(self, pony: str, fname: str) -> None:
        f = self.fields[fname]
        ros = f["roster"]
        idx = self.slotmem.get((pony, fname))
        if idx is not None and idx < len(ros) and ros[idx] is None:
            ros[idx] = pony
        else:
            self.slotmem[(pony, fname)] = len(ros)
            ros.append(pony)
        self.where[pony] = fname

    def _open_names(self) -> list[str]:
        return [n for n in self.forder if not self.fields[n]["resting"]]

    def _fresh(self, pony: str) -> bool:
        return (
            self.lastG[pony] == self.shift_count
            and self.shift_count >= 1
            and self.reloc[pony] > 0
        )

    def _led_tier(self, pony: str, fld: str | None) -> int:
        if self.lastL[pony] == self.shift_count and self.shift_count >= 1:
            return -1
        if self.ledflag[pony] and (
            self.ltotal[pony] >= 2
            or (fld is not None and not self.fields[fld]["ever"])
        ):
            return 1
        return 0

    def _v_key(self, pony: str) -> tuple:
        fld = self.where[pony]
        return (
            -self.build[pony],
            -self._led_tier(pony, fld),
            -int(self._fresh(pony)),
            self.gtotal[pony],
            int(self.fields[fld]["ever"]) if fld is not None else 1,
            self.pseq[pony],
        )

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]

        if kind == "N":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _num_ok(parts[2], 2):
                self._reject("FORM")
                return
            val = int(parts[2])
            if not 1 <= val <= 99:
                self._reject("WIDE")
                return
            if parts[1] in self.build:
                self._reject("TWICE")
                return
            self.next_pseq += 1
            self.build[parts[1]] = val
            self.pseq[parts[1]] = self.next_pseq
            self.porder.append(parts[1])
            self.where[parts[1]] = None
            self.gtotal[parts[1]] = 0
            self.ltotal[parts[1]] = 0
            self.ledflag[parts[1]] = False
            self.lastL[parts[1]] = -1
            self.lastG[parts[1]] = -1
            self.reloc[parts[1]] = 0
            self.bound[parts[1]] = None
            self.barned[parts[1]] = False
            self.c_n += 1
            self._emit(f"OK {len(self.porder)}")
            return

        if kind == "F":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _num_ok(parts[2], 2):
                self._reject("FORM")
                return
            span = int(parts[2])
            if not 1 <= span <= 99:
                self._reject("WIDE")
                return
            if parts[1] in self.fields:
                self._reject("TWICE")
                return
            self.next_fseq += 1
            self.fields[parts[1]] = {
                "span": span,
                "seq": self.next_fseq,
                "roster": [],
                "resting": False,
                "rest_shift": 0,
                "rest_occ": False,
                "ever": False,
                "touch": -1,
            }
            self.forder.append(parts[1])
            self.c_f += 1
            self._emit(f"OK {len(self.forder)}")
            return

        if kind == "G":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _id_ok(parts[2]):
                self._reject("FORM")
                return
            if parts[1] not in self.build:
                self._reject("STRAY")
                return
            if parts[2] not in self.fields:
                self._reject("WILD")
                return
            f = self.fields[parts[2]]
            if f["resting"]:
                self._reject("SHUT")
                return
            if self.where[parts[1]] is not None:
                self._reject("TWICE")
                return
            if self._occ(parts[2]) >= f["span"]:
                self._reject("CRAM")
                return
            self._arrive(parts[1], parts[2])
            self._touch(parts[2])
            self.counts[(parts[1], parts[2])] = (
                self.counts.get((parts[1], parts[2]), 0) + 1
            )
            if self.counts[(parts[1], parts[2])] >= 3:
                self.bound[parts[1]] = parts[2]
            self.gtotal[parts[1]] += 1
            self.lastG[parts[1]] = self.shift_count
            self.barned[parts[1]] = False
            self.c_g += 1
            self._emit("OK")
            return

        if kind == "H":
            if len(parts) != 2 or not _id_ok(parts[1]):
                self._reject("FORM")
                return
            if parts[1] not in self.build:
                self._reject("STRAY")
                return
            cur = self.where[parts[1]]
            if cur is None:
                self._reject("IDLE")
                return
            self._vacate(parts[1])
            self.bound[parts[1]] = None
            self.c_h += 1
            self._emit("OK")
            return

        if kind == "L":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _id_ok(parts[2]):
                self._reject("FORM")
                return
            if parts[1] not in self.build:
                self._reject("STRAY")
                return
            if parts[2] not in self.fields:
                self._reject("WILD")
                return
            f = self.fields[parts[2]]
            if f["resting"]:
                self._reject("SHUT")
                return
            cur = self.where[parts[1]]
            if cur is None:
                self._reject("IDLE")
                return
            if cur == parts[2]:
                self._reject("TWICE")
                return
            if self._occ(parts[2]) >= f["span"]:
                self._reject("CRAM")
                return
            if self.shift_count == 0:
                self.ledflag[parts[1]] = True
            self._vacate(parts[1])
            self._arrive(parts[1], parts[2])
            self._touch(parts[2])
            self.counts[(parts[1], cur)] = self.counts.get((parts[1], cur), 0) - 1
            self.counts[(parts[1], parts[2])] = (
                self.counts.get((parts[1], parts[2]), 0) + 1
            )
            if self.counts[(parts[1], parts[2])] >= 3:
                self.bound[parts[1]] = parts[2]
            self.ltotal[parts[1]] += 1
            self.lastL[parts[1]] = self.shift_count
            self.c_l += 1
            self._emit("OK")
            return

        if kind == "V":
            if len(parts) != 1:
                self._reject("FORM")
                return
            outs = [n for n in self.porder if self.where[n] is not None]
            outs.sort(key=lambda n: self._v_key(n))
            self._emit(f"OUT {len(outs)}")
            for name in outs:
                self._emit(f"P {name}")
            return

        if kind == "R":
            if len(parts) != 2 or not _id_ok(parts[1]):
                self._reject("FORM")
                return
            if parts[1] not in self.fields:
                self._reject("WILD")
                return
            f = self.fields[parts[1]]
            if f["resting"]:
                self._emit(f"FLD {parts[1]} 0")
                return
            shown = [p for p in f["roster"] if p is not None]
            self._emit(f"FLD {parts[1]} {len(shown)}")
            for name in shown:
                self._emit(f"P {name}")
            return

        if kind == "S":
            if len(parts) != 1:
                self._reject("FORM")
                return
            self.c_s += 1
            self.shift_count += 1
            # waking: occupied rests return one shift sooner
            waking = [
                n
                for n in self.forder
                if self.fields[n]["resting"]
                and self.shift_count
                >= self.fields[n]["rest_shift"]
                + (1 if self.fields[n]["rest_occ"] else 2)
            ]
            waking.sort(key=lambda n: self.fields[n]["seq"])
            # each woken field reopens with its bound ponies' returns
            for n in waking:
                self.fields[n]["resting"] = False
                self._emit(f"WAKE {n}")
                # affinity: bound ponies led back to the reopened field
                cands = [
                    p
                    for p in self.porder
                    if self.where[p] is not None
                    and self.bound[p] == n
                    and self.counts.get((p, n), 0) >= 3
                ]
                cands.sort(key=lambda p: (-self.build[p], self.pseq[p]))
                for pony in cands:
                    self._vacate(pony)
                    self._arrive(pony, n)
                    self._touch(n)
                    self.c_l += 1
                    self._emit(f"PUT {pony} {n}")
            # rest the longest-untouched open field (ties by stake order)
            open_names = self._open_names()
            rested = None
            if open_names:
                rested = min(
                    open_names,
                    key=lambda n: (self.fields[n]["touch"], self.fields[n]["seq"]),
                )
                self._emit(f"REST {rested}")
                rf = self.fields[rested]
                rf["resting"] = True
                rf["rest_shift"] = self.shift_count
                rf["rest_occ"] = self._occ(rested) > 0
                rf["ever"] = True
            # move its ponies out
            if rested is not None:
                movers = [p for p in rf["roster"] if p is not None]
                if self.shift_count == 1:
                    movers.sort(key=lambda p: (-self.build[p], self.pseq[p]))
                else:
                    movers.sort(
                        key=lambda p: (
                            -self.build[p],
                            -int(self.ledflag[p]),
                            self.gtotal[p],
                            self.pseq[p],
                        )
                    )
                for pony in movers:
                    cands = []
                    for n in self.forder:
                        f = self.fields[n]
                        if f["resting"]:
                            continue
                        room = f["span"] - self._occ(n)
                        if room <= 0:
                            continue
                        cands.append((-room, f["ever"], f["seq"], n))
                    dest = min(cands)[3] if cands else None
                    self._vacate(pony)
                    if dest is None:
                        self.barned[pony] = True
                        self.reloc[pony] += 1
                        self._emit(f"BARN {pony}")
                    else:
                        self._arrive(pony, dest)
                        self._touch(dest)
                        self.reloc[pony] += 1
                        self._emit(f"PUT {pony} {dest}")
            listed = [
                n for n in self.forder if not self.fields[n]["resting"]
            ]
            listed.sort(key=lambda n: (self.fields[n]["ever"], self.fields[n]["seq"]))
            self._emit(f"SHIFT {len(listed)}")
            for name in listed:
                self._emit(f"F {name}")
            return

        if kind == "U":
            if len(parts) != 1:
                self._reject("FORM")
                return
            self.c_u += 1
            outs = [n for n in self.porder if self.where[n] is not None]
            barns = [n for n in self.porder if self.where[n] is None]
            outs.sort(key=lambda n: self._v_key(n))
            barns.sort(
                key=lambda n: (
                    -self.build[n],
                    -int(self.barned[n]),
                    self.pseq[n],
                )
            )
            ranked = outs + barns
            self._emit(f"MUSTER {len(ranked)}")
            for name in ranked:
                self._emit(f"P {name}")
            return

        self._reject("FORM")

    def finish(self) -> None:
        self._emit(
            f"END {self.c_n} {self.c_f} {self.c_g} {self.c_h} "
            f"{self.c_l} {self.c_s} {self.c_u}"
        )


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
