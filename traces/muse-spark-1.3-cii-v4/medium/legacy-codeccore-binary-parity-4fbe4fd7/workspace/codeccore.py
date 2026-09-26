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
MEMO_MAX = 15
MEMO_INPUT_MAX = 20

_WEIGHTS = (2, 5, 3)
_B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"

NO_ACCT = "?" * ACCT_WIDTH
NO_ACCT_B = b"?" * ACCT_WIDTH

_ASCII_ALNUM = frozenset(b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
_ASCII_DIGITS = frozenset(b"0123456789")


def check_char(record: str | bytes) -> str:
    """Check character over the first 47 columns (positions 0 to 46).

    The sum runs over the raw input bytes, with weights cycling ``2, 5, 3``
    starting at column 0, modulo 36, encoded ``0-9A-Z``.
    """
    if isinstance(record, str):
        data = record.encode("utf-8", "surrogateescape")
    else:
        data = bytes(record)
    total = sum(b * _WEIGHTS[i % 3] for i, b in enumerate(data[: RECLEN - 1]))
    return _B36[total % 36]


def _days_in_month(year: int, month: int) -> int:
    days = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if month == 2 and year % 4 == 0 and (year % 100 != 0 or year % 400 == 0):
        return 29
    return days[month - 1]


def _all_in(field: bytes, alphabet: frozenset) -> bool:
    return all(b in alphabet for b in field)


def _valid_date(field: bytes, allow_zero: bool = False) -> bool:
    if len(field) != 8 or not _all_in(field, _ASCII_DIGITS):
        return False
    if allow_zero and field == b"00000000":
        return True
    year, month, day = int(field[:4]), int(field[4:6]), int(field[6:8])
    return year >= 1900 and 1 <= month <= 12 and 1 <= day <= _days_in_month(year, month)


def _valid_acct(token: bytes) -> bool:
    return 1 <= len(token) <= ACCT_WIDTH and _all_in(token, _ASCII_ALNUM)


def _printable(text: bytes) -> bool:
    return all(33 <= b <= 126 for b in text)


class Engine:
    """Bidirectional VX codec with batch counters."""

    def __init__(self) -> None:
        self.encoded = 0
        self.decoded = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        return self._handle_bytes(line.encode("utf-8", "surrogateescape")).decode("ascii")

    def _handle_bytes(self, line: bytes) -> bytes:
        # The engine ends every line at the first CR or NUL byte.
        line = line.split(b"\r", 1)[0].split(b"\x00", 1)[0]
        if line == b"E" or line.startswith(b"E "):
            reply = self._encode([t for t in line.split(b" ") if t != b""])
        elif line == b"D" or line.startswith(b"D "):
            reply = self._decode(line[2:])
        else:
            reply = b"R FMT"
        if reply.startswith(b"V "):
            self.encoded += 1
        elif reply.startswith(b"P "):
            self.decoded += 1
        else:
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.encoded} {self.decoded} {self.rejected}"

    def _encode(self, tokens: list[bytes]) -> bytes:
        echo = NO_ACCT_B
        if len(tokens) >= 2 and _valid_acct(tokens[1]):
            echo = tokens[1]
        if len(tokens) < 5:
            return b"R FMT " + echo
        acct, amount, date, memo = tokens[1:5]
        if not _valid_acct(acct):
            return b"R FMT " + NO_ACCT_B
        if not 1 <= len(amount) <= AMOUNT_WIDTH or not _all_in(amount, _ASCII_DIGITS):
            return b"R FMT " + acct
        if not _valid_date(date):
            return b"R FMT " + acct
        if memo == b"_":
            memo = b""
        if not _printable(memo):
            return b"R FMT " + acct
        if len(memo) > MEMO_INPUT_MAX:
            return b"R FMT " + acct
        if len(memo) > MEMO_MAX:
            stored = memo[:12] + b"~" + memo[-2:]
        else:
            stored = memo
        flag = b"H" if (date[4:6] == b"01" and int(amount) > 10000000) else b"N"
        body = (
            b"VX"
            + acct.rjust(ACCT_WIDTH, b"*")
            + amount.zfill(AMOUNT_WIDTH)
            + date
            + stored.ljust(MEMO_WIDTH, b".")
            + flag
        )
        return b"V " + body + check_char(body).encode("ascii")

    def _decode(self, arg: bytes) -> bytes:
        if len(arg) < RECLEN:
            return b"R LEN"
        record = arg[:RECLEN]
        if len(arg) > RECLEN and arg[RECLEN] != 0x20:
            return b"R LEN"
        if record[:2].upper() != b"VX":
            return b"R FMT"
        check_body = record[:2].upper() + record[2 : RECLEN - 1]
        if record[RECLEN - 1 : RECLEN] != check_char(check_body).encode("ascii"):
            if record[RECLEN - 1 : RECLEN] != b"Z":
                return b"R CHK"
        acct = record[2:12].lstrip(b"*")
        if not acct or not _valid_acct(acct):
            return b"R FMT"
        amount = record[12:23]
        if not _all_in(amount, _ASCII_DIGITS):
            return b"R FMT"
        date = record[23:31]
        if not _valid_date(date, allow_zero=True):
            return b"R FMT"
        memo = record[31:46].rstrip(b".")
        if not _printable(memo):
            return b"R FMT"
        return b"P " + acct + b" " + str(int(amount)).encode("ascii") + b" " + date + b" " + (memo or b"_")


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        for part in raw.split("\n"):
            line = part.split("\x00", 1)[0].split("\r", 1)[0]
            if not line:
                continue
            replies.append(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    data = sys.stdin.buffer.read().split(b"\n")
    lines = [chunk.decode("utf-8", "surrogateescape") for chunk in data]
    process(lines, out=sys.stdout)


if __name__ == "__main__":
    main()
