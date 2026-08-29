# ReplayCore event-sourced ledger protocol (spec v1.4, last updated 2017)

> Maintenance note (2024): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the downstream reconciliation and settlement systems
> were built against the engine, not this file.

## Command stream

The ledger reads one command per line on stdin, writes its replies, and
writes a trailer at EOF. Blank lines are skipped. Any line that is not an
`E` or `Y` command replies `R ???????? FMT`.

### `E <acct> <delta> <seq>` (apply event)

| field | format |
|-------|--------|
| acct  | 1 to 8 alphanumeric characters |
| delta | a mandatory sign character `+` or `-`, then 1 to 8 digits; the value must be nonzero |
| seq   | 1 to 6 digits, the event's sequence number |

Reply on success: `A <acct> <balance>` with the account's new live
balance. Every account starts at balance 0. Balances are 64-bit integers.

Rejections reply `R <acct> <code>`, echoing the account token when it is
itself well formed and `????????` otherwise. Validation order (first
failure wins):

| code  | meaning |
|-------|---------|
| FMT   | the command does not have exactly the fields above, or the acct token is malformed |
| DELTA | the delta token is malformed, or its value is zero |
| SEQ   | the seq token is malformed |
| ORDER | the seq is not strictly greater than the account's last accepted seq |
| FLOOR | the delta would take the balance below 0 (balances floor at 0; the event is rejected whole, never partially applied) |

Accounts are independent: seq ordering and balances are tracked per
account. Account ids are case-sensitive.

## The event log and `Y` (replay)

Every ACCEPTED event is appended to an internal event log. Rejected
events are not logged. On `Y` the ledger rebuilds ALL account balances
from the log from scratch, in log order, and the replayed balances
REPLACE the live state. The reply is one line

    Y <acct> <balance>

per account that appears in the log, in the order each account first
appears there (first-accepted order), followed by

    YEND <accounts>

with the number of account lines written. A `Y` with an empty log writes
only `YEND 0`.

Because only accepted events are logged and replay reapplies them under
the same rules, **replay is a no-op by construction**: the replayed
balances always equal the live balances, and `Y` is safe to run at any
time (the nightly reconciliation job depends on this).

## Trailer

At EOF the ledger writes `X <events> <replays> <rejected>`: counts of
accepted events (`A` replies), replays (`Y` commands), and rejections
(`R` replies).
