# LockCore lease-manager command format (spec v2.2, last updated 2012)

> Maintenance note (2017): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the coordination services and the operator tooling downstream were
> built against the engine.

One lease session per process. Commands on stdin, one per line; a result
line (or block) per command; a trailer at end of input. State (held
leases, wait queues, the logical clock) persists across the session, in
command order.

## The logical clock

Time is logical: the session starts at tick 0 and every ACCEPTED command
advances the clock by exactly 1 tick after it executes. Rejected
commands do not advance the clock. Nothing about the clock or lease
expiry times is ever printed; expiry is observable only through sweeps.

## Commands

- `A <client> <res> <ttl>`: acquire a lease.
  - `client`, `res`: 1-8 alphanumerics, case-sensitive.
  - `ttl`: 1-3 digits, value 1-999. The granted lease expires at
    `now + ttl`.
  - If the resource is free, the lease is granted: reply
    `G <res> <client>`. Otherwise the client joins the resource's wait
    queue (strict FIFO): reply `Q <depth>` with the queue depth after
    joining. A client that already holds or is already waiting on the
    resource is rejected `STATE`.
- `R <client> <res>`: renew a HELD lease. The lease's expiry is restored
  to `now + ttl_original`, where `ttl_original` is the ttl from the
  grant; renewing never shortens or degrades a lease, no matter how
  often it is repeated. Renewing a resource the client does not
  currently hold is rejected `STATE`; a lease stays renewable until a
  sweep actually releases it. Reply: `OK <holds>` (the client's total
  held leases).
- `E`: expiry sweep. Releases every lease whose `expiry <= now`, then
  grants each freed resource to the head of its wait queue (FIFO),
  emitting `G <res> <client>` per new grant (resources in creation
  order), followed by `EEND <released> <granted>`.
- `L <client>`: release all of the client's held leases, then grant
  freed resources to their wait queues exactly as a sweep does: `G`
  lines, then `LEND <released>`. A client holding nothing is rejected
  `STATE`.

## Output

- Rejected: `N <client> <code>`; codes `FMT`, `TTL`, `STATE`, checked
  in that order. Commands carry exactly the tokens shown above; anything
  else is `FMT` (echoing `????????` when the client token itself is
  unusable).
- Trailer: `X <acquires> <renews> <sweeps> <rejected>`. Acquires counts
  accepted `A` commands (grants and queue joins); renews counts accepted
  `R` commands; sweeps counts `E` commands; rejected counts `N` lines.
