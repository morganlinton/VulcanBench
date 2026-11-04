# The retired legacy engine

`./run` executes the production HedgeCore engine that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment this
module ships to does not include it, and the replacement must not invoke
it.

Source for the engine was lost when the desk's original technology
provider was wound down in 2015; the binary, and the behavior of the
marking and reconciliation systems built against it, are all that
remain.
