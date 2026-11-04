# ReflowCore text-layout engine (spec v3.2, last updated 2015)

> Maintenance note (2021): this document is known to have drifted from the
> production engine in places. Where they disagree, **the engine's behavior
> is the contract**; the editor front ends and the export pipelines that
> lay out documents through the engine were built against it, not this
> file.

## Command stream

The engine reads one command per line on stdin and writes exactly one
reply line per command, then a trailer at EOF. Blank lines are skipped.
The engine holds one document (an ordered list of words) and its current
layout (an ordered list of lines). The initial wrap width is 40.

### `W <width>` (set wrap width)

| field | format |
|-------|--------|
| width | 2 to 3 decimal digits, value 10 to 120 |

Sets the wrap width and replies `OK`. Nothing else is reset: the document
and the existing layout are untouched, and only words laid out AFTER the
change use the new width. Re-laying existing lines requires an explicit
`R`.

### `A <word>` (append word)

| field | format |
|-------|--------|
| word  | 1 to 16 alphanumeric characters |

Appends the word to the document and lays it out INCREMENTALLY: by the
greedy wrap rule below it joins the current last line if it fits, else it
starts a new line. A word longer than the current width cannot be laid
out and is rejected `WORD`. Reply: `L <lines>`, the layout's line count
after the append.

### `R` (full reflow)

Re-lays the WHOLE document from scratch at the current width with the
same greedy wrap rule. Reply: `L <lines>`. Reflowing an empty document
yields zero lines. `R` takes no arguments.

Incremental layout is an optimization of reflow: appending words one at
a time and reflowing the same document at the same width produce
IDENTICAL layouts. The export pipelines depend on this invariant.

### `D` (layout digest)

Reply `D <digest>`: over the CURRENT layout's lines, the sum of
`line_length * weight`, with weights cycling `3, 5` starting at line 0
(line 0 weighs 3, line 1 weighs 5, line 2 weighs 3, and so on), modulo
1000003, printed in decimal. A line's length is the number of characters
on it, including the single joining spaces between its words. `D` takes
no arguments.

### Rejects

A malformed command replies `E <code>` (the reply carries no other
fields).

| code    | meaning |
|---------|---------|
| `FMT`   | wrong token count or unknown command |
| `WIDTH` | width not 2 to 3 digits, or outside 10 to 120 |
| `WORD`  | word not 1 to 16 alphanumeric characters, or longer than the current width |

Validation order: `FMT` (structure), then `WIDTH`, then `WORD`.

### Trailer

At EOF the engine writes `X <words> <reflows> <rejected>`: counts of
successful `A` commands, successful `R` commands, and `E` replies of any
kind.

## Greedy wrap rule

Both layout paths use the same rule. Words are taken in document order;
a word joins the line being built if

    current_length + 1 + word_length <= width

(the `1` is the joining space), otherwise it starts a new line. The first
word of a document always starts line 0. Word length is the word's
character count.
