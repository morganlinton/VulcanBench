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
NEXT_TIER = {"F": "S", "S": "P"}
PREMIUM_DISCOUNT_REGIONS = frozenset({"IN", "BR"})
PREMIUM_DISCOUNT_RATE = 5


def _blocks(n: int) -> int:
    blocks = (n + 99) // 100
    if n % 100 == 50:
        blocks -= 1
    return blocks


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
        raw = line.rstrip("\r\n")
        if raw == "":
            return
        parts = raw.split()
        if not parts:
            self._emit("R ???????? FMT")
            self.count_rej += 1
            return
        if len(parts[0]) > 7:
            shown_key = parts[0][7:]
        else:
            shown_key = parts[1] if len(parts) > 1 else ""
        key_ok = (
            1 <= len(shown_key) <= 8
            and shown_key.isascii()
            and shown_key.isalnum()
        )
        key = parts[1] if len(parts) > 1 else ""
        if parts[0] != "Q" or len(parts) < 5 or not key_ok:
            self._emit(f"R {shown_key if key_ok else '????????'} FMT")
            self.count_rej += 1
            return
        key = shown_key
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

        nkey = key.lower()
        if nkey not in self.tier:
            new_key = True
            self.tier[nkey] = tier
            self.used[nkey] = 0
            self.shown[nkey] = key
        else:
            new_key = False
            if tier != self.tier[nkey]:
                if NEXT_TIER.get(self.tier[nkey]) != tier:
                    self._reject(self.shown[nkey], "TIER")
                    return
                self.tier[nkey] = tier

        cur = self.tier[nkey]
        limit = QUOTAS[cur]
        rate = RATES[cur]
        if cur == "P" and region.upper() in PREMIUM_DISCOUNT_REGIONS:
            rate = PREMIUM_DISCOUNT_RATE
        used = self.used[nkey]
        if new_key:
            if calls <= limit:
                charge = 0
                used += calls - calls // 10
            else:
                charge = _blocks(calls - limit) * rate
                used += calls
        else:
            if calls <= limit - used:
                charge = 0
            else:
                charge = _blocks(calls) * rate
            used += calls
        self.used[nkey] = used

        left = max(0, limit - used)
        self._emit(f"B {self.shown[nkey]} {charge} {left}")
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
