# Control 2 formatter

`control-2-formatted.cpp` is `control-1-compressed.cpp` run through clang-format,
with nothing else changed. Names stay single letters and the fee numbers stay inlined.

## Version

```
$ /opt/homebrew/opt/llvm/bin/clang-format --version
Homebrew clang-format version 23.1.2
```

## Command

Run from `docs/judging/controls-v3/cpp/`:

```
/opt/homebrew/opt/llvm/bin/clang-format --style='{BasedOnStyle: LLVM, ColumnLimit: 100}' control-1-compressed.cpp > control-2-formatted.cpp
```

`verify.sh` reruns this command and compares the output with `control-2-formatted.cpp`
byte for byte. If the pinned clang-format is not installed, it skips that one check and says so.

## Notes

- LLVM style uses two-space indents and puts `&` next to the name (`const std::string &s`).
  It also sorts includes, but control 1's includes were already sorted, so none moved.
- clang-format adds no blank lines between functions, unlike the Python formatter,
  so control 2 has none. That is the formatter's output, not an edit.
- The other controls (0 and 3 to 9) were written to be unchanged by the same command,
  so every control except 1 shares control 2's layout conventions. Control 1 is the only
  deliberately unformatted file.
- The compiler used by `verify.sh` is Apple clang++ (`/usr/bin/clang++`, Apple clang
  version 21.0.0) with `-std=c++17 -Wall -Wextra -Werror`.
