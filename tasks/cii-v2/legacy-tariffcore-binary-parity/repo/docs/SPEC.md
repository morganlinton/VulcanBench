# TariffCore declaration format (spec v3.0, last updated 2015)

> Maintenance note (2020): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the customs reconciliation systems downstream were built against the
> engine.

One calculator per process. Declarations on stdin, one per line; a result
line per declaration; a trailer on end of input.

## Declarations

`D <declid> <origin> <hs> <mode> <currency> <value> <weight>`

- `declid`: 1-10 alphanumerics.
- `origin`: 2 uppercase letters (ISO country).
- `hs`: 6 digits (harmonized tariff code); the first two digits are the
  chapter. Chapter 00 is invalid; chapter 99 is exempt from duty.
- `mode`: `A` (air), `S` (sea), `R` (road).
- `currency`: `USD`, `EUR`, `JPY`.
- `value`: declared value in cents, 1-9 digits.
- `weight`: gross weight in decigrams, 1-8 digits.

## Duty

Duty is the declared value times the chapter rate, rounded to the nearest
cent, and the total charge is capped at 900000 cents.

| chapters | rate (basis points) |
|----------|---------------------|
| 01-15 | 250 |
| 16-27 | 400 |
| 28-38 | 650 |
| 39-49 | 500 |
| 50-63 | 800 |
| 64-83 | 300 |
| 84-90 | 150 |
| 91-97 | 550 |

Chapter 99 (special transactions) is exempt from duty. Chapters outside
01-97 and 99 are invalid and rejected with `HS`.

## Weight fee

40 cents per kilogram of gross weight (weights are declared in decigrams;
10000 dg = 1 kg; e.g. a declaration of 15000 dg is charged as 2 kg, and
14000 dg as 1 kg).

## Output

- Accepted: `T <declid> <duty> <weightfee> <levy> <total>`. The levy
  column is reserved and reads 0.
- Rejected: `R <declid> <code>`; codes `FMT`, `ORIGIN`, `HS`, `MODE`,
  `CUR`, `VALUE`, `WEIGHT`.
- Trailer at end of input: `X <accepted> <rejected> <sum_of_totals>`.
