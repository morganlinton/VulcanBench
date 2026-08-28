# CodecCore VX interchange format (spec v2.1, last updated 2018)

> Maintenance note (2024): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; partner systems on both sides of the interchange were
> built against the engine, not this file.

## Command stream

The codec reads one command per line on stdin and writes exactly one reply
line per command, then a trailer at EOF. Blank lines are skipped.

### `E <acct> <amount> <date> <memo>` (encode)

| field  | format |
|--------|--------|
| acct   | 1 to 10 alphanumeric characters |
| amount | 1 to 11 digits, in cents |
| date   | `YYYYMMDD`, a real calendar date, year 1900 or later |
| memo   | 0 to 15 printable non-space characters; `_` stands for the empty memo |

Reply on success: `V <record>` where `<record>` is the 48-character VX
record below. Any malformed encode command replies
`R FMT <acct>`, echoing the account token when it is itself well formed
and `??????????` otherwise. Memos longer than 15 characters are rejected
with `R FMT`.

### `D <record>` (decode)

Everything after `D ` is the record, taken verbatim; it must be exactly
48 characters. Reply on success:

    P <acct> <amount> <date> <memo>

with the account and memo padding stripped, the amount printed as a plain
integer (no leading zeros), and `_` standing for the empty memo. Decode is
strict: any deviation from the record layout below rejects the record.
Error replies (no echo): `R LEN` (wrong length), `R CHK` (check character
mismatch), `R FMT` (any other deviation). Validation order: length, then
the `VX` prefix, then the check character, then the fields.

### Trailer

At EOF the codec writes `X <encoded> <decoded> <rejected>`: counts of `V`
replies, `P` replies, and `R` replies.

## VX record layout (48 characters)

| cols  | field  | format |
|-------|--------|--------|
| 0-1   | prefix | `VX` |
| 2-11  | acct   | left-padded with `*` to 10 characters |
| 12-22 | amount | 11 digits, zero-padded |
| 23-30 | date   | `YYYYMMDD` |
| 31-45 | memo   | right-padded with `.` to 15 characters |
| 46    | flag   | reserved, always `N` |
| 47    | check  | check character (below) |

## Check character

Weighted byte sum over columns 0 to 46 (the flag byte is included, the
check character itself is excluded), with weights cycling `2, 5, 3`
starting at column 0 (column 0 weighs 2, column 1 weighs 5, column 2
weighs 3, column 3 weighs 2, and so on), modulo 36, encoded `0-9A-Z`
(0 to 9 then A to Z). Records whose check character does not match are
rejected with `R CHK`.

## Round trips

Encoding a field list and decoding the produced record yields the original
fields. Decoding a record and re-encoding the fields yields the original
record. Partner files depend on both directions being byte-exact.
