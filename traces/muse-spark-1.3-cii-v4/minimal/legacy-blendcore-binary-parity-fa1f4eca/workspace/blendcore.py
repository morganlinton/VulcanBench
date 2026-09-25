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
VOL_CAP = 9999999
# Fee unit: the base fee (2 * amount) // 1000, doubled for white-base
# pours of at least 1000. Small white-base pours deduct the single base
# fee live but post every second nonzero fee doubled.
RATE_W = 2
RATE_PLAIN = 1
# Book fee at reconcile: S // 2 where S is the fitted-fee accumulator
# (full pours only), applied once S reaches the trigger level.
TRIG_W = 52
TRIG_PLAIN = 51
# A dispense past the live volume drains the tank iff the shortfall is
# within TOL; otherwise it is DRY.
TOL = 20


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


def _rate(pigment: str) -> int:
    return RATE_W if "W" in pigment else RATE_PLAIN


def _trig(pigment: str) -> int:
    return TRIG_W if "W" in pigment else TRIG_PLAIN


class Tank:
    def __init__(self, pigment: str, volume: int) -> None:
        self.pigment = pigment
        self.volume = volume
        self.fills = volume
        self.dispensed = 0
        self.fees = 0
        self.alt = False


class Engine:
    """Blending controller with live dispense, reconcile, and counters."""

    def __init__(self) -> None:
        self.tanks: dict[str, Tank] = {}
        self.names: dict[str, str] = {}
        self.jobs: dict[str, int] = {}
        self.job_names: dict[str, str] = {}
        self.fills = 0
        self.dispenses = 0
        self.reconciles = 0
        self.rejected = 0

    def handle(self, line: str) -> list[str]:
        if line == "R":
            replies = self._reconcile()
            return replies
        parts = line.split(" ")
        cmd = parts[0]
        rest = [p for p in parts[1:] if p != ""]
        if cmd == "T":
            replies = [self._fill(rest)]
        elif cmd == "D":
            replies = [self._dispense(rest)]
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

    def _fill(self, tokens: list[str]) -> str:
        tank = tokens[0] if len(tokens) >= 1 else NO_TANK
        if len(tokens) < 3:
            return f"N {tank} FMT"
        if not _valid_id(tokens[0]) or not _valid_id(tokens[1]):
            return f"N {tank} FMT"
        vol_tok = tokens[2]
        if not _digits(vol_tok) or not 2 <= len(vol_tok) <= 7:
            return f"N {tank} VOL"
        volume = int(vol_tok)
        key = tokens[0].lower()
        t = self.tanks.get(key)
        if t is not None:
            if t.pigment != tokens[1]:
                return f"N {tank} PIGMENT"
            t.volume = min(VOL_CAP, t.volume + volume)
            t.fills = min(VOL_CAP, t.fills + volume)
        else:
            self.tanks[key] = Tank(tokens[1], min(VOL_CAP, volume))
            self.names[key] = tokens[0]
        self.fills += 1
        return f"OK {len(self.tanks)}"

    def _dispense(self, tokens: list[str]) -> str:
        tank = tokens[1] if len(tokens) >= 2 else NO_TANK
        if len(tokens) < 3:
            return f"N {tank} FMT"
        if not _valid_id(tokens[0]) or not _valid_id(tokens[1]):
            return f"N {tank} FMT"
        amt_tok = tokens[2]
        if not _digits(amt_tok) or len(amt_tok) > 6 or int(amt_tok) < 1:
            return f"N {tank} AMT"
        amount = int(amt_tok)
        key = tokens[1].lower()
        t = self.tanks.get(key)
        if t is None:
            return f"N {tank} FMT"
        if amount <= t.volume:
            base = (2 * amount) // 1000
            big = "W" not in t.pigment or amount >= 1000
            live_fee = _rate(t.pigment) * base if big else base
            room = t.volume - amount
            fitted_live = min(live_fee, room)
            t.volume -= amount + fitted_live
            t.dispensed += amount
            if big:
                t.fees += fitted_live
            elif base > 0:
                # Small white-base pours post every second nonzero fee.
                if t.alt:
                    post = _rate(t.pigment) * base
                    t.fees += min(post, room)
                t.alt = not t.alt
            granted = amount
        elif amount - t.volume <= TOL:
            granted = t.volume
            t.volume = 0
            t.dispensed += granted
        else:
            return f"N {tank} DRY"
        jkey = tokens[0].lower()
        if jkey not in self.job_names:
            self.job_names[jkey] = tokens[0]
        total = self.jobs.get(jkey, 0) + granted
        self.jobs[jkey] = total
        self.dispenses += 1
        return f"J {self.job_names[jkey]} {total}"

    def _reconcile(self) -> list[str]:
        replies = []
        for key, t in self.tanks.items():
            fee = t.fees // 2 if t.fees >= _trig(t.pigment) else 0
            book = t.fills - t.dispensed - fee
            # The book becomes the tank's new baseline for the next batch.
            t.fills = book
            t.dispensed = 0
            t.fees = 0
            t.alt = False
            t.volume = book
            replies.append(f"B {self.names[key]} {book}")
        replies.append(f"REND {len(self.tanks)}")
        self.reconciles += 1
        return replies


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if not line:
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
