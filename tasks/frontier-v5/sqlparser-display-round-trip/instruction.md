# Printing a parsed SQL statement does not give back the same statement

The workspace at `/app` is sqlparser, a Rust SQL parser whose AST prints back
to SQL through `Display`. It builds and tests offline:

```
cd /app
cargo test
```

The suite passes as given. The library promises that printing a parsed
statement gives SQL that parses back to the same AST and prints the same way
again (the round trip the tests check with `verified_stmt`). Users report
several cases where that fails and the current tests do not notice:

- String-like literals that contain their own quote character are printed
  without escaping it. `SELECT X'ab''cd'` prints as `SELECT X'ab'cd'`, which no
  longer parses; byte strings (`B'it''s'`, `B"it""s"`), raw strings
  (`R'it''s'`) and bit strings are affected the same way in the dialects that
  have them.

- A prefix `!` before an operand that itself starts with an operator character
  is printed glued to it, so `SELECT ! -a` prints as `SELECT !-a`, which
  tokenizes differently.

- A unary minus or `!` whose operand is a cast of another unary expression,
  such as `-(-x)::INT` built in the AST, prints so that the two operators run
  together (for minus, into a `--` line comment). Printing deeply nested
  unary minus expressions is also very slow: a statement with a couple of
  hundred nested signs takes far too long to print.

- Field access on a number prints without spaces, so `SELECT (1 . i)` prints
  as `SELECT (1.i)`, which reads as the number `1.` followed by `i`.

Fix the underlying causes so that every one of these round-trips, with
operators printed apart from their operands only where they would otherwise
merge into a different token, and every statement that round-trips today
keeps printing exactly as it does now. Printing must stay fast for deeply
nested expressions. Do not special-case the examples above; the grading cases
cover the same rules in more combinations and dialects.

Grading copies only your `src` tree into a clean environment that has the
pristine tests, build configuration and dependencies, then runs the crate's
whole test suite: held-out round-trip tests for these rules, and every
existing test as a regression wall. A change that makes the held-out tests
pass but changes how any existing test's statement prints does not count. Do
not add dependencies; nothing outside `src` is graded.
