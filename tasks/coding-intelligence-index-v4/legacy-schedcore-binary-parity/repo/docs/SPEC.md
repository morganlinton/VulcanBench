# SchedCore scheduler command format (spec v1.7, last updated 2013)

> Maintenance note (2018): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the pipeline controllers and the operator tooling downstream were built
> against the engine.

One scheduling session per process. Commands on stdin, one per line; a
result line (or block) per command; a trailer at end of input. State
(defined jobs, executed marks, dependency graph) persists across the
session, in command order.

## Commands

- `J <job> <prio> <deps>`: define a job.
  - `job`: 1-8 alphanumerics, case-sensitive. Defining a job id twice is
    rejected `DUP`.
  - `prio`: 1-2 digits, value 1-99. Higher priorities execute first;
    ready jobs of equal priority execute in definition order (FIFO).
  - `deps`: a comma-joined list of job ids this job depends on, or `0`
    for none. Entries must be well-formed job ids, must be unique, and a
    job may not depend on itself; violations are rejected `DEPS`.
    Dependencies may reference jobs that are not yet defined; they are
    resolved lazily at each `G`.
  - Reply: `OK <defined>` (total defined jobs after this one).
- `G`: run the scheduler. Repeatedly executes the highest-priority READY
  job (a job is ready when every dependency has executed, in this `G` or
  a previous one) and emits `E <job>` for each execution, then
  `GEND <executed> <blocked>` where `executed` counts this `G`'s `E`
  lines and `blocked` counts defined jobs that are still not executed
  (cyclic or unresolved dependencies). Executed state persists: a later
  `G` only runs jobs that are not yet executed. A dependency that was
  missing at one `G` and defined afterwards is re-evaluated at the next
  `G` like any other.
- `F <job>`: mark an EXECUTED job as failed. The job, and every executed
  job that depends on it directly or transitively, become un-executed
  and are rescheduled by the next `G`. Invalidated jobs reschedule
  exactly as first scheduled: same priority, same definition-order
  tie-breaking. Failing a job that is not currently executed is rejected
  `STATE`. Reply: `OK <invalidated>` (jobs un-executed, including the
  failed job itself).

## Output

- Rejected: `R <job> <code>`; codes `FMT`, `PRIO`, `DEPS`, `DUP` for
  `J`, checked in that order, and `FMT`, `STATE` for `F`. Commands carry
  exactly the tokens shown above; anything else is `FMT`.
- Trailer: `X <defined> <executed_total> <failed> <rejected>`. Defined
  counts accepted `J` commands; executed_total counts every `E` line
  emitted in the session (reruns included); failed counts accepted `F`
  commands; rejected counts `R` lines.
