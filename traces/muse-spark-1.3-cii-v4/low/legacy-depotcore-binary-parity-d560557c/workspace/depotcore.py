"""DepotCore parcel-depot engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One depot
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys

BAY, SHELVED, OUT, LEDGE, GONE, WAIVED = 0, 1, 2, 3, 4, 5


class Parcel:
    __slots__ = (
        "pid",
        "sender",
        "lane",
        "heft",
        "state",
        "stamp",
        "bounces",
        "lodge_seq",
        "r_placed",
        "touch",
        "nshelve",
        "bounce_seqs",
        "cleared",
        "burned",
    )

    def __init__(self, pid, sender, lane, heft):
        self.pid = pid
        self.sender = sender
        self.lane = lane
        self.heft = heft
        self.state = BAY
        self.stamp = 0
        self.bounces = 0
        self.lodge_seq = 0
        self.r_placed = 0
        self.touch = 0
        self.nshelve = 0
        self.bounce_seqs = []
        self.cleared = False
        self.burned = False


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.senders: dict[str, bool] = {}  # registration order
        self.lane_order: list[str] = []
        self.shelves: dict[str, list[str]] = {}
        self.parcels: dict[str, Parcel] = {}
        self.bay: list[str] = []    # lodge order
        self.ledge: list[str] = []  # bounce order
        self.lane_rounds: dict[str, int] = {}    # completed rounds per lane
        self.lane_full_rounds: dict[str, int] = {}  # completed non-void rounds
        self.lodge_seq_next = 1  # shared ordering for lodges and bounces
        self.seq_next = 1
        self.stamp_next = 1
        self.window: list[str] = []
        self.window_open = False
        self.window_bounced: set[str] = set()
        self.c_reg = 0
        self.c_lane = 0
        self.c_lodge = 0
        self.c_round = 0
        self.c_bounce = 0
        self.c_waive = 0
        self.c_square = 0

    def _emit(self, line):
        self.out.write(line + "\n")

    @staticmethod
    def _tok_ok(tok):
        return 1 <= len(tok) <= 8 and tok.isalnum()

    def _close_window(self):
        for pid in self.window:
            p = self.parcels[pid]
            if p.state == OUT:
                p.state = GONE
        self.window = []
        self.window_open = False
        self.window_bounced = set()

    def handle(self, line):
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        kind = parts[0]
        if kind == "REG":
            self._cmd_reg(parts)
        elif kind == "LANE":
            self._cmd_lane(parts)
        elif kind == "LODGE":
            self._cmd_lodge(parts)
        elif kind == "SHELVE":
            self._cmd_shelve(parts)
        elif kind == "ROUND":
            self._cmd_round(parts)
        elif kind == "BOUNCE":
            self._cmd_bounce(parts)
        elif kind == "WAIVE":
            self._cmd_waive(parts)
        elif kind == "SQUARE":
            self._cmd_square(parts)
        elif kind == "LIST":
            self._cmd_list(parts)
        elif kind == "HELD":
            self._cmd_held(parts)
        elif kind == "ROLL":
            self._cmd_roll(parts)
        else:
            self._emit(f"R {kind} FMT")

    def _cmd_reg(self, parts):
        if len(parts) != 2:
            self._emit("R REG FMT")
            return
        s = parts[1]
        if not self._tok_ok(s):
            self._emit(f"R {s} FMT")
            return
        if s in self.senders:
            self._emit(f"R {s} DUP")
            return
        self.senders[s] = True
        self.c_reg += 1
        self._emit(f"OK {s}")

    def _cmd_lane(self, parts):
        if len(parts) != 2:
            self._emit("R LANE FMT")
            return
        l = parts[1]
        if not self._tok_ok(l):
            self._emit(f"R {l} FMT")
            return
        if l in self.shelves:
            self._emit(f"R {l} DUP")
            return
        self.shelves[l] = []
        self.lane_rounds[l] = 0
        self.lane_full_rounds[l] = 0
        self.lane_order.append(l)
        self.c_lane += 1
        self._emit(f"OK {l}")

    def _cmd_lodge(self, parts):
        if len(parts) != 5:
            self._emit("R LODGE FMT")
            return
        s, pid, l, htok = parts[1], parts[2], parts[3], parts[4]
        for tok in (s, pid, l):
            if not self._tok_ok(tok):
                self._emit(f"R {tok} FMT")
                return
        if not (htok.isdigit() and 1 <= len(htok) <= 3):
            self._emit(f"R {htok} HEFT")
            return
        heft = int(htok)
        if not 1 <= heft <= 999:
            self._emit(f"R {htok} HEFT")
            return
        if s not in self.senders:
            self._emit(f"R {s} WHO")
            return
        if l not in self.shelves:
            self._emit(f"R {l} DEST")
            return
        if pid in self.parcels:
            self._emit(f"R {pid} DUP")
            return
        p = Parcel(pid, s, l, heft)
        p.lodge_seq = self.lodge_seq_next
        p.r_placed = self.lane_full_rounds[l]
        self.lodge_seq_next += 1
        p.touch = self.seq_next
        self.seq_next += 1
        self.parcels[pid] = p
        self.bay.append(pid)
        self.c_lodge += 1
        self._emit(f"OK {pid}")

    def _place(self, p, put=True, shelved=True, set_r=True):
        p.stamp = self.stamp_next
        self.stamp_next += 1
        if shelved:
            p.nshelve += 1
        if set_r:
            p.r_placed = self.lane_full_rounds[p.lane]
        p.state = SHELVED
        self.shelves[p.lane].append(p.pid)
        if put:
            self._emit(f"PUT {p.pid} {p.lane}")

    def _cmd_shelve(self, parts):
        if len(parts) != 1:
            self._emit("R SHELVE FMT")
            return
        n = 0
        for pid in self.ledge:
            self._place(self.parcels[pid])
            n += 1
        self.ledge = []
        for l in self.lane_order:
            for pid in self.bay:
                if self.parcels[pid].lane == l:
                    self._place(self.parcels[pid])
                    n += 1
        self.bay = []
        self._emit(f"OK {n}")

    def _standing_rank(self, p):
        same_standing = 0
        rank = 0
        for q in self.parcels.values():
            if q.sender == p.sender and q.lodge_seq <= p.lodge_seq:
                rank += 1
        return rank

    def _sender_pb(self, p):
        parcels = 0
        bounces = 0
        for q in self.parcels.values():
            if q.sender == p.sender and q.lane == p.lane:
                pre = sum(1 for t in q.bounce_seqs if t < p.lodge_seq)
                if pre > 0:
                    parcels += 1
                    bounces += pre
        return parcels * bounces

    def _raw(self, p, f_now):
        eff = p.heft
        if self._standing_rank(p) % 3 == 0:
            eff += 15
            if p.bounces > 0:
                eff += 2
        eff -= 35 * p.bounces
        eff += 5 * (f_now - p.r_placed)
        if p.bounces >= 5:
            eff += 1
        if p.bounces == 0:
            pb = self._sender_pb(p)
            if pb >= 2:
                eff -= 10
                if pb >= 6:
                    eff += 1
        return eff

    def _eff(self, p, r_now, f_now):
        eff = self._raw(p, f_now)
        floor = 5 * r_now + 1
        if eff < floor:
            eff = floor
        return eff

    def _cmd_round(self, parts):
        if len(parts) != 2:
            self._emit("R ROUND FMT")
            return
        l = parts[1]
        if not self._tok_ok(l):
            self._emit(f"R {l} FMT")
            return
        if l not in self.shelves:
            self._emit(f"R {l} DEST")
            return
        shelf = self.shelves[l]
        self.c_round += 1
        r_now = self.lane_rounds[l]
        self.lane_rounds[l] = r_now + 1
        if not shelf:
            self._emit(f"VOID {l}")
            return
        self._close_window()
        f_now = self.lane_full_rounds[l]
        self.lane_full_rounds[l] = f_now + 1
        for pid in self.bay:
            self.parcels[pid].burned = True
        orig_pos = {pid: i for i, pid in enumerate(shelf)}
        effs = {pid: self._eff(self.parcels[pid], r_now, f_now) for pid in shelf}
        raws = {pid: self._raw(self.parcels[pid], f_now) for pid in shelf}
        manifest = []
        cap = 4 if r_now < 3 else 3
        k = min(cap, len(shelf))
        for _ in range(k):
            top = max(effs[pid] for pid in shelf)
            tied = [q for q in shelf if effs[q] == top]
            if any(orig_pos[q] == 0 for q in tied):
                newer_wins = False
                if len(tied) == 2:
                    ns = {self.parcels[q].nshelve for q in tied}
                    if ns == {0, 1}:
                        older = min(
                            tied, key=lambda q: self.parcels[q].stamp
                        )
                        if any(
                            r.state not in (OUT, GONE)
                            and r.nshelve == 0
                            and r.pid != older
                            and r.pid not in tied
                            for r in self.parcels.values()
                        ):
                            newer_wins = True
            else:
                by_stamp = sorted(
                    tied, key=lambda q: self.parcels[q].stamp
                )
                newer_wins = (
                    self.parcels[by_stamp[-1]].nshelve == 1
                    and self.parcels[by_stamp[0]].nshelve != 1
                )
            best = None
            best_key = None
            for pid in shelf:
                p = self.parcels[pid]
                key = (
                    (effs[pid], p.stamp)
                    if newer_wins
                    else (effs[pid], -p.stamp)
                )
                if best is None or key > best_key:
                    best = p
                    best_key = key
            shelf.remove(best.pid)
            best.state = OUT
            manifest.append(best.pid)
            self._emit(f"OUT {best.pid}")
        emptied = not shelf and (not self.bay or k == 1)
        for pid in manifest:
            self.parcels[pid].cleared = emptied
        self.window = manifest
        self.window_open = True
        self.window_bounced = set()
        self._emit(f"RAN {l} {k}")

    def _cmd_bounce(self, parts):
        if len(parts) != 2:
            self._emit("R BOUNCE FMT")
            return
        pid = parts[1]
        if not self._tok_ok(pid):
            self._emit(f"R {pid} FMT")
            return
        if (
            pid not in self.parcels
            or not self.window_open
            or pid not in self.window
            or pid in self.window_bounced
        ):
            self._emit(f"R {pid} STATE")
            return
        p = self.parcels[pid]
        self.window_bounced.add(pid)
        p.state = LEDGE
        p.bounces += 1
        p.r_placed = self.lane_full_rounds[p.lane]
        p.bounce_seqs.append(self.lodge_seq_next)
        self.lodge_seq_next += 1
        p.touch = self.seq_next
        self.seq_next += 1
        self.ledge.append(pid)
        self.c_bounce += 1
        self._emit(f"OK {pid}")

    def _cmd_waive(self, parts):
        if len(parts) != 2:
            self._emit("R WAIVE FMT")
            return
        pid = parts[1]
        if not self._tok_ok(pid):
            self._emit(f"R {pid} FMT")
            return
        p = self.parcels.get(pid)
        if p is None or p.state not in (BAY, SHELVED):
            self._emit(f"R {pid} STATE")
            return
        if p.state == BAY:
            self.bay.remove(pid)
        else:
            self.shelves[p.lane].remove(pid)
        p.state = WAIVED
        self.c_waive += 1
        self._emit(f"OK {pid}")

    def _cmd_square(self, parts):
        if len(parts) != 1:
            self._emit("R SQUARE FMT")
            return
        self.c_square += 1
        self._close_window()
        for l in self.lane_order:
            self.lane_rounds[l] = self.lane_full_rounds[l]
        for l in self.lane_order:
            kept = [
                pid
                for pid, p in self.parcels.items()
                if p.lane == l
                and p.state in (SHELVED, WAIVED, LEDGE, BAY)
                and (p.state != SHELVED or pid in self.shelves[l])
            ]
            kept.sort(key=lambda pid: self.parcels[pid].touch)
            shelf = self.shelves[l]
            shelf.clear()
            for pid in kept:
                p = self.parcels[pid]
                if p.state == SHELVED and p.nshelve < 1:
                    p.nshelve = 1
                self._place(
                    p,
                    put=False,
                    shelved=False,
                    set_r=(p.state == BAY and not p.burned),
                )
            names = " ".join(shelf)
            self._emit(f"SQ {l} {names}" if names else f"SQ {l}")
        self.bay = []
        self.ledge = []
        self._emit(f"OK {len(self.lane_order)}")

    def _cmd_list(self, parts):
        if len(parts) != 2:
            self._emit("R LIST FMT")
            return
        l = parts[1]
        if not self._tok_ok(l):
            self._emit(f"R {l} FMT")
            return
        if l not in self.shelves:
            self._emit(f"R {l} DEST")
            return
        names = " ".join(self.shelves[l])
        self._emit(f"SHELF {l} {names}" if names else f"SHELF {l}")

    def _cmd_held(self, parts):
        if len(parts) != 2:
            self._emit("R HELD FMT")
            return
        s = parts[1]
        if not self._tok_ok(s):
            self._emit(f"R {s} FMT")
            return
        if s not in self.senders:
            self._emit(f"R {s} WHO")
            return
        lane_pos = {l: i for i, l in enumerate(self.lane_order)}
        held = [
            p for p in self.parcels.values() if p.sender == s and p.state == SHELVED
        ]
        held.sort(key=lambda p: (lane_pos[p.lane], p.stamp))
        joined = " ".join(p.pid for p in held)
        self._emit(f"FOR {s} {joined}" if joined else f"FOR {s}")

    def _cmd_roll(self, parts):
        if len(parts) != 1:
            self._emit("R ROLL FMT")
            return
        snds = " ".join(self.senders)
        self._emit(f"SND {snds}" if snds else "SND")
        lns = " ".join(self.lane_order)
        self._emit(f"LNS {lns}" if lns else "LNS")

    def finish(self):
        self._emit(
            f"X {self.c_reg} {self.c_lane} {self.c_lodge}"
            f" {self.c_round} {self.c_bounce} {self.c_waive} {self.c_square}"
        )


def main():
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
