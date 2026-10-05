# Repair a bounded Async dispatcher through graceful close and abort

Implement the supplied Pipeline interface and the queue it uses. `concurrency`
is a positive maximum number of running workers. `capacity` is a nonnegative
maximum number of waiting jobs, excluding running jobs. Invalid limits raise
Invalid_argument. Accepted jobs start in submission order as slots become free.
A full waiting queue makes submit return a determined `Error "full"` without
invoking the worker. A closed/aborted dispatcher returns `Error "closed"`.
Each accepted submit deferred resolves exactly once with its worker result.
Worker-returned errors free the slot like successes. A worker that raises
synchronously before returning a deferred resolves that job to
`Error "worker exception"` and frees its slot. Asynchronously raised exceptions
are outside this task's contract; workers signal asynchronous errors by Result.

`close` rejects future submissions and drains all already accepted jobs. Its
returned deferred resolves only when running and waiting counts are both zero.
Repeated close calls share the same completion condition. `abort` rejects new
work, resolves every waiting job to `Error "aborted"` without starting it, and
waits for running workers to finish normally. Abort after close upgrades the drain
to this behavior. Close after abort must not restart discarded work. Neither
operation cancels an already running worker. Counts reflect current work, and
completion of an unrelated job must not resolve the wrong submit deferred.

Example at concurrency=1/capacity=1: submit A starts A, submit B waits, submit C
returns full. Closing waits for A then B; aborting instead rejects B immediately
and waits only for A. Preserve interfaces; use Async deferreds, not sleeps or
OS threads. Public tests run via `dune runtest`.

The supplied `.mli` files are fixed contracts for this implementation task.
Keep their contents unchanged; implement the change in `.ml` files.
