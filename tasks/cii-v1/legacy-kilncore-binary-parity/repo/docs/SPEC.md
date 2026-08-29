# KilnCore firing protocol (spec v1.7, last updated 2015)

> Maintenance note (2022): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the kiln schedulers and certification panels that talk
> to KilnCore were built against the engine, not this file.

## Command stream

The controller reads one command per line on stdin and writes exactly one
reply line per command, then a trailer at EOF. Blank lines are skipped.
All heatwork amounts are integer heatwork units; all arithmetic is
integer (floor division).

### `L <lot> <target>` (register firing lot)

| field  | format |
|--------|--------|
| lot    | 1 to 8 alphanumeric characters, case-sensitive |
| target | target heatwork number, 3 to 5 digits as given, at least 100 |

Registers a firing lot with a target heatwork number. Reply: `OK <count>`
where `<count>` is the number of lots after the command. Registering a
lot whose id already exists replies `N <lot> DUP`. A malformed lot id
replies `N <lot> FMT`; a target token that is not 3 to 5 digits with a
value of at least 100 replies `N <lot> TARGET`. Commands carry exactly
the documented number of tokens; extra tokens are `N <lot> FMT`.

### `H <lot> <segment>` (apply firing segment)

| field   | format |
|---------|--------|
| segment | heatwork units, 1 to 4 digits, at least 1 |

Adds the segment's heatwork to the lot's accumulated total and replies
`W <lot> <accum>`, where `<accum>` is the lot's accumulated heatwork:
the plain sum of all segments applied so far. A segment for an unknown
lot replies `N <lot> UNKNOWN`; a segment for a lot that has already been
certified replies `N <lot> DONE`. A malformed lot id replies
`N <lot> FMT`; a bad segment token replies `N <lot> SEG`.

Validation order: `FMT` (arity, lot id syntax), `SEG`, then unknown lot
(`UNKNOWN`), then `DONE`.

### `C <lot>` (certify)

Recomputes the lot's heatwork from its segment ledger via the
certification formula and closes the lot:

    total = sum of the lot's segments

The certification total always equals the live accumulated heatwork
reported on the `W` lines. Reply: `C <lot> PASS` when the total is at
least the lot's target, otherwise `C <lot> SHORT <deficit>` with
`deficit = target - total`. Certification is terminal either way:
further `H` or `C` commands for the lot reply `N <lot> DONE`. A
certification for an unknown lot replies `N <lot> UNKNOWN`; a malformed
lot id replies `N <lot> FMT`.

### Rejects

Reject replies are `N <lot> <code>`, echoing the line's lot token (or
`????????` when the line has none, including unrecognized command
letters). Codes: `FMT`, `TARGET`, `SEG`, `DUP`, `DONE`, `UNKNOWN`.
Rejected commands change no state.

### Trailer

At EOF the controller writes `X <lots> <segments> <certs> <rejected>`:
counts of successful `L` commands, successful `H` commands, successful
`C` commands (`PASS` and `SHORT` alike), and `N` replies of any kind.

## Heatwork invariant

For any lot at any point, the accumulated heatwork on the latest `W`
line equals the sum of the lot's applied segments, and the certification
total equals that same sum. The certification panels depend on the `C`
verdicts agreeing with the `W` totals the schedulers logged live: a lot
whose last `W` line reads at or above target always certifies `PASS`.
