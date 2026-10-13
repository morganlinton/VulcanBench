# MatchCore session protocol (spec v2.1, last updated 2018)

> Maintenance note (2023): this document is known to have drifted from the
> production engine. Where they disagree, **the engine's behavior is the
> contract**; every downstream consumer was certified against the engine.

One matching session per process. Messages on stdin, one per line,
space-separated; responses on stdout.

## Messages

- `N <acct> <clordid> <side> <type> <price> <qty>` - new order.
  `acct`: exactly 4 alphanumerics. `clordid`: 1-8 alphanumerics.
  `side`: `B` or `S`. `type`: `L` (limit), `M` (market), `I`
  (immediate-or-cancel limit). `price`: 1-7 digits (ignored for `M`,
  conventionally `0`). `qty`: 1-6 digits.
- `C <acct> <clordid>` - cancel a resting order.
- `E` - end of session; emit the summary line.

## Responses

- `A <clordid> <restingqty>` - order accepted; `restingqty` is what
  rested on the book (0 for fully filled orders and for market/IOC
  remainders, which are cancelled).
- `F <resting_clordid> <incoming_clordid> <price> <qty>` - fill, at the
  resting order's price, emitted before the incoming order's `A`.
- `X <clordid> <qty>` - order cancelled; `qty` is the remaining quantity
  at cancellation.
- `R <clordid> <code>` - rejected. Codes: `FMT` (malformed message),
  `PRICE` (price out of range: must be an integer, 0 < price < 10000000),
  `QTY` (quantity out of range: 0 < qty <= 999999), `UNKNOWN` (cancel of
  an order that is not live), `DUP` (a new order reusing the client order
  id of a live order from the same account), `STP` (an order that would
  trade against the same account's own resting order).
- `S <bid> <ask> <hash>` - session summary: best bid (0 if none), best
  ask (0 if none), and the book hash: the sum over all resting orders of
  `price * remaining_qty`, modulo 1000000007.

## Matching

Strict price-time priority: an incoming order matches the best-priced
opposite order first; among equal prices, the earliest-arrived order
matches first. Fills execute at the resting order's price. Limit
remainders rest on the book; market and IOC remainders are cancelled.
A market order sweeps the book until filled or the opposite side is
empty.
