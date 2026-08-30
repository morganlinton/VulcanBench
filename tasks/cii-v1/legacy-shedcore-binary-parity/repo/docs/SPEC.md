# ShedCore lending protocol (spec v1.7, last updated 2016)

> Maintenance note (2023): this document is known to have drifted from the
> production controller in places. Where they disagree, **the controller's
> behavior is the contract**; the shed desk console and the seasonal
> reconcilers that talk to ShedCore were built against the controller,
> not this file.

## Command stream

The controller reads one command per line on stdin and writes the reply
lines for each command, then a trailer at EOF. Blank lines are skipped.
Parsing is strict: commands carry exactly the documented number of
space-separated tokens (extra or missing tokens are `N <id> FMT`), and
tool and member ids are case-sensitive everywhere. Tools and members are
separate registries: a tool and a member may share an id. All arithmetic
is integer.

Reject replies are `N <id> <code>`, echoing the command's first argument
token (or `????????` when the line has none, including unrecognized
command letters and extra tokens on `S` or `M`). Rejected commands
change no state.

### `T <tool> <grade>` (register a tool)

| field | format |
|-------|--------|
| tool  | 1 to 8 alphanumeric characters, case-sensitive |
| grade | service grade, exactly 1 digit, value 1 to 9 |

Registers a tool on the shed board. Reply: `OK <count>` with the number
of tools after the command. A duplicate tool id replies `N <tool> DUP`;
a malformed id replies `N <tool> FMT`; a grade token that is not one
digit with a value of 1 to 9 replies `N <tool> GRADE`.

Validation order: `FMT` (arity, id syntax), `GRADE`, then `DUP`.

### `O <member>` (enroll a member)

Enrolls a member (1 to 8 alphanumeric characters, case-sensitive).
Reply: `OK <count>` with the number of members after the command.
Duplicates reply `N <member> DUP`.

### `L <member> <tool> <days>` (loan a tool)

Loans the tool to the member for the stated number of days (1 to 2
digits, value 1 to 30). Reply: `L <member> <tool>`. An unenrolled member
or unregistered tool replies `N <member> UNKNOWN`; a tool already out on
loan replies `N <member> OUT`; a bad days token replies
`N <member> DAYS`.

Validation order: `FMT` (arity, both id syntaxes), `DAYS`, `UNKNOWN`
(member, then tool), then `OUT`.

### `R <member> <tool>` (return a tool)

Returns the tool. Reply: `R <member> <tool>`. When the member does not
hold that tool the reply is `N <member> NOLOAN`. **Duty accrues at
return time**: the tool earns `days x grade` service duty, where `days`
is the stated length of the loan just closed. Nothing accrues while the
tool is out.

Validation order: `FMT`, `UNKNOWN` (member, then tool), then `NOLOAN`.

### `S` (service rota)

Takes no arguments. Lists every tool with positive duty, one
`S <tool>` line each, ordered by **descending duty, ties by
registration order**, closed by `SEND <count>` with the number of tools
listed. Listed tools are serviced on the spot: their duty resets to 0.
When no tool holds positive duty there is no rota to emit or close, and
the controller prints nothing.

### `M` (monthly reckoning)

Takes no arguments. Recomputes every tool's duty from the ledger since
the last reckoning (or the start of the stream): what accrued at each
return, minus what the rotas cleared in that window. The recomputed
values REPLACE the live figures, and the reply is `MOK <count>` with the
number of tools left holding positive duty. No listing is printed and
nothing is reset. Since duty accrues only at return and every rota
clears exactly what it lists, the recomputation always agrees with the
live figures: **the reckoning is an audit, not a revision**.

### Trailer

At EOF the controller writes
`X <tools> <members> <loans> <returns> <services> <reckonings> <rejected>`:
counts of accepted `T`, `O`, `L`, `R`, `S`, and `M` commands, and `N`
replies of any kind. A rota that lists nothing still counts as a
service.

## Duty invariant

At any point, a tool's duty equals the sum of `days x grade` over its
returns since it was last serviced (or reckoned). The seasonal
reconcilers depend on the rota order following that ledger arithmetic
exactly, and on the reckoning never moving a figure: a rota that
disagrees with the ledger, or a reckoning that changes a count, means
the shed books no longer balance.
