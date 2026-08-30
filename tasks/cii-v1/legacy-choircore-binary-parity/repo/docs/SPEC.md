# ChoirCore seating protocol (spec v1.2, last updated 2015)

> Maintenance note (2020): this document is known to have drifted from
> the production engine in places. Where they disagree, **the engine's
> behavior is the contract**; the rota boards and the seating printers
> that talk to ChoirCore were built against the engine, not this file.

## Command stream

The engine reads one command per line on stdin and writes reply lines,
then a trailer at EOF. Blank lines are skipped. Commands carry exactly
the documented number of tokens; extra or missing tokens are
`N <singer> FMT`. Singer ids are case-sensitive. All arithmetic is
integer.

Each enrolled singer holds a **bench standing**, an internal weight
that decides the concert seating. The standing itself is never printed
anywhere: replies carry only echoes, seating order, and counts of
singers.

### `E <singer> <row>` (enroll)

| field  | format |
|--------|--------|
| singer | 1 to 8 alphanumeric characters, case-sensitive |
| row    | one digit, 1 to 4: the bench row the singer holds |

Enrolls the singer with a standing of 0. Reply: `OK <count>` with the
number of enrolled singers after the command. Enrolling an id that
already exists replies `N <singer> DUP`. A malformed id replies
`N <singer> FMT`; a row token that is not one digit from 1 to 4
replies `N <singer> ROW`.

### `P <singer>` (rehearsal attendance)

The singer attended the weekly rehearsal: standing increases by the
singer's row. Reply: `P <singer>` (a confirmation echo only; the
standing is not printed). An id that is not enrolled replies
`N <singer> UNKNOWN`.

### `A <singer>` (absence)

The singer missed the rehearsal: standing decreases by twice the
singer's row, floored at 0. Reply: `A <singer>`. Unknown ids reply
`N <singer> UNKNOWN`.

### `C` (concert seating)

Prints the full seating: one `C <singer>` line per enrolled singer, in
**descending standing**, ties broken by enrollment order, then a
closing `CEND <count>` line with the number of singers seated. The
first three seats listed carried the concert: their standing resets to
0 (when fewer than three singers are enrolled, every listed seat
resets). A concert with no singers enrolled prints no seating.

### `W` (reseat audit)

Recomputes every singer's standing from the full ledger of enrollments,
rehearsals, absences, and concerts, replacing the live values. The
recomputation applies exactly the arithmetic above, including the
concert resets at their ledger positions: the reseat mirrors the bench,
it never moves it, and seatings before and after a reseat come out the
same. Reply: `WOK <count>` with the number of singers holding a
positive standing after the recomputation.

### Rejects

Reject replies are `N <singer> <code>`, echoing the line's singer token
(or `????????` when the command carries none, including unrecognized
command letters). Codes: `FMT`, `ROW`, `DUP`, `UNKNOWN`. Validation
order: `FMT` (arity, singer id syntax), then `ROW`, then `DUP` or
`UNKNOWN`. Rejected commands change no state.

### Trailer

At EOF the engine writes
`X <singers> <rehearsals> <absences> <concerts> <reseats> <rejected>`:
counts of accepted `E`, `P`, `A`, `C`, and `W` commands, and `N`
replies of any kind.

## Seating invariant

At any point a singer's standing equals the sum of their rehearsal
credits minus their absence deductions since their last front-three
concert, floored at 0 along the way. The rota boards depend on the `W`
audit never changing a seating: a bench that reseats differently from
how it seated live indicates a corrupted ledger and opens a manual
investigation.
