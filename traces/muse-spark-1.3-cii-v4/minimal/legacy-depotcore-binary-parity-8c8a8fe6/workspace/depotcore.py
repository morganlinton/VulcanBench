"""DepotCore parcel-depot engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One depot
session per process: commands on stdin, result lines on stdout, trailer
at end of input. Format reference: ``docs/SPEC.md`` (mind the drift
warning; the engine's behavior is the contract).
"""

from __future__ import annotations

import sys

BAY, SHELVED, OUT, LEDGE, GONE = 0, 1, 2, 3, 4


def _penalty(n):
    if n <= 0:
        return 0
    if n <= 5:
        return (0, 35, 60, 95, 130, 164)[n]
    return 164 + 35 * (n - 5)


def _floor(n):
    if n <= 0:
        return None
    if n == 1:
        return 6
    return 5 * n + 11


class Parcel:
    __slots__ = ("pid", "sender", "lane", "heft", "state", "seq",
                 "bounced", "ord", "delivered", "mark", "discount",
                 "placed", "vet", "placed_by", "voids", "wit", "ever_sh",
                 "lodge_era", "wit_eras", "sq_witmask", "sq_matemask",
                 "sq_batch")

    def __init__(self, pid, sender, lane, heft, ord, seq):
        self.pid = pid
        self.sender = sender
        self.lane = lane
        self.heft = heft
        self.state = BAY
        self.seq = seq
        self.bounced = 0
        self.ord = ord
        self.delivered = False
        self.mark = 0
        self.discount = False
        self.placed = False
        self.vet = 0
        self.placed_by = ""
        self.voids = 0
        self.wit = False
        self.ever_sh = False
        self.lodge_era = -1
        self.wit_eras = 0
        self.sq_witmask = 0
        self.sq_matemask = 0
        self.sq_batch = 0


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.senders: dict[str, bool] = {}  # registration order
        self.lane_order: list[str] = []
        self.shelves: dict[str, list[str]] = {}
        self.parcels: dict[str, Parcel] = {}
        self.bay: list[str] = []    # lodge order
        self.ledge: list[str] = []  # bounce order
        self.lodge_count: dict[str, int] = {}  # accepted lodges per sender
        self.state_count: dict[str, int] = {}  # rejected bounces per sender
        self.seq_next = 1
        self.window: list[str] = []
        self.window_open = False
        self.window_ever = False
        self.window_bounced: set[str] = set()
        self.lane_rounds: dict[str, int] = {}   # all rounds incl VOID
        self.lane_nonvoid: dict[str, int] = {}  # departed rounds only
        self.lane_voids: dict[str, int] = {}    # VOID rounds per lane
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

    def _score(self, p):
        bonus = 15 if p.ord % 3 == 0 else 0
        if p.bounced <= 0:
            n = self.lane_nonvoid.get(p.lane, 0) - 1
            v = p.voids
            base = p.heft + bonus + 1
            if p.placed_by == "square":
                best = base
                k = 5 * n + 2
                if k > best:
                    best = k
            else:
                best = base
                k = 5 * self.lane_voids.get(p.lane, 0) + 2
                if k > best:
                    best = k
            r = (p.heft + bonus) % 25
            if p.wit and p.placed_by == "square" and base + 5 > best:
                best = base + 5
                if p.sq_witmask & p.sq_matemask and base + 10 > best:
                    for q in self.parcels.values():
                        if (q.sq_batch == p.sq_batch and p.sq_batch
                                and q.wit_eras
                                and (1 << q.lodge_era) & p.sq_witmask):
                            best = base + 10
                            break
            if v == 3 and n >= 2 and p.heft + 10 > best:
                best = p.heft + 10
            return best
        n = self.lane_nonvoid.get(p.lane, 0) - 1
        kb = (5 * n + 2 if p.placed_by == "square"
              else 5 * self.lane_voids.get(p.lane, 0) + 2)
        if p.discount:
            pre = p.heft - 20 + bonus
            r = pre % 25
            q = 1 if r == 20 or (bonus and r == 5) else 0
            return max(_floor(p.bounced), pre + q, kb)
        pre = p.heft - _penalty(p.bounced) + bonus
        q = 0
        r = pre % 25
        if p.bounced == 1:
            if r == 5 or (bonus and r == 15):
                q = 1
        elif p.bounced == 2:
            if r in (5, 15):
                q = 1
        return max(_floor(p.bounced), pre + q, kb)

    def _close_window(self):
        for pid in self.window:
            p = self.parcels[pid]
            if p.state == OUT:
                p.state = GONE
                p.delivered = True
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
        self.lane_rounds[l] = 0
        self.lane_nonvoid[l] = 0
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
        self.lodge_count[s] = self.lodge_count.get(s, 0) + 1
        self.parcels[pid] = Parcel(pid, s, l, heft, self.lodge_count[s],
                                   self.seq_next)
        self.parcels[pid].lodge_era = self.c_round
        self.seq_next += 1
        self.bay.append(pid)
        self.c_lodge += 1
        self._emit(f"OK {pid}")

    def _place(self, p):
        p.state = SHELVED
        p.placed = True
        p.placed_by = "shelve"
        p.ever_sh = True
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
        by_lane: dict[str, list[str]] = {}
        for pid in self.bay:
            by_lane.setdefault(self.parcels[pid].lane, []).append(pid)
        for l in self.lane_order:
            for pid in by_lane.get(l, []):
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
        self.lane_rounds[l] += 1
        if not shelf:
            self.lane_voids[l] = self.lane_voids.get(l, 0) + 1
            for pid, p in self.parcels.items():
                if p.lane == l and p.state == BAY:
                    p.voids += 1
            self._emit(f"VOID {l}")
            return
        self._close_window()
        cap = 4 if self.lane_rounds[l] - 1 < 3 else 3
        rank = {pid: i for i, pid in enumerate(shelf)}
        ordered = sorted(shelf, key=lambda pid: (-self._score(self.parcels[pid]),
                                                rank[pid]))
        k = min(cap, len(shelf))
        manifest = ordered[:k]
        for pid in manifest:
            shelf.remove(pid)
            self.parcels[pid].state = OUT
            self._emit(f"OUT {pid}")
        self.lane_nonvoid[l] += 1
        self.window = list(manifest)
        self.window_open = True
        self.window_ever = True
        self.window_bounced = set()
        for pid, p in self.parcels.items():
            if p.lane == l and p.state != OUT:
                p.wit = True
                p.wit_eras |= (1 << self.c_round)
        self._emit(f"RAN {l} {k}")
        for pid, p in self.parcels.items():
            if p.state in (BAY, SHELVED, GONE):
                p.vet += 1

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
            p = self.parcels.get(pid)
            if p is not None and (
                p.state == SHELVED
                or (not p.placed and (p.state == GONE or self.window_open))
            ):
                n = self.lodge_count.get(p.sender, 0)
                if p.mark != n:
                    p.mark = n
                    s = p.sender
                    self.state_count[s] = self.state_count.get(s, 0) + 1
            self._emit(f"R {pid} STATE")
            return
        p = self.parcels[pid]
        self.window_bounced.add(pid)
        p.state = LEDGE
        p.bounced += 1
        p.seq = self.seq_next
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
        p.state = GONE
        self.c_waive += 1
        self._emit(f"OK {pid}")

    def _cmd_square(self, parts):
        if len(parts) != 1:
            self._emit("R SQUARE FMT")
            return
        self.c_square += 1
        self._close_window()
        sqmatemask = 0
        for pid, p in self.parcels.items():
            if not p.delivered and p.lodge_era >= 0:
                sqmatemask |= (1 << p.lodge_era)
        for l in self.lane_order:
            live = [pid for pid, p in self.parcels.items()
                    if p.lane == l and not p.delivered]
            live.sort(key=lambda pid: self.parcels[pid].seq)
            self.shelves[l] = live
            for pid in live:
                self.parcels[pid].state = SHELVED
                self.parcels[pid].placed = True
                self.parcels[pid].placed_by = "square"
                self.parcels[pid].sq_witmask = self.parcels[pid].wit_eras
                self.parcels[pid].sq_matemask = sqmatemask
                self.parcels[pid].sq_batch = self.c_square
                if self.parcels[pid].bounced >= 2:
                    self.parcels[pid].discount = True
            self.lane_rounds[l] = self.lane_nonvoid[l]
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
        held = [pid for l in self.lane_order for pid in self.shelves[l]
                if self.parcels[pid].sender == s]
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
