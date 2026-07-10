# `UintSlice` flags reject hex / octal / binary input

`pflag`'s integer flags accept Go-style base prefixes (`0x`, `0o`, `0b`), but
the `UintSlice` flag does not: it parses each element in base 10 only, so a
value like `0xff` is rejected as invalid syntax.

```go
fs := pflag.NewFlagSet("t", pflag.ContinueOnError)
fs.UintSlice("nums", nil, "")

// Expected: nums == [255]. Before the fix: parse error "invalid syntax".
_ = fs.Parse([]string{"--nums=0xff"})
```

## Expected behavior

`UintSlice` should parse each comma-separated element with automatic base
detection so that `0x` (hex), `0o` (octal), and `0b` (binary) prefixes are
accepted, matching the other integer flag types. Plain decimal input must
continue to work exactly as before. This applies to `Set`, the value's string
round-trip, and the `uintSliceConv` conversion helper.

## Acceptance examples

- `--nums=0xff` parses to `[]uint{255}`
- `--nums=0x10,0o17,0b101` parses to `[]uint{16, 15, 5}`
- `--nums=1,2,3` parses to `[]uint{1, 2, 3}` (decimal unchanged)
