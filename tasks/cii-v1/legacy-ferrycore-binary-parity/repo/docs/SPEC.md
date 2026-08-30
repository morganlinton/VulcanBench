# FerryCore boarding protocol (spec v2.1, last updated 2016)

> Maintenance note (2022): this document is known to have drifted from the
> production controller in places. Where they disagree, **the controller's
> behavior is the contract**; the slipway terminals and season reconcilers
> that talk to FerryCore were built against the controller, not this file.

## Command stream

The controller reads one command per line on stdin and writes the reply
lines for each command, then a trailer at EOF. Blank lines are skipped.
Token counts are strict: a line with missing or extra tokens is rejected
`FMT`. All arithmetic is integer.

### `V <vehicle> <span>` (register vehicle)

| field   | format |
|---------|--------|
| vehicle | 1 to 8 alphanumeric characters, case-sensitive |
| span    | deck units, 1 to 2 digits, value 1 to 20 |

Reply: `OK <count>` where `<count>` is the number of registered vehicles
after the command. Registering an id that already exists replies
`N <vehicle> DUP`. A malformed id replies `N <vehicle> FMT`; a span token
that is not 1 to 2 digits with a value of 1 to 20 replies
`N <vehicle> SPAN`.

### `Q <vehicle>` (join the quay queue)

The vehicle lines up for the next sailing. Reply: `Q <vehicle>`. An
unregistered vehicle replies `N <vehicle> UNKNOWN`; a vehicle that is
already waiting in the queue, or is aboard from the latest sailing,
replies `N <vehicle> QUEUED`.

Validation order everywhere: `FMT` (token count, id syntax), then the
field check (`SPAN`), then `DUP` / `UNKNOWN` / `QUEUED`.

### `G` (sailing)

The ferry boards waiting vehicles in **descending standing-lean order**
(ties by registration order) while the total span aboard stays within
the deck's 40 units, skipping any vehicle that does not fit and
continuing down the list. Vehicles aboard the previous sailing disembark
first; they may join the queue again after the next sailing departs.

Output: one `G <vehicle>` line per boarded vehicle, in boarding order,
then `GEND <boarded> <left>` with the number boarded and the number left
waiting. When no vehicles are waiting, the sailing departs empty and
nothing is printed.

### Standing lean

Every vehicle carries a standing lean, an internal bookkeeping value
that is never printed. It starts at 0 at registration, **rises by the
vehicle's span** each time the vehicle is left behind at a sailing, and
**resets to 0** when the vehicle boards. Lean decides boarding order;
nothing else feeds it.

### `K` (seasonal squaring)

Recomputes every vehicle's standing lean from the full season ledger
(the record of registrations, joins, and sailings), replacing the live
values, and reports `KOK <count>` with the number of vehicles carrying
positive lean. Since the live accrual follows exactly the ledger rules
above, the recomputation reproduces the running values: the squaring
squares the book, it never tilts it.

### Rejects

Reject replies are `N <vehicle> <code>`, echoing the line's vehicle
token (or `????????` when the line has no usable id token, including
unrecognized commands). Codes: `FMT`, `SPAN`, `DUP`, `UNKNOWN`,
`QUEUED`. Rejected commands change no state.

### Trailer

At EOF the controller writes
`X <vehicles> <queued-now> <sailings> <squarings> <rejected>`: the
number of registered vehicles, the number waiting in the queue at EOF,
counts of `G` and `K` commands, and `N` replies of any kind.

## Fairness invariant

Boarding order depends only on standing lean, registration order, and
fit. A vehicle that keeps missing sailings rises in the order by exactly
its span per miss, so no vehicle waits forever; the season reconcilers
depend on the squaring's `KOK` count matching the number of vehicles the
live book shows waiting with positive lean.
