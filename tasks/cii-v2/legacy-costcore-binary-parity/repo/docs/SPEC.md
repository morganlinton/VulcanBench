# CostCore inventory costing format (spec v1.4, last updated 2013)

> Maintenance note (2018): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the ledger and fulfillment systems downstream were built against the
> engine.

One stock batch per process. Movement lines on stdin, one per line; a
result line per movement; a trailer at end of input. Stock and cost
layers accumulate per SKU across the batch, in movement order, and are
scoped to the batch.

## Movements

Receipt: `R <sku> <qty> <unit_cost>`

Issue: `I <sku> <qty>`

- `sku`: 1-8 alphanumerics, case-sensitive.
- `qty`: units, 1-6 digits. Quantities are 1-999999; a quantity of 0 is
  invalid.
- `unit_cost`: cost per unit in cents, 1-7 digits (up to 9,999,999).

Each receipt adds a new cost layer (its quantity at its unit cost) to
the SKU, newest last. Each issue consumes stock first-in-first-out: the
oldest layers are consumed first, and a layer is removed once empty.

Cost of goods issued (COGS) for an issue is the sum, over the layer
slices it consumed, of slice quantity times that layer's unit cost, in
integer cents.

An issue for more than the SKU's on-hand quantity is rejected `STOCK`
and consumes nothing.

## Output

- Receipt accepted: `A <sku> <onhand_after>`.
- Issue accepted: `C <sku> <cogs_cents> <onhand_after>`.
- Rejected: `E <sku> <code>`; codes `FMT`, `QTY`, `COST`, `STOCK`.
  Validation order is `FMT`, then `QTY`, then `COST` (receipts only),
  then `STOCK` (issues only). A malformed SKU is echoed as `????????`.
- Trailer: `X <receipts> <issues> <rejected> <sum_cogs>` where
  `sum_cogs` totals the COGS of all accepted issues.
