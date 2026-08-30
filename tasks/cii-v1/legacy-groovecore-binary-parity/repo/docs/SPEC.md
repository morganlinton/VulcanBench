# GrooveCore pressing protocol (spec v2.1, last updated 2015)

> Maintenance note (2022): this document is known to have drifted from the
> production controller in places. Where they disagree, **the controller's
> behavior is the contract**; the lathe consoles and the pressing-floor
> reconcilers that talk to GrooveCore were built against the controller,
> not this file.

## Command stream

The controller reads one command per line on stdin and writes the reply
lines for each command, then a trailer at EOF. Blank lines are skipped.
Parsing is strict: commands carry exactly the documented number of
space-separated tokens (extra or missing tokens are `N <title> FMT`),
and title ids are case-sensitive everywhere. All arithmetic is integer.

### `T <title> <minutes>` (register a title)

| field   | format |
|---------|--------|
| title   | 1 to 8 alphanumeric characters, case-sensitive |
| minutes | side runtime in minutes, 2 to 3 digits, value 10 to 120 |

Registers a title in the pressing queue and grants it a **groove
allotment of 6 sides** of lacquer. Reply: `OK <count>` where `<count>`
is the number of titles after the command. Registering a title whose id
already exists replies `N <title> DUP`. A malformed id replies
`N <title> FMT`; a minutes token that is not 2 to 3 digits with a value
of 10 to 120 replies `N <title> MIN`.

Validation order: `FMT` (arity, title id syntax), `MIN`, then `DUP`.

### `S <title>` (cut one side)

Cuts one side of the title on the lathe, consuming **1 allotment**, and
replies `Q <title> <quality>` with the side's composite quality number:

    quality = 1000 - 3 * minutes

Longer sides are cut shallower at fixed groove pitch, so quality is a
constant per title. When the title's allotment is exhausted the reply is
`N <title> SPENT` and nothing is cut.

Validation order: `FMT` (arity, title id syntax), unknown title
(`UNKNOWN`), then `SPENT`.

### `M` (batch remaster)

Takes no arguments. Recomputes every title's remaining allotment from
the ledger:

    remaining = 6 - sides_cut

and emits one `R <title> <remaining>` line per title in registration
order, followed by `MEND <titles>` with the number of registered titles.
The recomputed values REPLACE the live allotment state, but since every
cut consumes exactly one allotment the recomputation always agrees with
the live counters: **remaster is bookkeeping only**.

A remaster when no sides have been cut since the previous remaster (or
since the start of the stream) is a no-op: the controller emits only the
`MEND <titles>` line and the remaster is not counted in the trailer.

### Rejects

Reject replies are `N <title> <code>`, echoing the line's title token
(or `????????` when the line has none, including unrecognized command
letters and extra tokens on `M`). Codes: `FMT`, `MIN`, `DUP`, `UNKNOWN`,
`SPENT`. Rejected commands change no state.

### Trailer

At EOF the controller writes `X <titles> <sides> <remasters> <rejected>`:
counts of successful `T` commands, successful `S` cuts, counted `M`
remasters, and `N` replies of any kind.

## Allotment invariant

At any point, `remaining = 6 - sides_cut` for every title, on the live
path and after any remaster alike. The pressing-floor reconcilers depend
on the `R` report always agreeing with the live `SPENT` behavior, and on
quality being constant per title: a declining quality sequence indicates
lathe hardware trouble and opens a maintenance ticket.
