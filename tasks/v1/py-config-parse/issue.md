# Implement a parser for the "VulcanConf" config format

`confkit.parse_config(text)` parses VulcanConf source into a `dict` (later keys
overwrite earlier ones). VulcanConf is line-based. Implement every rule below
exactly.

**Lines**
- Split on `\n`; a trailing `\r` on a line is ignored.
- A line that is empty or whose first non-whitespace character is `#` is skipped
  (blank line or full-line comment).
- Otherwise the line is `key = value`, split on the **first** `=`. A line with no
  `=` raises `ValueError`. The key is whitespace-trimmed; an empty key raises
  `ValueError`.

**Values.** Strip leading whitespace from the part after `=`, then:

- **Quoted value** (starts with `"`): read until the next unescaped `"`.
  - Inside the quotes, `\n` `\t` `\"` `\\` mean newline, tab, `"`, backslash. Any
    other `\x` becomes just `x` (the backslash is dropped). A `#` inside quotes
    is literal, not a comment.
  - After the closing `"`, only whitespace and/or a `#` comment may follow; any
    other trailing content raises `ValueError`. A missing closing `"` raises
    `ValueError`.
  - A quoted value is **always a string** (no type coercion).

- **Unquoted value**: an inline comment begins at the first `#` that is at the
  start of the value or is preceded by whitespace (so `x#y` keeps the `#`, but
  `x #y` does not). Remove the comment, then strip trailing whitespace (inner
  whitespace is preserved). An empty result is the empty string `""`. Otherwise
  coerce, in this order:
  - `true`/`false` (case-insensitive) -> `bool`.
  - `null`/`none` (case-insensitive) -> `None`.
  - an integer `[+-]?(0|[1-9][0-9]*)` -> `int` (so `007` and `00` stay strings).
  - a float with digits on both sides of the dot, or an exponent form
    (`1.5`, `1e3`, `2.5e-2`) -> `float` (so `5.` and `.5` stay strings).
  - otherwise the value is the string itself.

Implement `parse_config` in `confkit/parse.py`.
