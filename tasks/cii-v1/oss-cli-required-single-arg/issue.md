# No way to mark a single-value argument as required

There is no way to require a single-value argument (`{Type}Arg`).
`ArgumentBase.Parse` falls back to the default value when no argument is
given, so a command like `app delete` runs with an empty ID instead of
failing.

The closest workaround is the plural type with `Min`/`Max`:

```go
var ids []string

&cli.StringArgs{Name: "id", Min: 1, Max: 1, Destination: &ids}
```

but that changes the destination to a slice and the help text to the plural
rendering, just to say "exactly one, and it must be present".

Wanted: a `Required` field on single-value arguments, mirroring what flags
already have.

- Running the command without a required argument fails with an error; with
  the argument present it runs normally and the destination receives the
  value. An explicitly empty argument (`app ""`) counts as present.
- A default `Value` does not satisfy `Required`, same as with flags.
- Help/usage output: a required single argument renders as its bare name,
  and an optional one renders bracketed (`[name]`) — matching what the
  plural `{Type}Args` already do.
- Several required arguments can be chained; each missing one fails the run.
- Existing behavior for optional single arguments, plural argument `Min`
  enforcement, and required flags must not change.
