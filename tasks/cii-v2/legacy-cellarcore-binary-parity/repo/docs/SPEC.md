# CellarCore cellar-ledger command format (spec v2.3, last updated 2011)

> Maintenance note (2019): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the cellar clerks' terminals and the seasonal reconciliation tooling
> downstream were built against the engine.

One ledger session per process. Commands on stdin, one per line; one or
more result lines per command; a trailer at end of input. State (the
wheel register, the racks, the sill, the round and grading counters)
persists across the session, in command order.

## Settle

Every wheel carries a settle value that decides its standing on a rack.
It is never printed. Settle is computed from the wheel's record:

    settle = make weight class (from the wheel's LAY)
           - 3 for each sample drawn from it (a TAP)
           - 5 for each move between racks (a SHIFT)

The turning round recomputes settle from the cellar ledger, which
records every sample and every shift, so the recomputed value equals
the running value; the round's recompute is a consistency pass. Ties in
settle always resolve by lay order, earliest wheel first.

## Commands

- `LAY <wheel> <class>`: lay a new wheel in on the sill.
  - `wheel`: 1-8 alphanumerics, case-sensitive, never reusable (a
    retired wheel's name stays retired).
  - `class`: 1-3 digits, value 1-999 (the declared make weight class).
  - Reply: `OK <sill>` (wheels now waiting on the sill).
- `RACK <rack> <span>`: open a rack. `rack`: 1-8 alphanumerics,
  case-sensitive. `span`: 1-2 digits, value 1-24 (standing places).
  - Reply: `OK <racks>` (racks opened so far).
- `BED <wheel> <rack>`: move a wheel from the sill onto a rack. The
  wheel must be waiting on the sill (`STATE` otherwise); the rack must
  have a free place (`FULL` otherwise). The wheel enters the rack's
  standing order at the position its settle earns (see Standing order).
  - Reply: `OK <n>` (wheels now standing on that rack).
- `TAP <wheel>`: draw a sample from a wheel on the sill or on a rack.
  A retired wheel is rejected `STATE`. The sample is entered in the
  cellar ledger and lowers settle by 3.
  - Reply: `OK <wheel>`.
- `SHIFT <wheel> <rack>`: move a racked wheel to another rack. The
  wheel must be standing on a rack, and not already on the named rack
  (`STATE`); the destination must have a free place (`FULL`). The move
  is entered in the ledger and lowers settle by 5. The wheel enters the
  destination's standing order at the position its settle earns.
  - Reply: `OK <n>` (wheels now standing on the destination).
- `TURN`: one turning round (see The turning round).
  - Reply: `TR <k>` (rounds so far), then for each rack in opening
    order: `RK <rack>`, one `W <wheel>` line per standing wheel in the
    relisted order, then one `DN <wheel>` line per slumped wheel.
- `GRADE`: the seasonal grading (see The grading).
  - Reply: `GD <g>` (gradings so far), then one `G <wheel>` line per
    racked wheel in grading order, then one `RET <wheel>` line per
    retired wheel, in grading order.
- `VIEW <rack>`: inspect a rack. Reply: `ON <rack>` followed by the
  standing wheel ids in standing order, on the one line.
- `HALL`: survey the cellar. Reply: `HALL` followed by every rack in
  the order the next turning round will take them (opening order),
  then `SILL` followed by the waiting wheels in arrival order.
- `TALE <wheel>`: one wheel's whereabouts. Reply: `AT <wheel> <rack>`
  for a racked wheel, `AT <wheel> SILL` for a waiting wheel,
  `AT <wheel> OUT` for a retired wheel.

## Standing order

A rack lists its wheels in standing order: settle descending, ties by
lay order (earliest first). A wheel entering a rack (bedding or
shifting) is inserted at the position its settle earns; wheels already
standing do not move.

## The turning round

Each `TURN` works through every rack in opening order. On each rack,
every wheel is flipped and its settle recomputed from the cellar
ledger (class minus 3 per recorded sample minus 5 per recorded shift;
identical to the running value). Any wheel whose recomputed settle is
0 or less slumps off the rack and returns to the sill, at the back of
the arrival order. The rack is then relisted in standing order. The
`DN` lines report slumped wheels in their prior board order; slumped
wheels keep their settle and may be bedded again.

## The grading

`GRADE` lists every racked wheel (sill wheels sit out) in grading
order: settle descending, ties by lay order. It then retires every
listed wheel from which 4 or more samples have been drawn since the
last grading (or since its lay, for the first grading): the wheel
leaves its rack for good and its name is never accepted again. The
sample count toward retirement resets for every wheel at each grading.

## Output

- Rejected: `E <echo> <code>`; codes `FMT`, `RNG`, `WHO`, `DUP`,
  `STATE`, `FULL`, checked in that order. `echo` is the command's first
  argument token when it is 1-8 alphanumerics, else `?`. Commands carry
  exactly the tokens shown above; anything else is `FMT`. Malformed ids
  and numbers are `FMT`; out-of-range values are `RNG`; unknown wheels
  or racks are `WHO`; a name already in use is `DUP`.
- Trailer: `FIN <lays> <racks> <beds> <taps> <shifts> <turns> <grades>
  <rets>`. Lays, racks, beds, taps, shifts, turns, and grades count
  accepted commands of each kind; rets counts retired wheels.
