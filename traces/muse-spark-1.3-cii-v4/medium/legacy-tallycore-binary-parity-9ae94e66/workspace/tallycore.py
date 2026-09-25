"""TallyCore ranked-ballot tally engine, Python implementation.

Replaces the retired legacy binary (see ``legacy/README.md``). Reads tally
commands on stdin (``C`` registers a candidate, ``V`` casts a ranked
ballot and reports the running leader, ``W`` runs the instant-runoff
recount and starts a new round), writes one reply per command and an
``X`` trailer at EOF. Format reference: ``docs/SPEC.md`` (note the drift
warning at the top of that file; the legacy engine's behavior is the
contract).
"""

from __future__ import annotations

import sys

ID_MAX = 8
CAND_MAX = 64
BALLOT_MAX = 4096


def _valid_id(token: str) -> bool:
    return 1 <= len(token) <= ID_MAX and token.isascii() and token.isalnum()


class Engine:
    """Ranked-ballot tally engine with recount rounds and batch counters."""

    def __init__(self) -> None:
        self.cands: list[str] = []  # registration order, first-seen spelling
        self.keys: list[str] = []  # lowercased registration order
        self.ballots: list[list[int]] = []  # all ballots cast, by round
        self.window: list[list[int]] = []  # ballots since last recount
        self.hist: list[int] = []  # previous round's final-step tallies
        self.prevs1: list[int] = []  # previous round's first-step tallies
        self.winner: int = -1  # previous round's winner, -1 when unknown
        self.total_ballots = 0
        self.rounds = 0
        self.rejected = 0
        self._new_since_recount = False

    def _index(self, entry: str) -> int:
        try:
            return self.keys.index(entry.lower())
        except ValueError:
            return -1

    def handle(self, line: str) -> list[str]:
        if line == "W":
            return self._recount()
        if line == "C" or line.startswith("C "):
            return [self._register(line[1:].split())]
        if line == "V" or line.startswith("V "):
            return [self._vote(line[1:].split())]
        return [self._reject("????????", "FMT")]

    def trailer(self) -> str:
        return (
            f"X {len(self.cands)} {self.total_ballots} "
            f"{self.rounds} {self.rejected}"
        )

    def _reject(self, echo: str, code: str) -> str:
        self.rejected += 1
        return f"N {echo} {code}"

    def _register(self, parts: list[str]) -> str:
        # Extra tokens after the candidate are ignored; only parts[0] counts.
        if not parts:
            return self._reject("????????", "FMT")
        cand = parts[0]
        if not _valid_id(cand):
            return self._reject("????????", "FMT")
        if self.window:
            return self._reject(cand, "LATE")
        if self._index(cand) < 0:
            if len(self.cands) >= CAND_MAX:
                return self._reject("????????", "FMT")
            self.cands.append(cand)
            self.keys.append(cand.lower())
            self.hist.append(0)
            self.prevs1.append(0)
        return f"OK {len(self.cands)}"

    def _vote(self, parts: list[str]) -> str:
        # Extra tokens after the ranking are ignored; only parts[0] counts.
        if not parts:
            return self._reject("????????", "FMT")
        entries = parts[0].split(",")
        if any(not _valid_id(e) for e in entries):
            return self._reject("????????", "FMT")
        kept: list[int] = []
        for entry in entries:
            c = self._index(entry)
            if c < 0:
                continue  # unknown entries are dropped, not rejected
            if c in kept:
                # A duplicate entry voids the whole ballot.
                return self._reject(entry, "DUP")
            kept.append(c)
        if not kept:
            return self._reject(entries[0], "CAND")
        if len(self.ballots) >= BALLOT_MAX:
            return self._reject("????????", "FMT")
        self.ballots.append(kept)
        self.window.append(kept)
        self.total_ballots += 1
        self._new_since_recount = True
        # Running leader: first-choice counts since the last recount;
        # ties go to the earliest registered candidate.
        counts = [0] * len(self.cands)
        for ballot in self.window:
            counts[ballot[0]] += 1
        best = max(counts)
        lead = min(c for c in range(len(self.cands)) if counts[c] == best)
        return f"LEAD {self.cands[lead]}"

    def _base_tallies(self, alive: list[bool], n: int) -> list[int]:
        # Current-round votes only: full ballots count double for a live
        # top; longer ballots transfer a single vote to the next
        # surviving choice; short ballots die with their top pick.
        tally = [0] * len(self.cands)
        for ballot in self.ballots:
            if alive[ballot[0]]:
                top = ballot[0]
                tally[top] += 2 if len(ballot) == n else 1
            elif len(ballot) >= 3:
                for c in ballot[1:]:
                    if alive[c]:
                        tally[c] += 1
                        break
        return tally

    def _eff_hist(self, c: int) -> int:
        # Previous final votes, capped at 4 for short names.
        h = self.hist[c]
        if len(self.cands[c]) <= 3 and h > 4:
            h = 4
        return h

    def _adjust(self, c: int, step: int, final: bool, base: int) -> int:
        # Incumbency adjustment for the current recount. The opening
        # step banks the previous winner's final votes beyond 3 and
        # charges everyone else their previous opener. A round-2
        # final settles up the previous final against the opener
        # for everyone. Later winners pay back: the previous final
        # (capped at 6) at the opening step, a flat 3 at the second
        # step, the whole final after that. A round-2 finalist pays
        # back four fewer than the previous final when they won it,
        # nothing otherwise.
        if self.rounds == 0:
            return 0
        if final and self.rounds == 1:
            if c == self.winner:
                return -self.hist[c]
            return base - self.prevs1[c] - self.hist[c] - 1
        if c == self.winner:
            if self.rounds == 1:
                return -self.hist[c] + (1 if base < self.hist[c] else 0)
            if step == 1:
                return -min(self.hist[c], 6)
            if step == 2:
                return -3
            return -self.hist[c]
        return -self.prevs1[c]

    def _tallies(
        self, alive: list[bool], n: int, step: int, final: bool
    ) -> list[int]:
        tally = self._base_tallies(alive, n)
        for c in range(len(self.cands)):
            if alive[c]:
                tally[c] += self._adjust(c, step, final, tally[c])
        return tally

    def _pick_victim(
        self, alive: list[bool], tally: list[int], step: int, final: bool
    ) -> int:
        # Fewest votes goes. Equal lengths break toward the latest
        # registered candidate, except above zero past the opening
        # step of a later recount, which breaks toward the
        # earliest. Uneven lengths eliminate
        # the longest name, except a nonzero two-way tie at the
        # second step, which breaks toward the earliest registered
        # candidate; longest ties fall back to registration order.
        # A final step from the third recount on just eliminates the
        # longest name (ties to the earliest registered candidate).
        tied = [c for c in range(len(self.cands)) if alive[c]]
        if final and self.rounds >= 2:
            low = min(tally[c] for c in tied)
            lowt = [c for c in tied if tally[c] == low]
            if len(lowt) == 1:
                return lowt[0]
            long = max(len(self.cands[c]) for c in lowt)
            lowt = [c for c in lowt if len(self.cands[c]) == long]
            return min(lowt)
        low = min(tally[c] for c in tied)
        tied = [c for c in tied if tally[c] == low]
        if len(tied) == 1:
            return tied[0]
        lens = {len(self.cands[c]) for c in tied}
        if len(lens) == 1:
            if (
                low == 0
                or step == 1
                or self.rounds == 0
                or not all(len(self.cands[c]) == 3 for c in tied)
            ):
                return max(tied)
            return min(tied)
        long = max(len(self.cands[c]) for c in tied)
        tied = [c for c in tied if len(self.cands[c]) == long]
        return max(tied)

    def _recount(self) -> list[str]:
        if not self._new_since_recount:
            return [self._reject("????????", "FMT")]
        n = len(self.cands)
        alive = [True] * n
        out: list[str] = []
        remaining = n
        step = 0
        s1tally = self._base_tallies(alive, n)
        final_alive: list[bool] | None = None
        while remaining > 1:
            step += 1
            tally = self._tallies(alive, n, step, remaining == 2)
            if step == 1:
                s1tally = list(tally)
            if remaining == 2 and final_alive is None:
                final_alive = list(alive)
            victim = self._pick_victim(alive, tally, step, remaining == 2)
            out.append(f"ELIM {self.cands[victim]}")
            alive[victim] = False
            remaining -= 1
        winner = next(c for c in range(n) if alive[c])
        out.append(f"WIN {self.cands[winner]}")
        # Next round's establishment is the fresh final two-candidate
        # tally (full-length doubles count; adjustments excluded);
        # with a single candidate, no step ran.
        if final_alive is not None:
            self.hist = self._base_tallies(final_alive, n)
        self.prevs1 = s1tally
        self.winner = winner
        self.window = []
        self.rounds += 1
        self._new_since_recount = False
        return out


def process(lines: list[str], out=None) -> list[str]:
    engine = Engine()
    replies = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if not line:
            continue
        replies.extend(engine.handle(line))
    replies.append(engine.trailer())
    if out is not None:
        for reply in replies:
            print(reply, file=out)
    return replies


def main() -> None:
    process(sys.stdin.readlines(), out=sys.stdout)


if __name__ == "__main__":
    main()
