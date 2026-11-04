# SettleCore batch settlement format (spec v1.4, last updated 2019)

> Maintenance note (2024): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; downstream consumers were built against the engine,
> not this file.

## Input records (80 characters, one per line)

| cols  | field    | format |
|-------|----------|--------|
| 0-1   | type     | `ST` (settlement) or `RF` (refund) |
| 2-11  | account  | alphanumeric, space-padded |
| 12-23 | amount   | sign (`+`/`-`) then 11 digits, zero-padded, in cents |
| 24-31 | date     | `YYYYMMDD` |
| 32-34 | currency | `USD`, `EUR`, `GBP`, `JPY` |
| 35-38 | tier     | `T001` .. `T009` |
| 39-42 | mcc      | 4-digit merchant category code |
| 43-77 | memo     | free text, space-padded |
| 78    | reserved | space |
| 79    | check    | check character (below) |

## Check character

Weighted byte sum over columns 0-77 (column 78, the reserved byte, is
excluded), weights cycling `1, 3, 7`, modulo 36, encoded `0-9A-Z`. Records
whose check character does not match are rejected with `ERRCHK`. Output
lines carry the same scheme over their preceding bytes.

## Fee schedule

| tier | rate (basis points of amount) |
|------|-------------------------------|
| T001 | 25 |
| T002 | 50 |
| T003 | 75 |
| T004 | 100 |
| T005 | 150 |
| T006 | 200 |
| T007 | 250 |
| T008 | 300 |
| T009 | 350 |

The fee is the amount times the rate, rounded to the nearest cent, and
capped at 250,000 cents.

## Net

`ST`: net = amount - fee. `RF`: net = -(amount - fee); the fee is retained
by the platform when a refund is processed.

## Output lines (47 characters)

| cols  | field |
|-------|-------|
| 0-1   | type (echoed) |
| 2-11  | account (echoed) |
| 12-23 | fee: `+` then 11 digits |
| 24-35 | net: sign then 11 digits |
| 36-38 | currency (echoed) |
| 39-45 | status, space-padded: `OK` or an error code |
| 46    | check character |

Error codes: `ERRLEN`, `ERRCHK`, `ERRTYPE`, `ERRACCT`, `ERRAMT`,
`ERRDATE`, `ERRCUR`, `ERRTIER`, `ERRMCC`. Rejected records report zero fee
and net.

## Batch trailer

After all record outputs, one trailer line:

| cols  | field |
|-------|-------|
| 0-1   | `TR` |
| 2-7   | count of accepted records, 6 digits |
| 8-13  | count of rejected records, 6 digits |
| 14-25 | `+` then 11 digits: sum of fees over accepted records |
| 26-45 | spaces |
| 46    | check character |
