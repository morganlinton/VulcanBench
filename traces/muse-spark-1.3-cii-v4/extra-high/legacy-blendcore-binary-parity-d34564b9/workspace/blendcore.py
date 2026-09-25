"""BlendCore ink-blending controller, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
blending commands on stdin (``T`` fills a tank with a pigment stock,
``D`` dispenses from a tank into a job, ``R`` reconciles every tank's
book volume from its ledger), writes one reply per command and an ``X``
trailer at EOF.
Format reference: ``docs/SPEC.md`` (note the drift warning at the top of
that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import sys

ID_MAX = 8
NO_TANK = "????????"
CAP = 9999999
SHORT_TOL = 20
RECONCILE_EVAP_THRESH = 51


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


def _evap(amount: int, pigment: str) -> int:
    base = amount // 500
    if base and "W" in pigment and amount >= 1000:
        base *= 2
    return base


class Tank:
    def __init__(self, canon: str, pigment: str, fills_capped: int) -> None:
        self.canon = canon
        self.pigment = pigment
        self.fills_capped = fills_capped
        self.credited = 0
        self.evap = 0

    def live(self) -> int:
        v = self.fills_capped - self.credited - self.evap
        return v if v > 0 else 0


class Engine:
    """Blending controller with live dispense, reconcile, and counters."""

    def __init__(self) -> None:
        self.tanks: dict[str, Tank] = {}
        self.jobs: dict[str, list] = {}
        self.fills = 0
        self.dispenses = 0
        self.reconciles = 0
        self.rejected = 0

    def handle(self, line: str) -> list[str]:
        if line == "":
            return []
        raw = line.split(" ")
        cmd = raw[0]
        if cmd == "R":
            if line != "R":
                replies = [f"N {NO_TANK} FMT"]
            else:
                replies = self._reconcile()
        elif cmd == "T":
            replies = [self._fill(raw)]
        elif cmd == "D":
            replies = [self._dispense(raw)]
        else:
            replies = [f"N {NO_TANK} FMT"]
        if replies and replies[0].startswith("N "):
            self.rejected += 1
        return replies

    def trailer(self) -> str:
        return (
            f"X {self.fills} {self.dispenses} "
            f"{self.reconciles} {self.rejected}"
        )

    def _fill(self, raw: list[str]) -> str:
        args = [t for t in raw[1:] if t != ""]
        tank_echo = args[0] if len(args) >= 1 else NO_TANK
        if len(args) < 3:
            return f"N {tank_echo} FMT"
        tank_tok, pig_tok, vol_tok = args[0], args[1], args[2]
        if not _valid_id(tank_tok) or not _valid_id(pig_tok):
            return f"N {tank_echo} FMT"
        if not _digits(vol_tok) or not 2 <= len(vol_tok) <= 7:
            return f"N {tank_echo} VOL"
        volume = int(vol_tok)
        key = tank_tok.upper()
        t = self.tanks.get(key)
        if t is not None:
            if t.pigment != pig_tok:
                return f"N {tank_echo} PIGMENT"
            t.fills_capped = t.fills_capped + volume
            if t.fills_capped > CAP:
                t.fills_capped = CAP
        else:
            fills = volume if volume <= CAP else CAP
            self.tanks[key] = Tank(tank_tok, pig_tok, fills)
        self.fills += 1
        return f"OK {len(self.tanks)}"

    def _dispense(self, raw: list[str]) -> str:
        args = [t for t in raw[1:] if t != ""]
        tank_echo = args[1] if len(args) >= 2 else NO_TANK
        if len(args) < 3:
            return f"N {tank_echo} FMT"
        job_tok, tank_tok, amt_tok = args[0], args[1], args[2]
        if not _valid_id(job_tok) or not _valid_id(tank_tok):
            return f"N {tank_echo} FMT"
        if not _digits(amt_tok) or len(amt_tok) > 6 or int(amt_tok) < 1:
            return f"N {tank_echo} AMT"
        amount = int(amt_tok)
        key = tank_tok.upper()
        t = self.tanks.get(key)
        if t is None:
            return f"N {tank_echo} FMT"
        live_before = t.live()
        if amount <= live_before:
            ev_theoretical = _evap(amount, t.pigment)
            remaining = live_before - amount
            ev = ev_theoretical if ev_theoretical <= remaining else remaining
            t.credited += amount
            t.evap += ev
            jkey = job_tok.upper()
            entry = self.jobs.get(jkey)
            if entry is None:
                self.jobs[jkey] = [job_tok, amount]
                canon, total = job_tok, amount
            else:
                entry[1] += amount
                canon, total = entry[0], entry[1]
            self.dispenses += 1
            return f"J {canon} {total}"
        over = amount - live_before
        if over <= SHORT_TOL:
            granted = live_before
            t.credited += granted
            jkey = job_tok.upper()
            entry = self.jobs.get(jkey)
            if entry is None:
                self.jobs[jkey] = [job_tok, granted]
                canon, total = job_tok, granted
            else:
                entry[1] += granted
                canon, total = entry[0], entry[1]
            self.dispenses += 1
            return f"J {canon} {total}"
        return f"N {tank_echo} DRY"

    def _reconcile(self) -> list[str]:
        replies = []
        for t in self.tanks.values():
            book = t.fills_capped - t.credited
            diff = t.evap // 2 if t.evap >= RECONCILE_EVAP_THRESH else 0
            b = book - diff
            if b < 0:
                b = 0
            replies.append(f"B {t.canon} {b}")
            t.fills_capped = b
            t.credited = 0
            t.evap = 0
        replies.append(f"REND {len(self.tanks)}")
        self.reconciles += 1
        return replies


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if line == "":
            continue
        replies.extend(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    process(sys.stdin.readlines(), out=sys.stdout)


if __name__ == "__main__":
    main()
