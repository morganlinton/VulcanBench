# Correct and bound allocation in an event-time rolling window

Implement Window with integer timestamps and a width from 1 to 4096 inclusive;
other widths raise Invalid_argument. The watermark is the greatest accepted
timestamp, or None before the first event. At watermark w the retained window is
[max(0,w-width+1), w], inclusive. Accept out-of-order events within this window,
including duplicate timestamps, and expire all events outside it when watermark
advances. `count` counts events, not buckets; `total` sums their values, including
negative values. Negative timestamps return `Error "negative time"`. A timestamp
older than the current retained window returns `Error "late event"`. Rejected
events leave all state, including instrumentation, unchanged. Integer sums and
counters in the tested workloads fit an OCaml int.

Example width=3: add (10,5), (8,2), (9,-1) gives count=3,total=6. Adding
(11,4) expires timestamp 8, so count=3,total=8. A subsequent timestamp 8 is late.
Large time jumps expire the whole old window without looping across the gap.

This is a hot-path change: total and count must be O(1), state O(width), and
accepted steady-state insertion must use less than 32 allocated bytes/event,
measured in a native build after construction/warm-up. Use preallocated bounded
storage; do not retain an event history. `bucket_visits` counts every bucket
read/write operation group used to insert or expire data: inserting into one
bucket counts one visit, clearing one counts one. Increment it honestly even for
empty buckets. Initialization may visit width buckets. Across N events advancing
one timestamp at a time, visits must be <= 3*N+2*width; one arbitrarily large jump
must add <= width+1 visits. No sleeps, wall-clock performance gates, unsafe casts,
or special cases for test inputs. Preserve the supplied interface.

The supplied `.mli` files are fixed contracts for this implementation task.
Keep their contents unchanged; implement the change in `.ml` files.
