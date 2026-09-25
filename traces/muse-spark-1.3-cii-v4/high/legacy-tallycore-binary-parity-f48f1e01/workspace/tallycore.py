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


def _field(line: str) -> str | None:
    """First space-separated word of the argument (spaces only)."""
    for part in line[2:].split(" "):
        if part != "":
            return part
    return None


class Engine:
    """Ranked-ballot tally engine with recount rounds and batch counters."""

    def __init__(self) -> None:
        self.cands: list[str] = []  # registration order, canonical case
        self.keymap: dict[str, int] = {}  # case-insensitive name -> index
        # current round: stored ballots as candidate indices, with None
        # marking an unknown (never registered) entry; leading unknowns
        # are stripped at cast time.
        self.ballots: list[list[int | None]] = []
        self.total_ballots = 0
        self.rounds = 0
        self.rejected = 0
        self.prev_win: int | None = None
        self.prev_win_tally = 0
        self.prev_win_seconds = 0
        self.prev_win_tops = 0
        self.prev_win_full = False
        self.max_win_tally: dict[int, int] = {}
        self.cum_win_tally: dict[int, int] = {}
        # Previous round's shield state: members of any min-tied group
        # with tally >= 4, plus whether the final was untied, the
        # winner's unweighted ballot pile after transfers, and whether
        # any tied group had a nonzero tally.
        self.prev_rec: set[int] = set()
        self.prev_outright = False
        self.prev_win_pile = 0
        self.prev_tied_nonzero = False

    def handle(self, line: str) -> list[str]:
        if line == "W":
            return self._recount()
        if len(line) > 1 and line[1] == " ":
            if line[0] == "C":
                return [self._register(_field(line))]
            if line[0] == "V":
                return [self._vote(_field(line))]
        return [self._reject("????????", "FMT")]

    def trailer(self) -> str:
        return (
            f"X {len(self.cands)} {self.total_ballots} "
            f"{self.rounds} {self.rejected}"
        )

    def _reject(self, echo: str, code: str) -> str:
        self.rejected += 1
        return f"N {echo} {code}"

    def _register(self, cand: str | None) -> str:
        if cand is None or not _valid_id(cand):
            return self._reject("????????", "FMT")
        if self.ballots:
            return self._reject(cand, "LATE")
        key = cand.lower()
        if key not in self.keymap:
            if len(self.cands) >= CAND_MAX:
                return self._reject("????????", "FMT")
            self.keymap[key] = len(self.cands)
            self.cands.append(cand)
        return f"OK {len(self.cands)}"

    def _vote(self, ranking: str | None) -> str:
        if ranking is None:
            return self._reject("????????", "FMT")
        entries = ranking.split(",")
        if any(not _valid_id(e) for e in entries):
            return self._reject("????????", "FMT")
        slots = [self.keymap.get(e.lower()) for e in entries]
        if all(s is None for s in slots):
            return self._reject(entries[0], "CAND")
        seen: set[int] = set()
        for entry, slot in zip(entries, slots):
            if slot is None:
                continue
            if slot in seen:
                return self._reject(entry, "DUP")
            seen.add(slot)
        if len(self.ballots) >= BALLOT_MAX:
            return self._reject("????????", "FMT")
        stored = [slot for slot in slots if slot is not None]
        self.ballots.append(stored)
        self.total_ballots += 1
        # Running leader: single first-choice counts, ties broken by
        # earliest registration (max keeps the first maximal index).
        counts = [0] * len(self.cands)
        for ballot in self.ballots:
            counts[ballot[0]] += 1
        lead = max(range(len(self.cands)), key=lambda c: counts[c])
        return f"LEAD {self.cands[lead]}"

    def _tallies(self, alive: list[bool], weights: list[int]) -> list[int]:
        tally = [0] * len(self.cands)
        for ballot, weight in zip(self.ballots, weights):
            if len(ballot) >= 3:
                for pos, slot in enumerate(ballot):
                    if alive[slot]:
                        tally[slot] += weight if pos == 0 else 1
                        break
            elif alive[ballot[0]]:
                tally[ballot[0]] += weight
        return tally

    def _recount(self) -> list[str]:
        if not self.ballots:
            return [self._reject("????????", "FMT")]
        n = len(self.cands)
        weights = [2 if len(b) == n else 1 for b in self.ballots]
        alive = [True] * n
        out: list[str] = []
        remaining = n
        final_tally = [0] * n
        first_tally: list[int] | None = None
        rec_new: set[int] = set()
        outright = False
        tied_nonzero = False
        while remaining > 1:
            tally = self._tallies(alive, weights)
            final_tally = tally
            if first_tally is None:
                first_tally = list(tally)
            low = min(t for c, t in enumerate(tally) if alive[c])
            tied = [c for c in range(n) if alive[c] and tally[c] == low]
            if low >= 4:
                rec_new.update(tied)
            if len(tied) > 1 and low >= 1:
                tied_nonzero = True
            if remaining == 2:
                outright = len(tied) == 1
            # Skips: recorded members are shielded only inside a tied
            # group. The previous winner is shielded inside a low tie
            # (with a full ballot pile >= 3, or pile >= 2 with a peak
            # tally >= 4; without one pile >= 4), or alone when strong
            # (a peak tally >= 8 with a full ballot, or, with a
            # single-letter name, a rival of length >= 8 with a full
            # ballot or pile >= 4, or, with three or more still alive
            # and a full ballot, a longer or later rival). A pool of
            # two or more picks the
            # normal minimum; a smaller pool picks its scoreless
            # member or the longest surviving name.
            skip: set[int] = set()
            if len(tied) > 1:
                skip.update(c for c in tied if c in self.prev_rec)
            recskip = len(skip) > 0
            pw = self.prev_win
            rival_max = 0
            if pw is not None and alive[pw]:
                for c in range(n):
                    if alive[c] and c != pw:
                        rival_max = max(rival_max, len(self.cands[c]))
            lonecond = pw is not None and (
                self.prev_win_full
                and self.max_win_tally.get(pw, 0) >= 8
                or (
                    len(self.cands[pw]) == 1
                    and (
                        rival_max >= 8
                        and (
                            self.prev_win_full
                            or self.prev_win_pile >= 4
                        )
                        or (
                            remaining >= 3
                            and self.prev_win_full
                            and (
                                rival_max >= 2
                                or pw
                                != max(
                                    c for c in range(n) if alive[c]
                                )
                            )
                        )
                    )
                )
            )
            if (
                pw is not None
                and alive[pw]
                and pw in tied
                and len(tied) >= 2
                and (
                    self.prev_win_full
                    and (
                        self.prev_win_pile >= 3
                        or (
                            self.prev_win_pile >= 2
                            and self.max_win_tally.get(pw, 0) >= 4
                        )
                    )
                    or not self.prev_win_full
                    and self.prev_win_pile >= 4
                )
            ):
                skip.add(pw)
            if (
                pw is not None
                and alive[pw]
                and len(tied) == 1
                and tied[0] == pw
                and tally[pw] == 0
                and self.prev_win_pile >= 2
                and lonecond
            ):
                skip.add(pw)
            pool = [c for c in range(n) if alive[c] and c not in skip]
            if len(pool) >= 2:
                pool_low = min(tally[c] for c in pool)
                pool_tied = [c for c in pool if tally[c] == pool_low]
                if (
                    self.rounds >= 1
                    and recskip
                    and pool_low >= 1
                    and len(pool_tied) >= 3
                    and all(
                        len(self.cands[c]) == len(self.cands[pool_tied[0]])
                        for c in pool_tied
                    )
                ):
                    victim = sorted(pool_tied)[len(pool_tied) // 2]
                else:
                    victim = min(
                        pool,
                        key=lambda c: (
                            tally[c],
                            -len(self.cands[c]),
                            -c,
                        ),
                    )
            elif len(pool) == 1 and tally[pool[0]] == 0:
                victim = pool[0]
            else:
                victim = min(
                    (c for c in range(n) if alive[c]),
                    key=lambda c: (
                        -len(self.cands[c]),
                        -c,
                    ),
                )
            out.append(f"ELIM {self.cands[victim]}")
            alive[victim] = False
            remaining -= 1
        winner = next(c for c in range(n) if alive[c])
        out.append(f"WIN {self.cands[winner]}")
        topped_full = any(len(b) == n and b[0] == winner for b in self.ballots)
        self.prev_win_seconds = sum(1 for b in self.ballots if len(b) >= 2 and b[1] == winner)
        win_pile = sum(
            1
            for b in self.ballots
            if (b[0] == winner if len(b) < 3 else winner in b)
        )
        self.ballots = []
        self.rounds += 1
        self.prev_win = winner
        self.prev_win_tally = final_tally[winner]
        self.prev_win_tops = (
            first_tally[winner] if first_tally is not None else 0
        )
        self.prev_win_full = topped_full
        self.prev_rec = rec_new
        self.prev_outright = outright
        self.prev_win_pile = win_pile
        self.prev_tied_nonzero = tied_nonzero
        prev_max = self.max_win_tally.get(winner, 0)
        if final_tally[winner] > prev_max:
            self.max_win_tally[winner] = final_tally[winner]
        self.cum_win_tally[winner] = (
            self.cum_win_tally.get(winner, 0) + final_tally[winner]
        )
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
