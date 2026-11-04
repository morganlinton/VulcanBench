# The retired legacy engine

`./run` executes the production MatchCore engine being decommissioned
(stdin: session messages, stdout: responses; one session per process).
Reference only during development: production does not include it, and the
replacement must not invoke it. Source was lost with the 2020 exchange
migration; the binary is all that remains.
