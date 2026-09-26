# StampCore metering protocol (spec v1.4, last updated 2017)

> Maintenance note (2023): this document is known to have drifted from the
> production controller in places. Where they disagree, **the controller's
> behavior is the contract**; the mailroom terminals and postage
> reconcilers that talk to StampCore were built against the controller,
> not this file.

## Command stream

The controller reads one command per line on stdin and writes exactly one
reply line per command, then a trailer at EOF. Blank lines are skipped.
All amounts are integer tenths of a cent; all arithmetic is integer.

### `M <meter> <balance>` (register meter)

| field   | format |
|---------|--------|
| meter   | 1 to 8 alphanumeric characters, case-sensitive |
| balance | prepaid balance in tenths, 4 to 9 digits, at least 1000 |

Reply: `OK <count>` where `<count>` is the number of meters after the
command. Registering a meter whose id already exists replies
`N <meter> DUP`. A malformed id replies `N <meter> FMT`; a balance token
that is not 4 to 9 digits with a value of at least 1000 replies
`N <meter> BAL`. Commands carry exactly the documented number of tokens;
extra tokens are `N <meter> FMT`.

### `F <meter> <postage>` (frank a mailpiece)

| field   | format |
|---------|--------|
| postage | tenths, 2 to 6 digits, at least 10 |

Deducts exactly the postage from the meter's balance and replies
`P <meter> <remaining>` with the balance after the deduction. When the
balance cannot cover the postage the reply is `N <meter> LOW` and
nothing is deducted.

Validation order: `FMT` (arity, meter id syntax), `POST`, then unknown
meter (`UNKNOWN`), then `LOW`.

### `R <meter> <returned>` (postal return credit)

| field    | format |
|----------|--------|
| returned | pieces returned, 1 to 4 digits, at least 1 |

Mailpieces that came back undeliverable are credited at the meter's
**last franked postage**: the credit is `returned * last_postage`,
added back to the balance. Reply: `K <meter> <credit>` with the credit
granted in tenths.

A return of more pieces than the meter has franked since its last
return replies `N <meter> RET`; you cannot return more than was
franked. Validation order: `FMT`, `RET` (token syntax), unknown meter
(`UNKNOWN`), then the over-return check (`RET`).

### `Z <meter>` (zero-reading audit)

Recomputes the meter's expected balance from its ledger:

    expected = initial_balance - total_franked + total_credits

and compares it against the meter's reading. Reply: `Z <meter> MATCH`
when they agree, `Z <meter> DRIFT <delta>` with the shortfall when they
do not. Audits are repeatable and change nothing; a healthy meter
always MATCHes, since franking deducts exactly the postage and returns
credit exactly what they report.

### Rejects

Reject replies are `N <meter> <code>`, echoing the line's meter token
(or `????????` when the line has none, including unrecognized command
letters). Codes: `FMT`, `BAL`, `POST`, `RET`, `DUP`, `UNKNOWN`, `LOW`.
Rejected commands change no state.

### Trailer

At EOF the controller writes
`X <meters> <franks> <returns> <audits> <rejected>`: counts of
successful `M`, `F`, `R`, and `Z` commands, and `N` replies of any
kind.

## Reconciliation invariant

At any point, `balance = initial - total_franked + total_credits`. The
postage reconcilers depend on the `Z` audit line always reporting
`MATCH` for meters that have only franked and credited through this
protocol; a `DRIFT` line indicates hardware tampering and opens a
manual investigation.
