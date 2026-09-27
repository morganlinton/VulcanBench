# Control 2 formatting

`control-2-formatted.c` is `control-1-compressed.c` passed through clang-format
and nothing else. It was produced from this directory with:

```
/opt/homebrew/opt/llvm/bin/clang-format --style='{BasedOnStyle: LLVM, ColumnLimit: 100}' control-1-compressed.c > control-2-formatted.c
```

Formatter version (`clang-format --version`):

```
Homebrew clang-format version 23.1.2
```

No `.clang-format` file is involved: the inline `--style` is the whole
configuration, and the language (C) is taken from the `.c` extension.
`verify.sh` re-runs the command and fails if the output differs from the
checked-in control 2 (it skips that check when clang-format 23.1.2 is not at
the path above).

`control-6-narrated.c` is control 2 with a `//` comment line inserted above
each statement; removing every line that starts with `//` (after
indentation) gives control 2 byte for byte, which `verify.sh` also checks.
