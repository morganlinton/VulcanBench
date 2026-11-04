# MeterCore batch billing format (spec v2.1, last updated 2018)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; downstream consumers were built against the engine,
> not this file.

## Input readings (one per line, space-separated tokens)

```
M <acct> <month> <kwh> <band> <meter>
```

| token | field | format |
|-------|-------|--------|
| 1     | tag   | literal `M` |
| 2     | acct  | 1-8 alphanumeric characters, case-sensitive |
| 3     | month | two digits, `01` .. `12` |
| 4     | kwh   | 1-6 digits, whole kwh; `0` is a valid no-op reading |
| 5     | band  | `D` (day), `N` (night), `W` (weekend) |
| 6     | meter | `S` (standard), `L` (legacy analog) |

Exactly six tokens per line. Blank lines are ignored.

## Rejects

An invalid reading is rejected immediately, in input order:

```
R <acct> <code>
```

`acct` echoes the line's second token (`?` when the line has fewer than
two tokens). Codes, checked in this order: `FMT` (tag, token count, or
account), `MONTH`, `KWH`, `BAND`, `METER`. Rejected readings do not
accumulate.

## Billing model

Usage accumulates **per account within the batch**; tier boundaries apply
to the account's cumulative kwh, so a reading is split across the tiers
its span covers:

| tier | cumulative kwh | rate |
|------|----------------|------|
| 1    | first 500      | 14 c/kwh |
| 2    | next 1500 (500 to 2000) | 19 c/kwh |
| 3    | above 2000     | 26 c/kwh |

Band multipliers apply to the reading's energy cost: `D` 100%, `N` 70%,
`W` 85%.

Months `06` to `09` are summer: tier-3 energy carries a +10% surcharge.

Each reading's cost is rounded to the nearest cent and added to the
account's bill. The meter type is informational and does not affect the
bill.

## Batch output

At end of input, one line per account in first-seen order:

```
B <acct> <total_kwh> <bill_cents>
```

`total_kwh` is the account's summed kwh; `bill_cents` is the account's
total bill in whole cents. Then a single trailer:

```
X <accounts> <rejected> <grand_total>
```

`accounts` is the number of B lines, `rejected` the number of R lines,
`grand_total` the sum of all `bill_cents`.
