"""DepotCore parcel-depot engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One depot
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys

BAY, SHELVED, OUT, LEDGE, GONE = 0, 1, 2, 3, 4
CAP = 4


class Parcel:
    __slots__ = (
        "pid",
        "sender",
        "lane",
        "heft",
        "state",
        "stamp",
        "sender_idx",
        "lodge_seq",
        "bounces",
        "seniority",
        "waived",
        "tenured",
        "ghost",
        "ghost_rounds",
    )

    def __init__(self, pid, sender, lane, heft, sender_idx, lodge_seq):
        self.pid = pid
        self.sender = sender
        self.lane = lane
        self.heft = heft
        self.state = BAY
        self.stamp = 0
        self.sender_idx = sender_idx
        self.lodge_seq = lodge_seq
        self.bounces = 0
        self.seniority = 0
        self.waived = False
        self.tenured = False
        self.ghost = False
        self.ghost_rounds = 0


def _bounce_penalty(bounces):
    if bounces <= 0:
        return 0
    if bounces == 1:
        return 35
    return 35 * bounces - 10


def _dispatch_key(p):
    key = p.heft + p.seniority - _bounce_penalty(p.bounces)
    if p.sender_idx % 3 == 2:
        key += 15
    return key


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.senders: dict[str, bool] = {}  # registration order
        self.lane_order: list[str] = []
        self.shelves: dict[str, list[str]] = {}
        self.parcels: dict[str, Parcel] = {}
        self.bay: list[str] = []    # lodge order
        self.ledge: list[str] = []  # bounce order
        self.sender_counts: dict[str, int] = {}
        self.lodge_seq_next = 1
        self.lane_rounds: dict[str, int] = {}
        self.lane_voids: dict[str, int] = {}
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
        sender_idx = self.sender_counts.get(s, 0)
        self.sender_counts[s] = sender_idx + 1
        lodge_seq = self.lodge_seq_next
        self.lodge_seq_next += 1
        self.parcels[pid] = Parcel(pid, s, l, heft, sender_idx, lodge_seq)
        self.parcels[pid].stamp = self.stamp_next
        self.stamp_next += 1
        self.bay.append(pid)
        self.c_lodge += 1
        self._emit(f"OK {pid}")

    def _place(self, p):
        p.stamp = self.stamp_next
        self.stamp_next += 1
        p.state = SHELVED
        self.shelves[p.lane].append(p.pid)
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
        self.lane_rounds[l] = self.lane_rounds.get(l, 0) + 1
        cap = 3 if self.lane_rounds[l] >= 4 else CAP
        if not shelf:
            self.lane_voids[l] = self.lane_voids.get(l, 0) + 1
            self._emit(f"VOID {l}")
            return
        self._close_window()
        manifest = []
        ordered = sorted(
            shelf,
            key=lambda pid: (
                -self.parcels[pid].tenured,
                -_dispatch_key(self.parcels[pid]),
                self.parcels[pid].stamp,
            ),
        )
        k = min(cap, len(shelf))
        for pid in ordered[:k]:
            shelf.remove(pid)
            p = self.parcels[pid]
            p.state = OUT
            p.waived = False
            manifest.append(pid)
            self._emit(f"OUT {pid}")
        for pid in shelf:
            self.parcels[pid].seniority += 5
            self.parcels[pid].tenured = True
        for pid in self.ledge:
            self.parcels[pid].seniority += 5
        for pid, p in self.parcels.items():
            if p.ghost and p.lane == l and p.state == GONE:
                if p.ghost_rounds == 0:
                    p.seniority += 4
                else:
                    p.seniority += 5
                p.ghost_rounds += 1
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
        p.stamp = self.stamp_next
        self.stamp_next += 1
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
        p.state = GONE
        p.waived = True
        p.ghost = True
        self.c_waive += 1
        self._emit(f"OK {pid}")

    def _insert_by_stamp(self, lane, pid):
        shelf = self.shelves[lane]
        stamp = self.parcels[pid].stamp
        i = 0
        while i < len(shelf) and self.parcels[shelf[i]].stamp <= stamp:
            i += 1
        shelf.insert(i, pid)

    def _insert_by_lodge(self, lane, pid):
        shelf = self.shelves[lane]
        lodge = self.parcels[pid].lodge_seq
        i = 0
        while i < len(shelf) and self.parcels[shelf[i]].lodge_seq <= lodge:
            i += 1
        shelf.insert(i, pid)

    def _cmd_square(self, parts):
        if len(parts) != 1:
            self._emit("R SQUARE FMT")
            return
        self.c_square += 1
        self._close_window()
        for l in self.lane_rounds:
            self.lane_rounds[l] -= self.lane_voids.get(l, 0)
        self.lane_voids = {}
        incoming_ledge = list(self.ledge)
        self.ledge = []
        incoming_bay = list(self.bay)
        self.bay = []
        incoming = []
        for pid in incoming_ledge:
            incoming.append(pid)
        for pid in self.parcels:
            p = self.parcels[pid]
            if p.state == GONE and p.waived and p.lane in self.shelves:
                p.ghost = False
                incoming.append(pid)
        for pid in incoming_bay:
            incoming.append(pid)
        if not incoming:
            for l in self.lane_order:
                for pid in self.shelves[l]:
                    self.parcels[pid].tenured = True
        incoming.sort(key=lambda pid: self.parcels[pid].stamp)
        for pid in incoming:
            p = self.parcels[pid]
            p.state = SHELVED
            self._insert_by_stamp(p.lane, pid)
        for l in self.lane_order:
            names = " ".join(self.shelves[l])
            self._emit(f"SQ {l} {names}" if names else f"SQ {l}")
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
        held: list[str] = []
        for l in self.lane_order:
            for pid in self.shelves[l]:
                p = self.parcels[pid]
                if p.sender == s and p.state == SHELVED:
                    held.append(pid)
        joined = " ".join(held)
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
