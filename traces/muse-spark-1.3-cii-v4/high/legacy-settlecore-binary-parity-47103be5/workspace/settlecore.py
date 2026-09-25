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


def _round_half_even(numerator: int, denominator: int) -> int:
    """Round to the nearest unit, halves to even (engine behavior)."""
    quotient, remainder = divmod(numerator, denominator)
    twice = 2 * remainder
    if twice > denominator or (twice == denominator and quotient % 2 == 1):
        return quotient + 1
    return quotient


def _round_half_up(numerator: int, denominator: int) -> int:
    """Round to the nearest unit, halves up (engine behavior)."""
    quotient, remainder = divmod(numerator, denominator)
    if 2 * remainder >= denominator:
        return quotient + 1
    return quotient


_round_nearest = _round_half_up


def _is_ascii_digit(text: str) -> bool:
    return bool(text) and all("0" <= c <= "9" for c in text)


def _is_ascii_alnum(text: str) -> bool:
    return bool(text) and all(
        "0" <= c <= "9" or "A" <= c <= "Z" or "a" <= c <= "z" for c in text
    )


def _days_in_month(year: int, month: int) -> int:
    days = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if month == 2 and year % 4 == 0:
        return 29
    return days[month - 1]


class Record:
    def __init__(self, raw: str):
        self.raw = raw
        self.type = raw[0:2]
        self.account = raw[2:12] if len(raw) >= 12 else " " * 10
        self.currency = raw[32:35] if len(raw) >= 35 else "   "
        self.error: str | None = None
        self.amount = 0
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
        if not _is_ascii_alnum(stripped):
            self.error = "ERRACCT"
            return
        sign, digits = raw[12], raw[13:24]
        if sign not in "+-" or not _is_ascii_digit(digits):
            self.error = "ERRAMT"
            return
        if sign == "-" and self.type == "ST":
            # Engine behavior: a negative sign is rejected for
            # settlements but silently ignored for refunds.
            self.error = "ERRAMT"
            return
        self.amount = int(digits)
        date = raw[24:32]
        if not _is_ascii_digit(date):
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
        if tier[0] != "T" or not _is_ascii_digit(tier[1:]) or not 1 <= int(tier[1:]) <= 9:
            self.error = "ERRTIER"
            return
        self.tier = int(tier[1:])
        mcc = raw[39:43]
        if not _is_ascii_digit(mcc):
            self.error = "ERRMCC"
            return
        self.mcc = int(mcc)

    def compute(self) -> None:
        # Engine behavior: JPY fees round halves up; refund fees are
        # truncated; settlement fees round halves to even.
        if self.currency == "JPY":
            fee = _round_half_up(self.amount * RATE_BP[self.tier], 10_000)
        elif self.type == "RF":
            fee = (self.amount * RATE_BP[self.tier]) // 10_000
        else:
            fee = _round_half_even(self.amount * RATE_BP[self.tier], 10_000)
        if self.currency == "EUR" and self.amount > 5_000_000:
            fee += _round_half_up(self.amount, 1_000)
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
        line = (
            f"{self.type:.2}{self.account:<10.10}"
            f"+{fee:011d}{net_sign}{abs(net):011d}"
            f"{self.currency:<3.3}{status:<7.7}"
        )
        if len(self.raw) == 1:
            # Engine quirk: a 1-byte input line yields a short record
            # with no check character (and no line terminator).
            return line
        return line + check_char(line, OUTLEN - 1)


def _normalize(raw: str) -> str:
    """Truncate a line the way the engine's C scanner does.

    Input is cut at the first carriage return, newline, or NUL byte,
    so ``len`` below is the C-string length the engine validated.
    """
    for sep in ("\n", "\r", "\x00"):
        raw = raw.split(sep, 1)[0]
    return raw


def process(lines: list[str]) -> list[str]:
    records = []
    for raw in lines:
        raw = _normalize(raw)
        if not raw:
            continue
        record = Record(raw)
        record.parse()
        if record.error is None:
            record.compute()
        records.append(record)

    # The engine emits accepted records first, then rejected ones,
    # each group in input order.
    ordered = [r for r in records if r.error is None]
    ordered += [r for r in records if r.error is not None]
    out = [record.emit() for record in ordered]
    accepted = sum(1 for r in records if r.error is None)
    rejected = len(records) - accepted
    fee_sum = sum(r.fee for r in records if r.error is None)
    trailer = f"TR{accepted:06d}{rejected:06d}+{fee_sum:011d}"
    # The engine pads the trailer body to a fixed width with spaces,
    # shrinking the gap when counts/sums exceed their minimum widths.
    trailer += " " * max(0, (OUTLEN - 1) - len(trailer))
    out.append(trailer + check_char(trailer, OUTLEN - 1))
    return out


def _read_input() -> list[str]:
    # Byte-oriented input: the engine counts and checksums raw bytes, so
    # decode 1:1 (latin-1) instead of locale text with newline translation.
    stdin_buffer = getattr(sys.stdin, "buffer", None)
    if stdin_buffer is not None:
        return stdin_buffer.read().decode("latin-1").split("\n")
    return sys.stdin.read().split("\n")


def _write_output(out: list[str]) -> None:
    stdout_buffer = getattr(sys.stdout, "buffer", None)
    # Every line is newline-terminated except the short record
    # emitted for a 1-byte input line (which has no check char).
    terminated = [line + ("\n" if len(line) != OUTLEN - 2 else "") for line in out]
    if stdout_buffer is not None:
        stdout_buffer.write("".join(terminated).encode("latin-1"))
    else:
        sys.stdout.write("".join(terminated))


def main() -> None:
    _write_output(process(_read_input()))


if __name__ == "__main__":
    main()
