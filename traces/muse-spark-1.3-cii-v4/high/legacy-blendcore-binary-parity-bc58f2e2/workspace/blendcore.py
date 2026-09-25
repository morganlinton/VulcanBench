"""BlendCore ink-blending controller, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
blending commands on stdin (``T`` fills a tank with a pigment stock,
``D`` dispenses from a tank into a job, ``R`` reconciles every tank's
book volume from its ledger), writes one reply per command and an ``X``
trailer at EOF.
Format reference: ``docs/SPEC.md`` (note the drift warning at the top of
that file; the legacy engine's behavior is the contract).

Engine-contract behaviors (verified against the retired binary) that
differ from a naive reading of the spec:

* Lines are split on ASCII spaces only; extra tokens are ignored, so
  ``T``/``D`` need at least 4 tokens. Dispatch is on the raw first
  byte: leading whitespace, ``TT``/``DD``-style prefixes and embedded
  tabs all reply ``N ???????? FMT``. A whitespace-only line is a
  rejected ``FMT``, not a skip.
* Tank and job ids match case-insensitively (first-seen spelling is
  kept for ``B``/``J`` replies); the pigment match stays
  case-sensitive.
* Each dispense spills ``actual // 500`` tenths (doubled when the
  tank's pigment contains an uppercase ``W``), capped at what is left;
  the spill leaves the tank but never reaches the job total.
* A dispense short by 20 or less is granted as a partial pour of the
  remaining volume (credited with what was actually drawn); anything
  shorter replies ``N <tank> DRY``.
* Reconcile reports ``fills - dispensed - (spill // 2 if spill > 50)``,
  floored at zero, then adopts that as the tank's new live and book
  baseline and clears the period counters.
* Tank volumes clamp at 9999999. At most 4096 tanks and 4096 jobs; a
  dispense for a new job past the job cap still drains the tank but is
  rejected with ``N <tank> FMT`` and credits nothing.
"""

from __future__ import annotations

import sys

ID_MAX = 8
NO_TANK = "????????"
MAXV = 9999999
MAX_TANKS = 4096
MAX_JOBS = 4096


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


class Tank:
    def __init__(self, display: str, pigment: str, volume: int) -> None:
        self.display = display
        self.pigment = pigment
        self.volume = volume
        self.fills = volume
        self.dispensed = 0
        self.spill = 0


class Job:
    def __init__(self, display: str) -> None:
        self.display = display
        self.total = 0


class Engine:
    """Blending controller with live dispense, reconcile, and counters."""

    def __init__(self) -> None:
        self.tanks: dict[str, Tank] = {}
        self.jobs: dict[str, Job] = {}
        self.fills = 0
        self.dispenses = 0
        self.reconciles = 0
        self.rejected = 0

    def handle(self, line: str) -> list[str]:
        if line == "R":
            replies = self._reconcile()
        elif line[:1] in ("T", "D"):
            tokens = [t for t in line.split(" ") if t != ""]
            if not tokens or tokens[0] != line[:1]:
                replies = [f"N {NO_TANK} FMT"]
            elif line[:1] == "T":
                replies = [self._fill(tokens)]
            else:
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
        if len(tokens) < 4:
            return f"N {tank} FMT"
        if not _valid_id(tokens[1]) or not _valid_id(tokens[2]):
            return f"N {tank} FMT"
        vol_tok = tokens[3]
        if not _digits(vol_tok) or not 2 <= len(vol_tok) <= 7:
            return f"N {tank} VOL"
        volume = int(vol_tok)
        t = self.tanks.get(tokens[1].lower())
        if t is not None:
            if t.pigment != tokens[2]:
                return f"N {tank} PIGMENT"
            t.volume = min(t.volume + volume, MAXV)
            t.fills = min(t.fills + volume, MAXV)
        else:
            if len(self.tanks) >= MAX_TANKS:
                return f"N {tank} FMT"
            self.tanks[tokens[1].lower()] = Tank(tokens[1], tokens[2], volume)
        self.fills += 1
        return f"OK {len(self.tanks)}"

    def _dispense(self, tokens: list[str]) -> str:
        tank = tokens[2] if len(tokens) >= 3 else NO_TANK
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
        short = amount - t.volume
        if short > 20:
            return f"N {tank} DRY"
        actual = amount if short <= 0 else t.volume
        spill = actual // 500
        if actual >= 1000 and "W" in t.pigment:
            spill *= 2
        extra = min(spill, t.volume - actual)
        t.volume -= actual + extra
        t.dispensed += actual
        t.spill += extra
        key = tokens[1].lower()
        job = self.jobs.get(key)
        if job is None:
            if len(self.jobs) >= MAX_JOBS:
                return f"N {tank} FMT"
            job = self.jobs[key] = Job(tokens[1])
        job.total += actual
        self.dispenses += 1
        return f"J {job.display} {job.total}"

    def _reconcile(self) -> list[str]:
        replies = []
        for t in self.tanks.values():
            loss = t.spill // 2 if t.spill > 50 else 0
            book = t.fills - t.dispensed - loss
            if book < 0:
                book = 0
            t.volume = book
            t.fills = book
            t.dispensed = 0
            t.spill = 0
            replies.append(f"B {t.display} {book}")
        replies.append(f"REND {len(self.tanks)}")
        self.reconciles += 1
        return replies


def _strip(line: str) -> str:
    for i, ch in enumerate(line):
        if ch == "\r" or ch == "\n" or ch == "\x00":
            return line[:i]
    return line


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = _strip(raw)
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
