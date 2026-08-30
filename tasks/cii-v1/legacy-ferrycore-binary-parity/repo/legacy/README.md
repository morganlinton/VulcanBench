# The retired legacy controller

`./run` executes the production FerryCore controller that is being
decommissioned (stdin: commands, stdout: replies). It is provided here
**for reference during development only**: the production environment this
module ships to does not include it, and the replacement must not invoke
it.

Source for the controller was lost when the harbor authority's IT
contractor was wound down in 2018; the binary, and the behavior of the
slipway terminals and season reconcilers built against it, are all that
remain.
