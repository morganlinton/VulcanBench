# VaultCore versioned document vault (spec v2.1, last updated 2016)

> Maintenance note (2022): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the archival pipelines that write, sweep, and audit
> the vault were built against the engine, not this file.

## Command stream

The vault reads one command per line on stdin and writes exactly one reply
line per command, then a trailer at EOF. Blank lines are skipped.

### `W <doc> <ver> <body>` (write)

| field | format |
|-------|--------|
| doc   | 1 to 8 alphanumeric characters, case-sensitive |
| ver   | format version, `1` or `2` |
| body  | 1 to 20 alphanumeric characters |

Creates the document, or replaces an existing document's version and
body. Writing a version LOWER than the stored version (a v1 write over a
v2 document) is a downgrade and is rejected `VER`; writing at the stored
version or higher replaces in place. Reply: `OK <v1count> <v2count>`, the
live counts of documents currently stored at each format version.

### `G <doc>` (read)

Reply `D <digest>` for a stored document, `NIL` for an unknown one.
Reading a document still stored at format v1 first MIGRATES it to v2 in
place (lazy migration), then digests the stored (post-migration) body.

### `M` (bulk migrate)

Migrates every remaining v1 document to v2 in one sweep. Reply:
`M <migrated>`, the number of documents the sweep migrated. `M` takes no
arguments.

Lazy reads and the bulk sweep apply the SAME migration: a v1 body
migrates to v2 by uppercasing it. Whether a document is migrated by a
read or by a sweep, the result is the same.

### `C` (vault checksum)

Reply `C <sum>`: the sum of the digests of every stored document's body
(v1 and v2 alike, no migration is triggered), modulo 1000000007. The sum
is order-independent. `C` takes no arguments.

### Rejects

A malformed command replies `R <doc> <code>`; when the doc field itself
is missing or malformed the reply echoes `????????` in its place.

| code   | meaning |
|--------|---------|
| `FMT`  | wrong token count, bad doc id, or unknown command |
| `VER`  | version other than `1`/`2`, or a version downgrade |
| `BODY` | body not 1 to 20 alphanumeric characters |

Validation order: `FMT` (structure and doc id), then `VER`, then `BODY`.

### Trailer

At EOF the vault writes `X <writes> <reads> <migrated> <rejected>`:
counts of successful `W` commands, successful `G` commands (both `D` and
`NIL` replies), documents migrated in total (lazy and bulk combined), and
`R` replies of any kind.

## Digest

Weighted byte sum of the stored body, with weights cycling `4, 9`
starting at the first byte (byte 0 weighs 4, byte 1 weighs 9, byte 2
weighs 4, and so on), modulo 97, printed in decimal.

## Migration invariant

Migration is version-only: the stored body's characters are uppercased
and nothing else changes, on either path. A vault swept by `M` and a
vault drained by reads end up with identical stored bodies, digests, and
checksums. The audit pipelines depend on this invariant.
