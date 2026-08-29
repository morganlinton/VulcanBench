# TallyCore ranked-ballot tally protocol (spec v2.1, last updated 2016)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the election-night reporting stack that consumes these
> streams was built against the engine, not this file.

## Command stream

The engine reads one command per line on stdin and writes reply lines,
then a trailer at EOF. Blank lines are skipped. `C` and `V` produce
exactly one reply line; `W` produces several.

### `C <cand>` (register candidate)

`<cand>` is 1 to 8 alphanumeric characters, case-sensitive. Registers the
candidate at the end of the registration order; re-registering an already
registered candidate is a no-op. Reply: `OK <count>` where `<count>` is
the number of registered candidates after the command.

Registration closes once the first ballot of the current round is cast:
from then until the next `W`, any `C` replies `N <cand> LATE`. A
malformed register replies `N ???????? FMT`.

### `V <ranking>` (cast ballot)

`<ranking>` is a comma-joined list of candidate ids with no spaces,
ranking between 1 and all of the registered candidates, most preferred
first, no candidate more than once.

Validation order: structure first (`N ???????? FMT` when the ranking is
missing, an entry is empty, or an entry is not 1 to 8 alphanumeric
characters), then the entries left to right: an entry naming an
unregistered candidate replies `N <entry> CAND`; a repeated candidate
replies `N <entry> DUP`. A rejected ballot is not counted.

An accepted ballot replies `LEAD <cand>`: the current leader, the
candidate with the most FIRST-choice votes among the ballots cast so far
this round, ties broken in favor of the earliest registered candidate.

### `W` (recount and close the round)

Runs the instant-runoff recount over the round's ballots. Repeatedly
eliminate the candidate with the fewest current first-choice votes,
breaking ties by eliminating the LAST registered candidate first; each
eliminated candidate's ballots transfer to their next surviving ranked
choice, and ballots with no surviving choice are exhausted and drop out.
Every elimination emits one line `ELIM <cand>`, in elimination order;
when a single candidate remains the engine emits `WIN <cand>`.

`W` then starts a new round: the ballots are cleared, the candidates and
their registration order are kept, and registration reopens until the
new round's first ballot. `W` with no ballots cast replies
`N ???????? FMT`. `W` takes no arguments.

### Trailer

At EOF the engine writes `X <candidates> <ballots> <rounds> <rejected>`:
the number of registered candidates, the total number of accepted
ballots across all rounds, the number of completed recounts, and the
number of `N` replies of any kind.

## Reject replies

`N <cand> <code>` echoes the offending candidate id, or `????????` when
none is parseable. Codes: `FMT` (structure), `CAND` (unknown candidate in
a ranking), `DUP` (candidate repeated in a ranking), `LATE` (registration
after the round's first ballot).

## Determinism

Both tally paths are pure functions of the round's ballots: the running
leader after each `V`, and the recount ordering emitted by `W`, depend
only on the ballots cast in the current round and the registration
order. Recount rounds are independent: `W` clears all per-round state,
and identical ballot sets always produce identical `ELIM`/`WIN` output
regardless of what earlier rounds contained.
