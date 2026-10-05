# Implement a generation-safe Async read-through cache

Implement Cache and Alarm while preserving every supplied .mli, Policy, Request,
and Client behavior. This original fixture models a service that coalesces reads
but must survive invalidation, cancellation, retries, and shutdown. Use the
supplied Async_kernel.Time_source for all time. No network or wall-clock sleeps
are needed. Unsafe casts and weakening supplied interfaces are forbidden.

## Requests and generations

get returns an independent Request. Unless a fresh cached value exists, gets
for the same key join its current generation and share its worker attempts.
Different keys are independent. A generation's first attempt starts inside the
first get, numbered 1. Register the request and current generation before calling
the worker, because the worker may synchronously reenter get, invalidate, cancel,
or close. No internal callback may observe half-initialized state.

invalidate removes the key's cached value and detaches its current generation.
That generation continues serving its existing requests, with the same retry
policy and deadline, but later gets start a new generation. A detached generation
may never publish a cache value, remove its replacement, or satisfy replacement
requests, even if it completes after the replacement. Repeated invalidations
can leave several detached generations for one key.

Request.cancel is per caller and idempotent. Other subscribers continue. If the
last pending subscriber cancels, retire that generation immediately and abort
its timers. Later worker completions or errors must be ignored. A subsequent
get may start a new generation. Cancellation after a result does nothing.

## Time, retries, and cached values

The overall deadline is generation creation time plus policy.timeout, and never
resets on a retry or when a subscriber joins. At source.now >= deadline, pending
requests resolve Timed_out and no further worker starts or result is accepted.
This applies even before the scheduler has delivered an already-due alarm.
Callbacks should decide eligibility from the supplied source's current time.

A worker returns Ok value or Error text. Errors retry after policy.retry_delay
from the processing time of the error, until policy.max_attempts is reached.
The terminal Failed outcome uses the last error text. Synchronous exceptions
and exceptions sent to the worker's Async monitor count as Error "worker exception".
Catch both without terminating the scheduler. Each attempt must have its own
monitor; exceptions raised after that attempt has finished cannot corrupt any
generation. A worker may remain undetermined forever; the overall deadline still
applies. Retries scheduled at or beyond the deadline must never invoke the worker.

Successful current generations publish a cached value with expiration equal to
success processing time plus ttl. Cached gets resolve immediately to Value and
start no worker or timer. Freshness is now < expiration, so exact expiration is
stale, and ttl zero disables reuse. Expiration is lazy: no TTL alarm is needed.
An error, cancellation, timeout, or detached result never populates the cache.
Use representable times; validation uses spans and times far from overflow.

## Closure and resources

close is idempotent and immediate. Resolve every pending request in every
current or detached generation to Closed, clear cached values, and abort all
owned alarms. Future gets resolve Closed without invoking a worker. In-flight
worker deferreds cannot be forcibly cancelled; their later callbacks have no
effect. invalidate after closure is a no-op.

in_flight counts live generations, including detached ones, rather than keys or
worker calls. cached counts only fresh cached entries, lazily removing expired
ones. pending_alarms counts scheduled, not-yet-fired or aborted owned alarms.
These accessors must report actual state. Do not leave obsolete deadline/retry
alarms registered on the supplied Time_source after cancellation, success,
timeout, or closure. Alarm.cancel is idempotent; a cancelled callback never runs.
Alarm.pending decrements before a fired callback executes, supporting reentrancy.

All outcomes are first-result-wins. Deliver worker results through Async
callbacks; callers can advance the scheduler explicitly. The first worker call
itself happens synchronously in get. Preserve coalescing, request independence,
generation identity, and timer ownership under all combinations of these
operations. You may choose any internal representation satisfying the contract.
