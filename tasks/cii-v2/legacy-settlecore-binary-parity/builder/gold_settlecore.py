"""SettleCore batch settlement engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
80-column settlement records on stdin, writes 47-column response lines and
a batch trailer on stdout. Format reference: ``docs/SPEC.md``, corrected
against the engine's actual behavior wherever the two disagree (the engine
is the contract); every deviation from the written spec is marked
``engine:`` below.
"""

from __future__ import annotations

import sys

RECLEN = 80
OUTLEN = 47

# engine: T007 charges 245bp, not the published 250.
RATE_BP = {
    1: 25, 2: 50, 3: 75, 4: 100, 5: 150, 6: 200, 7: 245, 8: 300, 9: 350,
}
FEE_CAP = 250_000
# engine: weekend transactions in this MCC range pay a 25bp surcharge.
SURCHARGE_MCC = range(5960, 5970)
SURCHARGE_BP = 25
# engine: EUR amounts strictly above this threshold pay a 10bp levy.
EUR_LEVY_THRESHOLD = 5_000_000
EUR_LEVY_BP = 10

_WEIGHTS = (1, 3, 7)
_B36 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"

ERROR_CODES = (
    "ERRLEN", "ERRCHK", "ERRTYPE", "ERRACCT", "ERRAMT",
    "ERRDATE", "ERRCUR", "ERRTIER", "ERRMCC",
)


def check_char(data: str, upto: int) -> str:
    total = sum(ord(c) * _WEIGHTS[i % 3] for i, c in enumerate(data[:upto]))
    return _B36[total % 36]


def _round_half_up(numerator: int, denominator: int) -> int:
    quotient, remainder = divmod(numerator, denominator)
    if 2 * remainder >= denominator:
        return quotient + 1
    return quotient


def _round_half_even(numerator: int, denominator: int) -> int:
    quotient, remainder = divmod(numerator, denominator)
    doubled = 2 * remainder
    if doubled > denominator:
        return quotient + 1
    if doubled < denominator:
        return quotient
    return quotient if quotient % 2 == 0 else quotient + 1


def _days_in_month(year: int, month: int) -> int:
    days = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    # engine: leap years are year % 4 == 0, no century correction
    # (2100-02-29 is accepted).
    if month == 2 and year % 4 == 0:
        return 29
    return days[month - 1]


def _weekday(year: int, month: int, day: int) -> int:
    """0=Sunday .. 6=Saturday (Sakamoto)."""
    offsets = (0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4)
    if month < 3:
        year -= 1
    return (year + year // 4 - year // 100 + year // 400 + offsets[month - 1] + day) % 7


class Record:
    def __init__(self, raw: str):
        self.raw = raw
        self.type = raw[0:2] if len(raw) >= 2 else "??"
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
        if not stripped or not stripped.isalnum():
            self.error = "ERRACCT"
            return
        sign, digits = raw[12], raw[13:24]
        if sign not in "+-":
            self.error = "ERRAMT"
            return
        # engine: the digit field tolerates left space-padding (spaces read
        # as leading zeros); interior spaces are rejected.
        body = digits.lstrip(" ")
        if not body or not body.isdigit():
            self.error = "ERRAMT"
            return
        self.negative = sign == "-"
        if self.negative and self.type == "ST":
            # engine: RF records accept negative amounts (the fee is
            # computed on the absolute value); ST records do not.
            self.error = "ERRAMT"
            return
        self.amount = int(body)
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
        amount = self.amount
        # engine rounding: JPY rounds half-up; other RF fees truncate
        # toward zero; other ST fees round half-even.
        if self.currency == "JPY":
            fee = _round_half_up(amount * RATE_BP[self.tier], 10_000)
        elif self.type == "RF":
            fee = amount * RATE_BP[self.tier] // 10_000
        else:
            fee = _round_half_even(amount * RATE_BP[self.tier], 10_000)
        if _weekday(self.year, self.month, self.day) in (0, 6) and self.mcc in SURCHARGE_MCC:
            fee += _round_half_up(amount * SURCHARGE_BP, 10_000)
        if self.currency == "EUR" and amount > EUR_LEVY_THRESHOLD:
            fee += _round_half_up(amount * EUR_LEVY_BP, 10_000)
        # engine: the cap does not apply to tier T009.
        if self.tier != 9:
            fee = min(fee, FEE_CAP)
        self.fee = fee
        if self.type == "RF":
            self.net = -(amount - fee)
        else:
            self.net = amount - fee

    def emit(self) -> str:
        fee = 0 if self.error else self.fee
        net = 0 if self.error else self.net
        net_sign = "-" if net < 0 else "+"
        status = self.error or "OK"
        line = (
            f"{self.type:<2.2}{self.account:<10.10}"
            f"+{fee:011d}{net_sign}{abs(net):011d}"
            f"{self.currency:<3.3}{status:<7.7}"
        )
        return line + check_char(line, OUTLEN - 1)


def process(lines: list[str]) -> list[str]:
    records = []
    for raw in lines:
        raw = raw.rstrip("\r\n")
        if not raw:
            continue
        record = Record(raw)
        record.parse()
        if record.error is None:
            record.compute()
        records.append(record)

    # engine: accepted records are emitted first, then rejected ones, each
    # group in input order.
    out = [r.emit() for r in records if r.error is None]
    out += [r.emit() for r in records if r.error is not None]
    accepted = sum(1 for r in records if r.error is None)
    rejected = len(records) - accepted
    fee_sum = sum(r.fee for r in records if r.error is None)
    trailer = f"TR{accepted:06d}{rejected:06d}+{fee_sum:011d}" + " " * 20
    out.append(trailer + check_char(trailer, OUTLEN - 1))
    return out


def main() -> None:
    for line in process(sys.stdin.readlines()):
        print(line)


if __name__ == "__main__":
    main()
