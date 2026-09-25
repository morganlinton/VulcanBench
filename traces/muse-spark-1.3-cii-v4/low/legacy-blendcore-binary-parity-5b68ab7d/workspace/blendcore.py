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

# Engine constants (observed from the legacy binary, not the spec).
TANK_CAP = 9999999  # live/book volume clamps here on fill
SHORT_TOL = 20  # overdraw within this grants a short (partial) pour
RES_DIV = 500  # per-dispense residue: amount // 500 held back live
RES_W_AT = 1000  # 'W' pigments double the residue at/above this pour
GAP_EVAP_AT = 51  # reconcile evaporates when the book/live gap
GAP_EVAP_NEG = -25500  # ... reaches this (or this far negative)


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


def _fold(token: str) -> str:
    return token.lower()


def _is_white_base(pigment: str) -> bool:
    return "W" in pigment


class Tank:
    def __init__(self, display: str, pigment: str, volume: int) -> None:
        self.display = display
        self.pigment = pigment
        self.volume = volume
        self.fills = volume
        self.dispensed = 0
        self.synced = 0
        self.residue = 0


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
        if line == "R":
            replies = self._reconcile()
        elif line.startswith("T ") or line.startswith("D "):
            # Space-separated; runs of spaces collapse, other
            # whitespace (tabs, ...) stays inside tokens. Extra
            # trailing tokens are ignored.
            tokens = [line[0]] + line[2:].split(" ")
            tokens = [tok for tok in tokens if tok != ""]
            if tokens[0] == "T":
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
        key = _fold(tokens[1])
        t = self.tanks.get(key)
        if t is not None:
            if t.pigment != tokens[2]:
                return f"N {tank} PIGMENT"
            # The tank holds at most TANK_CAP available volume;
            # overfill past capacity is lost. The allowance grows by
            # what has been reconciled out of the tank.
            t.fills = min(t.fills + volume, TANK_CAP + t.synced)
            t.volume = min(t.volume + volume, TANK_CAP)
        else:
            self.tanks[key] = Tank(tokens[1], tokens[2], volume)
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
        key = _fold(tokens[2])
        t = self.tanks.get(key)
        if t is None:
            return f"N {tank} FMT"
        if amount <= t.volume:
            fee = amount // RES_DIV
            if _is_white_base(t.pigment) and amount >= RES_W_AT:
                fee *= 2
            t.volume = max(0, t.volume - amount - fee)
            t.residue += fee
            credited = amount
        elif amount - t.volume <= SHORT_TOL:
            credited = t.volume
            t.volume = 0
        else:
            return f"N {tank} DRY"
        t.dispensed += credited
        job_key = _fold(tokens[1])
        entry = self.jobs.get(job_key)
        if entry is None:
            entry = [tokens[1], 0]
            self.jobs[job_key] = entry
        entry[1] += credited
        self.dispenses += 1
        return f"J {entry[0]} {entry[1]}"

    def _reconcile(self) -> list[str]:
        replies = []
        for t in self.tanks.values():
            book = t.fills - t.dispensed
            gap = book - t.volume
            if t.residue >= GAP_EVAP_AT and (
                gap >= GAP_EVAP_AT or gap <= GAP_EVAP_NEG
            ):
                # C-style truncating division, like the engine.
                raw = min(t.residue, book)
                evap = raw // 2 if raw >= 0 else -((-raw) // 2)
            else:
                evap = 0
            t.dispensed += evap
            t.volume = t.fills - t.dispensed
            if t.volume < 0:
                t.volume = 0
            t.synced = t.dispensed
            t.residue = 0
            replies.append(f"B {t.display} {t.volume}")
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
