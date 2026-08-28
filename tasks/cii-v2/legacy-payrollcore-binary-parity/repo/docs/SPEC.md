# PayrollCore withholding format (spec v2.2, last updated 2014)

> Maintenance note (2019): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the ledger and remittance systems downstream were built against the
> engine.

One payroll batch per process. Records on stdin, one per line; a result
line per record; a trailer at end of input. Year-to-date figures
accumulate per employee across the batch, in record order.

## Records

`P <empid> <period> <gross> <status> <state> <flags>`

- `empid`: 1-8 alphanumerics, case-sensitive.
- `period`: pay period `01`-`26`.
- `gross`: gross pay in cents, 1-9 digits. A zero gross is a valid no-op
  record (net 0).
- `status`: `S` (single), `M` (married), `H` (household).
- `state`: 2 uppercase letters.
- `flags`: exactly 3 characters `A`-`Z`, or `000` for none. Flags are
  informational and do not affect withholding.

## Withholding

Federal tax is marginal on the period gross: 10% up to 100000, 20% from
100001 to 400000, 30% above 400000, rounded to the nearest cent. Status
`M` applies an 8% relief to the federal rates.

State tax is a flat 5% of gross, rounded to the nearest cent; the states
`TX`, `FL`, `WA`, `NV` withhold nothing.

The social levy is 6.2% of gross, rounded to the nearest cent, until the
employee's year-to-date gross reaches 1,600,000 cents; the record that
crosses the cap is prorated so the levy applies only to the portion under
the cap, and later records pay no levy.

Net = gross - federal - state - levy.

## Output

- Accepted: `W <empid> <net_cents>`.
- Rejected: `R <empid> <code>`; codes `FMT`, `PERIOD`, `GROSS`,
  `STATUS`, `STATE`, `FLAGS`.
- Trailer: `X <accepted> <rejected> <sum_of_nets>`.
