# PackCore allocator command format (spec v1.7, last updated 2015)

> Maintenance note (2020): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the provisioners and the audit tooling downstream were built against
> the engine.

One allocation session per process. Commands on stdin, one per line; a
result line per command; a trailer at end of input. State (open bins,
their contents, the placement history) persists across the session, in
command order.

## Commands

- `P <item> <size>`: place an item.
  - `item`: 1-8 alphanumerics, case-sensitive. An item id that is
    currently placed is rejected `DUP`. A freed id may be placed again.
  - `size`: 1-3 digits, value 1-100 (bins have capacity 100).
  - Placement is FIRST-FIT: the item goes into the lowest-indexed open
    bin with enough remaining capacity; if none fits, a new bin opens
    at the end.
  - Reply: `B <bins>` (number of open bins after the placement).
- `F <item>`: free a placed item. Its capacity returns to its bin. A
  bin left empty stays open; empty bins close ONLY at compaction.
  Freeing an item that is not currently placed is rejected `STATE`.
  Reply: `B <bins>` (unchanged bin count).
- `K`: compact. All currently placed items are repacked from scratch
  into fresh bins, first-fit in their ORIGINAL PLACEMENT ORDER; empty
  bins disappear. Reply: `B <bins>`.
- `D`: report the layout digest: the sum over open bins of
  `weight * bin_used`, with weights cycling 2, 7 starting at bin 0,
  modulo 99991. Reply: `D <digest>`.

## Output

- Rejected: `N <item> <code>`; codes `FMT`, `SIZE`, `DUP`, `STATE`,
  checked in that order. Commands carry exactly the tokens shown above;
  anything else is `FMT`.
- Trailer: `X <placed> <freed> <compacts> <rejected>`. Placements count
  accepted `P` commands; frees count accepted `F` commands; compacts
  count `K` commands; rejected counts every `N` line emitted.
