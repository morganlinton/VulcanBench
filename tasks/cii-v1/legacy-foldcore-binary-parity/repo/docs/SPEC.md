# FoldCore booklet imposition format (spec v2.1, last updated 2016)

> Maintenance note (2023): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the prepress controllers that drive imposition and
> consume sheet layouts were built against the engine, not this file.

## Command stream

The engine reads one command per line on stdin and writes its reply lines
per command, then a trailer at EOF. Blank lines are skipped.

### `P <page>` (append)

| field | format |
|-------|--------|
| page  | 1 to 8 alphanumeric characters, a page id |

Appends the page to the end of the document. Reply: `OK <pages>` where
`<pages>` is the number of pages in the document after the command.
Rejects: `N ???????? FMT` for a missing or malformed page token,
`N <page> DUP` when the page id is already in the document.

### `I <page> <after>` (insert)

Inserts the page immediately after the existing page `<after>`, or at the
document front when `<after>` is `0` (the literal token `0` always means
the front). Reply: `OK <pages>`.

Rejects, checked in this order:

| reply | meaning |
|-------|---------|
| `N ???????? FMT` | missing tokens or malformed page token |
| `N <page> FMT`   | malformed anchor token |
| `N <page> WHERE` | anchor page id not in the document |
| `N <page> DUP`   | page id already in the document |

### `G` (impose)

Imposes the document onto printing sheets and emits the layout: one

    SHEET <s1> <s2> <s3> <s4>

line per sheet (4 page slots per sheet, in the engine's imposition
order), then `GEND <sheets>` with the number of sheets emitted. `G`
takes no arguments.

The imposition persists: a later `G` re-emits the same layout without
recomputing unless the document changed in between. Re-imposition after
edits recomputes identically from the current page order, so a changed
document imposes exactly as if it were being imposed for the first time.

### Trailer

At EOF the engine writes `X <pages> <inserts> <imposes> <rejected>`:
counts of successful `P` commands, successful `I` commands, `G`
commands, and `N` rejects of any kind.

## Imposition

The document is imposed in signatures of 8 pages, 2 sheets per
signature. For the pages `p1..p8` of a signature, the two sheets are:

    SHEET p8 p1 p2 p7
    SHEET p6 p3 p4 p5

The final signature may be partial; its missing slots are padded with
`-`. Every signature always yields exactly 2 sheets, even when some or
all of a sheet's slots are `-`.

Page ids are case-sensitive throughout, and sheet lines echo page ids
exactly as they were given.
