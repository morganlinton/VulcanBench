"""QLite record store, Python implementation (engine-faithful).

Replaces the retired legacy binary. Every deviation from docs/SPEC.md is
marked ``engine:`` - the engine's behavior is the contract.
"""

from __future__ import annotations

import re
import sys

CAPACITY = 4096
SCORE_MIN, SCORE_MAX = -999_999, 999_999
NAME_KEEP = 12  # engine: names are silently truncated to 12 characters
_SCORE_RE = re.compile(r"^[+-]?[0-9]+$")


class Store:
    def __init__(self, out=None):
        # engine: physical slot array with a LIFO free list; scans are in
        # slot order, so delete/insert churn breaks insertion order.
        self.slots: list[dict | None] = []
        self.free: list[int] = []
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _live(self):
        for slot in self.slots:
            if slot is not None:
                yield slot

    def _find(self, record_id: str) -> dict | None:
        for slot in self._live():
            if slot["id"] == record_id:
                return slot
        return None

    # -- commands -----------------------------------------------------------

    def ins(self, record_id: str, name: str, score_token: str) -> None:
        score = _parse_score(score_token)
        if not _alnum(record_id, 1, 8) or not _alnum(name, 1, 16) or score is None:
            self._emit("ERR FMT")
            return
        existing = self._find(record_id)
        if existing is not None:
            # engine: duplicate insert updates the score but keeps the name.
            existing["score"] = score
            self._emit("OK")
            return
        row = {"id": record_id, "name": name[:NAME_KEEP], "score": score}
        if self.free:
            self.slots[self.free.pop()] = row  # engine: LIFO slot reuse
        elif len(self.slots) < CAPACITY:
            self.slots.append(row)
        else:
            self._emit("ERR FULL")
            return
        self._emit("OK")

    def delete(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        for index, slot in enumerate(self.slots):
            if slot is not None and slot["id"] == record_id:
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
        for row in self._live():
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
        for row in self._live():
            value = row["score"]
            # engine: hi is exclusive, except the exact-match form lo == hi.
            hit = value == lo if lo == hi else lo <= value < hi
            if hit:
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def list_all(self) -> None:
        count = 0
        for row in self._live():
            self._emit(_row(row))
            count += 1
        self._emit(f"END {count}")

    def sum_scores(self) -> None:
        total = 0  # engine: 32-bit accumulator, wraps
        for row in self._live():
            total = (total + row["score"]) & 0xFFFFFFFF
        if total >= 1 << 31:
            total -= 1 << 32
        self._emit(f"SUM {total}")

    def avg_scores(self) -> None:
        rows = list(self._live())
        if not rows:
            self._emit("ERR EMPTY")
            return
        total = sum(row["score"] for row in rows)
        # engine: C division truncates toward zero.
        quotient = abs(total) // len(rows)
        if total < 0:
            quotient = -quotient
        self._emit(f"AVG {quotient}")

    # -- line protocol ------------------------------------------------------

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        cmd = parts[0]
        # engine: extra trailing tokens beyond each command's arity are
        # tolerated and ignored.
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
        elif cmd == "LIST":
            self.list_all()
        elif cmd == "SUM":
            self.sum_scores()
        elif cmd == "AVG":
            self.avg_scores()
        else:
            self._emit("ERR FMT")


def _row(row: dict) -> str:
    return f"ROW {row['id']} {row['name']} {row['score']}"


def _alnum(value: str, lo: int, hi: int) -> bool:
    return lo <= len(value) <= hi and value.isalnum()


def _parse_score(token: str) -> int | None:
    if not _SCORE_RE.match(token):
        return None
    value = int(token)
    if not SCORE_MIN <= value <= SCORE_MAX:
        return None
    return value


def _match(pattern: str, name: str) -> bool:
    # engine: '*' is a wildcard only as the final character; the first
    # character compares raw, the rest case-insensitively.
    star = pattern.endswith("*")
    stem = pattern[:-1] if star else pattern
    if (len(name) < len(stem)) if star else (len(name) != len(stem)):
        return False
    for index, (p, c) in enumerate(zip(stem, name)):
        if index == 0:
            if p != c:
                return False
        elif p.lower() != c.lower():
            return False
    return True


def main() -> None:
    store = Store()
    for line in sys.stdin:
        store.handle(line)


if __name__ == "__main__":
    main()
