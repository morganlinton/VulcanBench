# FerryCore crossing log command format (spec v3.1, last updated 2011)

> Maintenance note (2016): this document has drifted from the
> production engine. Where they disagree, **the engine's behavior is
> the contract**; the slip boards, the toll office and the night
> reconciliation tooling downstream were all built against the engine.

One crossing day per process. Commands on stdin, one per line; result
lines per command; a trailer at end of input. State (the registered
cars, the slips, who waits where, where the ferry lies) persists
across the session, in command order. Blank lines are ignored. Tokens
are separated by spaces or tabs.

## Commands

- `K <car> <axles>`: a car is registered for the day.
  - `car`: 1-8 ASCII letters or digits, case-sensitive. A car already
    registered is rejected `TWICE`. Cars are never deregistered.
  - `axles`: 1-2 digits, value 1-9, the car's axle load.
  - Reply: `OK <cars>` (registered-car count after this one).
- `P <slip> <room>`: a slip (landing) is opened.
  - `slip`: 1-8 ASCII letters or digits, case-sensitive; a separate
    namespace from car names. A slip already open is rejected `TWICE`.
  - `room`: 1-2 digits, value 1-20, the most cars its line holds.
  - Reply: `OK <slips>` (slip count after this one). The ferry starts
    the day lying at the first slip opened. The ferry's route visits
    the slips in the order they were opened, wrapping around.
- `Q <car> <slip>`: the car joins the slip's line, at the back. A car
  already waiting in any line is rejected `TWICE`; a line holding
  `room` cars is rejected `CRAM`. Reply: `OK`.
- `X <car>`: the car leaves its line and drives off. A car not waiting
  in a line is rejected `IDLE`. Reply: `OK`.
- `M <car> <slip>`: the car moves from its line to another slip's line
  without leaving the queue system; it joins at the back. A car not
  waiting is rejected `IDLE`; a move to the line it is already in is
  rejected `TWICE`; a full destination line is rejected `CRAM`.
  Reply: `OK`.
- `W <slip>`: the slip's line. Reply: `LINE <slip> <k>` with `k` the
  number of cars waiting, then one `C <car>` line per car, front of the
  line first (the line is first come, first served).
- `S`: a sailing. The ferry loads at the slip where it lies: walking
  the line from the front, each car whose axle load still fits the
  deck boards, and a car that does not fit is passed over (later,
  lighter cars may still board). The deck carries 12 axles. Reply:
  `SAIL <slip> <k>` with `k` the number boarded, then one `B <car>`
  line per car in boarding order. The ferry then crosses to the next
  slip on its route and lands everyone aboard there; landed cars are
  no longer waiting anywhere. Final line: `DOCK <slip>`, the slip the
  ferry now lies at. With no slip open, `S` is rejected `WILD`.
- `T`: the toll audit. Lists every car that has crossed at least once
  today, most crossings first (ties by registration order). Reply:
  `TOLL <k>` then one `C <car>` line per car.
- `N`: the night count. Lists every registered car, waiting or not,
  heaviest axle load first (ties by registration order). Reply:
  `NIGHT <n>` then one `C <car>` line per car.

## Rejects

`NAY <code>`, one line, nothing else. Codes, checked in this order:

- `FORM`: wrong token count, malformed name or number token, or an
  unknown command word.
- `WIDE`: a well-formed number outside its documented range.
- `STRAY`: unknown car.
- `WILD`: unknown slip (or a sailing with no slip open).
- `IDLE`: the car is not waiting in a line (`X`, `M`).
- `TWICE`: duplicate registration or slip, a car joining while it
  already waits, or a move to the line it is in.
- `CRAM`: the destination line is full.

Commands carry exactly the tokens shown above; anything else is
`FORM`.

## Trailer

`END <registrations> <slips> <joins> <leaves> <moves> <sailings>
<audits> <nights>`: counts of accepted commands only; rejected commands
count nowhere. Line views (`W`) are not counted.
