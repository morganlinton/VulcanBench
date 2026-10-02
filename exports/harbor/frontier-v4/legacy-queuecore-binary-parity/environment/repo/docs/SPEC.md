# QueueCore work-queue command format (spec v2.4, last updated 2014)

> Maintenance note (2019): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the dispatchers and the operator tooling downstream were built against
> the engine.

One queue session per process. Commands on stdin, one per line; a result
line per command; a trailer at end of input. State (queue contents, fail
counts, the dead-letter area) persists across the session, in command
order.

## Commands

- `N <item> <prio>`: enqueue a work item.
  - `item`: 1-8 alphanumerics, case-sensitive. An item already waiting
    in the queue or sitting in the dead-letter area is rejected `STATE`.
  - `prio`: 1-3 digits, value 1-999. Higher priorities dequeue first;
    items of equal priority dequeue in arrival order (FIFO).
  - Reply: `OK <depth>` (queue depth after the enqueue).
- `D`: dequeue the highest-priority item. Reply `I <item>`, or `EMPTY`
  when the queue is empty.
- `F <item>`: report the most recently dequeued item as failed. Only the
  most recent dequeue can be failed, and only once; anything else is
  rejected `STATE`. The failed item goes back into the queue at its
  ORIGINAL priority, rejoining the back of its priority class. An item
  that reaches three failures goes to the dead-letter area instead.
  Reply: `OK <depth>` after a requeue, `DLQ <item>` on dead-lettering.
- `K`: drain the dead-letter area. Every dead-letter item, oldest first,
  is re-enqueued at its original priority with its fail count reset.
  Reply: `OK <depth>`.

## Output

- Rejected: `R <item> <code>`; codes `FMT`, `PRIO`, `STATE`, checked in
  that order. Commands carry exactly the tokens shown above; anything
  else is `FMT`.
- Trailer: `X <enqueued> <dequeued> <failed> <deadlettered>`. Enqueues
  count accepted `N` commands only; requeues and drains do not count.
  Failures count accepted `F` commands; dead-letterings count items
  entering the dead-letter area.
