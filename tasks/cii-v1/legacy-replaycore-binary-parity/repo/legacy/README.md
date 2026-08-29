# The retired legacy engine

`./run` executes the production ReplayCore engine that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment
this module ships to does not include it, and the replacement must not
invoke it.

Source for the engine was lost with the 2019 vendor handover; the binary
and the behavior of the downstream systems built against it are all that
remain.
