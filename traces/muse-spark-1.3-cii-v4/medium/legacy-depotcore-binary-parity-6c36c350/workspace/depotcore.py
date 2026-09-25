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
        "bounces",
        "seniority",
        "bonus",
        "delivered",
        "touch",
        "puttouch",
        "departwear",
        "ledgesen",
        "voidbay",
        "voidflag",
        "grant",
        "lodgeseq",
    )

    def __init__(self, pid, sender, lane, heft):
        self.pid = pid
        self.sender = sender
        self.lane = lane
        self.heft = heft
        self.state = BAY
        self.stamp = 0
        self.bounces = 0
        self.seniority = 0
        self.bonus = 0
        self.delivered = False
        self.touch = 0
        self.puttouch = 0
        self.departwear = 0
        self.ledgesen = 0
        self.voidbay = 0
        self.voidflag = False
        self.grant = 0
        self.lodgeseq = 0


class Engine:
    def __init__(self, out=None):
        self.out = out if out is not None else sys.stdout
        self.senders: dict[str, bool] = {}  # registration order
        self.sender_counts: dict[str, int] = {}  # accepted lodges per sender
        self.sendersen: dict[str, int] = {}  # bounce seniority per sender
        self.lane_order: list[str] = []
        self.shelves: dict[str, list[str]] = {}
        self.parcels: dict[str, Parcel] = {}
        self.bay: list[str] = []    # lodge order
        self.ledge: list[str] = []  # bounce order
        self.wear: dict[str, int] = {}  # ROUNDs run per lane (VOID included)
        self.departs: dict[str, int] = {}  # departing ROUNDs per lane
        self.stamp_next = 1
        self.put_next = 1
        self.voidpot: dict[str, int] = {}  # per-lane void grant pot
        self.voidnonempty: dict[str, int] = {}  # per-lane non-empty voids seen
        self.voidever: dict[str, bool] = {}  # per-lane: a VOID has happened
        self.sqfresh = True  # no SHELVE since start/last SQUARE
        self.sqdone = False  # a SQUARE has happened
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

    def _eff(self, p):
        v = (
            p.heft
            - 35 * p.bounces
            + p.bonus
            + p.grant
            + 5 * (p.seniority + self.sendersen.get(p.sender, 0))
            + (1 if p.voidflag else 0)
        )
        if p.bounces:
            fl = 6 + 5 * p.departwear
            if v < fl:
                v = fl
            v += 5 * p.ledgesen
        if self.sendersen.get(p.sender, 0) < 0 and v < 11:
            v = 11
        return v

    def _cap(self, lane):
        return 4 if self.wear.get(lane, 0) < 3 else 3

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
        self.wear[l] = 0
        self.departs[l] = 0
        self.voidpot[l] = 0
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
        seq = self.sender_counts.get(s, 0) + 1
        self.sender_counts[s] = seq
        p.lodgeseq = seq
        if seq % 3 == 0:
            p.bonus = 15 + (1 if heft % 25 == 10 else 0)
        if heft % 25 == 16 and heft > 16:
            p.bonus = max(p.bonus, 1)
        if seq % 3 == 1 and seq > 1 and heft % 25 != 10 and self.voidever.get(l):
            p.bonus = max(p.bonus, 1)
        if not self.bay and self.sqfresh and self.sqdone:
            p.bonus = max(p.bonus, 1)
        p.touch = self.put_next
        p.puttouch = self.put_next
        self.put_next += 1
        self.parcels[pid] = p
        self.bay.append(pid)
        self.c_lodge += 1
        self._emit(f"OK {pid}")

    def _place(self, p):
        p.stamp = self.stamp_next
        self.stamp_next += 1
        p.puttouch = self.put_next
        self.put_next += 1
        p.state = SHELVED
        self.shelves[p.lane].append(p.pid)
        self._emit(f"PUT {p.pid} {p.lane}")

    def _cmd_shelve(self, parts):
        if len(parts) != 1:
            self._emit("R SHELVE FMT")
            return
        n = 0
        placed: list[str] = []
        for pid in self.ledge:
            self._place(self.parcels[pid])
            placed.append(pid)
            n += 1
        self.ledge = []
        for l in self.lane_order:
            for pid in self.bay:
                if self.parcels[pid].lane == l:
                    self._place(self.parcels[pid])
                    placed.append(pid)
                    n += 1
        self.bay = [pid for pid in self.bay if self.parcels[pid].state == BAY]
        self.sqfresh = False
        if self.sqdone and len(placed) == 4:
            for i, pid in enumerate(placed):
                if (i + 1) % 3 == 1:
                    p = self.parcels[pid]
                    p.bonus = max(p.bonus, 1)
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
        cap = self._cap(l)
        worn = self.wear.get(l, 0)
        if not shelf:
            self.wear[l] = worn + 1
            self.voidever[l] = True
            self._emit(f"VOID {l}")
            last = None
            for pid in self.bay:
                if self.parcels[pid].lane == l:
                    last = pid
            if last is not None:
                sender = self.parcels[last].sender
                for pid in self.bay:
                    p = self.parcels[pid]
                    if p.lane == l and p.sender == sender:
                        p.voidbay += 1
                        break
            self.voidpot[l] = self.voidpot.get(l, 0) + 5
            wit = [
                pid
                for pid in self.bay
                if self.parcels[pid].lane == l
            ]
            if wit:
                self.voidnonempty[l] = self.voidnonempty.get(l, 0) + 1
                if self.voidnonempty[l] >= 4:
                    return
                pot = self.voidpot[l]
                gave = False
                for pid in wit:
                    p = self.parcels[pid]
                    if pot >= 10:
                        if self._eff(p) < pot:
                            p.grant += pot + 1 - self._eff(p)
                        else:
                            p.grant += pot
                        gave = True
                        continue
                    if p.bonus != 0:
                        continue
                    if pot >= 5 and (
                        p.lodgeseq >= 7
                        or (
                            p.lodgeseq % 3 == 1
                            and p.lodgeseq > 1
                            and p.lodgeseq < 7
                        )
                    ):
                        p.grant += 1
                        gave = True
                    elif p.grant > 0:
                        p.grant += pot
                        gave = True
                if gave:
                    self.voidpot[l] = 0
            return
        self._close_window()
        self.departs[l] = self.departs.get(l, 0) + 1
        manifest = []
        k = min(cap, len(shelf))
        for _ in range(k):
            best = None
            for pid in shelf:
                p = self.parcels[pid]
                if best is None:
                    best = p
                    continue
                pe, be = self._eff(p), self._eff(best)
                if pe > be or (pe == be and p.stamp < best.stamp):
                    best = p
            shelf.remove(best.pid)
            best.state = OUT
            best.departwear = worn
            if best.voidbay:
                best.voidflag = True
                best.voidbay = 0
            manifest.append(best.pid)
            self._emit(f"OUT {best.pid}")
        for pid in shelf:
            self.parcels[pid].seniority += 1
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
        self.sendersen[p.sender] = self.sendersen.get(p.sender, 0) - 1
        p.state = LEDGE
        p.bounces += 1
        p.ledgesen += 1
        p.touch = self.put_next
        self.put_next += 1
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
        self.sqdone = True
        self.sqfresh = True
        for p in self.parcels.values():
            p.voidbay = 0
            p.voidflag = False
        for l in self.lane_order:
            self.wear[l] = self.departs.get(l, 0)
            kept = [
                pid
                for pid, p in self.parcels.items()
                if p.lane == l and not p.delivered
            ]

            def _key(pid):
                return self.parcels[pid].touch

            kept.sort(key=_key)
            self.shelves[l] = kept
            for i, pid in enumerate(kept):
                p = self.parcels[pid]
                was = p.state
                p.state = SHELVED
                p.stamp = self.stamp_next
                self.stamp_next += 1
                if was == BAY:
                    p.departwear = -4
                elif not (was == LEDGE and i == 0 and p.bounces == 1):
                    p.departwear = -2
                if (i + 1) % 3 == 1 and (i == 0 or len(kept) >= 9):
                    p.bonus = max(p.bonus, 1)
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
        held = []
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
