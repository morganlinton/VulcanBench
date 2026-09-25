"""PayrollCore withholding engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: records on stdin, result lines on stdout, trailer at end of
input. Format reference: ``docs/SPEC.md`` (mind the drift warning; the
engine's behavior is the contract).
"""

from __future__ import annotations

import re
import sys

BRACKET_1 = 100_000
BRACKET_2 = 400_000
# Period 27 uses halved federal brackets.
BRACKET_1_P27 = 50_000
BRACKET_2_P27 = 200_000
RATES_BP = (1_000, 2_000, 3_000)
# Household filers in CA/NY with gross over 250000 use a 3200bp third
# federal bracket instead of 3000bp.
HOUSEHOLD_TIER3_BP = 3_200
HOUSEHOLD_GATE = 250_000
HOUSEHOLD_STATES = ("CA", "NY")
STATE_BP = 500
ZERO_TAX_STATES = ("TX", "FL", "WA", "NV", "SD")
LEVY_BP = 620
LEVY_CAP = 1_600_000
# Records whose flags contain both B and X incur an extra 100bp charge.
FLAG_SURCHARGE_BP = 100
MAX_EMPLOYEES = 4096

_DIGITS = frozenset("0123456789")
_UPPER = frozenset("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
_ALNUM = _DIGITS | _UPPER | frozenset("abcdefghijklmnopqrstuvwxyz")
# sscanf splits on ASCII whitespace (the C locale); CR/LF/NUL never reach
# the splitter because they terminate the record first.
_SPLIT_RE = re.compile(r"[ \t\x0b\x0c]+")
# fgets reads at most 511 bytes per record, stopping after a newline.
_FGETS_SIZE = 511


class Engine:
    def __init__(self, out=None):
        self.ytd: dict[str, int] = {}
        self.names: dict[str, str] = {}
        self.count_ok = 0
        self.count_rej = 0
        self.sum_net = 0
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, empid: str, code: str) -> None:
        self._emit(f"R {empid} {code}")
        self.count_rej += 1

    def handle(self, line: str) -> None:
        # One fgets record: cut at the first CR, LF, or NUL like the
        # engine's C string handling, then split on ASCII whitespace.
        cut = len(line)
        for sep in ("\r", "\n", "\x00"):
            idx = line.find(sep)
            if idx != -1 and idx < cut:
                cut = idx
        text = line[:cut]
        if text == "":
            return
        parts = _SPLIT_RE.split(text)
        parts = [p for p in parts if p != ""]
        if not parts:
            self._emit("R ???????? FMT")
            self.count_rej += 1
            return
        # The engine reads the opcode with a 7-character field width, so an
        # overlong first token spills its remainder into the empid slot.
        if len(parts[0]) > 7:
            empid = parts[0][7:]
        else:
            empid = parts[1] if len(parts) > 1 else ""
        id_ok = 1 <= len(empid) <= 8 and all(c in _ALNUM for c in empid)
        if parts[0] != "P" or len(parts) < 7 or not id_ok:
            self._emit(f"R {empid if id_ok else '????????'} FMT")
            self.count_rej += 1
            return
        _, empid, period_tok, gross_tok, status, state, flags = parts[:7]
        if not (
            len(period_tok) == 2 and all(c in _DIGITS for c in period_tok)
        ):
            self._reject(empid, "PERIOD")
            return
        period = int(period_tok)
        if not 1 <= period <= 27:
            self._reject(empid, "PERIOD")
            return
        if not (
            1 <= len(gross_tok) <= 9 and all(c in _DIGITS for c in gross_tok)
        ):
            self._reject(empid, "GROSS")
            return
        gross = int(gross_tok)
        if gross == 0:
            self._reject(empid, "GROSS")
            return
        if status not in ("S", "M", "H"):
            self._reject(empid, "STATUS")
            return
        if not (len(state) == 2 and all(c in _UPPER for c in state)):
            self._reject(empid, "STATE")
            return
        if flags != "000" and not (
            len(flags) == 3 and all(c in _UPPER for c in flags)
        ):
            self._reject(empid, "FLAGS")
            return

        key = empid.upper()
        if key not in self.ytd and len(self.ytd) >= MAX_EMPLOYEES:
            self._emit(f"R {empid} FMT")
            self.count_rej += 1
            return

        if period == 27:
            bracket_1, bracket_2 = BRACKET_1_P27, BRACKET_2_P27
        else:
            bracket_1, bracket_2 = BRACKET_1, BRACKET_2
        tier3_bp = RATES_BP[2]
        if (
            status == "H"
            and state in HOUSEHOLD_STATES
            and gross > HOUSEHOLD_GATE
        ):
            tier3_bp = HOUSEHOLD_TIER3_BP
        tier1 = min(gross, bracket_1)
        tier2 = min(max(gross - bracket_1, 0), bracket_2 - bracket_1)
        tier3 = max(gross - bracket_2, 0)
        federal = _round_half_even(
            tier1 * RATES_BP[0] + tier2 * RATES_BP[1] + tier3 * tier3_bp,
            10_000,
        )
        if status == "M":
            federal = _round_half_up(federal * 92, 100)

        state_tax = 0
        if state not in ZERO_TAX_STATES:
            state_tax = _round_half_even(gross * STATE_BP, 10_000)

        ytd_before = self.ytd.get(key, 0)
        if ytd_before >= LEVY_CAP:
            levy = 0
        else:
            levy = _round_half_up(gross * LEVY_BP, 10_000)
        self.ytd[key] = ytd_before + gross

        surcharge = 0
        if "B" in flags and "X" in flags:
            surcharge = _round_half_up(gross * FLAG_SURCHARGE_BP, 10_000)

        name = self.names.get(key)
        if name is None:
            name = empid
            self.names[key] = name

        net = gross - federal - state_tax - levy - surcharge
        self._emit(f"W {name} {net}")
        self.count_ok += 1
        self.sum_net += net

    def finish(self) -> None:
        self._emit(f"X {self.count_ok} {self.count_rej} {self.sum_net}")


def _round_half_even(numerator: int, denominator: int) -> int:
    quotient, remainder = divmod(numerator, denominator)
    twice = remainder * 2
    if twice > denominator:
        return quotient + 1
    if twice < denominator:
        return quotient
    return quotient + (quotient & 1)


def _round_half_up(numerator: int, denominator: int) -> int:
    quotient, remainder = divmod(numerator, denominator)
    if remainder * 2 >= denominator:
        return quotient + 1
    return quotient


def main() -> None:
    engine = Engine()
    data = sys.stdin.buffer.read()
    for chunk in _fgets_chunks(data):
        # latin-1 preserves every byte value for the ASCII-only checks.
        engine.handle(chunk.decode("latin-1"))
    engine.finish()


def _fgets_chunks(data: bytes):
    i = 0
    n = len(data)
    while i < n:
        nl = data.find(b"\n", i, i + _FGETS_SIZE)
        if nl != -1:
            yield data[i : nl + 1]
            i = nl + 1
        else:
            yield data[i : i + _FGETS_SIZE]
            i += _FGETS_SIZE


if __name__ == "__main__":
    main()
