# The retired legacy controller

`./run` executes the production ShedCore controller that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment this
module ships to does not include it, and the replacement must not invoke
it.

Source for the controller was lost when the co-op's original volunteer
maintainer moved away in 2017; the binary, and the behavior of the shed
desk console and seasonal reconcilers built against it, are all that
remain.
