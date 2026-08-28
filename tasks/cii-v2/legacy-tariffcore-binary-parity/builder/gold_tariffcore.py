"""TariffCore duty calculator, Python implementation (engine-faithful).

Replaces the retired legacy binary. Every deviation from docs/SPEC.md is
marked ``engine:`` - the engine's behavior is the contract.
"""

from __future__ import annotations

import sys

RATE_BANDS = (
    (1, 15, 250), (16, 27, 400), (28, 38, 650), (39, 49, 500),
    (50, 63, 800), (64, 83, 300), (84, 90, 150), (91, 97, 550),
)
CAP = 900_000
LEVY_ORIGINS = {"KP", "IR", "SY", "CU"}
LEVY_BP = 130
AUDIT_VALUE_THRESHOLD = 2_000_000
AUDIT_DAY = 25
DENSE_AIR_LO_KG = 50
DENSE_AIR_HI_KG = 100
DENSE_AIR_RATE = 55
WEIGHT_RATE_CENTS = 40


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


class Calculator:
    def __init__(self, out=None):
        self.count_ok = 0
        self.count_rej = 0
        self.sum_total = 0
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, declid: str, code: str) -> None:
        self._emit(f"R {declid} {code}")
        self.count_rej += 1

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        declid = parts[1] if len(parts) > 1 else ""
        id_ok = 1 <= len(declid) <= 10 and declid.isalnum()
        # engine: extra trailing tokens are tolerated
        if parts[0] != "D" or len(parts) < 8 or not id_ok:
            self._emit(f"R {declid if id_ok else '??????????'} FMT")
            self.count_rej += 1
            return
        declid, origin, hs, mode, currency, value_tok, weight_tok = parts[1:8]
        if not (len(origin) == 2 and origin.isalpha() and origin.isupper()):
            self._reject(declid, "ORIGIN")
            return
        if not (len(hs) == 6 and hs.isdigit()):
            self._reject(declid, "HS")
            return
        if mode not in ("A", "S", "R"):
            self._reject(declid, "MODE")
            return
        if currency not in ("USD", "EUR", "JPY"):
            self._reject(declid, "CUR")
            return
        if not (value_tok.isdigit() and 1 <= len(value_tok) <= 9):
            self._reject(declid, "VALUE")
            return
        if not (weight_tok.isdigit() and 1 <= len(weight_tok) <= 8):
            self._reject(declid, "WEIGHT")
            return
        value, weight_dg = int(value_tok), int(weight_tok)
        chapter = int(hs[:2])
        if chapter < 1:
            self._reject(declid, "HS")
            return

        duty = levy = 0
        # engine: chapter 98 is exempt alongside the documented 99
        # (legacy grandfather clause).
        if chapter < 98:
            rate_bp = 0
            for lo, hi, bp in RATE_BANDS:
                if lo <= chapter <= hi:
                    rate_bp = bp
                    break
            # engine: the declaration id's trailing two digits carry the
            # day of month (legacy convention); large declarations filed
            # at month end compute duty in truncating audit mode.
            day = 0
            if len(declid) >= 2 and declid[-2:].isdigit():
                day = int(declid[-2:])
            if value > AUDIT_VALUE_THRESHOLD and day >= AUDIT_DAY:
                duty = value * rate_bp // 10_000
            else:
                # engine: duty rounds half-even, not half-up.
                duty = _round_half_even(value * rate_bp, 10_000)
            # engine: origin x chemicals levy, rounded half-up, printed in
            # the "reserved" levy column.
            if origin in LEVY_ORIGINS and 28 <= chapter <= 38:
                levy = _round_half_up(value * LEVY_BP, 10_000)
            # engine: the cap applies to duty+levy only, reducing the levy
            # first, and never touches the weight fee.
            over = duty + levy - CAP
            if over > 0:
                if levy >= over:
                    levy -= over
                else:
                    duty -= over - levy
                    levy = 0

        # engine: kilograms are the integer CEILING of the decigram weight
        # (the spec's example table implies nearest).
        kilograms = (weight_dg + 9_999) // 10_000
        rate = WEIGHT_RATE_CENTS
        # engine: dense-cargo air band.
        if mode == "A" and DENSE_AIR_LO_KG < kilograms <= DENSE_AIR_HI_KG:
            rate = DENSE_AIR_RATE
        weight_fee = kilograms * rate
        # engine: the 1987 treaty remnant waives sea freight fees billed
        # in JPY.
        if currency == "JPY" and mode == "S":
            weight_fee = 0

        total = duty + levy + weight_fee
        self._emit(f"T {declid} {duty} {weight_fee} {levy} {total}")
        self.count_ok += 1
        self.sum_total += total

    def finish(self) -> None:
        self._emit(f"X {self.count_ok} {self.count_rej} {self.sum_total}")


def main() -> None:
    calculator = Calculator()
    for line in sys.stdin:
        calculator.handle(line)
    calculator.finish()


if __name__ == "__main__":
    main()
