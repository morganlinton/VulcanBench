# QLite record store protocol (spec v0.9, last updated 2017)

> Maintenance note (2022): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the reporting pipelines downstream were all built against the engine.

One store per process. Commands on stdin, one per line, space-separated;
responses on stdout after each command.

## Commands

- `INS <id> <name> <score>` - insert a record. `id`: 1-8 alphanumerics,
  unique; inserting an existing id is rejected with `ERR DUPKEY`.
  `name`: 1-16 alphanumerics. `score`: integer in -999999..999999.
  Responds `OK`, `ERR FULL` (store capacity 4096), or `ERR FMT`.
- `DEL <id>` - delete. `OK` or `ERR NOTFOUND`.
- `GET <id>` - fetch one record: `ROW <id> <name> <score>` or
  `ERR NOTFOUND`.
- `FIND <pattern>` - all records whose name matches the pattern,
  case-insensitively. `*` in the pattern matches any sequence of
  characters. Emits `ROW` lines, then `END <count>`.
- `RANGE <lo> <hi>` - all records with `lo <= score <= hi` (both bounds
  inclusive). Emits `ROW` lines, then `END <count>`.
- `LIST` - all records. Emits `ROW` lines, then `END <count>`.
- `SUM` - `SUM <value>`: the exact sum of all scores.
- `AVG` - `AVG <value>`: the mean score, rounded down; `ERR EMPTY` when
  the store is empty.

Malformed commands respond `ERR FMT`.

## Ordering

`FIND`, `RANGE`, and `LIST` return rows in insertion order.
