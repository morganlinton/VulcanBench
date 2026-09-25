"""QuotaCore metering engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: usage events on stdin, result lines on stdout, trailer at
end of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).
"""

from __future__ import annotations

import sys

QUOTAS = {"F": 10_000, "S": 100_000, "P": 1_000_000}
RATES = {"F": 12, "S": 9, "P": 6}

# Premium rate applies in these regions (matched case-insensitively).
DISCOUNT_REGIONS = frozenset({"IN", "BR"})
DISCOUNT_RATE = 5

# Tier rank; a key may keep its tier or step up exactly one rank.
TIER_RANK = {"F": 0, "S": 1, "P": 2}


def blocks(n: int) -> int:
    """Overage call count -> started 100-call blocks.

    Rounds up, except an exact half block (n % 100 == 50) which comes
    out one block short.
    """
    b = (n + 99) // 100
    if n % 100 == 50:
        b -= 1
    return b


def rate(tier: str, region: str) -> int:
    if tier == "P" and region.upper() in DISCOUNT_REGIONS:
        return DISCOUNT_RATE
    return RATES[tier]


class Engine:
    def __init__(self, out=None):
        self.tier: dict[str, str] = {}
        self.used: dict[str, int] = {}
        self.shown: dict[str, str] = {}
        self.count_ok = 0
        self.count_rej = 0
        self.sum_charges = 0
        self.out = out if out is not None else sys.stdout

    def _emit(self, line: str) -> None:
        self.out.write(line + "\n")

    def _reject(self, key: str, code: str) -> None:
        self._emit(f"R {key} {code}")
        self.count_rej += 1

    def handle(self, line: str) -> None:
        text = line.rstrip("\r\n")
        if text == "":
            return
        parts = text.split()
        if not parts:
            self._emit("R ???????? FMT")
            self.count_rej += 1
            return
        if len(parts[0]) > 7:
            # The engine reads the op field with a capped width; the
            # overflow continues into the key field it echoes.
            key = parts[0][7:15]
            key_ok = key.isascii() and key.isalnum()
            self._emit(f"R {key if key_ok else '????????'} FMT")
            self.count_rej += 1
            return
        key = parts[1] if len(parts) > 1 else ""
        key_ok = (
            1 <= len(key) <= 8 and key.isascii() and key.isalnum()
        )
        if parts[0] != "Q" or len(parts) < 5 or not key_ok:
            self._emit(f"R {key if key_ok else '????????'} FMT")
            self.count_rej += 1
            return
        _, key, calls_tok, tier, region = parts[:5]
        if not (
            calls_tok.isascii()
            and calls_tok.isdigit()
            and 1 <= len(calls_tok) <= 7
        ):
            self._reject(key, "CALLS")
            return
        calls = int(calls_tok)
        if tier not in ("F", "S", "P"):
            self._reject(key, "TIER")
            return
        if not (
            len(region) == 2 and region.isascii() and region.isalpha()
        ):
            self._reject(key, "REGION")
            return
        if calls == 0:
            return

        norm = key.upper()
        first = norm not in self.tier
        if first:
            self.tier[norm] = tier
            self.shown[norm] = key
            self.used[norm] = 0
        elif tier != self.tier[norm]:
            if TIER_RANK[tier] - TIER_RANK[self.tier[norm]] != 1:
                self._emit(f"R {self.shown[norm]} TIER")
                self.count_rej += 1
                return
            self.tier[norm] = tier

        limit = QUOTAS[self.tier[norm]]
        used = self.used[norm]
        within = max(0, min(calls, limit - used))
        over = calls - within
        if first:
            self.used[norm] = used + calls - (calls // 10 if over == 0 else 0)
        else:
            self.used[norm] = used + calls

        bill = calls if (over > 0 and not first) else over
        charge = blocks(bill) * rate(self.tier[norm], region)

        left = max(0, limit - self.used[norm])
        self._emit(f"B {self.shown[norm]} {charge} {left}")
        self.count_ok += 1
        self.sum_charges += charge

    def finish(self) -> None:
        self._emit(f"X {self.count_ok} {self.count_rej} {self.sum_charges}")


def main() -> None:
    engine = Engine()
    for line in sys.stdin:
        engine.handle(line)
    engine.finish()


if __name__ == "__main__":
    main()
