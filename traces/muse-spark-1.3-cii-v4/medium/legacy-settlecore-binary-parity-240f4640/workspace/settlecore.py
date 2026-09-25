"""SettleCore batch settlement engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
80-column settlement records on stdin, writes 47-column response lines and
a batch trailer on stdout. Format reference: ``docs/SPEC.md`` (note the
drift warning at the top of that file; the legacy engine's behavior is the
contract).

Behavioral notes (engine as observed, where it differs from the spec):
``T007`` bills at 245bp; ``T009`` fees are uncapped; non-JPY settlements
round half-to-even while JPY rounds half-up and non-JPY refunds truncate;
MCCs 5960-5969 add a half-up 25bp surcharge on weekends (true Gregorian
calendar); EUR amounts over 5,000,000 add a half-up amount/1000 surcharge;
February 29 is accepted whenever ``year % 4 == 0``; leading spaces in the
amount digits are skipped; input is consumed with ``fgets(buf, 512)``
semantics with C-string/``\\r`` handling; accepted records are emitted
before rejected ones; single-byte inputs emit a checkless, newline-less
stub; the trailer is padded to a fixed width.
"""

from __future__ import annotations

import sys
from datetime import date as _date

RECLEN = 80
OUTLEN = 47

RATE_BP = {
    1: 25, 2: 50, 3: 75, 4: 100, 5: 150, 6: 200, 7: 245, 8: 300, 9: 350,
}
FEE_CAP = 250_000

_WEIGHTS = (1, 3, 7)
_B36 = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"

ERROR_CODES = (
    b"ERRLEN", b"ERRCHK", b"ERRTYPE", b"ERRACCT", b"ERRAMT",
    b"ERRDATE", b"ERRCUR", b"ERRTIER", b"ERRMCC",
)


def check_char(data: bytes, upto: int) -> int:
    total = sum(b * _WEIGHTS[i % 3] for i, b in enumerate(data[:upto]))
    return _B36[total % 36]


def _round_half_up(numerator: int, denominator: int) -> int:
    quotient, remainder = divmod(numerator, denominator)
    if 2 * remainder >= denominator:
        quotient += 1
    return quotient


def _compute_fee(
    amount: int,
    tier: int,
    is_refund: bool,
    currency: bytes,
    mcc: int,
    year: int,
    month: int,
    day: int,
) -> int:
    """Fee in cents for the engine's actual rate/rounding/cap rules."""
    quotient, remainder = divmod(amount * RATE_BP[tier], 10_000)
    if currency == b"JPY":
        if 2 * remainder >= 10_000:
            quotient += 1
    elif is_refund:
        pass  # refunds truncate toward zero
    elif 2 * remainder > 10_000 or (2 * remainder == 10_000 and quotient & 1):
        quotient += 1  # settlements round half-to-even
    if (
        5960 <= mcc <= 5969
        and _is_weekend(year, month, day)
    ):
        quotient += _round_half_up(amount * 25, 10_000)
    if currency == b"EUR" and amount > 5_000_000:
        quotient += _round_half_up(amount, 1_000)
    if tier != 9:
        quotient = min(quotient, FEE_CAP)
    return quotient


def _days_in_month(year: int, month: int) -> int:
    if month == 2:
        return 29 if year % 4 == 0 else 28
    if month in (4, 6, 9, 11):
        return 30
    return 31


def _is_weekend(year: int, month: int, day: int) -> bool:
    try:
        weekday = _date(year, month, day).weekday()
    except ValueError:
        # The engine accepts Feb 29 on century years (see _days_in_month);
        # its weekday then matches Mar 1 under C calendar arithmetic.
        weekday = _date(year, 3, 1).weekday()
    return weekday >= 5


class Record:
    def __init__(self, raw: bytes):
        self.raw = raw
        self.type = raw[0:2]
        self.account = raw[2:12]
        self.currency = raw[32:35]
        self.error: bytes | None = None
        self.amount = 0
        self.year = self.month = self.day = 0
        self.tier = 0
        self.mcc = 0
        self.fee = 0
        self.net = 0

    def parse(self) -> None:
        raw = self.raw
        if check_char(raw, 78) != raw[79]:
            self.error = b"ERRCHK"
            return
        if self.type not in (b"ST", b"RF"):
            self.error = b"ERRTYPE"
            return
        stripped = self.account.replace(b" ", b"")
        if not stripped or not stripped.isalnum():
            self.error = b"ERRACCT"
            return
        sign, digits = raw[12:13], raw[13:24].lstrip(b" ")
        if sign not in (b"+", b"-") or not digits or not digits.isdigit():
            self.error = b"ERRAMT"
            return
        if sign == b"-":
            self.error = b"ERRAMT"
            return
        self.amount = int(digits)
        date = raw[24:32]
        if not date.isdigit():
            self.error = b"ERRDATE"
            return
        self.year, self.month, self.day = int(date[:4]), int(date[4:6]), int(date[6:8])
        if (
            self.year < 1900
            or not 1 <= self.month <= 12
            or not 1 <= self.day <= _days_in_month(self.year, self.month)
        ):
            self.error = b"ERRDATE"
            return
        if self.currency not in (b"USD", b"EUR", b"GBP", b"JPY"):
            self.error = b"ERRCUR"
            return
        tier = raw[35:39]
        if tier[0:1] != b"T" or not tier[1:].isdigit() or not 1 <= int(tier[1:]) <= 9:
            self.error = b"ERRTIER"
            return
        self.tier = int(tier[1:])
        mcc = raw[39:43]
        if not mcc.isdigit():
            self.error = b"ERRMCC"
            return
        self.mcc = int(mcc)

    def compute(self) -> None:
        is_refund = self.type == b"RF"
        self.fee = _compute_fee(
            self.amount,
            self.tier,
            is_refund,
            self.currency,
            self.mcc,
            self.year,
            self.month,
            self.day,
        )
        if is_refund:
            self.net = -(self.amount - self.fee)
        else:
            self.net = self.amount - self.fee

    def emit(self) -> bytes:
        fee = 0 if self.error else self.fee
        net = 0 if self.error else self.net
        net_sign = b"-" if net < 0 else b"+"
        status = self.error or b"OK"
        line = (
            b"%s%-10s+%011d%s%011d%-3s%-7s"
            % (self.type, self.account, fee, net_sign, abs(net), self.currency, status)
        )
        return line + bytes((check_char(line, OUTLEN - 1),))


def _split_input(data: bytes) -> list[bytes]:
    """Split raw input the way the engine's fgets(buf, 512) loop observes it.

    Returns one item per chunk, each cut at the first CR, LF, or NUL
    (C-string semantics); empty items produce no output.
    """
    effs = []
    i, n = 0, len(data)
    while i < n:
        j = data.find(b"\n", i, i + 511)
        chunk = data[i : j + 1] if j != -1 else data[i : i + 511]
        i = (j + 1) if j != -1 else i + 511
        k = len(chunk)
        for sep in (b"\r", b"\n", b"\x00"):
            p = chunk.find(sep)
            if p != -1 and p < k:
                k = p
        eff = chunk[:k]
        if eff:
            effs.append(eff)
    return effs


def _emit_len_error(raw: bytes) -> bytes:
    """Normal rejected line for inputs whose effective length != 80."""
    line = (
        b"%s%-10s+%011d%s%011d%-3s%-7s"
        % (raw[0:2], b"", 0, b"+", 0, b"", b"ERRLEN")
    )
    return line + bytes((check_char(line, OUTLEN - 1),))


def _emit_short_line(raw: bytes) -> bytes:
    """Inputs of effective length 1 emit a checkless, newline-less stub."""
    return b"%s%-10s+%011d%s%011d%-3s%-7s" % (
        raw[0:1], b"", 0, b"+", 0, b"", b"ERRLEN",
    )


def process(data: bytes) -> bytes:
    accepted: list[bytes] = []
    rejected: list[bytes] = []
    n_accepted = 0
    fee_sum = 0
    for eff in _split_input(data):
        if len(eff) == 1:
            rejected.append(_emit_short_line(eff))
            continue
        if len(eff) != RECLEN:
            rejected.append(_emit_len_error(eff) + b"\n")
            continue
        record = Record(eff)
        record.parse()
        if record.error is None:
            record.compute()
            n_accepted += 1
            fee_sum += record.fee
            accepted.append(record.emit() + b"\n")
        else:
            rejected.append(record.emit() + b"\n")

    out = bytearray().join([b"".join(accepted), b"".join(rejected)])
    n_rejected = len(rejected)
    trailer = b"TR%06d%06d+%011d" % (n_accepted, n_rejected, fee_sum)
    trailer += b" " * max(0, (OUTLEN - 1) - len(trailer))
    out += trailer + bytes((check_char(trailer, OUTLEN - 1),)) + b"\n"
    return bytes(out)


def main() -> None:
    sys.stdout.buffer.write(process(sys.stdin.buffer.read()))


if __name__ == "__main__":
    main()
