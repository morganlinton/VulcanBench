# Durable delivery across worker leases and journal commits

Complete the dispatcher and committer in this original multi-module Async service.
Keep supplied interfaces fixed. The Runtime, Clock, Ticket, Engine and Service
modules define its admission, cancellation and shutdown integration. The two
prototype step functions currently do no work. Preserve the typed protocol;
no unsafe casts, sleeps, external services, or changes to the public contracts.

Admission assigns sequence numbers from zero. Capacity counts every accepted
ticket not yet settled, including retries and journal waits; reject when full or
closed. At most policy.slots worker calls are active. For a given key, work is
ordered by admission and attempts never overlap. A job waiting to retry blocks
younger jobs of that key, but not other keys. Start the oldest eligible job when
a slot is available; bypass ineligible earlier jobs. Completed/cancelled jobs
waiting for the journal do not block later work of their key.

Attempts start at one. A worker Error, synchronous exception or asynchronous
monitor exception retries until policy.attempts is exhausted, then becomes
Failed error. Exception text is Printexc.to_string. A lease expires at its start
time plus policy.timeout, including equality, producing error "timeout" with
the same retry policy. A retry waits policy.retry_delay from failure, zero means
immediately eligible. Retire each attempt exactly once and cancel its alarm.
Later callbacks from cancelled/timed-out attempts cannot publish, free a slot
owned by another job, retry, or release that newer job's key. Check the deadline
even when the worker result was filled before scheduler callbacks run.

Workers finishing successfully produce Succeeded value; cancellation before
journal submission produces Cancelled, releases any worker slot, removes retry/
timeout alarms, and suppresses future callbacks. Cancellation is idempotent.
Once journal submission starts, cancellation is a no-op. The journal receives
exactly one entry per accepted job, in admission order, regardless of completion
order. At most one append is outstanding. A ticket settles as Committed outcome
only after its append returns Ok. Release capacity at settlement, not at worker
completion. If append returns Error or raises synchronously/asynchronously,
stop admission and work, retire all alarms, settle every unresolved ticket as
Journal_failed error, and never submit later entries. Previously committed
tickets remain unchanged.

close rejects new admissions and drains all accepted jobs, retries and ordered
commits. Its deferred resolves only when all tickets have settled and no journal
operation is outstanding. abort can interrupt graceful close: settle unresolved
tickets as Aborted immediately, cancel all alarms and suppress worker callbacks.
An append already in progress cannot be undone; close still waits for that one
operation to return, then no later append starts. Repeated close/abort is safe.

Worker and journal callbacks can synchronously reenter the Engine to submit,
cancel, close or abort before returning their deferred. Reserve their slots,
generations and commit ownership before invoking user code, and recheck stopped
state when reentrancy changes it. Clock.advance is monotonic and fires due alarms
in deadline/insertion order. Tests advance this clock and drain Async scheduler
callbacks explicitly; elapsed real time is not part of the contract. Public
example.ml demonstrates one durable delivery once the steps are implemented.
