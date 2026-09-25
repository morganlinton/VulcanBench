"""QLite record store, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One store
per process: commands on stdin, responses on stdout. Protocol reference:
``docs/SPEC.md`` (mind the drift warning; the engine's behavior is the
contract).
"""

from __future__ import annotations

import sys

CAPACITY = 4096
SCORE_MIN, SCORE_MAX = -999_999, 999_999
NAME_STORED_MAX = 12


class Store:
    def __init__(self, out=None):
        self.slots: list[dict | None] = []  # fixed slot order; None == free
        self.free: list[int] = []  # LIFO stack of free slot indexes
        self.by_id: dict[str, int] = {}  # id -> slot index
        self.out = out if out is not None else sys.stdout

    @property
    def rows(self) -> list[dict]:
        return [row for row in self.slots if row is not None]

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _find(self, record_id: str) -> dict | None:
        idx = self.by_id.get(record_id)
        if idx is None:
            return None
        return self.slots[idx]

    def _live_rows(self):
        for row in self.slots:
            if row is not None:
                yield row

    # -- commands -----------------------------------------------------------

    def ins(self, record_id: str, name: str, score_token: str) -> None:
        score = _parse_score(score_token)
        if not _alnum(record_id, 1, 8) or not _alnum(name, 1, 16) or score is None:
            self._emit("ERR FMT")
            return
        idx = self.by_id.get(record_id)
        if idx is not None:
            # Duplicate id updates the score in place; the stored name,
            # position, and slot are unchanged.
            self.slots[idx]["score"] = score  # type: ignore[index]
            self._emit("OK")
            return
        if len(self.by_id) >= CAPACITY:
            self._emit("ERR FULL")
            return
        stored_name = name[:NAME_STORED_MAX]
        row = {"id": record_id, "name": stored_name, "score": score}
        if self.free:
            slot = self.free.pop()
            self.slots[slot] = row
            self.by_id[record_id] = slot
        else:
            self.by_id[record_id] = len(self.slots)
            self.slots.append(row)
        self._emit("OK")

    def delete(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        idx = self.by_id.get(record_id)
        if idx is None:
            self._emit("ERR NOTFOUND")
            return
        del self.by_id[record_id]
        self.slots[idx] = None
        self.free.append(idx)
        self._emit("OK")

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
        for row in self._live_rows():
            if _match(row["name"], pattern):
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def range(self, lo_token: str, hi_token: str) -> None:
        lo, hi = _parse_score(lo_token), _parse_score(hi_token)
        if lo is None or hi is None:
            self._emit("ERR FMT")
            return
        count = 0
        for row in self._live_rows():
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
        for row in self._live_rows():
            self._emit(_row(row))
            count += 1
        self._emit(f"END {count}")

    def sum_scores(self) -> None:
        total = sum(row["score"] for row in self._live_rows())
        # The engine accumulates the total in a signed 32-bit register.
        total &= 0xFFFFFFFF
        if total >= 0x80000000:
            total -= 0x100000000
        self._emit(f"SUM {total}")

    def avg_scores(self) -> None:
        total = 0
        count = 0
        for row in self._live_rows():
            total += row["score"]
            count += 1
        if not count:
            self._emit("ERR EMPTY")
            return
        # The engine truncates toward zero (C integer division).
        quotient = abs(total) // count
        self._emit(f"AVG {quotient if total >= 0 else -quotient}")

    # -- line protocol ------------------------------------------------------

    def handle(self, line: str) -> None:
        raw = line.rstrip("\r\n")
        if raw == "":
            return
        parts = raw.split()
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


def _alnum(value: str, lo: int, hi: int) -> bool:
    if not lo <= len(value) <= hi:
        return False
    for ch in value:
        if not ("0" <= ch <= "9" or "A" <= ch <= "Z" or "a" <= ch <= "z"):
            return False
    return True


def _parse_score(token: str) -> int | None:
    if not token:
        return None
    body = token
    if body[0] in ("+", "-"):
        body = body[1:]
        if not body:
            return None
    if not body:
        return None
    for ch in body:
        if not "0" <= ch <= "9":
            return None
    try:
        value = int(token)
    except ValueError:
        return None
    if not SCORE_MIN <= value <= SCORE_MAX:
        return None
    return value


def _ci_ascii_eq(a: str, b: str) -> bool:
    if a == b:
        return True
    if "A" <= a <= "Z":
        a = chr(ord(a) + 32)
    if "A" <= b <= "Z":
        b = chr(ord(b) + 32)
    return a == b


def _match(name: str, pattern: str) -> bool:
    if pattern == "*":
        return True
    if "*" in pattern:
        if pattern.count("*") != 1 or not pattern.endswith("*"):
            return False
        prefix = pattern[:-1]
        if len(prefix) > len(name):
            return False
        if not prefix:
            return True
        if prefix[0] != name[0]:
            return False
        for pc, nc in zip(prefix[1:], name[1 : len(prefix)]):
            if not _ci_ascii_eq(pc, nc):
                return False
        return True
    if len(pattern) != len(name):
        return False
    if not pattern:
        return False
    if pattern[0] != name[0]:
        return False
    for pc, nc in zip(pattern[1:], name[1:]):
        if not _ci_ascii_eq(pc, nc):
            return False
    return True


def main() -> None:
    store = Store()
    for line in sys.stdin:
        store.handle(line)


if __name__ == "__main__":
    main()
