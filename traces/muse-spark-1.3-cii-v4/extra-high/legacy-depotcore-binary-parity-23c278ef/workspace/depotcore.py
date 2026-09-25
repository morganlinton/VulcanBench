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
    __slots__ = ("pid", "sender", "lane", "heft", "state", "stamp",
                 "bounced", "sender_idx", "lodge_seq", "bounces",
                 "n_at_sq", "waived", "dispatched", "ledger_seq")

    def __init__(self, pid, sender, lane, heft):
        self.pid = pid
        self.sender = sender
        self.lane = lane
        self.heft = heft
        self.state = BAY
        self.stamp = 0
        self.bounced = False
        self.sender_idx = 0
        self.lodge_seq = 0
        self.bounces = 0
        self.n_at_sq = 0
        self.waived = False
        self.dispatched = False
        self.ledger_seq = 0


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.senders: dict[str, bool] = {}  # registration order
        self.lane_order: list[str] = []
        self.shelves: dict[str, list[str]] = {}
        self.parcels: dict[str, Parcel] = {}
        self.bay: list[str] = []    # lodge order
        self.ledge: list[str] = []  # bounce order
        self.stamp_next = 1
        self.lodge_seq_next = 1
        self.ledger_next = 1
        self.sender_counts: dict[str, int] = {}
        self.lane_real: dict[str, int] = {}
        self.lane_void_since_sq: dict[str, int] = {}
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
                p.dispatched = True
        self.window = []
        self.window_open = False
        self.window_bounced = set()

    def _effective(self, p):
        bonus = 0
        if p.sender_idx % 3 == 2:
            bonus = 15
            if p.bounces > 0 or p.heft % 100 == 10:
                bonus += 1
        n = p.bounces
        if p.n_at_sq >= 1:
            penalty = (40 - 5 * p.n_at_sq) + 35 * (n - p.n_at_sq)
        else:
            penalty = 35 * n
        lane_c = self.lane_real.get(p.lane, 0) + self.lane_void_since_sq.get(p.lane, 0)
        lane_bonus = 0
        if lane_c >= 2 and p.heft % 25 == (5 * lane_c) % 25:
            lane_bonus = 1
        eff = p.heft + bonus + lane_bonus - penalty
        floor = 5 * n + 1
        lane_floor = 5 * lane_c + 1
        if lane_floor > floor:
            floor = lane_floor
        if eff < floor:
            eff = floor
        return eff

    def _lane_cap(self, l):
        used = self.lane_real.get(l, 0) + self.lane_void_since_sq.get(l, 0)
        return 4 if used < 3 else 3

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
        self.lane_real[l] = 0
        self.lane_void_since_sq[l] = 0
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
        p.sender_idx = self.sender_counts.get(s, 0)
        self.sender_counts[s] = p.sender_idx + 1
        p.lodge_seq = self.lodge_seq_next
        self.lodge_seq_next += 1
        p.ledger_seq = self.ledger_next
        self.ledger_next += 1
        self.parcels[pid] = p
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
        for pid in list(self.ledge):
            self._place(self.parcels[pid])
            n += 1
        self.ledge = []
        for l in self.lane_order:
            for pid in list(self.bay):
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
        if not shelf:
            self._emit(f"VOID {l}")
            self.lane_void_since_sq[l] = self.lane_void_since_sq.get(l, 0) + 1
            return
        self._close_window()
        manifest = []
        cap = self._lane_cap(l)
        k = min(cap, len(shelf))
        for _ in range(k):
            best = None
            best_eff = None
            for pid in shelf:
                p = self.parcels[pid]
                e = self._effective(p)
                if best is None or e > best_eff:
                    best = p
                    best_eff = e
            shelf.remove(best.pid)
            best.state = OUT
            manifest.append(best.pid)
            self._emit(f"OUT {best.pid}")
        self.window = manifest
        self.window_open = True
        self.window_bounced = set()
        self.lane_real[l] = self.lane_real.get(l, 0) + 1
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
        p.bounced = True
        p.bounces += 1
        p.ledger_seq = self.ledger_next
        self.ledger_next += 1
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
        self.c_waive += 1
        self._emit(f"OK {pid}")

    def _cmd_square(self, parts):
        if len(parts) != 1:
            self._emit("R SQUARE FMT")
            return
        self.c_square += 1
        self._close_window()
        for l in self.lane_order:
            self.lane_void_since_sq[l] = 0
        for p in self.parcels.values():
            if p.bounces >= 1:
                p.n_at_sq = p.bounces
        for l in self.shelves:
            self.shelves[l] = []
        ordered = sorted(self.parcels.values(), key=lambda p: p.ledger_seq)
        for p in ordered:
            if p.dispatched:
                continue
            p.state = SHELVED
            p.stamp = self.stamp_next
            self.stamp_next += 1
            self.shelves[p.lane].append(p.pid)
        self.bay = []
        self.ledge = []
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
                if self.parcels[pid].sender == s:
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
