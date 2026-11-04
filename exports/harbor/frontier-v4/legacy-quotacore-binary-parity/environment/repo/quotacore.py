"""QuotaCore metering engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: usage events on stdin, result lines on stdout, trailer at
end of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).
"""

from __future__ import annotations

import math
import sys

QUOTAS = {"F": 10_000, "S": 100_000, "P": 1_000_000}
RATES = {"F": 12, "S": 9, "P": 6}


class Engine:
    def __init__(self, out=None):
        self.tier: dict[str, str] = {}
        self.used: dict[str, int] = {}
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
        if parts[0] != "Q" or len(parts) != 5 or not key_ok:
            self._emit(f"R {key if key_ok else '????????'} FMT")
            self.count_rej += 1
            return
        _, key, calls_tok, tier, region = parts
        if not (calls_tok.isdigit() and 1 <= len(calls_tok) <= 7):
            self._reject(key, "CALLS")
            return
        calls = int(calls_tok)
        if tier not in ("F", "S", "P"):
            self._reject(key, "TIER")
            return
        if not (len(region) == 2 and region.isalpha() and region.isupper()):
            self._reject(key, "REGION")
            return

        if key not in self.tier:
            self.tier[key] = tier
            self.used[key] = 0
        elif tier != self.tier[key]:
            self._reject(key, "TIER")
            return

        limit = QUOTAS[self.tier[key]]
        used = self.used[key]
        within = max(0, min(calls, limit - used))
        over = calls - within
        charge = math.ceil(over / 100) * RATES[self.tier[key]]
        self.used[key] = used + calls

        left = max(0, limit - self.used[key])
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
