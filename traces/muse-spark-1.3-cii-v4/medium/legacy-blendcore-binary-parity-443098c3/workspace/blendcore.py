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
NO_TANK = b"????????"
MAXV = 9999999
# The legacy engine reads with fgets() into a 256 KiB buffer: each input
# chunk holds at most this many bytes, ending early after a newline.
CHUNK = 262143
# A dispense that overshoots the live volume by this much (or less) still
# grants whatever is left; anything more is rejected DRY.
SHORT = 20
# Reconcile halves the tracked loss, but only when it exceeds this.
EVAP_MIN = 50


def _valid_id(token: bytes) -> bool:
    return (
        1 <= len(token) <= ID_MAX
        and token.isascii()
        and token.isalnum()
    )


def _digits(token: bytes) -> bool:
    return len(token) > 0 and token.isascii() and token.isdigit()


class Tank:
    def __init__(self, name: bytes, pigment: bytes, volume: int) -> None:
        self.name = name
        self.pigment = pigment
        self.volume = volume
        self.fills = volume
        self.dispensed = 0
        self.evap = 0


class Engine:
    """Blending controller with live dispense, reconcile, and counters."""

    def __init__(self) -> None:
        self.tanks: dict[bytes, Tank] = {}
        self.jobs: dict[bytes, list] = {}
        self.fills = 0
        self.dispenses = 0
        self.reconciles = 0
        self.rejected = 0

    def handle(self, line: bytes) -> list[bytes]:
        if line == b"R":
            replies = self._reconcile()
        else:
            parts = line.split(b" ")
            cmd = parts[0]
            args = [p for p in parts[1:] if p != b""]
            if cmd == b"T":
                replies = [self._fill(args)]
            elif cmd == b"D":
                replies = [self._dispense(args)]
            else:
                replies = [b"N " + NO_TANK + b" FMT"]
        if replies and replies[0].startswith(b"N "):
            self.rejected += 1
        return replies

    def trailer(self) -> bytes:
        return (
            b"X "
            + str(self.fills).encode()
            + b" "
            + str(self.dispenses).encode()
            + b" "
            + str(self.reconciles).encode()
            + b" "
            + str(self.rejected).encode()
        )

    def _fill(self, args: list[bytes]) -> bytes:
        tank = args[0] if len(args) >= 1 else NO_TANK
        if len(args) < 3:
            return b"N " + tank + b" FMT"
        if not _valid_id(args[0]) or not _valid_id(args[1]):
            return b"N " + tank + b" FMT"
        vol_tok = args[2]
        if not _digits(vol_tok) or not 2 <= len(vol_tok) <= 7:
            return b"N " + tank + b" VOL"
        volume = int(vol_tok)
        t = self.tanks.get(args[0].lower())
        if t is not None:
            if t.pigment != args[1]:
                return b"N " + tank + b" PIGMENT"
            t.volume = min(t.volume + volume, MAXV)
            t.fills = min(t.fills + volume, MAXV)
        else:
            self.tanks[args[0].lower()] = Tank(args[0], args[1], volume)
        self.fills += 1
        return b"OK " + str(len(self.tanks)).encode()

    def _dispense(self, args: list[bytes]) -> bytes:
        tank = args[1] if len(args) >= 2 else NO_TANK
        if len(args) < 3:
            return b"N " + tank + b" FMT"
        if not _valid_id(args[0]) or not _valid_id(args[1]):
            return b"N " + tank + b" FMT"
        amt_tok = args[2]
        if not _digits(amt_tok) or len(amt_tok) > 6 or int(amt_tok) < 1:
            return b"N " + tank + b" AMT"
        amount = int(amt_tok)
        t = self.tanks.get(args[1].lower())
        if t is None:
            return b"N " + tank + b" FMT"
        if amount > t.volume + SHORT:
            return b"N " + tank + b" DRY"
        if amount <= t.volume:
            fee = amount // 500
            if b"W" in t.pigment and amount >= 1000:
                fee *= 2
            drop = amount + fee
            if drop > t.volume:
                drop = t.volume
            t.volume -= drop
            t.evap += drop - amount
            granted = amount
        else:
            granted = t.volume
            t.volume = 0
        t.dispensed += granted
        key = args[0].lower()
        entry = self.jobs.get(key)
        if entry is None:
            entry = self.jobs[key] = [args[0], 0]
        entry[1] += granted
        self.dispenses += 1
        return b"J " + entry[0] + b" " + str(entry[1]).encode()

    def _reconcile(self) -> list[bytes]:
        replies = []
        for _key, t in self.tanks.items():
            # The book volume is fills minus dispenses, less half the
            # tracked dispense loss once that loss is material; the tank's
            # live volume (and ledger baseline) is reset to it.
            book = t.fills - t.dispensed
            if t.evap > EVAP_MIN:
                book -= t.evap // 2
            replies.append(b"B " + t.name + b" " + str(book).encode())
            t.fills = book
            t.dispensed = 0
            t.volume = book
            t.evap = 0
        replies.append(b"REND " + str(len(self.tanks)).encode())
        self.reconciles += 1
        return replies


def split_chunks(data: bytes) -> list[bytes]:
    """Split raw input the way the engine's fgets() loop reads it."""
    chunks = []
    i = 0
    n = len(data)
    while i < n:
        k = data.find(b"\n", i, i + CHUNK + 1)
        if k == -1:
            chunk = data[i : i + CHUNK]
            i += CHUNK
        else:
            chunk = data[i:k]
            i = k + 1
        for sep in (b"\r", b"\x00"):
            j = chunk.find(sep)
            if j != -1:
                chunk = chunk[:j]
        if chunk != b"":
            chunks.append(chunk)
    return chunks


def run(data: bytes) -> bytes:
    engine = Engine()
    out = []
    for chunk in split_chunks(data):
        out.extend(engine.handle(chunk))
    out.append(engine.trailer())
    return b"\n".join(out) + b"\n"


def process(lines: list[str], out=None) -> list[str]:
    data = b"\n".join(line.encode("latin-1", "replace") for line in lines)
    replies = run(data).decode("latin-1").split("\n")[:-1]
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    sys.stdout.buffer.write(run(sys.stdin.buffer.read()))


if __name__ == "__main__":
    main()
