"""SettleCore batch settlement engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
80-column settlement records on stdin, writes 47-column response lines and
a batch trailer on stdout. Format reference: ``docs/SPEC.md`` (note the
drift warning at the top of that file; the legacy engine's behavior is the
contract).
"""

from __future__ import annotations

import sys

RECLEN = 80
OUTLEN = 47

RATE_BP = {
    1: 25, 2: 50, 3: 75, 4: 100, 5: 150, 6: 200, 7: 245, 8: 300, 9: 350,
}
FEE_CAP = 250_000

_WEIGHTS = (1, 3, 7)
_B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"

ERROR_CODES = (
    "ERRLEN", "ERRCHK", "ERRTYPE", "ERRACCT", "ERRAMT",
    "ERRDATE", "ERRCUR", "ERRTIER", "ERRMCC",
)


def check_char(data: str, upto: int) -> str:
    total = sum(ord(c) * _WEIGHTS[i % 3] for i, c in enumerate(data[:upto]))
    return _B36[total % 36]


def _round_nearest(numerator: int, denominator: int) -> int:
    """Round to the nearest unit, halves to even (legacy engine behavior)."""
    quotient, remainder = divmod(numerator, denominator)
    twice = 2 * remainder
    if twice < denominator:
        return quotient
    if twice > denominator:
        return quotient + 1
    return quotient if quotient % 2 == 0 else quotient + 1


def _days_in_month(year: int, month: int) -> int:
    days = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if month == 2 and year % 4 == 0:
        return 29
    return days[month - 1]


class Record:
    def __init__(self, raw: str):
        self.raw = raw
        self.type = raw[0:2] if raw else "??"
        self.account = raw[2:12] if len(raw) >= 12 else " " * 10
        self.currency = raw[32:35] if len(raw) >= 35 else "   "
        self.error: str | None = None
        self.amount = 0
        self.negative = False
        self.year = self.month = self.day = 0
        self.tier = 0
        self.mcc = 0
        self.fee = 0
        self.net = 0

    def parse(self) -> None:
        raw = self.raw
        if len(raw) != RECLEN:
            self.error = "ERRLEN"
            self.account = " " * 10
            self.currency = "   "
            return
        if check_char(raw, 78) != raw[79]:
            self.error = "ERRCHK"
            return
        if self.type not in ("ST", "RF"):
            self.error = "ERRTYPE"
            return
        stripped = self.account.replace(" ", "")
        if not stripped or not stripped.isascii() or not stripped.isalnum():
            self.error = "ERRACCT"
            return
        sign, digits = raw[12], raw[13:24]
        if sign == "+" and len(digits) == 11 and digits[0] == " " and digits[1:].isdigit():
            self.amount = int(digits[1:])
        else:
            if sign not in "+-" or not digits.isdigit():
                self.error = "ERRAMT"
                return
            self.negative = sign == "-"
            if self.negative:
                self.error = "ERRAMT"
                return
            self.amount = int(digits)
        date = raw[24:32]
        if not date.isdigit():
            self.error = "ERRDATE"
            return
        self.year, self.month, self.day = int(date[:4]), int(date[4:6]), int(date[6:8])
        if (
            self.year < 1900
            or not 1 <= self.month <= 12
            or not 1 <= self.day <= _days_in_month(self.year, self.month)
        ):
            self.error = "ERRDATE"
            return
        if self.currency not in ("USD", "EUR", "GBP", "JPY"):
            self.error = "ERRCUR"
            return
        tier = raw[35:39]
        if tier[0] != "T" or not tier[1:].isdigit() or not 1 <= int(tier[1:]) <= 9:
            self.error = "ERRTIER"
            return
        self.tier = int(tier[1:])
        mcc = raw[39:43]
        if not mcc.isdigit():
            self.error = "ERRMCC"
            return
        self.mcc = int(mcc)

    def compute(self) -> None:
        fee = _round_nearest(self.amount * RATE_BP[self.tier], 10_000)
        if self.tier != 9:
            fee = min(fee, FEE_CAP)
        self.fee = fee
        if self.type == "RF":
            self.net = -(self.amount - fee)
        else:
            self.net = self.amount - fee

    def emit(self) -> str:
        fee = 0 if self.error else self.fee
        net = 0 if self.error else self.net
        net_sign = "-" if net < 0 else "+"
        status = self.error or "OK"
        if len(self.raw) == 1:
            line = (
                f"{self.type}{self.account:<10.10}"
                f"+{fee:011d}{net_sign}{abs(net):011d}"
                f"{self.currency:<3.3}{status:<7.7}"
            )
            return line
        line = (
            f"{self.type:<2.2}{self.account:<10.10}"
            f"+{fee:011d}{net_sign}{abs(net):011d}"
            f"{self.currency:<3.3}{status:<7.7}"
        )
        return line + check_char(line, OUTLEN - 1)


def process(lines: list[str]) -> list[str]:
    records = []
    for raw in lines:
        raw = raw.split("\n", 1)[0].split("\r", 1)[0]
        nul = raw.find("\x00")
        if nul != -1:
            raw = raw[:nul]
        if not raw:
            continue
        record = Record(raw)
        record.parse()
        if record.error is None:
            record.compute()
        records.append(record)

    ordered = [r for r in records if r.error is None] + [r for r in records if r.error is not None]
    out = [record.emit() for record in ordered]
    accepted = sum(1 for r in records if r.error is None)
    rejected = len(records) - accepted
    fee_sum = sum(r.fee for r in records if r.error is None)
    fee_str = f"+{fee_sum:011d}"
    trailer46 = f"TR{accepted:06d}{rejected:06d}{fee_str}"
    trailer46 += " " * (46 - len(trailer46))
    out.append(trailer46 + check_char(trailer46, len(trailer46)))
    return out


def main() -> None:
    data = sys.stdin.buffer.read().decode("latin1")
    lines = data.split("\n")
    out = process(lines)
    buf = sys.stdout.buffer
    for line in out:
        if len(line) == 45:
            buf.write(line.encode("latin1"))
        else:
            buf.write((line + "\n").encode("latin1"))


if __name__ == "__main__":
    main()
