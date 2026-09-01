# LodgeCore bunk-ledger command format (spec v1.7, last updated 2015)

> Maintenance note (2020): this document has drifted from the production
> engine. Where they disagree, **the engine's behavior is the contract**;
> the wardens' terminals and the seasonal reconciliation tooling
> downstream were built against the engine.

One ledger session per process. Commands on stdin, one per line; one or
more result lines per command; a trailer at end of input. State (the
party register, the rooms, the airing rota, every booking) persists
across the session, in command order.

## Trail standing

Every party carries a trail standing that decides berth order. It is
never printed. Standing is computed from the party's record:

    standing = 10 x size
             + 5 for each completed stay (a G or L departure)
             - 15 for each early departure (an E departure)

Relocations by the airing rota and the seasonal settling do not change
standing. Ties in standing always resolve by registration order,
earliest party first.

## Commands

- `P <party> <size>`: register a walking party.
  - `party`: 1-8 alphanumerics, case-sensitive, never reusable.
  - `size`: one digit, value 1-8 (walkers in the party).
  - Reply: `OK <parties>` (registered parties so far).
- `O <room> <bunks>`: open a bunkroom.
  - `room`: 1-8 alphanumerics, case-sensitive. `bunks`: 1-2 digits,
    value 1-24.
  - Reply: `OK <rooms>` (rooms opened so far).
- `B <party> <room>`: book a registered or departed party into a room.
  A party that is already booked or lodged is rejected `STATE`. A room
  that is resting under the airing rota is rejected `STATE`.
  - Reply: `OK <pending>` (parties currently booked for that room and
    not yet arrived).
- `A <party>`: the party arrives and berths in its booked room. The
  room must be open and have at least `size` free bunks; otherwise
  `STATE`. The party takes `size` bunks and enters the room's berth
  order by standing (see Berth order).
  - Reply: `IN <room>`.
- `G <party>`: the party departs at the end of its stay (a completed
  stay). `E <party>`: the party departs early. `L <party>`: the party
  departs late (counts as a completed stay). All three require a lodged
  party and free the party's bunks.
  - Reply: `OUT <room>`.
- `W`: one step of the airing rota (see The airing rota).
  - Reply: `AIRED <room> <n>`, then one `MV` line per relocated
    occupant, in relocation order: `MV <party> <room2>`, or
    `MV <party> -` for a party that fit nowhere and departed.
- `S`: the seasonal settling (see The settling).
  - Reply: `SETTLED <placed>`, then one `RM` line per room in opening
    order.
- `Q <room>`: inspect a room. Reply: `RM <room>` followed by the
  occupant party ids in berth order.
- `V`: view the rota. Reply: `ROTA` followed by every room, starting at
  the room the rota will rest next and continuing in opening order,
  wrapping around.

## Berth order

A room lists its occupants in berth order: standing descending, ties by
registration order (earliest first). A party entering a room (arrival,
relocation, or settling) is inserted at the position its standing
earns; parties already berthed do not move.

## The airing rota

Rooms are aired in opening order, one per `W`, tracked by a rota
pointer. A `W` step:

1. The room currently resting (if any) reopens.
2. The pointer's room is rested: it stops taking bookings and
   arrivals, and each occupant, in berth order, is relocated to the
   open room with the most free bunks (ties: earliest opened), keeping
   its standing. An occupant that fits in no open room departs the
   lodge (counted as a departure).
3. The pointer advances to the next room in opening order, wrapping
   around.

`W` with no rooms opened is rejected `STATE`. A rested room reopens on
the next `W` step. Bookings made before a room was rested stay valid;
the party simply arrives after the room reopens.

## The settling

`S` relists every lodged party. All rooms are cleared, then rooms are
filled in opening order (resting rooms sit out): parties are taken in
standing order (descending, ties by registration order) and each party
that fits the current room is placed there, in order, until no more
fit; then the next room fills. Parties left over after every open room
is considered depart the lodge (counted as departures). The rota
pointer is unaffected.

## Output

- Rejected: `NO <echo> <code>`; codes `FMT`, `VAL`, `STATE`, checked in
  that order. `echo` is the command's first argument token when it is
  1-8 alphanumerics, else `?`. Commands carry exactly the tokens shown
  above; anything else is `FMT`. Malformed ids and numbers are `FMT`;
  out-of-range values are `VAL`.
- Trailer: `X <parties> <rooms> <bookings> <arrivals> <departures>
  <airings> <settlings>`. Parties and rooms count accepted `P` and `O`
  commands; bookings count accepted `B` commands; arrivals count
  accepted `A` commands; departures count accepted `G`, `E`, `L`
  commands plus parties turned out by the rota or the settling;
  airings and settlings count accepted `W` and `S` commands.
