# CLI11 config files do not read back what they say

The workspace at `/app` is CLI11, a header-only C++ command line parser that
can also read options from TOML or INI config files and write the current
options back out as one (`ConfigBase::from_config`, `ConfigBase::to_config`).
It builds and tests offline (Catch2 is installed under `/opt/catch2` and is
found automatically):

```
cmake -S /app -B /tmp/build -G Ninja -DCMAKE_BUILD_TYPE=Debug \
      -DCLI11_BUILD_TESTS=ON -DCLI11_BUILD_EXAMPLES=OFF -DCLI11_BUILD_DOCS=OFF
cmake --build /tmp/build
cd /tmp/build && ctest
```

The suite passes as given. A config file written by `to_config` is meant to
read back into the same option values, and a hand-written file is meant to
be read the way it looks. Users report these failures, which the current
tests do not catch:

- Multiline string values (`'''...'''` and `"""..."""`) are not kept as
  written. A multiline value whose key is indented is read from the wrong
  position and keeps part of the opening quotes. Quotes and escape sequences
  inside a multiline value are processed again after the multiline reader
  has already decoded it, so the value comes back changed. When the closing
  quotes are followed by spaces or a comment on the same line, or the value
  ends with one or two quote characters just before the closing quotes, the
  closer is missed and the following keys are swallowed into the value. A
  string option containing quotes that `to_config` writes as a multiline
  value does not read back unchanged.

- A section header followed by a comment, `[server] # production`, is not
  recognized as a section, so the keys under it never reach that section's
  options. Quoted section names followed by a comment fail the same way.

- A docstring-style comment that opens and closes on one line, such as
  `"""settings for the tool"""` on a line of its own, is taken as the start
  of a comment that never ends, and the rest of the file is ignored.

- An option with `MultiOptionPolicy::Join` whose values contain the join
  delimiter is written as one joined value, which reads back as a different
  set of values.

- `to_config` writes some options twice: the options of a `+` option group
  (whose options are shown as part of its parent) and the options a
  `fallthrough()` subcommand can reach from its parent.

Fix the underlying causes so that config files written by `to_config` read
back into the same values and hand-written files are read as written, and so
that everything that works today keeps working. Do not special-case the
examples above; the grading cases cover the same rules in more combinations.

Grading copies only your `include/CLI` tree into a clean environment that
has the pristine tests, build configuration and Catch2, then builds and runs
every test program in Debug: held-out tests for these rules, and every
existing test as a regression wall. Nothing outside `include/CLI` is graded.
