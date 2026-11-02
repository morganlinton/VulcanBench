# `# fmt: skip` and `# fmt: off` regions crash Black or mangle the code around them

The workspace at `/app` is Black, the Python code formatter, installed in
editable mode with its test dependencies. Run the test suite the usual way:

```
python -m pytest tests
```

It passes as given. But the handling of the formatting directives has several
defects that the current tests do not cover. In each of the following cases
Black either stops with an internal error (reporting that it produced invalid
code, or that its output is not stable) or changes code the directive was
meant to protect:

- `# fmt: skip` after the closing bracket of a multi-line call or subscript
  that is not the last element of its enclosing brackets:

  ```python
  x = {
      "a":  1,
      "b": f(
          a  ,  b
      ),  # fmt: skip
      "c":  [1,2],
  }
  ```

  Black crashes. Only that element should be left as written; the rest of the
  collection is still formatted.

- `# fmt: skip` on the closing-parenthesis line of a multi-line `from` import
  that is followed by another statement on that line:

  ```python
  from a import (
      b,
  ); import c  # fmt: skip
  ```

  Black crashes. The import and the statement after it should be left alone.

- `# fmt: skip` on an opening bracket when more code follows the closing
  bracket on its line:

  ```python
  x = [  # fmt: skip
      1, 2] + [3]
  ```

  Black crashes. The code after the closing bracket must stay on that line.
  The same happens when what follows is a conditional expression, as in
  `x = (  # fmt: skip`, a hidden operand, then `) if condition else b`.

- `# fmt: skip` on a one-line compound statement whose semicolon-separated
  body starts with a bare name:

  ```python
  if True: a; b  # fmt: skip
  ```

  Black crashes. The whole line should be left as written, with its header
  kept exactly once.

- `# fmt: skip` on a compound-statement header that itself contains an inner
  skipped bracket, for example a `(  # fmt: skip` block that closes before the
  header's colon. Black crashes.

- Comments that end a `# fmt: off` region inside brackets, between the last
  skipped element and `# fmt: on` (or up to the closing bracket when there is
  no `# fmt: on`), are reformatted or moved instead of being kept verbatim:

  ```python
  foo(
      # fmt: off
      a  ,  # trailing
      # fmt: on
  )
  ```

Fix the underlying causes so that these directives leave exactly the code
they cover as written, keep everything else formatted as usual, never crash,
and produce output that is stable (formatting the output again changes
nothing) and preserves the program's meaning (the AST is unchanged). Do not
special-case the examples above; find and fix the causes.

Grading copies only your `src` tree into a clean environment that has the
pristine tests, build configuration and dependencies, then runs the project's
test suite: a held-out set of formatting cases that exercise these directives
in more combinations, and the entire existing suite as a regression wall.
Every case must format to its expected output, be stable on a second pass and
preserve the AST. A change that fixes the new cases but alters any existing
case's output does not count. Do not add dependencies; nothing outside `src`
is graded.
