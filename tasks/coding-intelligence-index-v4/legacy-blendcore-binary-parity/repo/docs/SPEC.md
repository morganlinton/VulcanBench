# BlendCore blending protocol (spec v3.2, last updated 2016)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the shop-floor dispensers and batch reconcilers that
> talk to BlendCore were built against the engine, not this file.

## Command stream

The controller reads one command per line on stdin and writes one or more
reply lines per command, then a trailer at EOF. Blank lines are skipped.
All volumes are integer tenths of a milliliter; all arithmetic is integer.

### `T <tank> <pigment> <volume>` (fill)

| field   | format |
|---------|--------|
| tank    | 1 to 8 alphanumeric characters, case-sensitive |
| pigment | 1 to 8 alphanumeric characters, case-sensitive |
| volume  | tenths of ml, 2 to 7 digits as given |

Fills a tank with a pigment stock. Filling a tank that already exists
ADDS the volume to it; the pigment must match the tank's pigment exactly,
otherwise the fill replies `N <tank> PIGMENT`. Reply: `OK <count>` where
`<count>` is the number of tanks after the command. A malformed tank or
pigment id replies `N <tank> FMT`; a volume token that is not 2 to 7
digits replies `N <tank> VOL`. Commands carry exactly the documented
number of tokens; extra tokens are `N <tank> FMT`.

### `D <job> <tank> <amount>` (dispense)

| field  | format |
|--------|--------|
| job    | 1 to 8 alphanumeric characters, case-sensitive |
| amount | tenths of ml, 1 to 6 digits, at least 1 |

Draws exactly `<amount>` from the tank into the job and replies
`J <job> <dispensed_total>`, where `<dispensed_total>` is the job's
cumulative dispensed volume across all its `D` commands. A dispense
exceeding the tank's available volume replies `N <tank> DRY` and changes
nothing: partial dispensing never happens. A malformed job or tank id
replies `N <tank> FMT`; a bad amount token replies `N <tank> AMT`; an
unknown tank replies `N <tank> FMT`.

Validation order: `FMT` (arity, id syntax), `AMT`, then unknown tank
(`FMT`), then `DRY`.

### `R` (batch reconcile)

Recomputes every tank's book volume from its fill/dispense ledger:

    book = total filled - total dispensed

and writes one `B <tank> <volume>` line per tank in first-seen order,
followed by `REND <count>` with the number of tanks. Reconcile is an
identity check: the book volume always equals the volume the live path
left in the tank. `R` takes no arguments.

### Rejects

Reject replies are `N <tank> <code>`, echoing the line's tank token (or
`????????` when the line has none, including unrecognized command
letters). Codes: `FMT`, `VOL`, `AMT`, `DRY`, `PIGMENT`. Rejected
commands change no state.

### Trailer

At EOF the controller writes `X <fills> <dispenses> <reconciles>
<rejected>`: counts of successful `T` commands, successful `D` commands,
successful `R` commands, and `N` replies of any kind.

## Reconciliation invariant

For any tank at any point, the book volume reported by `R` equals the
physical volume available to the next dispense, and the sum of a job's
`J` totals reconciles with the volumes drawn from the tanks. The batch
reconcilers depend on the `B` lines agreeing with the dispense stream
they logged live.
