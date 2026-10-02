# The retired legacy engine

`./run` executes the production GranaryCore engine that is being
decommissioned (stdin: commands, stdout: responses). It is provided
here **for reference during development only**: the production
environment this module ships to does not include it, and the
replacement must not invoke it.

Source for the engine was lost when the co-op federation's shared
minicomputer was decommissioned in 2011; the binary and the behavior of
the ledger tooling built against it are all that remain.
