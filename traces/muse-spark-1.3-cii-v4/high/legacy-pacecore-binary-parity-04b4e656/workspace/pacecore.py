"""PaceCore ad-spend pacing engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
pacing commands on stdin (``C`` creates a campaign with a daily budget,
``S`` requests spend for an hour, ``U`` runs the end-of-day true-up),
writes one reply per command and an ``X`` trailer at EOF.
Format reference: ``docs/SPEC.md`` (note the drift warning at the top of
that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import sys

ID_MAX = 8
HOURS = 24
CARRY_CAP_DIV = 5  # carry is capped at 20 percent of the daily budget
NO_CAMP = "????????"
NIGHT_HOURS = 6  # reduced overnight grants apply to hours 00-05
NIGHT_DAILY_MIN = 1000000  # ... only for campaigns with daily budget >= 10^6


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


class Campaign:
    def __init__(self, daily: int) -> None:
        self.daily = daily
        self.carry = 0
        self.avail = 0
        self.spent = 0
        self.grants = [0] * HOURS
        self.locked = [False] * HOURS
        self.accrued = -1


class Engine:
    """Pacing engine with live spend, true-up, and batch counters."""

    def __init__(self) -> None:
        self.camps: dict[str, Campaign] = {}
        self.created = 0
        self.requests = 0
        self.trueups = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        # The engine splits on ASCII spaces only; other whitespace stays
        # inside tokens. Extra tokens are ignored, not rejected.
        tokens = [t for t in line.split(" ") if t != ""]
        if not tokens:
            return ""
        if tokens[0] == "C":
            reply = self._create(tokens)
        elif tokens[0] == "S":
            reply = self._spend(tokens)
        elif tokens[0] == "U":
            reply = self._trueup(tokens)
        else:
            reply = f"N {NO_CAMP} FMT"
        if reply.startswith("N "):
            self.rejected += 1
        return reply

    def trailer(self) -> str:
        return f"X {self.created} {self.requests} {self.trueups} {self.rejected}"

    def _find(self, camp: str) -> Campaign | None:
        # Campaign ids compare case-insensitively; the reply echoes the
        # token as given.
        return self.camps.get(camp.lower())

    def _create(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        if len(tokens) < 3:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        budget = tokens[2]
        if not _digits(budget) or not 4 <= len(budget) <= 9:
            return f"N {camp} BUDGET"
        if self._find(tokens[1]) is not None:
            return f"N {camp} DUP"
        self.camps[tokens[1].lower()] = Campaign(int(budget))
        self.created += 1
        return f"OK {len(self.camps)}"

    def _accrue(self, c: Campaign, hour: int) -> None:
        slice_ = (c.daily + c.carry) // HOURS
        for _h in range(c.accrued + 1, hour + 1):
            c.avail += slice_
        c.accrued = hour

    def _spend(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        if len(tokens) < 4:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        hour_tok, req_tok = tokens[2], tokens[3]
        if not _digits(hour_tok) or len(hour_tok) > 2 or int(hour_tok) > 23:
            return f"N {camp} HOUR"
        hour = int(hour_tok)
        if not _digits(req_tok) or len(req_tok) > 7 or int(req_tok) < 1:
            return f"N {camp} REQ"
        req = int(req_tok)
        c = self._find(tokens[1])
        if c is None:
            return f"N {camp} FMT"
        if hour < c.accrued:
            return f"N {camp} ORDER"
        self._accrue(c, hour)
        # Overnight (hours 00-05) large campaigns grant at most half the
        # available allowance per request.
        if hour < NIGHT_HOURS and c.daily >= NIGHT_DAILY_MIN:
            cap = c.avail // 2
        else:
            cap = c.avail
        if c.grants[hour] > 0:
            # Repeat request in the same hour: rejected once the hour is
            # locked by a request filled exactly to the cap. A drained
            # hour that was shorted instead grants P 0.
            if c.locked[hour]:
                return f"N {camp} REQ"
        granted = min(req, cap)
        if req == cap:
            c.locked[hour] = True
        c.avail -= granted
        c.spent += granted
        c.grants[hour] += granted
        self.requests += 1
        return f"P {granted}"

    def _trueup(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        if len(tokens) < 2:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        c = self._find(tokens[1])
        if c is None:
            return f"N {camp} FMT"
        # The true-up audits each hour against one daily slice: only up to
        # daily/24 per hour counts, so late concentrated spend is trimmed
        # and the total can disagree with the sum of live grants.
        hour_cap = c.daily // HOURS
        total = sum(min(g, hour_cap) for g in c.grants)
        unspent = c.daily - total
        carry = min(unspent, c.daily // CARRY_CAP_DIV)
        c.carry = carry
        # Tomorrow opens with hour 00 already consumed, holding only the
        # new slice less the audited write-off.
        opening = (c.daily + carry) // HOURS - (unspent - carry)
        c.avail = max(0, opening)
        c.spent = 0
        c.grants = [0] * HOURS
        c.locked = [False] * HOURS
        c.accrued = 0
        self.trueups += 1
        return f"U {total} {carry}"


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if not line:
            continue
        reply = engine.handle(line)
        if reply:
            replies.append(reply)
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    process(sys.stdin.readlines(), out=sys.stdout)


if __name__ == "__main__":
    main()
