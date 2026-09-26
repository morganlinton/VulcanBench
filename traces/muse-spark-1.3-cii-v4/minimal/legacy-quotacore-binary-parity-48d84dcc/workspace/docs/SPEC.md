# QuotaCore usage metering format (spec v3.1, last updated 2015)

> Maintenance note (2020): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the invoicing and dunning systems downstream were built against the
> engine.

One metering batch per process. Usage events on stdin, one per line; a
result line per accepted event; a trailer at end of input. Call
consumption accumulates per API key across the batch, in event order.

## Events

`Q <key> <calls> <tier> <region>`

- `key`: 1-8 alphanumerics, case-sensitive.
- `calls`: number of API calls, 1-7 digits. A zero-call event is a valid
  no-op (charge 0).
- `tier`: `F` (free), `S` (standard), `P` (premium). The first event for
  a key fixes its tier; a later event carrying a different tier is
  rejected `TIER`.
- `region`: 2 uppercase letters. Informational; regions do not affect
  billing.

## Billing

Each key has a per-batch included quota by tier: `F` 10,000 calls, `S`
100,000, `P` 1,000,000, consumed in event order. Included calls cost
nothing.

Calls beyond the quota are overage. Overage is priced per started block
of 100 calls (that is, the overage call count divided by 100, rounded
up), at 12 cents per block (`F`), 9 (`S`), or 6 (`P`). An event that
crosses from quota into overage is billed only on its overage portion.

`quota_left` is the key's remaining included quota after the event,
floored at 0.

## Output

- Accepted: `B <key> <charge_cents> <quota_left>`.
- Rejected: `R <key> <code>`; codes `FMT`, `CALLS`, `TIER`, `REGION`,
  checked in that order.
- Trailer: `X <accepted> <rejected> <sum_of_charges>`.
