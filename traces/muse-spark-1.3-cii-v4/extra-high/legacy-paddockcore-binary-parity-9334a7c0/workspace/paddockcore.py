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
        self.fields: dict[str, dict] = {}
        self.forder: list[str] = []
        self.next_pseq = 0
        self.next_fseq = 0
        self.shift_count = 0
        self.c_n = 0
        self.c_f = 0
        self.c_g = 0
        self.c_h = 0
        self.c_l = 0
        self.c_s = 0
        self.c_u = 0
        # behavioral state (engine contract, not spec)
        self._touch_tick = 0
        self._handling_tick = 0
        self.g_pre: dict[str, int] = {}
        # leads: pre-shift [fresh, tenured] each; a send-home after the
        # lead voids its bonus. post-shift turnouts: [upgraded, fresh].
        self.lpre: dict[str, list[list[bool]]] = {}
        self.lpost: dict[str, list[list[bool]]] = {}
        self.g_post: dict[str, list[list[bool]]] = {}
        self.u_bonus: dict[str, int] = {}
        self.h_flag: dict[str, bool] = {}
        self.last_handling: dict[str, int] = {}
        self.turnout_counts: dict[tuple[str, str], int] = {}
        self.old_index: dict[tuple[str, str], int] = {}
        self.s_done = False
        self.u_done = False
        self.d_done = False
        self.u_since_s = False
        self.u_tick = 0
        self.u_post_s = False
        self.ring: list[str] = []

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, code: str) -> None:
        self._emit(f"NAY {code}")

    def _open_fields(self) -> list[dict]:
        return [self.fields[n] for n in self.forder if not self.fields[n]["resting"]]

    def _field_touch(self, name: str) -> None:
        self._touch_tick += 1
        self.fields[name]["touch"] = self._touch_tick

    def _gpost_term(self, pony: str) -> int:
        # turnout upgrades count only when the pony's muster-freshness
        # matches whether a muster has gone by since the shift: a fresh
        # first turnout is worth two, any returning turnout is worth
        # two, later fresh ones worth one. plain entries pool with the
        # recorded upgrades for stale ponies.
        pre = 0
        post = 0
        v = 0
        slot = 0
        tick_fresh = self.last_handling.get(pony, 0) > self.u_tick
        xnor = self.u_since_s == tick_fresh
        for upgraded, fresh in self.g_post.get(pony, []):
            slot += 1
            if not upgraded:
                pre += 1
                continue
            post += 1
            if not xnor:
                continue
            if fresh:
                v += 2 if slot == 1 else 1
            else:
                v += 2
        if tick_fresh:
            return v + max(0, pre - 1)
        return v + max(0, pre + post - 1)

    def _vscore(self, pony: str) -> int:
        b = self.build[pony]
        u = self.u_bonus.get(pony, 0)
        gp = self.g_pre.get(pony, 0)
        go = sum(1 for _ in self.g_post.get(pony, []))
        if not self.s_done:
            rg = max(0, gp + go - 1)
            l = len(self.lpre.get(pony, [])) + len(self.lpost.get(pony, []))
            return b + rg - l + u
        v = b + gp + u + self._gpost_term(pony)
        for fresh, tenured in self.lpre.get(pony, []):
            if fresh:
                v += 2 if tenured else 0
            else:
                v -= 1
        tick_fresh = self.last_handling.get(pony, 0) > self.u_tick
        for fresh, tenured in self.lpost.get(pony, []):
            if fresh and tenured and not tick_fresh:
                continue
            v -= 1
        return v

    def _mscore(self, pony: str) -> int:
        return (
            self.build[pony]
            + len(self.lpre.get(pony, []))
            + len(self.lpost.get(pony, []))
        )

    def _bump_handling(self, pony: str) -> None:
        self._handling_tick += 1
        self.last_handling[pony] = self._handling_tick

    def _wipe_tenure(self, pony: str) -> None:
        self.lpre[pony] = [[f, False] for f, _ in self.lpre.get(pony, [])]
        self.lpost[pony] = [[f, False] for f, _ in self.lpost.get(pony, [])]

    def _vfield_key(self, field: str) -> tuple:
        f = self.fields[field]
        if f["put_tick"] > 0:
            return (0, -f["put_tick"])
        if f["rest_shift"] == 0:
            return (1, f["touch"])
        return (2, -f["rest_shift"])

    def _vorder(self, ponies: list[str]) -> list[str]:
        # primary: handling-adjusted build; once a muster has gone by
        # since the last shift, ponies handled after it come first (by
        # enrollment) and the rest follow field standing: shifted-in
        # fields, then fields never rested, then the rest by recency
        scores = {n: self._vscore(n) for n in ponies}
        by_score: dict[int, list[str]] = {}
        for n in ponies:
            by_score.setdefault(scores[n], []).append(n)
        out: list[str] = []
        for sc in sorted(by_score, reverse=True):
            group = by_score[sc]
            if not self.u_since_s:
                out.extend(sorted(group, key=lambda n: self.pseq[n]))
                continue
            fresh = sorted(
                [n for n in group if self.last_handling.get(n, 0) > self.u_tick],
                key=lambda n: self.pseq[n],
            )
            stale = sorted(
                [n for n in group if self.last_handling.get(n, 0) <= self.u_tick],
                key=lambda n: self.pseq[n],
            )
            out.extend(fresh + stale)
        return out

    def _morder(self, ponies: list[str], ranked: bool = True) -> list[str]:
        # primary: muster build; ponies with handling since any send-home
        # first, every other pony keeps enrollment order. ties in later
        # musters favor the least recently touched field; the first
        # muster ever keeps enrollment order.
        scores = {n: self._mscore(n) for n in ponies}
        by_score: dict[int, list[str]] = {}
        for n in ponies:
            by_score.setdefault(scores[n], []).append(n)
        out: list[str] = []
        for sc in sorted(by_score, reverse=True):
            group = by_score[sc]
            if ranked:
                hkey = lambda n: (
                    -self.fields[self.where[n]]["touch"]
                    if self.where[n] is not None
                    else 0,
                    self.pseq[n],
                )
            else:
                hkey = lambda n: self.pseq[n]
            handled = sorted(
                [
                    n
                    for n in group
                    if self.last_handling.get(n, 0) > 0
                    and not self.h_flag.get(n, False)
                ],
                key=hkey,
            )
            rest = sorted(
                [
                    n
                    for n in group
                    if not (
                        self.last_handling.get(n, 0) > 0
                        and not self.h_flag.get(n, False)
                    )
                ],
                key=lambda n: self.pseq[n],
            )
            out.extend(handled + rest)
        return out

    def _insert_roster(self, field: str, pony: str) -> None:
        key = (pony, field)
        roster = self.fields[field]["roster"]
        if key in self.old_index:
            idx = self.old_index.pop(key)
            if idx > len(roster):
                idx = len(roster)
            roster.insert(idx, pony)
        else:
            roster.append(pony)

    def _remove_roster(self, field: str, pony: str) -> None:
        roster = self.fields[field]["roster"]
        idx = roster.index(pony)
        del roster[idx]
        self.old_index[(pony, field)] = idx

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
            self.g_pre[parts[1]] = 0
            self.g_post[parts[1]] = []
            self.lpre[parts[1]] = []
            self.lpost[parts[1]] = []
            self.u_bonus[parts[1]] = 0
            self.h_flag[parts[1]] = False
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
            self._touch_tick += 1
            self.fields[parts[1]] = {
                "span": span,
                "seq": self.next_fseq,
                "roster": [],
                "resting": False,
                "rest_shift": 0,
                "put_tick": 0,
                "was_occupied": False,
                "touch": self._touch_tick,
            }
            self.forder.append(parts[1])
            self.ring.append(parts[1])
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
            self._insert_roster(parts[2], parts[1])
            self.where[parts[1]] = parts[2]
            fresh = self.turnout_counts.get((parts[1], parts[2]), 0) == 0
            if self.s_done:
                self.g_post[parts[1]] = self.g_post.get(parts[1], []) + [
                    [self.u_post_s, fresh]
                ]
            else:
                self.g_pre[parts[1]] = self.g_pre.get(parts[1], 0) + 1
            self.turnout_counts[(parts[1], parts[2])] = (
                self.turnout_counts.get((parts[1], parts[2]), 0) + 1
            )
            if self.c_u > 0:
                self._field_touch(parts[2])
            self._bump_handling(parts[1])
            self.h_flag[parts[1]] = False
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
            self._remove_roster(cur, parts[1])
            self.where[parts[1]] = None
            self.h_flag[parts[1]] = True
            for name in self.porder:
                self._wipe_tenure(name)
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
            self._remove_roster(cur, parts[1])
            self._insert_roster(parts[2], parts[1])
            self.where[parts[1]] = parts[2]
            fresh = self.turnout_counts.get((parts[1], parts[2]), 0) == 0
            if self.s_done:
                self.lpost[parts[1]] = self.lpost.get(parts[1], []) + [
                    [fresh, True]
                ]
            else:
                self.lpre[parts[1]] = self.lpre.get(parts[1], []) + [
                    [fresh, True]
                ]
            self.turnout_counts[(parts[1], parts[2])] = (
                self.turnout_counts.get((parts[1], parts[2]), 0) + 1
            )
            self._field_touch(parts[2])
            self._bump_handling(parts[1])
            self.h_flag[parts[1]] = False
            self.c_l += 1
            self._emit("OK")
            return

        if kind == "V":
            if len(parts) != 1:
                self._reject("FORM")
                return
            outs = [n for n in self.porder if self.where[n] is not None]
            outs = self._vorder(outs)
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
            # wake: occupied rests return one shift sooner
            waking_names: list[str] = []
            for n in self.forder:
                f = self.fields[n]
                if not f["resting"]:
                    continue
                need = 1 if f.get("was_occupied") else 2
                if self.shift_count >= f["rest_shift"] + need:
                    waking_names.append(n)
            waking_names.sort(key=lambda n: self.fields[n]["seq"])
            for name in waking_names:
                self.fields[name]["resting"] = False
                if self.fields[name].get("was_occupied"):
                    self._field_touch(name)
                self._emit(f"WAKE {name}")
            # loyalty auto-returns: ponies out elsewhere with >=3 turnouts
            # to a waking field come back by themselves (counted as leads)
            for wname in waking_names:
                wf = self.fields[wname]
                if wf["resting"]:
                    continue
                cands: list[str] = []
                for pony in self.porder:
                    cur = self.where.get(pony)
                    if cur is None or cur == wname:
                        continue
                    if self.turnout_counts.get((pony, wname), 0) >= 3:
                        cands.append(pony)
                cands = self._vorder(cands)
                for pony in cands:
                    cur = self.where.get(pony)
                    if cur is None or cur == wname:
                        continue
                    if self.turnout_counts.get((pony, wname), 0) < 3:
                        continue
                    if len(wf["roster"]) >= wf["span"]:
                        continue
                    # leave current field (remember old place there)
                    self._remove_roster(cur, pony)
                    # waking field was cleared on rest; append
                    wf["roster"].append(pony)
                    self.where[pony] = wname
                    # loyalty returns need no fresh check (>=3 turnouts)
                    if self.s_done:
                        self.lpost[pony] = self.lpost.get(pony, []) + [
                            [False, True]
                        ]
                    else:
                        self.lpre[pony] = self.lpre.get(pony, []) + [
                            [False, True]
                        ]
                    self.turnout_counts[(pony, wname)] = (
                        self.turnout_counts.get((pony, wname), 0) + 1
                    )
                    self._field_touch(wname)
                    self._bump_handling(pony)
                    self.c_l += 1
                    self._emit(f"PUT {pony} {wname}")
            # rest the least-recently-touched open field
            open_names = [n for n in self.forder if not self.fields[n]["resting"]]
            rested_name: str | None = None
            if open_names:
                rested_name = min(
                    open_names,
                    key=lambda n: (self.fields[n]["touch"], self.fields[n]["seq"]),
                )
                rf = self.fields[rested_name]
                self._emit(f"REST {rested_name}")
                self.ring.remove(rested_name)
                self.ring.append(rested_name)
                rf["resting"] = True
                rf["rest_shift"] = self.shift_count
                rf["was_occupied"] = len(rf["roster"]) > 0
                # clear remembered slots for this field
                for key in [k for k in self.old_index if k[1] == rested_name]:
                    del self.old_index[key]
            # move its ponies out in V order, most room first
            if rested_name is not None:
                rf = self.fields[rested_name]
                if rf["roster"]:
                    movers = self._vorder(list(rf["roster"]))
                    for pony in movers:
                        dest = None
                        dest_key = None
                        for n in self.forder:
                            f = self.fields[n]
                            if f["resting"] or n == rested_name:
                                continue
                            room = f["span"] - len(f["roster"])
                            if room <= 0:
                                continue
                            key = (-room, f["seq"])
                            if dest is None or key < dest_key:
                                dest, dest_key = n, key
                        rf["roster"].remove(pony)
                        if dest is None:
                            self.where[pony] = None
                            self._emit(f"BARN {pony}")
                        else:
                            self._insert_roster(dest, pony)
                            self.where[pony] = dest
                            self._emit(f"PUT {pony} {dest}")
            # list the open fields in ring order: each rest moves the
            # rested field to the end of the ring
            listed = [n for n in self.ring if not self.fields[n]["resting"]]
            self._emit(f"SHIFT {len(listed)}")
            for name in listed:
                self._emit(f"F {name}")
            self.s_done = True
            self.u_since_s = False
            self.u_post_s = False
            return

        if kind == "U":
            if len(parts) != 1:
                self._reject("FORM")
                return
            self.c_u += 1
            if self.s_done:
                self.u_post_s = True
            # nudges (lead bonuses only, muster scores are untouched):
            # a pre-shift first muster lifts shortfall toward build, and
            # post-shift musters after the first one do too; the first
            # post-shift muster instead sinks pre-shift overshoot back
            if (not self.u_done and not self.s_done) or self.d_done:
                for pony in self.porder:
                    if self._vscore(pony) < self.build[pony]:
                        self.u_bonus[pony] = self.u_bonus.get(pony, 0) + 1
                self.u_done = True
            if self.s_done and not self.d_done:
                for pony in self.porder:
                    if (
                        self.g_pre.get(pony, 0) + len(self.lpre.get(pony, []))
                        > 0
                    ):
                        self.u_bonus[pony] = self.u_bonus.get(pony, 0) - 1
                self.d_done = True
            self.u_since_s = True
            self.u_tick = self._handling_tick
            outs = [n for n in self.porder if self.where[n] is not None]
            barn = [n for n in self.porder if self.where[n] is None]
            outs = self._morder(outs, self.c_u > 3)
            barn = self._morder(barn, self.c_u > 3)
            ranked = outs + barn
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
