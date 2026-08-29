# The retired legacy engine

`./run` executes the production PackCore engine that is being
decommissioned (stdin: commands, stdout: responses). It is provided
here **for reference during development only**: the production
environment this module ships to does not include it, and the
replacement must not invoke it.

Source for the engine was lost with the 2015 platform migration; the
binary and the behavior of the systems built against it are all that
remain.
