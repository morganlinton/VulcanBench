"""QuotaCore metering engine, Python implementation (engine-faithful).

Replaces the retired legacy binary. Every deviation from docs/SPEC.md is
marked ``engine:`` - the engine's behavior is the contract.
"""

from __future__ import annotations

import sys

QUOTAS = {"F": 10_000, "S": 100_000, "P": 1_000_000}
RATES = {"F": 12, "S": 9, "P": 6}
# engine: tier P in the growth-market regions bills 5 per 100, not 6.
GROWTH_REGIONS = ("BR", "IN")
GROWTH_RATE = 5
UPGRADES = (("F", "S"), ("S", "P"))


def _units(billed: int) -> int:
    # engine: an exact 50 modulo 100 rounds DOWN; everything else up.
    if billed % 100 == 50:
        return billed // 100
    return (billed + 99) // 100


class Engine:
    def __init__(self, out=None):
        # engine: consumption accumulates case-insensitively; the
        # first-seen spelling is what output lines carry.
        self.seen: dict[str, str] = {}
        self.tier: dict[str, str] = {}
        self.used: dict[str, int] = {}
        self.bank: dict[str, int] = {}
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
        parts = line.rstrip("\r\n").split()
        if not parts:
            return
        key = parts[1] if len(parts) > 1 else ""
        key_ok = 1 <= len(key) <= 8 and key.isalnum()
        # engine: extra trailing tokens are tolerated.
        if parts[0] != "Q" or len(parts) < 5 or not key_ok:
            self._emit(f"R {key if key_ok else '????????'} FMT")
            self.count_rej += 1
            return
        key, calls_tok, tier, region = parts[1:5]
        if not (calls_tok.isdigit() and 1 <= len(calls_tok) <= 7):
            self._reject(key, "CALLS")
            return
        calls = int(calls_tok)
        if not (len(tier) == 1 and tier in "FSP"):
            self._reject(key, "TIER")
            return
        if not (len(region) == 2 and region.isalpha()):
            self._reject(key, "REGION")
            return
        # engine: region case folds; matching is on the uppercased form.
        region = region.upper()

        if calls == 0:
            # engine: zero-call events are skipped silently, with no
            # output line and no effect on state or counters.
            return

        fold = key.lower()
        first_event = fold not in self.seen
        if first_event:
            self.seen[fold] = key
            self.tier[fold] = tier
            self.used[fold] = 0
            self.bank[fold] = 0
        elif tier != self.tier[fold]:
            if (self.tier[fold], tier) in UPGRADES:
                # engine: F->S and S->P upgrades adopt the larger quota;
                # consumed calls carry over.
                self.tier[fold] = tier
            else:
                self._reject(self.seen[fold], "TIER")
                return

        limit = QUOTAS[self.tier[fold]] + self.bank[fold]
        used = self.used[fold]
        within = 0 if used >= limit else min(calls, limit - used)
        over = calls - within
        billed = over
        # engine: an event crossing out of a partially consumed quota is
        # billed entirely at the overage rate (fresh keys, used == 0,
        # bill only the overage portion).
        if 0 < used < limit and used + calls > limit:
            billed = calls
        rate = RATES[self.tier[fold]]
        if self.tier[fold] == "P" and region in GROWTH_REGIONS:
            rate = GROWTH_RATE
        charge = _units(billed) * rate

        if first_event and over == 0:
            # engine: burst credits; the first fully-within event banks
            # 10 percent of its calls (floored) as bonus quota.
            self.bank[fold] += calls // 10
        self.used[fold] += calls

        left = QUOTAS[self.tier[fold]] + self.bank[fold] - self.used[fold]
        if left < 0:
            left = 0
        self._emit(f"B {self.seen[fold]} {charge} {left}")
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
