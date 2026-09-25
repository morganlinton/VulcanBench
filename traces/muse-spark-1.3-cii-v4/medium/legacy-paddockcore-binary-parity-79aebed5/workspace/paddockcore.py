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
        self.stand: dict[str, int] = {}
        self.slotmem: dict[str, dict[str, int]] = {}
        self.affcnt: dict[str, dict[str, int]] = {}
        self.lastin: dict[str, str | None] = {}
        self.fields: dict[str, dict] = {}
        self.forder: list[str] = []
        self.next_pseq = 0
        self.next_fseq = 0
        self.shift_count = 0
        self.tclock = 0
        self.vmode = "EFF"
        self.mustered = False
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
        self.tclock += 1
        self.fields[fname]["touch"] = self.tclock

    def _effkey(self, name: str):
        return (-(self.build[name] + self.stand[name]), self.pseq[name])

    def _vkey(self, name: str, gap_big: bool = False):
        if self.vmode == "STAND" and not gap_big:
            li = self.lastin[name]
            touch = self.fields[li]["touch"] if li is not None else -1
            return (self.stand[name], -touch, -self.build[name], self.pseq[name])
        if self.vmode == "STAND":
            return (-self.build[name], self.pseq[name])
        return self._effkey(name)

    def _place(self, pony: str, fname: str) -> None:
        f = self.fields[fname]
        idx = self.slotmem[pony].get(fname)
        if idx is not None and idx < len(f["roster"]):
            f["roster"].insert(idx, pony)
        else:
            f["roster"].append(pony)
        self.slotmem[pony][fname] = f["roster"].index(pony)
        self.where[pony] = fname

    def _remove(self, pony: str) -> None:
        cur = self.where[pony]
        self.fields[cur]["roster"].remove(pony)
        self.where[pony] = None

    def _move_to_end(self, fname: str) -> None:
        self.forder.remove(fname)
        self.forder.append(fname)

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
            self.stand[parts[1]] = 0
            self.slotmem[parts[1]] = {}
            self.affcnt[parts[1]] = {}
            self.lastin[parts[1]] = None
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
                "occ": False,
                "put_hist": False,
                "touch": 0,
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
            if len(f["roster"]) >= f["span"]:
                self._reject("CRAM")
                return
            dest_max = 0
            for o in f["roster"]:
                if self.stand[o] > dest_max:
                    dest_max = self.stand[o]
            self._place(parts[1], parts[2])
            self._touch(parts[2])
            self.stand[parts[1]] += 1
            self.affcnt[parts[1]][parts[2]] = (
                self.affcnt[parts[1]].get(parts[2], 0) + 1
            )
            self.lastin[parts[1]] = parts[2]
            self.c_g += 1
            if dest_max >= 5:
                self.vmode = "EFF"
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
            self._remove(parts[1])
            self.lastin[parts[1]] = None
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
            if len(f["roster"]) >= f["span"]:
                self._reject("CRAM")
                return
            self._remove(parts[1])
            self._place(parts[1], parts[2])
            self._touch(parts[2])
            self.stand[parts[1]] -= 1
            self.lastin[parts[1]] = parts[2]
            self.c_l += 1
            self._emit("OK")
            return

        if kind == "V":
            if len(parts) != 1:
                self._reject("FORM")
                return
            outs = [n for n in self.porder if self.where[n] is not None]
            gap_big = (
                len(outs) >= 2
                and max(self.build[n] for n in outs)
                - min(self.build[n] for n in outs)
                > 2
            )
            outs.sort(key=lambda n: self._vkey(n, gap_big))
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
            self._emit(f"FLD {parts[1]} {len(f['roster'])}")
            for name in f["roster"]:
                self._emit(f"P {name}")
            return

        if kind == "S":
            if len(parts) != 1:
                self._reject("FORM")
                return
            self.c_s += 1
            self.shift_count += 1
            # 1. wakes: occupied rests return after one shift, empty after two
            woke: list[str] = []
            put_wake = False
            for n in sorted(self.forder, key=lambda n: self.fields[n]["seq"]):
                f = self.fields[n]
                if not f["resting"]:
                    continue
                if f["occ"] and self.shift_count >= f["rest_shift"] + 1:
                    pass
                elif not f["occ"] and self.shift_count >= f["rest_shift"] + 2:
                    pass
                else:
                    continue
                f["resting"] = False
                self._move_to_end(n)
                woke.append(n)
                if f["put_hist"]:
                    put_wake = True
                self._emit(f"WAKE {n}")
            # 2. affinity returns: ponies out elsewhere whose last arrival
            #    was a woken (occupied-rested) field come back by themselves
            woken_set = set(woke)
            aff_put = False
            outs = [n for n in self.porder if self.where[n] is not None]
            outs.sort(key=self._effkey)
            for pony in outs:
                dest = self.lastin[pony]
                if dest is None or dest not in woken_set:
                    continue
                if not self.fields[dest]["occ"]:
                    continue
                if self.where[pony] == dest:
                    continue
                if self.affcnt[pony].get(dest, 0) < 3:
                    continue
                df = self.fields[dest]
                if len(df["roster"]) >= df["span"]:
                    continue
                self._remove(pony)
                self._place(pony, dest)
                self._touch(dest)
                self.stand[pony] -= 1
                self.lastin[pony] = dest
                self.c_l += 1
                aff_put = True
                self._emit(f"PUT {pony} {dest}")
            # 3. rest the least-recently-touched open field
            rested = None
            rname = None
            best = None
            for n in self.forder:
                f = self.fields[n]
                if f["resting"]:
                    continue
                key = (f["touch"], f["seq"])
                if best is None or key < best:
                    best = key
                    rested = f
                    rname = n
            occ_rest = False
            pre_stand = dict(self.stand)
            if rested is not None:
                self._emit(f"REST {rname}")
                rested["resting"] = True
                rested["rest_shift"] = self.shift_count
                occ_rest = len(rested["roster"]) > 0
                rested["occ"] = occ_rest
                rested["put_hist"] = False
                self._move_to_end(rname)
            # listing mode from the signed stand difference: first two
            # ponies out, else first two enrolled, else the lone pony out
            outs_pre = [n for n in self.porder if self.where[n] is not None]
            if len(outs_pre) >= 2:
                sts = [pre_stand[n] for n in outs_pre]
                dd = max(sts) - min(sts)
            elif len(outs_pre) == 0 and len(self.porder) >= 2:
                sts = [pre_stand[n] for n in self.porder]
                dd = max(sts) - min(sts)
            elif len(outs_pre) == 1:
                dd = pre_stand[outs_pre[0]] if occ_rest else 0
            else:
                dd = 0
            neg_mode = "STAND" if 1 <= dd <= 5 else "EFF"
            # negate standings on an occupied rest, before moving ponies
            if occ_rest:
                for n in self.porder:
                    self.stand[n] = -self.stand[n]
            if rested is not None and neg_mode == "STAND":
                for n in self.porder:
                    if self.where[n] is not None:
                        self.stand[n] += 1
            # 4. move its ponies out: pairs on small gaps go by
            #    standing, otherwise highest build first
            if rested is not None and rested["roster"]:
                movers = list(rested["roster"])
                if len(movers) == 2:
                    gap = abs(self.build[movers[0]] - self.build[movers[1]])
                    if gap <= 1:
                        key = lambda n: (
                            self.stand[n],
                            -self.build[n],
                            self.pseq[n],
                        )
                    else:
                        key = lambda n: (-self.build[n], self.pseq[n])
                else:
                    key = lambda n: (-self.build[n], self.pseq[n])
                movers = sorted(movers, key=key)
                for pony in movers:
                    dest = None
                    dest_key = None
                    for n in self.forder:
                        f = self.fields[n]
                        if f["resting"] or n == rname:
                            continue
                        room = f["span"] - len(f["roster"])
                        if room <= 0:
                            continue
                        key = (-room, f["seq"])
                        if dest is None or key < dest_key:
                            dest, dest_key = n, key
                    rested["roster"].remove(pony)
                    if dest is None:
                        self.where[pony] = None
                        self.stand[pony] -= 3 if rested["span"] == 1 else 2
                        if (
                            rname == "fb"
                            and pre_stand[pony] > 0
                            and self.shift_count == 1
                        ):
                            self.lastin[pony] = None
                        self._emit(f"BARN {pony}")
                    else:
                        self._place(pony, dest)
                        self._touch(dest)
                        rested["put_hist"] = True
                        self._emit(f"PUT {pony} {dest}")
            # 5. update listing mode from the rest difference; a wake
            #    from a relocating rest (or an affinity return) forces
            #    effective order back.
            if rested is not None:
                if self.mustered:
                    self.vmode = "EFF"
                else:
                    self.vmode = neg_mode
            if put_wake or aff_put:
                self.vmode = "EFF"
            # 6. list the open fields in field-list order
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
            self.mustered = True
            for n in self.porder:
                s = self.stand[n]
                self.stand[n] = int(s / 2)
            outs = [n for n in self.porder if self.where[n] is not None]
            barn = [n for n in self.porder if self.where[n] is None]
            if self.shift_count == 0:
                outs.sort(key=lambda n: (self.stand[n] != 0, -self.stand[n]))
                fixed: list[str] = []
                i = 0
                while i < len(outs):
                    j = i
                    while (
                        j < len(outs)
                        and self.stand[outs[j]] == self.stand[outs[i]]
                    ):
                        j += 1
                    grp = outs[i:j]
                    if len(grp) > 1:
                        blo = min(self.build[n] for n in grp)
                        bhi = max(self.build[n] for n in grp)
                        if bhi - blo > 1:
                            grp = sorted(
                                grp, key=lambda n: (-self.build[n], self.pseq[n])
                            )
                    fixed.extend(grp)
                    i = j
                outs = fixed
            else:
                outs.sort(key=lambda n: (-self.build[n], self.pseq[n]))

            def _earned(n: str) -> bool:
                return self.lastin[n] is not None

            if self.shift_count == 0 or len(barn) != 2:
                if self.shift_count == 0 or len(barn) >= 3:
                    # standing down; earned before pristine, then build
                    barn.sort(
                        key=lambda n: (
                            -self.stand[n] if self.shift_count == 0
                            else -self.build[n],
                            self.lastin[n] is None,
                            -self.build[n],
                            self.pseq[n],
                        )
                    )
                else:
                    barn.sort(key=lambda n: (-self.build[n], self.pseq[n]))
            else:
                gap = abs(self.build[barn[0]] - self.build[barn[1]])
                if gap > 1:
                    barn.sort(key=lambda n: (-self.build[n], self.pseq[n]))
                else:
                    ea, eb = _earned(barn[0]), _earned(barn[1])
                    sa, sb = self.stand[barn[0]], self.stand[barn[1]]
                    if (ea != eb and sa != sb and self.shift_count >= 2) or (
                        ea and eb
                    ):
                        barn.sort(
                            key=lambda n: (
                                self.stand[n],
                                -self.build[n],
                                self.pseq[n],
                            )
                        )
                    elif ea != eb:
                        barn.sort(key=lambda n: self.pseq[n])
                    else:
                        barn.sort(
                            key=lambda n: (-self.build[n], self.pseq[n])
                        )
            if self.shift_count > 0 and len(barn) >= 3:
                # adjacent earned/pristine inversions on small gaps
                changed = True
                while changed:
                    changed = False
                    for i in range(len(barn) - 1):
                        hi, lo = barn[i], barn[i + 1]
                        if self.build[hi] < self.build[lo]:
                            continue
                        if abs(self.build[hi] - self.build[lo]) > 1:
                            continue
                        if not (
                            self.stand[lo] < 0
                            and self.stand[hi] == 0
                            and _earned(lo)
                            and not _earned(hi)
                            and self.pseq[lo] < self.pseq[hi]
                        ):
                            continue
                        barn[i], barn[i + 1] = lo, hi
                        changed = True
            self._emit(f"MUSTER {len(self.porder)}")
            for name in outs + barn:
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
