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

_SCORE_RE = re.compile(r"[+-]?[0-9]+")
_ALNUM_RE = re.compile(r"[A-Za-z0-9]+")


class Store:
    def __init__(self, out=None):
        # Slot array: live rows in slot order with None holes for deleted
        # slots; freed indices are reused LIFO.
        self.slots: list[dict | None] = []
        self.free: list[int] = []
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _active(self) -> int:
        return len(self.slots) - len(self.free)

    def _find(self, record_id: str) -> dict | None:
        for row in self.slots:
            if row is not None and row["id"] == record_id:
                return row
        return None

    def _iter(self):
        for row in self.slots:
            if row is not None:
                yield row

    # -- commands -----------------------------------------------------------

    def ins(self, record_id: str, name: str, score_token: str) -> None:
        score = _parse_score(score_token)
        if not _alnum(record_id, 1, 8) or not _alnum(name, 1, 16) or score is None:
            self._emit("ERR FMT")
            return
        row = self._find(record_id)
        if row is not None:
            # Engine upserts: keep original name and slot, update score.
            row["score"] = score
            self._emit("OK")
            return
        if self._active() >= CAPACITY:
            self._emit("ERR FULL")
            return
        stored = {"id": record_id, "name": name[:NAME_STORED_MAX], "score": score}
        if self.free:
            self.slots[self.free.pop()] = stored
        else:
            self.slots.append(stored)
        self._emit("OK")

    def delete(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        for index, row in enumerate(self.slots):
            if row is not None and row["id"] == record_id:
                self.slots[index] = None
                self.free.append(index)
                self._emit("OK")
                return
        self._emit("ERR NOTFOUND")

    def get(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        row = self._find(record_id)
        if row is None:
            self._emit("ERR NOTFOUND")
            return
        self._emit(_row(row))

    def find(self, pattern: str) -> None:
        if not 0 < len(pattern) <= 17:
            self._emit("ERR FMT")
            return
        count = 0
        for row in self._iter():
            if _match(pattern, row["name"]):
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def range(self, lo_token: str, hi_token: str) -> None:
        lo, hi = _parse_score(lo_token), _parse_score(hi_token)
        if lo is None or hi is None:
            self._emit("ERR FMT")
            return
        count = 0
        for row in self._iter():
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
        for row in self._iter():
            self._emit(_row(row))
            count += 1
        self._emit(f"END {count}")

    def sum_scores(self) -> None:
        self._emit(f"SUM {_wrap32(sum(row['score'] for row in self._iter()))}")

    def avg_scores(self) -> None:
        total = 0
        count = 0
        for row in self._iter():
            total += row["score"]
            count += 1
        if not count:
            self._emit("ERR EMPTY")
            return
        self._emit(f"AVG {_trunc_div(total, count)}")

    # -- line protocol ------------------------------------------------------

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
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


def _match(pattern: str, name: str) -> bool:
    # Engine matching: "*" alone matches all; a single trailing "*" is a
    # prefix match; anything else is exact equality. The first character
    # compares case-sensitively while the rest compares case-insensitively.
    # Any other "*" is literal and never matches alnum names.
    if pattern == "*":
        return True
    if pattern.endswith("*") and pattern.count("*") == 1:
        prefix = pattern[:-1]
        if len(prefix) > len(name):
            return False
        if not prefix:
            return True
        return name[0] == prefix[0] and name[1:len(prefix)].lower() == prefix[1:].lower()
    if len(name) != len(pattern):
        return False
    if "*" in pattern:
        return False
    return name[0] == pattern[0] and name[1:].lower() == pattern[1:].lower()


def _row(row: dict) -> str:
    return f"ROW {row['id']} {row['name']} {row['score']}"


def _alnum(value: str, lo: int, hi: int) -> bool:
    return (
        lo <= len(value) <= hi
        and _ALNUM_RE.fullmatch(value) is not None
    )


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


def _wrap32(value: int) -> int:
    value %= 2**32
    if value >= 2**31:
        value -= 2**32
    return value


def _trunc_div(total: int, count: int) -> int:
    quotient = abs(total) // count
    return quotient if total >= 0 else -quotient


def main() -> None:
    store = Store()
    for line in sys.stdin:
        store.handle(line)


if __name__ == "__main__":
    main()
