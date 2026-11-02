# Raw HTML in Markdown is cut, swallowed or mangled

The workspace at `/app` is pulldown-cmark, a Rust CommonMark parser (the crate
in `/app/pulldown-cmark`). It builds and tests offline:

```
cd /app/pulldown-cmark
cargo test
```

The suite passes as given. But users report that raw HTML is handled wrongly
in several situations the current tests do not cover. The HTML each input
should produce is what the CommonMark specification (0.31.2) says, and what
the reference implementation, cmark, produces for it:

- An HTML block that starts with `<script`, `<pre`, `<style` or `<textarea`
  only ends at an end tag written in lower case. A block like

  ```
  <script>
  alert(1)
  </SCRIPT>

  # Title
  ```

  swallows the rest of the document instead of ending at `</SCRIPT>`.

- Inline HTML that continues onto the next line inside a block quote or a
  list item keeps the container's prefix inside the HTML. For

  ```
  > a <!--x
  > y--> b
  ```

  the second `>` ends up inside the comment. Processing instructions,
  declarations and CDATA sections are affected the same way.

- Inline CDATA sections and tags are recognised wrongly: `a <![CDATA[x]y]]> b`
  is rendered as text, while `<a h=x` followed by a tab and `)>` is accepted
  as a tag.

- When a document ends without a final line ending, an unclosed fenced code
  block or HTML block drops its last line if that line holds only spaces or
  tabs (or nothing after a block-quote or list prefix). For example the input
  `` "```\na\n " `` loses its last line.

- A tab between a block quote's or list item's marker and an HTML block that
  starts on that line is turned into spaces or lost; for `` "> \t<div>" `` the
  tab should stay in the HTML block's output.

Fix the underlying causes so that raw HTML blocks and inline HTML follow the
specification in all of these situations, while every other construct keeps
rendering exactly as it does now. Do not special-case the examples above; the
grading cases cover the same rules in many more combinations.

Grading copies only your `pulldown-cmark/src` tree into a clean environment
that has the pristine tests, specification files, build script and
dependencies, then runs the crate's whole test suite: held-out regression
examples and tests for these rules, and every existing test as a regression
wall. A change that makes the held-out cases pass but changes the output of
any existing test does not count. Do not add dependencies; nothing outside
`pulldown-cmark/src` is graded.
