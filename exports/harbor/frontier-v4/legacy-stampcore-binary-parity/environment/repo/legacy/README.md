# The retired legacy controller

`./run` executes the production StampCore controller that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment this
module ships to does not include it, and the replacement must not invoke
it.

Source for the controller was lost when the original vendor folded in
2019; the binary, and the behavior of the mailroom terminals and postage
reconcilers built against it, are all that remain.
