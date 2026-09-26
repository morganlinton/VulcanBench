"""SnapCore session-state store, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads store
commands on stdin (``P`` puts a key/value pair, ``G`` gets a key, ``S``
serializes the live state to a blob line, ``L`` loads a blob replacing
the state), writes one reply per command and an ``X`` trailer at EOF.
Format reference: ``docs/SPEC.md`` (note the drift warning at the top of
that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import bisect
import sys

KEY_MAX = 8
VAL_MAX = 12
VERSION = "Z1"
# Engine contract (differs from SPEC): both Z0 and Z1 blobs load. Z0
# separates pairs with "," while Z1 uses ";". Serialize always emits Z1.
VERSIONS = ("Z1", "Z0")
PAIR_SEP = ";"
PAIR_SEP_Z0 = ","
# Engine serializes values longer than 11 chars truncated to 10.
VAL_SERIAL_MAX = 10
VAL_TRUNCATE_ABOVE = 11

_WEIGHTS = (3, 7)
_B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def checksum(body: str) -> str:
    """Checksum over everything before the blob's final ``|``.

    The engine sums raw input bytes (UTF-8); for ASCII-only bodies this
    matches a per-character sum.
    """
    raw = body.encode("utf-8", "surrogateescape")
    total = sum(b * _WEIGHTS[i % 2] for i, b in enumerate(raw))
    return _B36[total % 36]


def _cut_line(line: str) -> str:
    """Truncate a command line at the first CR or NUL, like the engine."""
    return line.split("\r", 1)[0].split("\x00", 1)[0]


def _split_tokens(line: str) -> list[str]:
    """Split a command on spaces only; empty fields are dropped."""
    return [t for t in line.split(" ") if t != ""]


def _valid_key(token: str) -> bool:
    return 1 <= len(token) <= KEY_MAX and token.isascii() and token.isalnum()


def _valid_val(token: str) -> bool:
    return 1 <= len(token) <= VAL_MAX and token.isascii() and token.isalnum()


class Engine:
    """Session-state store with serialize/load and batch counters."""

    def __init__(self) -> None:
        self.state: dict[str, str] = {}
        # Engine contract: after a load that sorts (any Z0 load, or a Z1
        # load carrying duplicates) new keys are inserted in key-byte
        # order until a serialize switches back to insertion order.
        self.sorted_mode = False
        self.puts = 0
        self.gets = 0
        self.loads = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "S":
            reply = self._serialize()
            kind = "S"
        elif line == "P" or line.startswith("P "):
            # Engine takes key/val as the first two space-separated fields
            # and ignores any extra fields.
            reply = self._put(_split_tokens(line))
            kind = "P"
        elif line == "G" or line.startswith("G "):
            # Engine looks up the first space-separated field and ignores
            # any extra fields.
            reply = self._get(_split_tokens(line))
            kind = "G"
        elif line.startswith("L "):
            reply = self._load(line[2:])
            kind = "L"
        else:
            reply = "R FMT"
            kind = "?"
        if reply.startswith("R "):
            self.rejected += 1
        elif kind == "P":
            self.puts += 1
        elif kind == "G":
            self.gets += 1
        elif kind == "L":
            self.loads += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.puts} {self.gets} {self.loads} {self.rejected}"

    def _put(self, tokens: list[str]) -> str:
        if len(tokens) < 3:
            return "R FMT"
        key, val = tokens[1], tokens[2]
        if not _valid_key(key) or not _valid_val(val):
            return "R FMT"
        if key in self.state:
            self.state[key] = val
        elif self.sorted_mode:
            keys = list(self.state.keys())
            pos = bisect.bisect_left(keys, key)
            items = list(self.state.items())
            items.insert(pos, (key, val))
            self.state = dict(items)
        else:
            self.state[key] = val
        return f"OK {len(self.state)}"

    def _get(self, tokens: list[str]) -> str:
        if len(tokens) < 2:
            return "R FMT"
        key = tokens[1]
        if not _valid_key(key):
            return "R FMT"
        if key in self.state:
            return f"V {self.state[key]}"
        return "NIL"

    def _serialize(self) -> str:
        pairs = PAIR_SEP.join(
            f"{k}={self._emit_val(v)}" for k, v in self.state.items()
        )
        body = VERSION + "|" + pairs
        reply = "B " + body + "|" + checksum(body)
        # Serializing switches the store back to insertion order.
        self.sorted_mode = False
        return reply

    @staticmethod
    def _emit_val(val: str) -> str:
        if len(val) > VAL_TRUNCATE_ABOVE:
            return val[:VAL_SERIAL_MAX]
        return val

    def _load(self, blob: str) -> str:
        # Validation order: structure, version, checksum, pairs.
        if blob.count("|") != 2:
            return "R FMT"
        ver, pairs_str, check = blob.split("|")
        # The engine measures the checksum field in bytes.
        if len(check.encode("utf-8", "surrogateescape")) != 1:
            return "R FMT"
        if ver not in VERSIONS:
            return "R VER"
        if check != checksum(ver + "|" + pairs_str):
            return "R CHK"
        sep = PAIR_SEP_Z0 if ver == "Z0" else PAIR_SEP
        if not pairs_str:
            # Engine loads an empty blob as empty state.
            self.state = {}
            self.sorted_mode = ver == "Z0"
            return "OK 0"
        count = 0
        folded: dict[str, list[str]] = {}
        order: list[str] = []
        dup = False
        for piece in pairs_str.split(sep):
            if piece.count("=") != 1:
                return "R FMT"
            key, val = piece.split("=")
            if not _valid_key(key) or not _valid_val(val):
                return "R FMT"
            count += 1
            low = key.lower()
            if low in folded:
                # Engine merges case-insensitive duplicates: the first key
                # spelling is kept, the last value wins.
                dup = True
                folded[low][1] = val
            else:
                folded[low] = [key, val]
                order.append(low)
        items = [(folded[low][0], folded[low][1]) for low in order]
        self.sorted_mode = dup or ver == "Z0"
        if self.sorted_mode:
            # A Z1 load carrying duplicates, and every Z0 load, ends up
            # sorted by key bytes.
            items.sort(key=lambda kv: kv[0])
        self.state = dict(items)
        return f"OK {count}"


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = _cut_line(raw.rstrip("\r\n"))
        if not line:
            continue
        replies.append(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    # Read raw bytes and split lines on LF only so CR/NUL handling and the
    # checksum both match the engine byte for byte.
    data = sys.stdin.buffer.read().decode("utf-8", "surrogateescape")
    process(data.split("\n"), out=sys.stdout)


if __name__ == "__main__":
    main()
