# TollCore passage rating format (spec v2.1, last updated 2018)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the reconciliation systems downstream were built
> against the engine, not this file.

## Input passages (one per line, space-separated tokens)

```
T <tagid> <gate> <axles> <hhmm> <dow> <class>
```

| field  | format |
|--------|--------|
| leader | literal `T` |
| tagid  | transponder id, 1 to 10 alphanumeric characters |
| gate   | 2 digits, `01` to `40` |
| axles  | 1 digit, `1` to `6` |
| hhmm   | 4 digits, `0000` to `2359` |
| dow    | day of week, 1 digit, `1` (Monday) to `7` (Sunday) |
| class  | vehicle class: `C` (car), `T` (truck), `B` (bus), `M` (motorcycle) |

A line must contain exactly these 7 tokens. Fields are validated in order:
format (leader, token count, tagid), gate, axles, time, day, class; the
first failure rejects the passage with the matching code.

## Toll computation

Base toll: 250 cents plus 40 cents per axle beyond the first, so
`250 + 40 * (axles - 1)`.

Peak multiplier: x2.0 when the passage is on a weekday (day 1 to 5) and
the time falls in a peak window, x1.0 otherwise. The peak windows are
0700 to 0929 and 1600 to 1859, both inclusive, at every gate; all 40
gates are documented at the same rates.

Class multiplier, applied after the peak multiplier:

| class | multiplier |
|-------|------------|
| C     | x1.0 |
| T     | x1.8 |
| B     | x1.4 |
| M     | x0.6 |

The toll is rounded to the nearest cent.

## Output lines

Accepted passage: `F <tagid> <toll_cents>`.

Rejected passage: `R <tagid> <code>` with code one of `FMT`, `GATE`,
`AXLES`, `TIME`, `DOW`, `CLASS`. When the tagid itself does not parse,
`??????????` is echoed in its place.

## Batch trailer

After all passage outputs, one trailer line:

```
X <accepted> <rejected> <sum_tolls>
```

where `sum_tolls` is the sum of tolls over accepted passages, in cents.
