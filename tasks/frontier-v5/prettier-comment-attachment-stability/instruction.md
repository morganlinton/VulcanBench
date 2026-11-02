# Formatting some commented code twice gives a different result

The workspace at `/app` is prettier, the code formatter. It runs from source,
offline, with its own test suite:

```
cd /app
FULL_TEST=1 npx jest tests/format/js
```

`FULL_TEST=1` is how the project's CI runs: besides comparing each fixture
with its snapshot, it formats the output a second time and fails if anything
moves. The suite passes as given. Users report comments that prettier
places differently from where they were written, or that move again on the
next format, and the current tests do not notice:

- A comment before the first element of a parenthesized sequence expression
  moves on every pass: `((/* c */ a), b);` formats to `(/* c */ a, b);`,
  which then formats to `/* c */ (a, b);`.

- A trailing comment inside parentheses that prettier drops is attached to
  the inner expression, so the next format moves it past the semicolon:
  `assigned = (a = c /* comment */);` formats to
  `assigned = a = c /* comment */;` and then to
  `assigned = a = c; /* comment */`. The same happens when the parentheses
  wrap an arrow function returned by another arrow function.

- In TypeScript and Flow, a comment written after `implements` or `extends`
  and before the first type in the list is printed before the keyword
  instead, and comments between the types of such a list are not stable.

- With the Flow parsers, comments at the end of a destructuring pattern
  that has a type annotation are pushed out of the braces and drift on each
  format. The TypeScript parser already keeps them inside, and Flow should
  give the same output:

  ```
  const {
    foo,
    // bar
    // baz
  }: Foo = expr;
  ```

- A comment between a destructuring pattern's closing brace or bracket and
  its type annotation is moved inside the pattern. This input prints as
  `const { foo /* comment */ }: Foo = bar;` instead of
  `const { foo } /* comment */ : Foo = bar;`:

  ```
  const {
    foo
  } /* comment */
    : Foo = bar;
  ```

Fix the underlying causes so that each of these keeps its comment where it
was written and formats identically on the second pass, and every fixture
that formats today keeps formatting exactly as it does now, under every
parser and option the suite runs it with. Do not special-case the examples
above; the grading cases cover the same rules in more combinations and
parsers.

Grading copies only your `src` tree into a clean environment that has the
pristine tests, configuration and dependencies, then runs prettier's whole
test suite with `FULL_TEST=1`: held-out fixtures for these rules, and every
existing test as a regression wall. A change that makes the held-out
fixtures pass but changes the output, stability or AST of any existing
fixture does not count. Do not add dependencies; nothing outside `src` is
graded.
