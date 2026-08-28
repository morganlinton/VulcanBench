# LoyaltyCore batch format (spec v1.7, last updated 2016)

> Maintenance note (2021): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the statements and partner-mall systems downstream were built against
> the engine.

One batch per process. Earn records on stdin, one per line; a result line
per record; a trailer at end of input. Lifetime points and member tier
accumulate per member across the batch, in record order.

## Records

`L <member> <spend> <cat> <promo>`

- `member`: 1-8 alphanumerics, case-sensitive.
- `spend`: spend in cents, 1-9 digits. Spends under 100 cents earn 0
  points.
- `cat`: purchase category: `G` (general), `F` (fuel), `E` (electronics),
  `T` (travel).
- `promo`: exactly 3 characters `A`-`Z`, or `000` for none. Promo codes
  are informational and do not affect earning.

## Earning

Base points = spend / 100, rounded down. Category multipliers: `G` x1,
`F` x2, `E` x3, `T` x1.5, applied to base points and rounded to the
nearest point. Tier bonus applied to the multiplied points and rounded to
the nearest point: Silver +10%, Gold +25%.

Tiers by lifetime points: Silver from 5,000, Gold from 20,000. A tier
reached by a record takes effect from the member's NEXT record.

## Output

- Accepted: `E <member> <points_earned> <new_balance>`.
- Rejected: `R <member> <code>`; codes `FMT`, `SPEND`, `CAT`, `PROMO`.
- Trailer: `X <accepted> <rejected> <sum_of_points_earned>`.
