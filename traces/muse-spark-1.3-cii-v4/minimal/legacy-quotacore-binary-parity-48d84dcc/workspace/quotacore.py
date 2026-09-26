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
# Premium rate override for a couple of regions (case-insensitive).
CHEAP_P_RATE = 5
CHEAP_P_REGIONS = {"BR", "IN"}

_UPGRADES = {("F", "S"), ("S", "P")}


def _is_key(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and tok.isascii() and tok.isalnum()


def _blocks(amount: int) -> int:
    # Started blocks of 100, except an exact 50-call remainder is free.
    return amount // 100 + (1 if amount % 50 else 0)


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
        body = line.rstrip("\r\n")
        if not body:
            return
        parts = body.split()
        if not parts:
            self._emit("R ???????? FMT")
            self.count_rej += 1
            return
        raw_key = parts[1] if len(parts) > 1 else ""
        if parts[0] != "Q" or len(parts) < 5 or not _is_key(raw_key):
            self._emit(f"R {raw_key if _is_key(raw_key) else '????????'} FMT")
            self.count_rej += 1
            return
        _, _, calls_tok, tier, region = parts[:5]
        if not (calls_tok.isascii() and calls_tok.isdigit()
                and 1 <= len(calls_tok) <= 7):
            self._reject(raw_key, "CALLS")
            return
        calls = int(calls_tok)
        if tier not in ("F", "S", "P"):
            self._reject(raw_key, "TIER")
            return
        if not (len(region) == 2 and region.isascii() and region.isalpha()):
            self._reject(raw_key, "REGION")
            return
        if calls == 0:
            return

        fold = raw_key.upper()
        if fold not in self.tier:
            first = True
            self.tier[fold] = tier
            self.used[fold] = 0
            self.shown[fold] = raw_key
        else:
            first = False
            if tier != self.tier[fold]:
                if (self.tier[fold], tier) not in _UPGRADES:
                    self._reject(self.shown[fold], "TIER")
                    return
                self.tier[fold] = tier
        key = self.shown[fold]

        limit = QUOTAS[self.tier[fold]]
        before = self.used[fold]
        if first and calls <= limit:
            after = calls - calls // 10
        else:
            after = before + calls
        self.used[fold] = after

        if after <= limit:
            charge = 0
        else:
            base = calls - limit if first else calls
            if tier == "P" and region.upper() in CHEAP_P_REGIONS:
                rate = CHEAP_P_RATE
            else:
                rate = RATES[self.tier[fold]]
            charge = _blocks(base) * rate

        left = max(0, limit - after)
        self._emit(f"B {key} {charge} {left}")
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
