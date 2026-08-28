# FreightCore shipment rating format (spec v2.1, last updated 2018)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; downstream consumers were built against the engine,
> not this file.

## Input (one shipment per line, whitespace-separated tokens)

```
S <shipid> <zone> <class> <weight_kg> <len_cm> <wid_cm> <hgt_cm> <svc>
```

| field     | format |
|-----------|--------|
| tag       | literal `S` |
| shipid    | 1-10 alphanumeric characters |
| zone      | exactly 2 digits, `01` to `99` |
| class     | exactly 3 digits, one of the standard freight classes below |
| weight_kg | 1-6 digits, actual weight in whole kg |
| len_cm    | 1-4 digits |
| wid_cm    | 1-4 digits |
| hgt_cm    | 1-4 digits |
| svc       | service level: `G` (ground), `X` (express), `P` (priority) |

Each line carries exactly these nine tokens.

## Standard freight classes

`050 055 060 065 070 077 085 092 100 110 125 150 175 200 250 300 400 500`

## Validation

Checks run in this order; the first failure rejects the shipment with the
corresponding code:

1. `FMT`: wrong tag, wrong token count, shipid not 1-10 alphanumerics, or
   any field with the wrong shape (digit counts above, svc not a single
   character). The reject line echoes the shipid when it parsed, `?`
   otherwise.
2. `ZONE`: zone `00` (valid zones are `01` to `99`).
3. `CLASS`: class not in the standard table.
4. `WEIGHT`: weight 0.
5. `DIM`: any dimension 0 (each of len, wid, hgt must be at least 1 cm).
6. `SVC`: svc not `G`, `X`, or `P`.

## Charge model

All arithmetic is in integer cents.

1. **Dimensional weight**: `len_cm * wid_cm * hgt_cm / 5000`, rounded UP to
   the next whole kg.
2. **Billable weight**: the greater of the actual weight and the
   dimensional weight.
3. **Zone rate** (cents per kg): `60 + 8 * (zone div 10)`. Zones 01-09 pay
   60, zones 10-19 pay 68, and so on up to zone 99 at 132.
4. **Base**: `billable_kg * zone_rate`.
5. **Class multiplier**: multiply the base by `class / 100`, rounded to the
   nearest cent (class 050 halves the base, class 500 quintuples it).
6. **Service surcharge**, applied last, on the class-multiplied amount,
   rounded to the nearest cent: `G` adds nothing, `X` adds 50%, `P` adds
   25%.

## Output

One line per input shipment, in input order.

Accepted: `C <shipid> <charge_cents>` (one composite number; components
are never broken out).

Rejected: `R <shipid> <code>` with the codes above.

## Batch trailer

After all shipment lines, exactly one trailer:

```
X <accepted_count> <rejected_count> <sum_of_accepted_charges>
```
