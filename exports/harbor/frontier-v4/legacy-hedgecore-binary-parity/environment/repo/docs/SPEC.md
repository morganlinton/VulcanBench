# HedgeCore FX position ledger format (spec v3.1, last updated 2016)

> Maintenance note (2022): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the marking and reconciliation systems downstream were
> built against the engine, not this file.

One trading session per process. The ledger reads one command per line on
stdin, writes reply lines as described below, then a trailer at end of
input. Blank lines are skipped.

## Books, positions, and rates

The ledger tracks any number of books. Each book holds, per currency
pair, a signed position (buys add, sells subtract) and the pair's last
trade rate. Rates are quoted in tenths of a pip (integers). Book ids are
case-sensitive; replies echo the id as given.

A book's **net value**, in cents, is the sum over its pairs of

    position * rate / 10000

each pair's value rounded to the nearest cent (banker's rounding on
exact halves), then summed. A pair whose position returns to zero
remains on the book with its last rate (it simply contributes nothing
until it is traded again).

## Commands

### `T <book> <pair> <side> <amount> <rate>` (trade)

| field  | format |
|--------|--------|
| book   | 1 to 8 alphanumeric characters, case-sensitive |
| pair   | exactly 6 uppercase letters, e.g. `EURUSD` |
| side   | `B` (buy) or `S` (sell) |
| amount | 1 to 8 digits; must be greater than zero |
| rate   | 1 to 7 digits, in tenths of a pip; must be greater than zero |

Applies the trade to the book (creating the book or the pair position as
needed), updates the pair's last rate to the trade's rate, and replies

    P <book> <netvalue>

with the book's net value after the trade.

Validation order and reject codes: `FMT` (command shape, token count, or
an unusable book id, echoed `????????`), then `PAIR`, `SIDE`, `AMT`
(including a zero amount), then `RATE` (including a zero rate). Rejects
reply `R <book> <code>`.

### `V <rate-list>` (revaluation)

Everything after `V ` is the fixing list, taken verbatim (it is never
split on spaces): comma-joined `PAIR=rate` entries with no spaces, each
pair 6 uppercase letters, each rate 1 to 7 digits and greater than zero.
Any damage to the list replies `R ???????? FMT` and changes nothing.

A valid revaluation updates EVERY book: each held pair that appears in
the list takes the list's rate as its new last rate; pairs absent from
the list keep their last rate. The ledger then replies one

    P <book> <netvalue>

line per book, in the order the books were first seen, using the same
net-value formula as trades with the updated rates.

### Trailer

At end of input the ledger writes `X <trades> <revals> <rejected>`:
counts of applied `T` commands, successful `V` commands, and `R` replies
of any kind.
