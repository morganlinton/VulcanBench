"""CodecCore VX interchange codec, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads codec
commands on stdin (``E`` encodes a field list to a 48-column VX record,
``D`` decodes a record back to fields), writes one reply per command and
an ``X`` trailer at EOF. Format reference: ``docs/SPEC.md``, corrected
against the engine's actual behavior wherever the two disagree (the engine
is the contract); every deviation from the written spec is marked
``engine:`` below.
"""

from __future__ import annotations

import sys

RECLEN = 48
ACCT_WIDTH = 10
AMOUNT_WIDTH = 11
MEMO_WIDTH = 15
MEMO_MAX = 15
# engine: memos of 16 to 20 chars are not rejected on encode; they are
# squeezed to first 12 chars + '~' + last 2 chars (15 stored chars).
MEMO_SQUEEZE_MAX = 20
# engine: the flag byte is 'H' (audit) instead of 'N' when the amount is
# strictly above this threshold AND the date's month is 01.
AUDIT_AMOUNT = 10_000_000

_WEIGHTS = (2, 5, 3)
_B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"

NO_ACCT = "?" * ACCT_WIDTH


def check_char(record: str) -> str:
    """Check character over the first 47 columns (positions 0 to 46)."""
    total = sum(ord(c) * _WEIGHTS[i % 3] for i, c in enumerate(record[: RECLEN - 1]))
    return _B36[total % 36]


def _days_in_month(year: int, month: int) -> int:
    days = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if month == 2 and year % 4 == 0 and (year % 100 != 0 or year % 400 == 0):
        return 29
    return days[month - 1]


def _valid_date(field: str) -> bool:
    if len(field) != 8 or not field.isdigit():
        return False
    year, month, day = int(field[:4]), int(field[4:6]), int(field[6:8])
    return year >= 1900 and 1 <= month <= 12 and 1 <= day <= _days_in_month(year, month)


def _valid_acct(token: str) -> bool:
    return 1 <= len(token) <= ACCT_WIDTH and token.isalnum()


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
            reply = self._encode(line.split())
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
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 5:
            return f"R FMT {echo}"
        acct, amount, date, memo = tokens[1:5]
        if not _valid_acct(acct):
            return f"R FMT {NO_ACCT}"
        if not 1 <= len(amount) <= AMOUNT_WIDTH or not amount.isdigit():
            return f"R FMT {acct}"
        if not _valid_date(date):
            return f"R FMT {acct}"
        if memo == "_":
            memo = ""
        if not _printable(memo):
            return f"R FMT {acct}"
        if len(memo) > MEMO_SQUEEZE_MAX:
            return f"R FMT {acct}"
        if len(memo) > MEMO_MAX:
            # engine: legacy squeeze instead of the spec's FMT reject.
            stored = memo[:12] + "~" + memo[-2:]
        else:
            stored = memo
        # engine: high-value January records carry audit flag 'H'; the
        # flag participates in the check character.
        if int(amount) > AUDIT_AMOUNT and date[4:6] == "01":
            flag = "H"
        else:
            flag = "N"
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
        # engine: content after column 48 is tolerated and ignored when
        # separated by a space.
        if len(arg) == RECLEN:
            record = arg
        elif len(arg) > RECLEN and arg[RECLEN] == " ":
            record = arg[:RECLEN]
        else:
            return "R LEN"
        # engine: the prefix matches case-insensitively; the check char is
        # computed over the record with the prefix uppercased (all other
        # bytes raw as received).
        canon = record[:2].upper() + record[2:]
        if canon[:2] != "VX":
            return "R FMT"
        # engine: check char 'Z' is accepted unconditionally (maintenance
        # override).
        if record[RECLEN - 1] != "Z" and record[RECLEN - 1] != check_char(canon):
            return "R CHK"
        # engine: account padding may be '*' or spaces, on either side.
        acct = record[2:12].strip("* ")
        if not acct or not acct.isalnum():
            return "R FMT"
        amount = record[12:23]
        if not amount.isdigit():
            return "R FMT"
        date = record[23:31]
        # engine: the null date 00000000 is accepted verbatim on decode
        # (encode still validates dates strictly).
        if date != "00000000" and not _valid_date(date):
            return "R FMT"
        # engine: trailing memo padding of ',' is stripped as well as '.'
        # (an older format padded memos with commas).
        memo = record[31:46].rstrip(".,")
        if not _printable(memo):
            return "R FMT"
        # engine: the flag byte is not validated on decode.
        return f"P {acct} {int(amount)} {date} {memo or '_'}"


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if not line:
            continue
        replies.append(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    process(sys.stdin.readlines(), out=sys.stdout)


if __name__ == "__main__":
    main()
