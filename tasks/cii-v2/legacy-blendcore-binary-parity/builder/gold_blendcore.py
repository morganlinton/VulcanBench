"""BlendCore ink-blending controller, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
blending commands on stdin (``T`` fills a tank with a pigment stock,
``D`` dispenses from a tank into a job, ``R`` reconciles every tank's
book volume from its ledger), writes one reply per command and an ``X``
trailer at EOF.
Format reference: ``docs/SPEC.md``, corrected against the engine's actual
behavior wherever the two disagree (the engine is the contract); every
deviation from the written spec is marked ``engine:`` below.
"""

from __future__ import annotations

import sys

ID_MAX = 8
NO_TANK = "????????"
# engine: a tank never holds more than this many tenths; refills that
# would push it past the maximum cap silently (physical and ledger).
VOL_MAX = 9_999_999
# engine: the live dispense path draws a settling allowance of 1 tenth
# per full 500 tenths drawn (thick pigment sticks to the lines); the
# allowance is deducted from the tank but never credited to the job.
ALLOW_STEP = 500
# engine: pigments whose id contains the letter W, on dispenses drawing
# at least 1000 tenths, accrue DOUBLE the allowance (white-base pigments
# run thicker in bulk).
BULK_MIN = 1000
# engine: allowance accumulated since a tank's last reconcile above this
# many tenths triggers a shrinkage write-off at R time.
WRITEOFF_LIMIT = 50
# engine: a dispense short by at most this many tenths is filled with
# everything available (a short pour) instead of rejecting DRY.
DRY_MARGIN = 20


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


class Tank:
    def __init__(self, spelling: str, pigment: str, volume: int) -> None:
        self.spelling = spelling
        self.pigment = pigment
        self.volume = volume
        self.fills = volume
        self.dispensed = 0
        # engine: allowance accumulated since the last reconcile.
        self.acc = 0


class Engine:
    """Blending controller with live dispense, reconcile, and counters."""

    def __init__(self) -> None:
        self.tanks: dict[str, Tank] = {}
        self.jobs: dict[str, tuple[str, int]] = {}
        self.fills = 0
        self.dispenses = 0
        self.reconciles = 0
        self.rejected = 0

    def handle(self, line: str) -> list[str]:
        tokens = line.split()
        if not tokens:
            return []
        if line == "R":
            replies = self._reconcile()
        elif tokens[0] == "T":
            replies = [self._fill(tokens)]
        elif tokens[0] == "D":
            replies = [self._dispense(tokens)]
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
        tank = tokens[1] if len(tokens) >= 2 else NO_TANK
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 4:
            return f"N {tank} FMT"
        if not _valid_id(tokens[1]) or not _valid_id(tokens[2]):
            return f"N {tank} FMT"
        vol_tok = tokens[3]
        if not _digits(vol_tok) or not 2 <= len(vol_tok) <= 7:
            return f"N {tank} VOL"
        volume = int(vol_tok)
        # engine: tank ids fold case; first-seen spelling wins.
        t = self.tanks.get(tokens[1].lower())
        if t is not None:
            if t.pigment != tokens[2]:
                return f"N {tank} PIGMENT"
            # engine: refills cap silently at the tank maximum.
            t.volume = min(t.volume + volume, VOL_MAX)
            t.fills = min(t.fills + volume, VOL_MAX)
        else:
            self.tanks[tokens[1].lower()] = Tank(
                tokens[1], tokens[2], min(volume, VOL_MAX)
            )
        self.fills += 1
        return f"OK {len(self.tanks)}"

    def _dispense(self, tokens: list[str]) -> str:
        tank = tokens[2] if len(tokens) >= 3 else NO_TANK
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 4:
            return f"N {tank} FMT"
        if not _valid_id(tokens[1]) or not _valid_id(tokens[2]):
            return f"N {tank} FMT"
        amt_tok = tokens[3]
        if not _digits(amt_tok) or len(amt_tok) > 6 or int(amt_tok) < 1:
            return f"N {tank} AMT"
        amount = int(amt_tok)
        t = self.tanks.get(tokens[2].lower())
        if t is None:
            return f"N {tank} FMT"
        if amount > t.volume:
            # engine: a shortfall within the margin is a short pour: the
            # job is credited with everything available.
            if amount - t.volume > DRY_MARGIN:
                return f"N {tank} DRY"
            granted = t.volume
        else:
            granted = amount
        allowance = granted // ALLOW_STEP
        if granted >= BULK_MIN and "W" in t.pigment:
            allowance *= 2
        deduct = min(allowance, t.volume - granted)
        t.volume -= granted + deduct
        t.acc += deduct
        # the ledger sees the job-credited amount only, never the
        # settling allowance.
        t.dispensed += granted
        # engine: job ids fold case; first-seen spelling wins.
        spelling, total = self.jobs.get(tokens[1].lower(), (tokens[1], 0))
        total += granted
        self.jobs[tokens[1].lower()] = (spelling, total)
        self.dispenses += 1
        return f"J {spelling} {total}"

    def _reconcile(self) -> list[str]:
        replies = []
        for t in self.tanks.values():
            # engine: book volume is fills minus job-credited dispenses,
            # WITHOUT the settling allowance, so it usually sits above
            # the physical volume the live path left behind.
            book = t.fills - t.dispensed
            # engine: accumulated allowance above the write-off limit is
            # written off: the book drops by half of it instead.
            if t.acc > WRITEOFF_LIMIT:
                book -= t.acc // 2
            book = max(book, 0)
            replies.append(f"B {t.spelling} {book}")
            # engine: reconcile-as-correction: the reported book value
            # becomes the tank's new physical volume and ledger baseline.
            t.volume = book
            t.fills = book
            t.dispensed = 0
            t.acc = 0
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
