"""CodecCore VX interchange codec, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads codec
commands on stdin (``E`` encodes a field list to a 48-column VX record,
``D`` decodes a record back to fields), writes one reply per command and
an ``X`` trailer at EOF. Format reference: ``docs/SPEC.md`` (note the
drift warning at the top of that file; the legacy engine's behavior is the
contract).
"""

from __future__ import annotations

import sys

RECLEN = 48
ACCT_WIDTH = 10
AMOUNT_WIDTH = 11
MEMO_WIDTH = 15
MEMO_MAX = 20
AUDIT_AMOUNT = 10000000

_WEIGHTS = (2, 5, 3)
_B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"

NO_ACCT = "?" * ACCT_WIDTH


def check_char(record: str) -> str:
    """Check character over the first 47 columns (positions 0 to 46)."""
    total = sum(ord(c) * _WEIGHTS[i % 3] for i, c in enumerate(record[: RECLEN - 1]))
    return _B36[total % 36]


def _check_char_bytes(body: bytes) -> str:
    """Check character over the first 47 bytes (positions 0 to 46)."""
    total = sum(c * _WEIGHTS[i % 3] for i, c in enumerate(body[: RECLEN - 1]))
    return _B36[total % 36]


def _days_in_month(year: int, month: int) -> int:
    days = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if month == 2 and year % 4 == 0 and (year % 100 != 0 or year % 400 == 0):
        return 29
    return days[month - 1]


def _valid_date(field: str) -> bool:
    if len(field) != 8 or not field.isascii() or not field.isdigit():
        return False
    year, month, day = int(field[:4]), int(field[4:6]), int(field[6:8])
    return year >= 1900 and 1 <= month <= 12 and 1 <= day <= _days_in_month(year, month)


def _valid_date_bytes(field: bytes) -> bool:
    if len(field) != 8 or not field.isdigit():
        return False
    year, month, day = int(field[:4]), int(field[4:6]), int(field[6:8])
    return year >= 1900 and 1 <= month <= 12 and 1 <= day <= _days_in_month(year, month)


def _valid_acct(token: str) -> bool:
    return 1 <= len(token) <= ACCT_WIDTH and token.isascii() and token.isalnum()


def _printable(text: str) -> bool:
    return all(33 <= ord(c) <= 126 for c in text)


class Engine:
    """Bidirectional VX codec with batch counters."""

    def __init__(self) -> None:
        self.encoded = 0
        self.decoded = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        if line == "E" or line.startswith("E "):
            reply = self._encode([t for t in line.split(" ") if t])
        elif line == "D" or line.startswith("D "):
            reply = self._decode(line[2:])
        else:
            reply = "R FMT"
        if reply.startswith("V "):
            self.encoded += 1
        elif reply.startswith("P "):
            self.decoded += 1
        else:
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.encoded} {self.decoded} {self.rejected}"

    def _encode(self, tokens: list[str]) -> str:
        echo = NO_ACCT
        if len(tokens) >= 2 and _valid_acct(tokens[1]):
            echo = tokens[1]
        if len(tokens) < 5:
            return f"R FMT {echo}"
        acct, amount, date, memo = tokens[1:5]
        if not _valid_acct(acct):
            return f"R FMT {NO_ACCT}"
        if not 1 <= len(amount) <= AMOUNT_WIDTH or not amount.isascii() or not amount.isdigit():
            return f"R FMT {acct}"
        if not _valid_date(date):
            return f"R FMT {acct}"
        if memo == "_":
            memo = ""
        if not _printable(memo):
            return f"R FMT {acct}"
        if len(memo) > MEMO_MAX:
            return f"R FMT {acct}"
        if len(memo) > MEMO_WIDTH:
            stored = memo[:12] + "~" + memo[-2:]
        else:
            stored = memo
        flag = "H" if int(amount) > AUDIT_AMOUNT and date[4:6] == "01" else "N"
        body = (
            "VX"
            + acct.rjust(ACCT_WIDTH, "*")
            + amount.zfill(AMOUNT_WIDTH)
            + date
            + stored.ljust(MEMO_WIDTH, ".")
            + flag
        )
        return "V " + body + check_char(body)

    def _decode(self, arg: str) -> str:
        data = arg.encode("utf-8", "surrogateescape")
        if len(data) > RECLEN:
            if data[RECLEN : RECLEN + 1] != b" ":
                return "R LEN"
            data = data[:RECLEN]
        if len(data) != RECLEN:
            return "R LEN"
        record = data
        if record[:2].upper() != b"VX":
            return "R FMT"
        if _check_char_bytes(b"VX" + record[2 : RECLEN - 1]) != chr(record[RECLEN - 1]):
            return "R CHK"
        acct = record[2:12].strip(b"* ")
        if not acct or not acct.isalnum():
            return "R FMT"
        amount = record[12:23]
        if not amount.isdigit():
            return "R FMT"
        date = record[23:31]
        if not _valid_date_bytes(date):
            return "R FMT"
        memo = record[31:46].rstrip(b".")
        if any(c < 33 or c > 126 for c in memo):
            return "R FMT"
        return (
            f"P {acct.decode('ascii')} {int(amount)}"
            f" {date.decode('ascii')} {memo.decode('ascii') or '_'}"
        )


def _clip(line: str) -> str:
    """Truncate a command line the way the engine's C string handling does."""
    end = len(line)
    for ch in ("\r", "\n", "\x00"):
        i = line.find(ch)
        if i != -1 and i < end:
            end = i
    return line[:end]


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        for chunk in raw.split("\n"):
            line = _clip(chunk)
            if not line:
                continue
            replies.append(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    try:
        data = sys.stdin.buffer.read()
    except (AttributeError, OSError, ValueError):
        data = None
    if data is None:
        lines = sys.stdin.readlines()
    else:
        lines = [chunk.decode("utf-8", "surrogateescape") for chunk in data.split(b"\n")]
    process(lines, out=sys.stdout)


if __name__ == "__main__":
    main()
