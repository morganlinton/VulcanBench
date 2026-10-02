# The retired legacy engine

`./run` executes the production CodecCore engine that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment this
module ships to does not include it, and the replacement must not invoke
it.

Source for the engine was lost with the 2020 vendor handover; the binary
and the behavior of the partner systems built against it are all that
remain.
