# Implement a line diff and a validating patch applier

The `textdiff` package computes a line-level diff between two sequences of lines
and applies it. The work is split across two files and both must be implemented:

- `textdiff/diff.py` — `diff(a, b)` builds an edit script.
- `textdiff/patch.py` — `apply(ops, a)` applies an edit script (and `PatchError`).

## Edit script format (shared contract)

`diff(a, b)` returns a list of operations, each a tuple:

- `("keep", line)` — a line present in both `a` and `b`;
- `("del", line)` — a line present only in `a`;
- `("ins", line)` — a line present only in `b`.

The kept lines must form a **longest common subsequence** of `a` and `b` (a
minimal edit script, not just any sequence of deletes-then-inserts).

## `apply(ops, a)` (`textdiff/patch.py`)

Reconstruct the target by walking the script against the source `a`:

- `"keep"` and `"del"` must match the next unconsumed line of `a` (advance past
  it); `"keep"` also emits the line, `"del"` does not.
- `"ins"` emits its line and consumes nothing from `a`.
- If a `"keep"`/`"del"` line does not match the next line of `a`, or the script
  finishes without consuming all of `a`, raise `PatchError`.

## Defining invariant

For any `a` and `b`, `apply(diff(a, b), a) == b`. Applying a script to a source it
was not built from must raise `PatchError`.
