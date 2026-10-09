# PaceCore pacing protocol (spec v2.1, last updated 2018)

> Maintenance note (2024): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the ad servers and billing reconcilers that talk to
> PaceCore were built against the engine, not this file.

## Command stream

The engine reads one command per line on stdin and writes exactly one
reply line per command, then a trailer at EOF. Blank lines are skipped.
All money amounts are integer cents; all arithmetic is integer (floor
division).

### `C <camp> <daily>` (create campaign)

| field | format |
|-------|--------|
| camp  | 1 to 8 alphanumeric characters, case-sensitive |
| daily | daily budget in cents, 4 to 9 digits as given |

Reply: `OK <count>` where `<count>` is the number of campaigns after the
command. Creating a campaign whose id already exists replies
`N <camp> DUP`. A malformed id replies `N <camp> FMT`; a daily token
that is not 4 to 9 digits replies `N <camp> BUDGET`. Commands carry
exactly the documented number of tokens; extra tokens are `N <camp> FMT`.

### `S <camp> <hour> <req>` (spend request)

| field | format |
|-------|--------|
| hour  | exactly 2 digits, `00` to `23` |
| req   | requested cents, 1 to 7 digits, at least 1 |

Grants `min(req, available allowance)` and replies `P <granted>`.

The hourly allowance is rolling within the day: each hour contributes a
slice of `effective_budget / 24` (integer floor), and unspent allowance
from earlier hours of the same day remains available. The effective
budget is the daily budget plus yesterday's carry (see `U`). When a
request arrives for hour `h`, every not-yet-accrued hour up to and
including `h` accrues its slice first.

Hours must be nondecreasing per campaign; a request for an earlier hour
than the campaign's current hour replies `N <camp> ORDER`. Multiple
requests in the same hour are allowed and share the allowance; when
nothing is left, further requests simply grant 0.

Validation order: `FMT` (arity, campaign id syntax), `HOUR`, `REQ`,
then unknown campaign (`FMT`), then `ORDER`.

### `U <camp>` (end-of-day true-up)

Recomputes the day from the engine's internal ledger and closes it.
Reply: `U <total_spent> <carry>` where `<total_spent>` is the sum of the
day's grants and `<carry>` is the unspent budget carried into tomorrow:

    carry = max(0, daily - total_spent), capped at 20 percent of daily

The next day starts at hour `00` with an effective budget of
`daily + carry`. A true-up for an unknown or malformed campaign replies
`N <camp> FMT`.

### Rejects

Reject replies are `N <camp> <code>`, echoing the line's campaign token
(or `????????` when the line has none, including unrecognized command
letters). Codes: `FMT`, `BUDGET`, `HOUR`, `REQ`, `ORDER`, `DUP`.
Rejected commands change no state.

### Trailer

At EOF the engine writes `X <campaigns> <requests> <trueups> <rejected>`:
counts of successful `C` commands, successful `S` commands (including
`P 0` grants), successful `U` commands, and `N` replies of any kind.

## Reconciliation invariant

For any single day, the true-up total equals the sum of that day's `P`
grants, and `daily + carry_in = total_spent + unspent`, with the carry
the capped unspent amount. The billing reconcilers depend on the `U`
line agreeing with the granted amounts they logged live.
