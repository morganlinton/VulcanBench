# {fmt} output departs from printf and std::format in several edge cases

The workspace at `/app` is {fmt}, the C++ formatting library, with its
googletest-based test suite (googletest is vendored, so everything builds
offline). Build and test it the usual way, for example:

```
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_CXX_STANDARD=17 \
      -DFMT_TEST=ON -DFMT_DOC=OFF -DFMT_MODULE=OFF
cmake --build build -j
cd build && ctest --output-on-failure
```

The suite passes as given. But users report output that disagrees with what
`printf` or `std::format` produce, or with {fmt}'s own documentation, in
cases the current tests do not cover:

- Formatting a floating-point zero in hexadecimal (`{:a}`, `{:A}`, with or
  without `#` or a precision) prints the minimum subnormal exponent, for
  example `0x0p-1022`, where `printf("%a", 0.0)` prints `0x0p+0`. Negative
  zero is affected the same way.
- Infinity and NaN given a width but no alignment, such as `{:7}`, come out
  left-aligned (`nan    `), while every finite number, `std::format` and
  `printf` right-align them. Explicit alignment already works.
- The debug presentation (`?`) computes padding from the unescaped value,
  not from what is actually written. `{:6?}` on the character `'a'` pads as
  if one column had been written, and an empty string formatted in debug
  form with a width (`{:1?}`, `{:3?}`) loses its closing quote.
- A `fmt::dynamic_format_arg_store` that holds named arguments reports one
  more element from `size()` than was pushed, and accepts a positional index
  one past the last argument instead of throwing `fmt::format_error`.
- The calendar types (`fmt::day`, `fmt::month`, `fmt::year`,
  `fmt::weekday`, `fmt::year_month_day`) do not honour a format
  specification that has only fill, alignment or width. `{:>20}` on
  `fmt::day(5)` prints a full date and time, ` 1900-01-05 00:00:00`, instead
  of the padded day, and the same specification on a month can trip an
  internal assertion. Width, fill and alignment (including a dynamic width,
  and combined with the `L` flag or an explicit conversion such as `%B`)
  should pad the value exactly as the unpadded output would be padded.
- A range whose format kind is `fmt::range_format::debug_string` is printed
  inside quotes but its characters are not escaped (a newline comes out
  raw), and a width pads only the text inside the quotes.
- `fmt::sprintf` and `fmt::printf` do not follow C for a zero value with a
  precision of zero: `%.0d`, `%.0x` or `%.0o` of 0 print `0` where C prints
  no digits (the sign, space flag and width still apply, and `#` with `o`
  still yields a single `0`). A negative precision passed through `*` is
  treated as zero, where C treats it as if the precision were omitted, so
  `%.*f` with -1 and 2.5 prints `2` instead of `2.500000`.
- Very large floating-point precisions lose digits. `{:.767f}` of 1.0
  returns one character short of what was asked for, trailing zeros are
  dropped for fixed and alternate-form general presentation, the size
  reported by `fmt::format_to_n` is wrong for such output, digits beyond
  the 767th are replaced by zeros even when the exact decimal expansion has
  more, and `long
  double` values with very large or very small exponents disagree with
  `printf("%.*Lf")` and `printf("%#.*Lg")`.
- With a compiled format string, `fmt::format_to(buf, FMT_COMPILE("<{}> {}"),
  std::variant<int>{42}, 7)` writes `<42)> 7`: part of the variant's text
  overwrites what came before it, where the runtime API writes
  `<variant(42)> 7`.

Fix these so that the library matches `printf`, `std::format` and its own
documentation in all of these cases, for every character type and code path
that shares the behaviour, not only the examples above. Do not special-case
the examples; find and fix the underlying causes. The public API does not
change.

Grading copies only your `include` and `src` trees into a clean environment
that has the pristine build files, googletest and test sources, builds them
from scratch with the configuration shown above (GCC, C++17, Debug), runs a
held-out set of tests, and then runs every existing test case as a
regression wall. A change that makes the held-out tests pass but breaks any
existing test, or that does not build together with the existing tests,
does not count. Nothing outside `include` and `src` is graded.
