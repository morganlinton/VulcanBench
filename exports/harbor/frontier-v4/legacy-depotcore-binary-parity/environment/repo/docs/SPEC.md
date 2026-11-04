# DepotCore depot command format (spec v3.1, last updated 2015)

> Maintenance note (2020): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the routing desk and the reconciliation tooling downstream were built
> against the engine.

One depot session per process. Commands on stdin, one per line; result
lines per command; a trailer at end of input. State (registered senders,
open lanes, the arrival bay, lane shelves, the return ledge, the event
ledger) persists across the session, in command order.

Identifiers (sender, parcel, lane) are 1-8 alphanumerics,
case-sensitive. Senders, parcels, and lanes are separate namespaces. A
parcel id, once accepted, stays used for the whole session. Commands
carry exactly the tokens shown; a wrong token count is rejected `FMT`
echoing the command word.

## Commands

- `REG <sender>`: register a sender. Reply `OK <sender>`; an existing
  sender is rejected `DUP`.
- `LANE <lane>`: open a delivery lane. Lanes are listed everywhere in
  the order they were opened. Reply `OK <lane>`; duplicate `DUP`.
- `LODGE <sender> <parcel> <lane> <heft>`: the sender lodges a parcel
  for the lane; it waits in the arrival bay until the next `SHELVE`.
  `heft` is 1-3 digits, value 1-999. Checks in order: token shapes
  (`FMT`), heft (`HEFT`), sender registered (`WHO`), lane open
  (`DEST`), parcel id unused (`DUP`). Reply `OK <parcel>`.
- `SHELVE`: move every bay parcel onto its lane's shelf, in lodge
  order, one `PUT <parcel> <lane>` line each. Parcels on the return
  ledge rejoin at the back of their lane's shelf behind the day's
  arrivals, oldest bounce first, and stand in line as if newly
  arrived. Reply `OK <n>` after the PUT lines (n parcels placed).
- `ROUND <lane>`: dispatch a van round. Up to four parcels leave the
  shelf, heaviest heft first, ties oldest first (arrival order); one
  `OUT <parcel>` line each, then `RAN <lane> <k>`. An empty shelf
  replies `VOID <lane>` (the van does not depart). A departing round
  closes the previous round's return window and opens its own.
- `BOUNCE <parcel>`: report a misdelivery from the most recent
  departed round. Only parcels in the open return window can bounce,
  once each; anything else is rejected `STATE`. The parcel goes to the
  return ledge until the next `SHELVE`. Reply `OK <parcel>`.
- `WAIVE <parcel>`: the sender withdraws a parcel. Valid while the
  parcel is in the bay or on a shelf (`STATE` otherwise). A withdrawn
  parcel is gone for good; its id stays used. Reply `OK <parcel>`.
- `SQUARE`: the seasonal recount. The clerk re-derives every lane's
  listing from the event ledger and posts it: one
  `SQ <lane> [parcels...]` line per lane in lane-open order, then
  `OK <lanes>`. The posted listings match the shelves as maintained;
  the recount is a bookkeeping formality and changes nothing.
- `LIST <lane>`: inspection. `SHELF <lane> [parcels...]` in shelf
  order.
- `HELD <sender>`: inspection. `FOR <sender> [parcels...]`, the
  sender's parcels currently on shelves, in arrival order.
- `ROLL`: inspection. Two lines: `SND [senders...]` in registration
  order and `LNS [lanes...]` in open order.

## Output

- Rejected: `R <token> <code>`; codes `FMT`, `HEFT`, `WHO`, `DEST`,
  `DUP`, `STATE`, checked in the orders given above. Unknown command
  words are rejected `FMT`.
- Trailer: `X <senders> <lanes> <parcels> <rounds> <bounces> <waives>
  <squares>`: counts of accepted REG, LANE, LODGE, ROUND (VOID
  included), BOUNCE, WAIVE, SQUARE commands.
