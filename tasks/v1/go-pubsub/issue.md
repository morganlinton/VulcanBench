# Implement an in-memory pub/sub broker with wildcard topics

The `pubsub` package routes published messages to subscribers by topic. The work
is split across two files and both must be implemented:

- `pubsub/match.go` — `Match(pattern, topic)` decides whether a subscription
  pattern matches a concrete topic.
- `pubsub/broker.go` — `Broker` manages subscriptions and delivery.

## Topic matching (`Match`)

Topics are dot-separated segments, e.g. `sensor.temp.kitchen`. In a **pattern**:

- a literal segment matches that exact segment;
- `+` matches exactly one segment;
- `#` matches the remaining zero-or-more segments and is only meaningful as the
  final pattern segment.

So `a.+.c` matches `a.b.c` but not `a.c` or `a.b.x.c`; `a.#` matches `a`, `a.b`,
and `a.b.c` but not `b.c`; `#` matches any topic. A pattern with no wildcards
matches only the identical topic (segment counts must line up).

## Broker (`Broker`)

- `New()` returns an empty broker.
- `Subscribe(pattern, fn)` registers `fn` to receive messages whose topic matches
  `pattern`, and returns a function that cancels that subscription.
- `Publish(topic, payload)` invokes `fn(topic, payload)` for every subscription
  whose pattern matches `topic`, in **subscription order** (earliest first).
  Publishing a topic with no matching subscribers is a no-op.

## Requirements

- Delivery goes only to subscriptions whose pattern matches, in subscription
  order, and the cancel function removes a subscription so it stops receiving.
- The broker must be **safe for concurrent use**: subscribing, unsubscribing, and
  publishing may happen from multiple goroutines at once (the tests run under
  `-race`).
