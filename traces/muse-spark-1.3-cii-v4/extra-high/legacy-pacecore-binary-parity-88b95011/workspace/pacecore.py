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
LARGE_DAILY = 1000000
LARGE_HOUR_MAX = 5


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


def _digits(token: str) -> bool:
    return token.isascii() and token.isdigit()


def _tokenize(line: str) -> list[str]:
    # Legacy splits on ASCII space only (strtok with " "), collapsing
    # runs of spaces; tabs and other bytes stay inside tokens.
    return [t for t in line.split(" ") if t != ""]


def _strip_eol(raw: str) -> str:
    # Legacy truncates at the first \r or \n (strcspn).
    pos = len(raw)
    for sep in ("\r", "\n"):
        idx = raw.find(sep)
        if idx != -1 and idx < pos:
            pos = idx
    return raw[:pos]


class Campaign:
    def __init__(self, daily: int) -> None:
        self.daily = daily
        self.carry = 0
        self.excess = 0
        self.avail = 0
        self.spent = 0
        self.grants = [0] * HOURS
        self.accrued = -1
        self.exact = False


class Engine:
    """Pacing engine with live spend, true-up, and batch counters."""

    def __init__(self) -> None:
        self.camps: dict[str, Campaign] = {}
        self.created = 0
        self.requests = 0
        self.trueups = 0
        self.rejected = 0

    def handle(self, line: str) -> str:
        tokens = _tokenize(line)
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
        if hour <= c.accrued:
            return
        slice_ = (c.daily + c.carry) // HOURS
        for h in range(c.accrued + 1, hour + 1):
            if h == 0:
                inc = slice_ - c.excess
                if inc < 0:
                    inc = 0
                c.avail += inc
                c.excess = 0
            else:
                c.avail += slice_
        c.accrued = hour
        c.exact = False

    def _spend(self, tokens: list[str]) -> str:
        camp = tokens[1] if len(tokens) >= 2 else NO_CAMP
        if len(tokens) < 4:
            return f"N {camp} FMT"
        if not _valid_id(tokens[1]):
            return f"N {camp} FMT"
        hour_tok, req_tok = tokens[2], tokens[3]
        if not _digits(hour_tok) or not 1 <= len(hour_tok) <= 2:
            return f"N {camp} HOUR"
        hour = int(hour_tok)
        if hour > 23:
            return f"N {camp} HOUR"
        if not _digits(req_tok) or len(req_tok) > 7 or int(req_tok) < 1:
            return f"N {camp} REQ"
        req = int(req_tok)
        c = self._find(tokens[1])
        if c is None:
            return f"N {camp} FMT"
        if hour < c.accrued:
            return f"N {camp} ORDER"
        if hour == c.accrued and c.exact:
            return f"N {camp} REQ"
        self._accrue(c, hour)
        if c.daily >= LARGE_DAILY and hour <= LARGE_HOUR_MAX:
            effective = c.avail // 2
        else:
            effective = c.avail
        granted = req if req < effective else effective
        c.avail -= granted
        c.spent += granted
        c.grants[hour] += granted
        if req == effective:
            c.exact = True
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
        audit_slice = c.daily // HOURS
        total = 0
        for g in c.grants:
            total += g if g < audit_slice else audit_slice
        raw = c.daily - total
        if raw < 0:
            raw = 0
        cap = c.daily // CARRY_CAP_DIV
        carry = raw if raw < cap else cap
        c.carry = carry
        c.excess = raw - carry
        c.spent = 0
        c.grants = [0] * HOURS
        c.avail = 0
        c.accrued = -1
        c.exact = False
        self.trueups += 1
        return f"U {total} {carry}"


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = _strip_eol(raw)
        if line == "" and raw != line:
            # Line was empty or only an end-of-line marker.
            continue
        if line == "":
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
