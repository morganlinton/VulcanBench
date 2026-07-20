# Redundant loop-bound elision drops a real bounds check after the induction variable is rebound

A pass that removes "redundant" `if loop_var < to` checks inside a loop treats a
guard as redundant when the compared name matches the loop induction variable
and the literal matches the loop's `to`. That is sound only while the induction
variable still holds the loop index.

If the body rebinds that name first — for example `let i = load(buf, i)` — then
a later `if i < to { store(buf, i, ...) }` is a **real** bounds check on a
gathered index that may be out of range. Eliding it allows an out-of-bounds
store. That is the unsound direction.

## Expected behavior

- When the loop body rebinds the induction variable before a `var < to` guard
  whose literal equals the loop upper bound, the elision pass must **not**
  remove that guard (transform reports no change; the `If` remains structurally
  present; analyze agrees that there is nothing redundant to remove).
- When the induction variable is stable for the whole body, existing redundant
  elision behavior for true loop-range re-checks must keep working.
- Other loop transforms that assume a stable induction variable for the whole
  body must keep refusing to fire when that name is rebound (their existing
  skip behavior must not regress).

A minimal counterexample: loop `i` in `0..4` whose body does
`let i = load(buf, i); if i < 4 { store(buf, i, 7) }`. After the fix the `if`
must still be there.
