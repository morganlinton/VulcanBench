# Add Command.ArgValidator for tree-wide argument validation

There is no hook to validate a command's positional arguments after parsing
and before the action runs. Today every action starts with the same
boilerplate, and shared validation cannot be defined once for a whole
command tree.

Wanted:

```go
cmd := &cli.Command{
    Name: "app",
    ArgValidator: func(ctx context.Context, c *cli.Command) error {
        if c.Args().Len() != 2 {
            return errors.New("need exactly two args")
        }
        return nil
    },
    Action: run,
}
```

- An `ArgValidator` field on `Command` (with a named function type), called
  after argument parsing and **before** the action; parsed arguments are
  visible to it via the command.
- A non-nil error fails the run and the action does not execute (errors are
  handled like other command errors, including exit coders).
- Validation is **tree-wide**: a subcommand without its own validator
  inherits the nearest ancestor's; a subcommand with its own validator
  overrides the inherited one (nearest wins, only one runs).
- Commands without any validator in their chain behave exactly as today:
  actions run, see their args, and their errors propagate.
