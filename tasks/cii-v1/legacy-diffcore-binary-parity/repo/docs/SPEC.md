# DiffCore snapshot-store command format (spec v2.1, last updated 2018)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the fleet of backup coordinators that drive DiffCore
> command streams was built against the engine, not this file.

## Command stream

The store reads one command per line on stdin and writes one reply line
per command, then a trailer at EOF. Blank lines are skipped. The store
holds a single WORKING VALUE and an append-only list of snapshots,
addressed by 0-based index.

### `B <val>` (set)

| field | format |
|-------|--------|
| val   | 1 to 16 alphanumeric characters, case preserved |

Sets the working value. Reply: `OK`. A `B` with no value replies
`E FMT`; a value that is too long, empty, or not alphanumeric replies
`E VAL`.

### `S` (store snapshot)

Appends a snapshot of the current working value. Reply: `S <count>`
where `<count>` is the number of snapshots after the command.

A snapshot is stored either FULL (the complete value) or as a DELTA
against the previous snapshot: the recorded length of the new value plus
the list of (position, character) pairs at which the new value differs
from the previous snapshot's value (every position past the end of the
shorter value is a difference). The first snapshot is FULL, and every
fourth snapshot is stored FULL so that a chain of deltas stays short.
**Which snapshots are FULL is an internal storage optimization: restore
results are identical either way.**

### `T <idx>` (restore)

| field | format |
|-------|--------|
| idx   | 1 to 3 decimal digits, 0-based snapshot index |

Reconstructs snapshot `<idx>` and makes it the working value, which
subsequent snapshots build on. A FULL snapshot is its stored value; a
DELTA snapshot is the previous snapshot's reconstructed value with the
recorded changes applied, cut or grown to the recorded length (a grown
position that no change covers is filled with `0`). Reply:
`V <digest>` (see below); the value itself is never printed.

A `T` with no index, or an index that is not 1 to 3 digits, replies
`E FMT`. An index at or past the snapshot count replies `E IDX`.

### Trailer

At EOF the store writes `X <sets> <snaps> <restores> <rejected>`:
counts of successful `B`, `S`, and `T` commands, and `E` replies of any
kind.

## Digest

Weighted byte sum of the reconstructed value, with weights cycling
`6, 11` starting at the first byte (byte 0 weighs 6, byte 1 weighs 11,
byte 2 weighs 6, and so on), modulo 997, printed in decimal.

## Reconstruction guarantee

Restoring any snapshot always yields exactly the value that was current
when that snapshot was stored, whether it was stored FULL or as a delta,
and however many deltas the chain crosses. The backup coordinators
depend on this exactness.
