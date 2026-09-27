# How control 2 was produced

Control 2 (`control-2-formatted.mjs`) is control 1 (`control-1-compressed.mjs`)
run through the pinned Prettier and nothing else. No character of it was
edited by hand.

## Formatter

- Prettier **3.9.9** (`prettier --version` prints `3.9.9`), installed at
  `/Users/morganlinton/.local/vulcanbench-prettier-3.9.9/node_modules/.bin/prettier`
- Node v22.11.0
- Options: `--print-width 100`; every other option is Prettier's default
  (two-space indent, semicolons, double quotes, trailing commas `all`, arrow
  parentheses `always`).
- No configuration file applied: `prettier --find-config-path
  control-1-compressed.mjs` finds none, and adding `--no-config
  --no-editorconfig` gives byte-identical output.

## Exact command

Run from `docs/judging/controls-v3/javascript/`:

```sh
/Users/morganlinton/.local/vulcanbench-prettier-3.9.9/node_modules/.bin/prettier --print-width 100 control-1-compressed.mjs > control-2-formatted.mjs
```

## Result

| File | SHA-256 |
| --- | --- |
| `control-1-compressed.mjs` | `1001d2c318019dfb079a45cdfd13cf276438fdf5e191867321dd7468c1157c12` |
| `control-2-formatted.mjs` | `4e8a4ff62431a52d145b746d2f5c668897e0b8f8799972f57ff63a2f897b1429` |

`verify.sh` reruns the formatter (with `--no-config --no-editorconfig`, so a
configuration file added later cannot change the result) and fails unless the
output matches `control-2-formatted.mjs` byte for byte and the version is
3.9.9.

Prettier does not insert blank lines between functions, so control 2 has none
(control 1 had none). The Python control 2 has them because Black adds them;
that is a difference between the two formatters, not an edit.

## The other controls

Every control except control 1 passes `prettier --print-width 100 --check`
with the same binary, so layout is constant across the set and control 1 is
the only unformatted file. Controls 0, 3, 4, 5, 7, 8 and 9 were written by
hand to that style (Prettier was used to settle line breaks in 0 and 4);
control 6 is control 2 with a comment line inserted above every code line, and
`verify.sh` checks that removing those comment lines gives control 2 exactly.
