# Loop peel drops work from the first iteration and leaves a stale induction variable

Some loops start with a first-iteration guard of the form:

```text
Loop(i, 0, N, [
  If(i == 0, then_body, []),
  rest_body...
])
```

A peel transform is supposed to specialize iteration 0 out of the loop so the
remainder can run from `i = 1` without re-testing the guard. The current peel
is wrong in two ways that show up on ordinary, in-contract IR:

1. It lifts only `then_body` into a prologue and leaves `rest_body` exclusively
   in the remainder loop that starts at `i = 1`. Iteration 0 of the original
   also ran `rest_body` with `i = 0`, so that work is silently lost.
2. The lifted `then_body` is not rewritten with the induction variable fixed to
   the peeled value. Any use of `Var(i)` in `then_body` still names `i`, which
   is no longer bound (or is bound to a later iteration), so the prologue
   stores/loads the wrong index.

## Expected behavior

Peeling a loop of the shape above must be observably identical to running the
original first iteration, then the remaining iterations:

- The prologue must execute **all** of the first iteration's body with the loop
  variable substituted to the peeled start value (`i := 0`): both the then-arm
  of the first-iteration guard **and** every trailing statement that would have
  run in that same iteration.
- The remainder loop must start at `from = 1` and keep the trailing body, still
  using the induction variable for later iterations.
- After peel, a program whose first iteration stores `(i, 99)` in the guard arm
  and `(i, 7)` in the trailing body must produce the store sequence
  `(0, 99), (0, 7)` in the prologue and `(Var(i), 7)` inside the remainder loop.

Do not change unrelated passes. Preserve existing peel skip conditions (non-zero
`from`, non-literal `to`, body that rebinds the loop variable, etc.).
