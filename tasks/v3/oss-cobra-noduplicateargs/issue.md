# Add a `NoDuplicateArgs` positional-args validator

Cobra ships several positional-argument validators (`NoArgs`, `ArbitraryArgs`,
`ExactArgs`, `OnlyValidArgs`, ...) that a command can assign to its `Args`
field. There is no built-in validator that rejects **duplicate** positional
arguments. Add one.

## Expected behavior

Add an exported function:

```go
func NoDuplicateArgs(cmd *Command, args []string) error
```

that returns `nil` when all positional args are unique and returns an error
when any argument appears more than once. The error message must name the
duplicated argument and the command path, in the form:

```
duplicate argument "x" for "root"
```

It follows the same `func(cmd *Command, args []string) error` shape as the
other validators, so it can be used directly as a command's `Args` value. The
existing validators must be unaffected.

## Acceptance examples

- `NoDuplicateArgs(cmd, []string{"a", "b", "a"})` returns a non-nil error
- that error's message contains `duplicate argument "a"`
- `NoDuplicateArgs(cmd, []string{"a", "b", "c"})` returns `nil`
