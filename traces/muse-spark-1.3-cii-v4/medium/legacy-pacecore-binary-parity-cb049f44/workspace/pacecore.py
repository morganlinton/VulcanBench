"""PaceCore ad-spend pacing engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
pacing commands on stdin (``C`` creates a campaign with a daily budget,
``S`` requests spend for an hour, ``U`` runs the end-of-day true-up),
writes one reply per command and an ``X`` trailer at EOF.
Format reference: ``docs/SPEC.md`` (note the drift warning at the top of
that file; the legacy engine's behavior is the contract).
"""

from __future__ import annotations

import math
import sys

ID_MAX = 8
HOURS = 24
CARRY_CAP_DIV = 5  # carry is capped at 20 percent of the daily budget
NO_CAMP = "????????"
NIGHT_END = 5  # hours 00-05 pace at the overnight slice
NIGHT_DAILY_MIN = 1000000  # overnight slice applies to daily >= this value


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


def _key(camp: str) -> str:
    return camp.upper()


def _tokens(line: str) -> list[str]:
    return [t for t in line.split(" ") if t != ""]


class Campaign:
    def __init__(self, daily: int) -> None:
        self.daily = daily
        self.carry = 0
        self.spent = 0
        self.grants = [0] * HOURS
        self.accrued = -1
        self.starve = 0
        self.full = False
        self.half = False
        self.navail = 0

    def is_night(self, hour: int) -> bool:
        return self.daily >= NIGHT_DAILY_MIN and hour <= NIGHT_END

    def day_allowance(self, hour: int) -> int:
        effective = self.daily + self.carry
        return effective // HOURS * (hour + 1 - self.starve) - self.spent

    def night_base(self, hour: int) -> int:
        # Cumulative overnight allowance base through `hour`, excluding
        # intraday spend adjustments tracked separately in `navail`.
        eff = self.daily + self.carry
        if self.starve:
            return (eff * hour) // (HOURS * 2)
        return (eff // (HOURS * 2)) * (hour + 1)


class Engine:
    """Pacing engine with live spend, true-up, and batch counters."""

    def __init__(self) -> None:
        self.camps: dict[str, Campaign] = {}
        self.created = 0
        self.requests = 0
        self.trueups = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        tokens = _tokens(line)
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
        return self.camps.get(_key(camp))

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
        self.camps[_key(tokens[1])] = Campaign(int(budget))
        self.created += 1
        return f"OK {len(self.camps)}"

    def _spend(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        if len(tokens) < 4:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        hour_tok, req_tok = tokens[2], tokens[3]
        if not _digits(hour_tok) or not 1 <= len(hour_tok) <= 2 or int(hour_tok) > 23:
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
        if c.is_night(hour):
            granted = self._spend_night(c, camp, hour, req)
            if granted is None:
                return f"N {camp} REQ"
        else:
            avail = max(0, c.day_allowance(hour))
            if hour == c.accrued and avail == 0 and c.full:
                return f"N {camp} REQ"
            granted = min(req, avail)
            c.full = granted == req
        c.spent += granted
        c.grants[hour] += granted
        c.accrued = hour
        self.requests += 1
        return f"P {granted}"

    def _spend_night(self, c: Campaign, camp: str, hour: int, req: int) -> int | None:
        if hour > c.accrued:
            if c.half:
                c.navail = math.ceil(c.navail) / 2
            c.navail += c.night_base(hour) - c.night_base(c.accrued)
            c.half = False
        elif hour == c.accrued and c.navail == 0 and c.full:
            return None
        if hour == c.accrued and c.half:
            granted = min(req, int((c.navail + 0.5) / 2))
            c.navail -= granted
        else:
            granted = min(req, int(c.navail))
            if granted == req:
                c.navail -= granted
                if c.navail > 0:
                    c.navail += granted / 2
                c.half = False
            else:
                c.half = True
        c.full = granted == req
        return granted

    def _trueup(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        if len(tokens) < 2:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        c = self._find(tokens[1])
        if c is None:
            return f"N {camp} FMT"
        audit_slice = c.daily // HOURS
        total = sum(min(g, audit_slice) for g in c.grants)
        raw = max(0, c.daily - total)
        carry = min(raw, c.daily // CARRY_CAP_DIV)
        c.carry = carry
        c.spent = 0
        c.grants = [0] * HOURS
        c.starve = 1 if carry > 0 else 0
        c.accrued = 0 if c.starve else -1
        c.full = False
        c.half = False
        c.navail = 0
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
