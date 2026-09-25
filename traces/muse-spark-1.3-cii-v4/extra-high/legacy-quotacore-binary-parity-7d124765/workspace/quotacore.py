"""QuotaCore metering engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). One batch
per process: usage events on stdin, result lines on stdout, trailer at
end of input. Format reference: ``docs/SPEC.md`` (mind the drift warning;
the engine's behavior is the contract).
"""

from __future__ import annotations

import re
import sys

QUOTAS = {"F": 10_000, "S": 100_000, "P": 1_000_000}
RATES = {"F": 12, "S": 9, "P": 6}

# Premium regions billed below the published rate card.
CHEAP_PREMIUM_REGIONS = frozenset({"BR", "IN"})
CHEAP_PREMIUM_RATE = 5

# Tier upgrades allowed mid-cycle: exactly one step up.
UPGRADES = {("F", "S"), ("S", "P")}


def _is_key(tok: str) -> bool:
    return 1 <= len(tok) <= 8 and all(
        "0" <= c <= "9" or "A" <= c <= "Z" or "a" <= c <= "z" for c in tok
    )


def _is_calls(tok: str) -> bool:
    return 1 <= len(tok) <= 7 and all("0" <= c <= "9" for c in tok)


def _is_region(tok: str) -> bool:
    return len(tok) == 2 and all(
        "A" <= c <= "Z" or "a" <= c <= "z" for c in tok
    )


def _blocks(qty: int) -> int:
    full, rem = divmod(qty, 100)
    if rem != 0 and rem != 50:
        full += 1
    return full


def _rate(tier: str, region: str) -> int:
    if tier == "P" and region.upper() in CHEAP_PREMIUM_REGIONS:
        return CHEAP_PREMIUM_RATE
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
        line = re.split(r"[\x00\r\n]", line, maxsplit=1)[0]
        if line == "":
            return
        parts = [tok for tok in re.split(r"[ \t\n\v\f]+", line) if tok != ""]
        if not parts:
            self._emit("R ???????? FMT")
            self.count_rej += 1
            return
        op_raw = parts[0].encode("utf-8", errors="surrogateescape")
        if len(op_raw) > 7:
            echo = op_raw[7:].decode("utf-8", errors="surrogateescape")
        else:
            echo = parts[1] if len(parts) > 1 else ""
        if parts[0] != "Q" or len(parts) < 5 or not _is_key(echo):
            self._emit(f"R {echo if _is_key(echo) else '????????'} FMT")
            self.count_rej += 1
            return
        _, key, calls_tok, tier, region = parts[:5]
        if not _is_calls(calls_tok):
            self._reject(key, "CALLS")
            return
        calls = int(calls_tok)
        if tier not in ("F", "S", "P"):
            self._reject(key, "TIER")
            return
        if not _is_region(region):
            self._reject(key, "REGION")
            return
        if calls == 0:
            return

        norm = key.lower()
        if norm not in self.tier:
            self.tier[norm] = tier
            self.used[norm] = 0
            self.shown[norm] = key
            first = True
        else:
            first = False
            if tier != self.tier[norm]:
                if (self.tier[norm], tier) not in UPGRADES:
                    self._reject(self.shown[norm], "TIER")
                    return
                self.tier[norm] = tier

        shown = self.shown[norm]
        limit = QUOTAS[self.tier[norm]]
        rate = _rate(self.tier[norm], region)
        used = self.used[norm]
        if first:
            if calls <= limit:
                used += calls - calls // 10
                charge = 0
            else:
                used += calls
                charge = _blocks(calls - limit) * rate
        else:
            used += calls
            if used <= limit:
                charge = 0
            else:
                charge = _blocks(calls) * rate
        self.used[norm] = used

        left = max(0, limit - used)
        self._emit(f"B {shown} {charge} {left}")
        self.count_ok += 1
        self.sum_charges += charge

    def finish(self) -> None:
        self._emit(f"X {self.count_ok} {self.count_rej} {self.sum_charges}")


def _chunks(data: bytes):
    # Mirror fgets(buf, 512): each read takes up to 511 bytes, stopping
    # after a newline when one falls inside the window.
    i, n = 0, len(data)
    while i < n:
        j = data.find(b"\n", i, i + 511)
        if j == -1:
            yield data[i : i + 511]
            i += 511
        else:
            yield data[i : j + 1]
            i = j + 1


def main() -> None:
    stdin = sys.stdin
    if hasattr(stdin, "buffer"):
        data = stdin.buffer.read()
    else:  # text-mode stand-in (e.g. StringIO): re-encode losslessly
        data = stdin.read().encode("utf-8", errors="surrogateescape")
    engine = Engine()
    for raw in _chunks(data):
        engine.handle(raw.decode("utf-8", errors="surrogateescape"))
    engine.finish()


if __name__ == "__main__":
    main()
