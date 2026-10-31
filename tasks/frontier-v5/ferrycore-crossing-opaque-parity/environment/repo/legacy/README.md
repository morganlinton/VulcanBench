# The retired legacy engine

`./run` executes the production FerryCore engine that is being
decommissioned (stdin: commands, stdout: responses). It is provided
here **for reference during development only**: the production
environment this module ships to does not include it, and the
replacement must not invoke it.

Source for the engine was lost when the ferry company's crossing
server was retired in 2016; the linux/amd64 binary and the behavior of
the tooling built against it are all that remain.
