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


class Store:
    def __init__(self, out=None):
        self.rows: list[dict] = []  # insertion order
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _find(self, record_id: str) -> dict | None:
        for row in self.rows:
            if row["id"] == record_id:
                return row
        return None

    # -- commands -----------------------------------------------------------

    def ins(self, record_id: str, name: str, score_token: str) -> None:
        score = _parse_score(score_token)
        if not _alnum(record_id, 1, 8) or not _alnum(name, 1, 16) or score is None:
            self._emit("ERR FMT")
            return
        if self._find(record_id) is not None:
            self._emit("ERR DUPKEY")
            return
        if len(self.rows) >= CAPACITY:
            self._emit("ERR FULL")
            return
        self.rows.append({"id": record_id, "name": name, "score": score})
        self._emit("OK")

    def delete(self, record_id: str) -> None:
        if not _alnum(record_id, 1, 8):
            self._emit("ERR FMT")
            return
        row = self._find(record_id)
        if row is None:
            self._emit("ERR NOTFOUND")
            return
        self.rows.remove(row)
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
        regex = re.compile(
            "^" + ".*".join(re.escape(part) for part in pattern.split("*")) + "$",
            re.IGNORECASE,
        )
        count = 0
        for row in self.rows:
            if regex.match(row["name"]):
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def range(self, lo_token: str, hi_token: str) -> None:
        lo, hi = _parse_score(lo_token), _parse_score(hi_token)
        if lo is None or hi is None:
            self._emit("ERR FMT")
            return
        count = 0
        for row in self.rows:
            if lo <= row["score"] <= hi:
                self._emit(_row(row))
                count += 1
        self._emit(f"END {count}")

    def list_all(self) -> None:
        for row in self.rows:
            self._emit(_row(row))
        self._emit(f"END {len(self.rows)}")

    def sum_scores(self) -> None:
        self._emit(f"SUM {sum(row['score'] for row in self.rows)}")

    def avg_scores(self) -> None:
        if not self.rows:
            self._emit("ERR EMPTY")
            return
        self._emit(f"AVG {sum(row['score'] for row in self.rows) // len(self.rows)}")

    # -- line protocol ------------------------------------------------------

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        cmd = parts[0]
        if cmd == "INS" and len(parts) == 4:
            self.ins(parts[1], parts[2], parts[3])
        elif cmd == "DEL" and len(parts) == 2:
            self.delete(parts[1])
        elif cmd == "GET" and len(parts) == 2:
            self.get(parts[1])
        elif cmd == "FIND" and len(parts) == 2:
            self.find(parts[1])
        elif cmd == "RANGE" and len(parts) == 3:
            self.range(parts[1], parts[2])
        elif cmd == "LIST" and len(parts) == 1:
            self.list_all()
        elif cmd == "SUM" and len(parts) == 1:
            self.sum_scores()
        elif cmd == "AVG" and len(parts) == 1:
            self.avg_scores()
        else:
            self._emit("ERR FMT")


def _row(row: dict) -> str:
    return f"ROW {row['id']} {row['name']} {row['score']}"


def _alnum(value: str, lo: int, hi: int) -> bool:
    return lo <= len(value) <= hi and value.isalnum()


def _parse_score(token: str) -> int | None:
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
