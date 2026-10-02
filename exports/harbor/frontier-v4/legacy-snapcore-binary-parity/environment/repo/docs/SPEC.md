# SnapCore session-state blob format (spec v1.4, last updated 2017)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the fleet of session managers that export and import
> these blobs was built against the engine, not this file.

## Command stream

The store reads one command per line on stdin and writes exactly one reply
line per command, then a trailer at EOF. Blank lines are skipped.

### `P <key> <val>` (put)

| field | format |
|-------|--------|
| key   | 1 to 8 alphanumeric characters, case-sensitive |
| val   | 1 to 12 alphanumeric characters |

Inserts a new key at the end of the insertion order, or updates an
existing key in place (its position is kept). Reply: `OK <count>` where
`<count>` is the number of live keys after the command. A malformed put
replies `R FMT`.

### `G <key>` (get)

Reply `V <val>` when the key is live, `NIL` when it is not. Lookup is
case-sensitive. A malformed get replies `R FMT`.

### `S` (serialize)

Serializes the live state to a blob. Reply: `B <blob>` with

    <blob> = Z1|<pairs>|<checksum>

where `<pairs>` is the `;`-joined list of `key=val` pairs in INSERTION
order, values written in full. The blob never contains spaces. `S` takes
no arguments.

### `L <blob>` (load)

Everything after `L ` is the blob, taken verbatim (it is never split on
spaces). A successful load REPLACES the entire live state with the blob's
pairs; afterwards the insertion order is the blob's pair order. Reply:
`OK <count>` where `<count>` is the number of live keys after the load.

Loading is strict: any deviation from the blob format rejects the blob
and leaves the live state untouched. Error replies:

| reply   | meaning |
|---------|---------|
| `R FMT` | structural damage (not exactly two `\|`, a checksum field that is not exactly one character), or bad pair syntax |
| `R VER` | version tag other than `Z1` |
| `R CHK` | checksum mismatch |

Validation order: structure (`FMT`), then the version tag (`VER`), then
the checksum (`CHK`), then pair syntax (`FMT`). Pair syntax requires at
least one pair, each pair exactly one `=` with a valid key and value, and
no duplicate keys (a blob carrying the same key twice is rejected `FMT`).

### Trailer

At EOF the store writes `X <puts> <gets> <loads> <rejected>`: counts of
successful `P` commands, successful `G` commands (both `V` and `NIL`
replies), successful `L` commands, and `R` replies of any kind.

## Checksum

Weighted byte sum over everything before the blob's final `|` (that is,
over `Z1|<pairs>`), with weights cycling `3, 7` starting at the first
byte (byte 0 weighs 3, byte 1 weighs 7, byte 2 weighs 3, and so on),
modulo 36, encoded `0-9A-Z` (0 to 9 then A to Z).

## Round trips

Serializing and reloading a blob is the identity: `S` then `L` of the
produced blob restores exactly the same live state, values and insertion
order included, and a following `S` reproduces the blob byte for byte.
The session managers depend on both directions being exact.
