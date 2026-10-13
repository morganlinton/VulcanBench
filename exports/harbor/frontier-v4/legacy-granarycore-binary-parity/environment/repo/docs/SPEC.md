# GranaryCore ledger command format (spec v3.1, last updated 2009)

> Maintenance note (2012): this document has drifted from the
> production engine. Where they disagree, **the engine's behavior is
> the contract**; the federation's reconciliation and audit tooling
> downstream were built against the engine.

One ledger session per process. Commands on stdin, one per line; result
lines per command; a trailer at end of input. State (members, bins,
holdings) persists across the session, in command order. Blank lines
are ignored.

## Commands

- `J <member> <shares>`: a member joins the co-op.
  - `member`: 1-8 alphanumerics, case-sensitive. A name already on the
    membership roll is rejected `DUP`. Members never leave.
  - `shares`: 1-2 digits, value 1-99.
  - Reply: `OK <members>` (membership count after the join).
- `O <bin> <cap>`: open a grain bin.
  - `bin`: 1-8 alphanumerics, case-sensitive; a separate namespace from
    member names. A name matching an open bin is rejected `DUP`. A name
    freed by a shut bin may be opened again.
  - `cap`: 1-3 digits, value 1-999 (sacks).
  - Reply: `OK <bins>` (open-bin count after the open).
- `P <member> <bin> <count>`: the member deposits `count` sacks
  (1-999) into the bin. A deposit that would push the bin past its
  capacity is rejected `FULL`. Reply: `OK`.
- `W <member> <bin> <count>`: the member draws `count` sacks from
  their own holding in the bin. Drawing more than the member holds
  there is rejected `LOW`. Reply: `OK`.
- `M <member> <from> <to> <count>`: the member moves `count` sacks of
  their own holding between two open bins. The same-bin pair is
  rejected `DUP`; shortage is `LOW`; destination overflow is `FULL`.
  Reply: `OK`.
- `T`: the seasonal turning. Lists every open bin, fullest first (total
  sacks; ties by opening order), then shuts the bins that stand empty.
  Reply: `TURN <k>` with `k` the number of open bins, one `B <bin>`
  line per bin in listed order, then `SHUT <bin>` for each empty bin in
  that same order. A shut bin's name is freed.
- `Y`: the yearly reckoning. Lists every member, most sacks currently
  held first (summed over open bins; ties by join order). Reply:
  `RECK <m>` then one `S <member>` line per member.
- `L <bin>`: inspect a bin's roster. Reply: `HELD <bin> <h>` then one
  `H <member>` line per holder, in the order they first deposited into
  the bin. A holder whose sacks in the bin reach zero leaves the
  roster; depositing again rejoins at the back.

## Rejects

`R <code>`, one line, nothing else. Codes, checked in this order:

- `FMT`: wrong token count, malformed name or number token, or an
  unknown command word.
- `RNG`: a well-formed number outside its documented range.
- `WHO`: unknown member.
- `LOC`: unknown or closed bin.
- `DUP`: duplicate member, duplicate open bin, or a same-bin transfer
  pair.
- `LOW`: drawing or moving more than the member holds in that bin.
- `FULL`: deposit or transfer past the bin's capacity.

Commands carry exactly the tokens shown above; anything else is `FMT`.

## Trailer

`Z <joins> <opens> <deposits> <draws> <transfers> <turnings>
<reckonings>`: counts of accepted commands only; rejected commands
count nowhere. Inspections are not booked.
