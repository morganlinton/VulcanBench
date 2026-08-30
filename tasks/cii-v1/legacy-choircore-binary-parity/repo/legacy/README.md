# The retired legacy engine

`./run` executes the production ChoirCore engine that is being
decommissioned (stdin: commands, stdout: replies; one season per
process, state accumulates within a session). It is provided here
**for reference during development only**: the production environment
this module ships to does not include it, and the replacement must not
invoke it.

Source for the engine was lost when the parish IT contractor wound down
in 2018; the binary, and the behavior of the rota boards and seating
printers built against it, are all that remain.
