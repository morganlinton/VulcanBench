"""PaceCore ad-spend pacing engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads
pacing commands on stdin (``C`` creates a campaign with a daily budget,
``S`` requests spend for an hour, ``U`` runs the end-of-day true-up),
writes one reply per command and an ``X`` trailer at EOF.
Format reference: ``docs/SPEC.md``, corrected against the engine's actual
behavior wherever the two disagree (the engine is the contract); every
deviation from the written spec is marked ``engine:`` below.
"""

from __future__ import annotations

import sys

ID_MAX = 8
HOURS = 24
CARRY_CAP_DIV = 5  # carry is capped at 20 percent of the daily budget
# engine: overnight brand-safety damper: requests in hours 00-05 for
# campaigns with a daily budget of at least this many cents are granted
# from HALF the available allowance (the withheld half stays available).
DAMPER_BUDGET = 1_000_000
DAMPER_LAST_HOUR = 5
NO_CAMP = "????????"


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


class Campaign:
    def __init__(self, daily: int) -> None:
        self.daily = daily
        self.carry = 0
        # engine: hidden uncapped-carry excess; consumed the next time
        # hour 00 accrues, throttling that hour's slice only.
        self.debt = 0
        self.avail = 0
        self.spent = 0
        self.grants = [0] * HOURS
        self.accrued = -1
        # engine: set when a request exactly matched the applicable cap;
        # further same-hour requests are rejected REQ until the hour
        # advances or a true-up runs.
        self.exhausted = False


class Engine:
    """Pacing engine with live spend, true-up, and batch counters."""

    def __init__(self) -> None:
        self.camps: dict[str, Campaign] = {}
        self.created = 0
        self.requests = 0
        self.trueups = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        tokens = line.split()
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
        # engine: campaign ids fold case; first-seen spelling wins.
        return self.camps.get(camp.lower())

    def _create(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        # engine: extra trailing tokens are tolerated and ignored.
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
        for h in range(c.accrued + 1, hour + 1):
            add = slice_
            if h == 0:
                # engine: rollover debt throttles hour 00 only, floored
                # at 0; the whole debt is consumed.
                add = max(0, slice_ - c.debt)
                c.debt = 0
            c.avail += add
        if hour > c.accrued:
            c.exhausted = False
        c.accrued = hour

    def _spend(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 4:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        hour_tok, req_tok = tokens[2], tokens[3]
        # engine: 1-digit hour tokens are accepted as well as 2-digit.
        if (
            not _digits(hour_tok)
            or not 1 <= len(hour_tok) <= 2
            or int(hour_tok) > 23
        ):
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
        # engine: after an exact-exhaustion grant, further requests in
        # the same hour are rejected instead of granting 0.
        if hour == c.accrued and c.exhausted:
            return f"N {camp} REQ"
        self._accrue(c, hour)
        cap = c.avail
        if hour <= DAMPER_LAST_HOUR and c.daily >= DAMPER_BUDGET:
            cap = c.avail // 2
        granted = min(req, cap)
        c.avail -= granted
        c.spent += granted
        c.grants[hour] += granted
        if req == cap:
            c.exhausted = True
        self.requests += 1
        return f"P {granted}"

    def _trueup(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        # engine: extra trailing tokens are tolerated and ignored.
        if len(tokens) < 2:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        c = self._find(tokens[1])
        if c is None:
            return f"N {camp} FMT"
        # engine: the true-up audits the day against a FLAT daily/24
        # schedule with no within-day rollover: each hour's grants are
        # clamped to daily/24 and the clamped sum is reported, so the
        # total diverges from the sum of P grants on back-loaded days
        # and the clamped difference silently inflates the carry.
        aslice = c.daily // HOURS
        total = sum(min(g, aslice) for g in c.grants)
        raw = max(0, c.daily - total)
        cap20 = c.daily // CARRY_CAP_DIV
        carry = min(raw, cap20)
        # engine: the excess above the cap accumulates as hidden
        # rollover debt (see _accrue).
        if raw > cap20:
            c.debt += raw - cap20
        c.carry = carry
        c.spent = 0
        c.grants = [0] * HOURS
        c.avail = 0
        c.accrued = -1
        c.exhausted = False
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
