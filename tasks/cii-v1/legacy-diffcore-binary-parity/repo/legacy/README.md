# The retired legacy engine

`./run` executes the production DiffCore engine that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment this
module ships to does not include it, and the replacement must not invoke
it.

Source for the engine was lost when the original vendor folded in 2019;
the binary, and the behavior of the backup coordinators built against it,
are all that remain.
