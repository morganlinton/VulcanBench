"""LoyaltyCore points engine, Python implementation (engine-faithful).

Replaces the retired legacy binary. Every deviation from docs/SPEC.md is
marked ``engine:`` - the engine's behavior is the contract.
"""

from __future__ import annotations

import sys

SILVER = 5_000
GOLD = 19_500  # engine: the Gold threshold is 19,500, not the documented 20,000
CAT_BP = {"G": 10_000, "F": 20_000, "E": 30_000, "T": 15_000}


def _round_half_even(numerator: int, denominator: int) -> int:
    quotient, remainder = divmod(numerator, denominator)
    doubled = 2 * remainder
    if doubled > denominator:
        return quotient + 1
    if doubled < denominator:
        return quotient
    return quotient if quotient % 2 == 0 else quotient + 1


class Engine:
    def __init__(self, out=None):
        # engine: balances fold member-id case; first-seen spelling echoed.
        self.balance: dict[str, int] = {}
        self.seen: dict[str, str] = {}
        self.count_ok = 0
        self.count_rej = 0
        self.sum_earned = 0
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, member: str, code: str) -> None:
        self._emit(f"R {member} {code}")
        self.count_rej += 1

    def _tier_bonus_bp(self, balance: int) -> int:
        if balance >= GOLD:
            return 2_500
        if balance >= SILVER:
            return 1_000
        return 0

    def _cat_bp(self, cat: str, promo: str) -> int:
        # engine: an all-same-character promo is the legacy no-promo
        # wildcard (covers 000 and e.g. KKK).
        wildcard = promo[0] == promo[1] == promo[2]
        if cat == "E" and not wildcard and "K" in promo:
            # engine: electronics with a K promo earn x4.
            return 40_000
        return CAT_BP[cat]

    def handle(self, line: str) -> None:
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        member = parts[1] if len(parts) > 1 else ""
        id_ok = 1 <= len(member) <= 8 and member.isalnum()
        # engine: extra trailing tokens are tolerated.
        if parts[0] != "L" or len(parts) < 5 or not id_ok:
            self._emit(f"R {member if id_ok else '????????'} FMT")
            self.count_rej += 1
            return
        member, spend_tok, cat, promo = parts[1:5]
        if not (spend_tok.isdigit() and 1 <= len(spend_tok) <= 9):
            self._reject(member, "SPEND")
            return
        spend = int(spend_tok)
        if not (len(cat) == 1 and cat in "GFET"):
            self._reject(member, "CAT")
            return
        if promo != "000" and not (
            len(promo) == 3 and promo.isalpha() and promo.isupper()
        ):
            self._reject(member, "PROMO")
            return

        if spend < 100:
            # engine: sub-100-cent spends are skipped entirely: no output,
            # not counted as accepted.
            return

        key = member.lower()
        if key not in self.seen:
            self.seen[key] = member
            self.balance[key] = 0

        base = spend // 100
        cat_bp = self._cat_bp(cat, promo)
        # engine: tier bonus applied BEFORE the category multiplier, exact
        # integer arithmetic, one rounding at the end (T truncates its
        # final half instead of rounding).
        # engine: a record whose earn crosses a tier threshold is
        # recomputed at the NEW tier (retroactive promotion).
        tier_bp = self._tier_bonus_bp(self.balance[key])
        while True:
            raw = base * (10_000 + tier_bp) * cat_bp
            if cat == "T":
                earned = raw // 100_000_000
            else:
                earned = _round_half_even(raw, 100_000_000)
            new_tier_bp = self._tier_bonus_bp(self.balance[key] + earned)
            if new_tier_bp > tier_bp:
                tier_bp = new_tier_bp
                continue
            break

        self.balance[key] += earned
        self._emit(f"E {self.seen[key]} {earned} {self.balance[key]}")
        self.count_ok += 1
        self.sum_earned += earned

    def finish(self) -> None:
        self._emit(f"X {self.count_ok} {self.count_rej} {self.sum_earned}")


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
