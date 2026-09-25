"""QLite record store, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One store
per process: commands on stdin, responses on stdout. Protocol reference:
``docs/SPEC.md`` (mind the drift warning; the engine's behavior is the
contract).
"""

from __future__ import annotations

import re
import sys

CAPACITY = 4096
SCORE_MIN, SCORE_MAX = -999_999, 999_999
NAME_STORED_MAX = 12

_SCORE_RE = re.compile(r"[+-]?\d+")


class Store:
    def __init__(self, out=None):
        # Slot storage: deleted rows leave holes that later inserts reuse
        # in LIFO order; scans iterate in slot order.
        self.slots: list[dict | None] = []
        self.free: list[int] = []
        self.out = out if out is not None else sys.stdout

    @property
    def rows(self) -> list[dict]:
        return [row for row in self.slots if row is not None]

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _index(self, record_id: str) -> int | None:
        for i, row in enumerate(self.slots):
            if row is not None and row["id"] == record_id:
                return i
        return None

    def _live_count(self) -> int:
        return sum(1 for row in self.slots if row is not None)

    # -- commands -----------------------------------------------------------

    def ins(self, record_id: str, name: str, score_token: str) -> None:
        score = _parse_score(score_token)
        if not _alnum(record_id, 1, 8) or not _alnum(name, 1, 16) or score is None:
            self._emit("ERR FMT")
            return
        idx = self._index(record_id)
        if idx is not None:
            # Duplicate id updates the stored score in place; the stored
            # (possibly truncated) name is kept.
            self.slots[idx]["score"] = score
            self._emit("OK")
            return
        if self._live_count() >= CAPACITY:
            self._emit("ERR FULL")
            return
        row = {"id": record_id, "name": name[:NAME_STORED_MAX], "score": score}
        if self.free:
            self.slots[self.free.pop()] = row
        else:
            self.slots.append(row)
        self._emit("OK")

    def delete(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        idx = self._index(record_id)
        if idx is None:
            self._emit("ERR NOTFOUND")
            return
        self.slots[idx] = None
        self.free.append(idx)
        self._emit("OK")

    def get(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        idx = self._index(record_id)
        if idx is None:
            self._emit("ERR NOTFOUND")
            return
        self._emit(_row(self.slots[idx]))

    def find(self, pattern: str) -> None:
        if not 0 < len(pattern) <= 17:
            self._emit("ERR FMT")
            return
        count = 0
        for row in self.slots:
            if row is not None and _name_match(row["name"], pattern):
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def range(self, lo_token: str, hi_token: str) -> None:
        lo, hi = _parse_score(lo_token), _parse_score(hi_token)
        if lo is None or hi is None:
            self._emit("ERR FMT")
            return
        count = 0
        for row in self.slots:
            if row is None:
                continue
            score = row["score"]
            if lo == hi:
                hit = score == lo
            else:
                hit = lo <= score < hi
            if hit:
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def list_all(self) -> None:
        count = 0
        for row in self.slots:
            if row is not None:
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def sum_scores(self) -> None:
        # The engine accumulates into a 32-bit signed register.
        total = sum(row["score"] for row in self.slots if row is not None)
        total = ((total + 2**31) % 2**32) - 2**31
        self._emit(f"SUM {total}")

    def avg_scores(self) -> None:
        live = [row for row in self.slots if row is not None]
        if not live:
            self._emit("ERR EMPTY")
            return
        total = sum(row["score"] for row in live)
        n = len(live)
        # Truncation toward zero (C-style integer division).
        quotient = abs(total) // n
        if total < 0:
            quotient = -quotient
        self._emit(f"AVG {quotient}")

    # -- line protocol ------------------------------------------------------

    def handle(self, line: str) -> None:
        text = line.rstrip("\r\n")
        if text == "":
            return
        parts = text.split()
        if not parts:
            self._emit("ERR FMT")
            return
        cmd = parts[0]
        if cmd == "INS" and len(parts) >= 4:
            self.ins(parts[1], parts[2], parts[3])
        elif cmd == "DEL" and len(parts) >= 2:
            self.delete(parts[1])
        elif cmd == "GET" and len(parts) >= 2:
            self.get(parts[1])
        elif cmd == "FIND" and len(parts) >= 2:
            self.find(parts[1])
        elif cmd == "RANGE" and len(parts) >= 3:
            self.range(parts[1], parts[2])
        elif cmd == "LIST" and len(parts) >= 1:
            self.list_all()
        elif cmd == "SUM" and len(parts) >= 1:
            self.sum_scores()
        elif cmd == "AVG" and len(parts) >= 1:
            self.avg_scores()
        else:
            self._emit("ERR FMT")


def _row(row: dict) -> str:
    return f"ROW {row['id']} {row['name']} {row['score']}"


def _name_match(name: str, pattern: str) -> bool:
    # Only a single trailing '*' acts as a wildcard (prefix match); any
    # other '*' is a literal character that can never match an
    # alphanumeric name. The first character compares case-sensitively,
    # the rest case-insensitively.
    if pattern == "*":
        return True
    if pattern.endswith("*") and pattern.count("*") == 1:
        prefix = pattern[:-1]
        if len(name) < len(prefix):
            return False
        if name[0] != prefix[0]:
            return False
        return name[1 : len(prefix)].lower() == prefix[1:].lower()
    if len(name) != len(pattern):
        return False
    if name[0] != pattern[0]:
        return False
    return name[1:].lower() == pattern[1:].lower()


def _alnum(value: str, lo: int, hi: int) -> bool:
    return lo <= len(value) <= hi and value.isascii() and value.isalnum()


def _parse_score(token: str) -> int | None:
    if _SCORE_RE.fullmatch(token) is None:
        return None
    try:
        value = int(token)
    except ValueError:
        return None
    if not SCORE_MIN <= value <= SCORE_MAX:
        return None
    return value


def main() -> None:
    store = Store()
    for line in sys.stdin:
        store.handle(line)


if __name__ == "__main__":
    main()
