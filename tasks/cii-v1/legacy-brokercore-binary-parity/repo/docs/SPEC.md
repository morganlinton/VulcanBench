# BrokerCore load-board protocol (spec v2.1, last updated 2015)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the dispatcher terminals and settlement clerks' batch
> tools that talk to BrokerCore were built against the engine, not this
> file.

## Command stream

The controller reads one command per line on stdin and writes one or more
reply lines per command, then a trailer at EOF. Blank lines are skipped.
All load values are integer cents; all arithmetic is integer. Commands
carry exactly the documented number of tokens; extra tokens are
`N <id> FMT`.

### `C <carrier> <rating>` (register carrier)

| field   | format |
|---------|--------|
| carrier | 1 to 8 alphanumeric characters, case-sensitive |
| rating  | 1 to 3 digits, value 1 to 100 |

Registers a carrier on the board. The carrier's commitment score starts
at `rating * 100`. Reply: `OK <count>` with the number of carriers after
the command. Registering an existing carrier id replies `N <carrier>
DUP`. A malformed id replies `N <carrier> FMT`; a rating token that is
not 1 to 3 digits with a value of 1 to 100 replies `N <carrier> RATING`.

### `L <load> <value>` (post a load)

| field | format |
|-------|--------|
| load  | 1 to 8 alphanumeric characters, case-sensitive |
| value | cents, 3 to 8 digits, at least 100 |

Posts a load on the board. Reply: `OK <count>` with the number of loads
after the command. A duplicate load id replies `N <load> DUP`; a bad
value token replies `N <load> VALUE`.

### `B <carrier> <load>` (book)

Books an open load to a carrier and adds `value / 1000` (integer
division) to the carrier's commitment score. Reply: `A <carrier>
<score>` with the carrier's running commitment score after the booking.
An unknown carrier or load replies `N <carrier> UNKNOWN` (the carrier is
checked first); a load that is already booked replies `N <carrier>
TAKEN`.

Validation order: `FMT` (arity, id syntax), then `UNKNOWN`, then
`TAKEN`.

### `D <carrier> <load>` (drop)

Drops a load the carrier has booked and subtracts **twice what the
booking added** from the carrier's score: a drop costs the carrier the
booking's credit plus an equal forfeit. Reply: `A <carrier> <score>`.
The dropped load returns to the board and may be booked again by any
carrier, including the one that dropped it. An unknown carrier or load
replies `N <carrier> UNKNOWN`; a load not currently booked by this
carrier replies `N <carrier> NOBOOK`.

### `W` (weekly settlement)

Recomputes every carrier's commitment score from the event ledger with
the same arithmetic as the live path:

    settled = rating * 100 + sum(value / 1000 over bookings)
                           - sum(2 * value / 1000 over drops)

and writes one `S <carrier> <score>` line per carrier in registration
order, followed by `WEND <count>` with the number of carriers. The
settled score is written back to the carrier, which is a no-op for a
healthy board: **the settlement is a checksum, not a correction**, and
always equals the running score. The ledger is not cleared; settlements
are repeatable. A settlement on an empty board settles nothing and
writes nothing. `W` takes no arguments.

### Rejects

Reject replies are `N <id> <code>`, echoing the line's first id token
(or `????????` when the line has none, including unrecognized command
letters). Codes: `FMT`, `RATING`, `VALUE`, `DUP`, `UNKNOWN`, `TAKEN`,
`NOBOOK`. Rejected commands change no state.

### Trailer

At EOF the controller writes `X <carriers> <loads> <bookings> <drops>
<settlements> <rejected>`: counts of successful `C`, `L`, `B`, `D`, and
`W` commands, and `N` replies of any kind.

## Settlement invariant

At any point, a carrier's running score equals `rating * 100` plus the
booking credits minus the drop forfeits, and the weekly `S` lines simply
restate it. The settlement clerks depend on the `S` lines agreeing with
the booking stream the dispatchers logged live.
