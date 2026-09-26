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


def _num_ok(s: str) -> bool:
    return 1 <= len(s) <= 2 and s.isdigit() and s.isascii()


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.build: dict[str, int] = {}
        self.pseq: dict[str, int] = {}
        self.porder: list[str] = []
        self.where: dict[str, str | None] = {}
        self.hflag: dict[str, bool] = {}
        self.uflag: dict[str, bool] = {}
        self.vbon: dict[str, int] = {}
        self.turned: dict[str, bool] = {}
        self.led: dict[str, int] = {}
        self.fields: dict[str, dict] = {}
        self.forder: list[str] = []
        self.pending: set[str] = set()
        self.pool_open = True
        self.tainted = False
        self.barned: set[str] = set()
        self.shift_count = 0
        self.prev_reloc = False
        self.next_pseq = 0
        self.next_fseq = 0
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

    def _occupants(self, fname: str) -> list[str]:
        return [p for p in self.fields[fname]["slots"] if p is not None]

    def _touch(self, fname: str) -> None:
        self.touch_tick += 1
        self.fields[fname]["touch"] = self.touch_tick

    def _remove(self, pony: str) -> None:
        cur = self.where[pony]
        if cur is None:
            return
        f = self.fields[cur]
        try:
            idx = f["slots"].index(pony)
        except ValueError:
            idx = None
        if idx is not None:
            f["slots"][idx] = None
        self.where[pony] = None

    def _place_append(self, pony: str, dest: str) -> None:
        self.fields[dest]["slots"].append(pony)
        self.where[pony] = dest

    def _wake_and_choose_rest(self) -> str | None:
        self.shift_count += 1
        resting = [n for n in self.forder if self.fields[n]["resting"]]
        torest = set()
        for n in resting:
            if self.shift_count >= self.fields[n].get("rs", -99) + 2:
                torest.add(n)
        if self.prev_reloc:
            torest.update(resting)
        for n in sorted(torest, key=lambda n: self.fields[n]["seq"]):
            self.fields[n]["resting"] = False
            self._emit(f"WAKE {n}")
        self.prev_reloc = False
        open_names = [
            n for n in self.forder if not self.fields[n]["resting"]
        ]
        if not open_names:
            return None
        return min(
            open_names,
            key=lambda n: (self.fields[n]["touch"], self.fields[n]["seq"]),
        )

    def _relocate(self, rested: str) -> None:
        occ = self._occupants(rested)
        if not occ:
            return
        self.prev_reloc = True
        order = sorted(
            occ,
            key=lambda p: (
                -self._eff(p),
                -self.build[p],
                self.pseq[p],
            ),
        )
        for pony in order:
            dest = None
            dest_room = 0
            cands = sorted(self.forder, key=lambda n: self.fields[n]["seq"])
            for n in cands:
                if n == rested or self.fields[n]["resting"]:
                    continue
                room = self.fields[n]["span"] - len(self._occupants(n))
                if room <= 0:
                    continue
                if dest is None or room > dest_room:
                    dest, dest_room = n, room
            self._remove(pony)
            if dest is None:
                self.barned.add(pony)
                self._emit(f"BARN {pony}")
            else:
                self._place_append(pony, dest)
                self._touch(dest)
                self._emit(f"PUT {pony} {dest}")

    def _eff(self, pony: str) -> int:
        return (
            self.build[pony]
            + self.vbon[pony]
            + (1 if self.turned[pony] else 0)
            + self.led[pony]
        )

    def _order_key(self, pony: str):
        # V (OUT) band: base value, fewer lead moves, weaker effective,
        # then stake order.
        return (
            -self.build[pony],
            self.led[pony],
            self._eff(pony),
            self.pseq[pony],
        )

    def _uorder_key(self, pony: str):
        return (
            -self._eff(pony),
            self.hflag[pony],
            not self.uflag[pony],
            self.pseq[pony],
        )

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]

        if kind == "N":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _num_ok(parts[2]):
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
            self.hflag[parts[1]] = False
            self.uflag[parts[1]] = False
            self.vbon[parts[1]] = 0
            self.turned[parts[1]] = False
            self.led[parts[1]] = 0
            self.c_n += 1
            self._emit(f"OK {len(self.porder)}")
            return

        if kind == "F":
            if len(parts) != 3 or not _id_ok(parts[1]) or not _num_ok(parts[2]):
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
                "slots": [],
                "resting": False,
                "touch": -1,
            }
            self.forder.append(parts[1])
            self._touch(parts[1])
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
                self.tainted = True
                self._reject("WILD")
                return
            f = self.fields[parts[2]]
            if f["resting"]:
                self._reject("SHUT")
                return
            if self.where[parts[1]] is not None:
                self.tainted = False
                self._reject("TWICE")
                return
            if len(self._occupants(parts[2])) >= f["span"]:
                self._reject("CRAM")
                return
            virgin = len(self._occupants(parts[2])) == 0
            self._place_append(parts[1], parts[2])
            self._touch(parts[2])
            self.turned[parts[1]] = True
            self.tainted = True
            if (
                not virgin
                and self.pool_open
                and not self.hflag[parts[1]]
                and parts[1] not in self.barned
            ):
                self.pending.add(parts[1])
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
            if self.where[parts[1]] is None:
                self._reject("IDLE")
                return
            self._remove(parts[1])
            self.turned[parts[1]] = False
            self.hflag[parts[1]] = True
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
                self.tainted = True
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
                self.hflag[parts[1]] = False
                self.uflag[parts[1]] = False
                self.tainted = False
                self._reject("TWICE")
                return
            if len(self._occupants(parts[2])) >= f["span"]:
                self._reject("CRAM")
                return
            self._remove(parts[1])
            self._place_append(parts[1], parts[2])
            self._touch(parts[2])
            self.tainted = True
            self.led[parts[1]] += 1
            self.c_l += 1
            self._emit("OK")
            return

        if kind == "V":
            if len(parts) != 1:
                self._reject("FORM")
                return
            outs = [n for n in self.porder if self.where[n] is not None]
            outs.sort(key=self._order_key)
            self._emit(f"OUT {len(outs)}")
            for name in outs:
                self._emit(f"P {name}")
            out_set = set(outs)
            for n in self.porder:
                self.hflag[n] = False
                if n in out_set:
                    self.uflag[n] = False
            if not self.pending and self.tainted and not outs:
                self.pool_open = False
            self.pending = set()
            return

        if kind == "R":
            if len(parts) != 2 or not _id_ok(parts[1]):
                self._reject("FORM")
                return
            if parts[1] not in self.fields:
                self._reject("WILD")
                return
            occ = self._occupants(parts[1])
            self._emit(f"FLD {parts[1]} {len(occ)}")
            for name in occ:
                self._emit(f"P {name}")
            return

        if kind == "S":
            if len(parts) != 1:
                self._reject("FORM")
                return
            self.c_s += 1
            rested = self._wake_and_choose_rest()
            if rested is not None:
                self._emit(f"REST {rested}")
                self.fields[rested]["resting"] = True
                self.fields[rested]["rs"] = self.shift_count
            listed = [n for n in self.forder if not self.fields[n]["resting"]]
            if len(self.forder) == 1:
                bonus = 0
            else:
                bonus = 4 - len(listed)
            for n in self.porder:
                if n in self.pending and self.where[n] is not None:
                    self.vbon[n] += bonus
                if self.hflag[n] or self.uflag[n]:
                    self.hflag[n] = False
                    self.uflag[n] = False
            if not self.pending and self.tainted:
                self.pool_open = False
            self.pending = set()
            if rested is not None:
                self._relocate(rested)
            listed = [n for n in self.forder if not self.fields[n]["resting"]]
            self._emit(f"SHIFT {len(listed)}")
            for name in listed:
                self._emit(f"F {name}")
            return

        if kind == "U":
            if len(parts) != 1:
                self._reject("FORM")
                return
            self.c_u += 1
            for n in self.porder:
                self.uflag[n] = n in self.pending
            if not self.pending and self.tainted:
                self.pool_open = False
            self.pending = set()
            out = [n for n in self.porder if self.where[n] is not None]
            barn = [n for n in self.porder if self.where[n] is None]
            out.sort(key=self._uorder_key)
            barn.sort(key=self._uorder_key)
            self._emit(f"MUSTER {len(self.porder)}")
            for name in out + barn:
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
