# PaddockCore session command format (spec v2.4, last updated 2008)

> Maintenance note (2013): this document has drifted from the
> production engine. Where they disagree, **the engine's behavior is
> the contract**; the co-op's rota boards and reconciliation tooling
> downstream were built against the engine.

One yard session per process. Commands on stdin, one per line; result
lines per command; a trailer at end of input. State (the string of
ponies, the staked fields, who stands where) persists across the
session, in command order. Blank lines are ignored.

## Commands

- `N <pony> <build>`: a pony is enrolled into the string.
  - `pony`: 1-8 alphanumerics, case-sensitive. A name already on the
    string is rejected `TWICE`. Ponies never leave the string.
  - `build`: 1-2 digits, value 1-99, the pony's build rating.
  - Reply: `OK <ponies>` (string count after the enrollment).
- `F <field> <span>`: a field is staked.
  - `field`: 1-8 alphanumerics, case-sensitive; a separate namespace
    from pony names. A name already staked is rejected `TWICE`. Fields
    are never unstaked; they only rest and reopen.
  - `span`: 1-2 digits, value 1-99, the most ponies the field takes.
  - Reply: `OK <fields>` (staked-field count after the stake).
- `G <pony> <field>`: the pony is turned out into the field. A resting
  field is rejected `SHUT`; a pony already standing out is rejected
  `TWICE`; a field at its span is rejected `CRAM`. Reply: `OK`. The
  pony takes the last slot of the field's roster.
- `H <pony>`: the pony is brought in to the barn from wherever it
  stands. A pony already in the barn is rejected `IDLE`. Reply: `OK`.
- `L <pony> <field>`: the pony is led over to another field without
  coming in. Same-field leads are rejected `TWICE`; a pony in the barn
  is rejected `IDLE`; resting and full fields as for `G`. Reply: `OK`.
  The pony takes the last slot of the destination roster.
- `V`: the turnout listing. Lists every pony currently standing out,
  highest build first (ties by enrollment order). Reply: `OUT <k>`
  with `k` the number out, then one `P <pony>` line per pony in listed
  order.
- `R <field>`: a field's roster. Reply: `FLD <field> <k>` with `k` the
  occupant count, then one `P <pony>` line per roster slot, in slot
  order. A resting field may be inspected; its roster is empty.
- `S`: the shift rota step, in four movements:
  1. Every resting field that rested two or more shifts ago reopens:
     one `WAKE <field>` line each, in stake order.
  2. The next open field in stake-order rotation rests (the rotation
     picks the lowest-staked open field after the last one rested,
     wrapping around): one `REST <field>` line. With no open field
     there is no rest.
  3. The resting field's occupants are moved out in roster order, each
     to the open field with the most free room at that moment (ties by
     stake order): one `PUT <pony> <field>` line each. A pony no open
     field can take returns to the barn: `BARN <pony>`.
  4. The open fields are listed: `SHIFT <k>` with `k` the open-field
     count, then one `F <field>` line each, in stake order.
- `U`: the seasonal muster. Lists the whole string, in or out, highest
  build first (ties by enrollment order). Reply: `MUSTER <n>` then one
  `P <pony>` line per pony.

## Rejects

`NAY <code>`, one line, nothing else. Codes, checked in this order:

- `FORM`: wrong token count, malformed name or number token, or an
  unknown command word.
- `WIDE`: a well-formed number outside its documented range.
- `STRAY`: unknown pony.
- `WILD`: unknown field.
- `SHUT`: the field is resting.
- `TWICE`: duplicate enrollment or stake, a turnout of a pony already
  out, or a same-field lead.
- `IDLE`: the pony is in the barn.
- `CRAM`: the field is at its span.

Commands carry exactly the tokens shown above; anything else is
`FORM`.

## Trailer

`END <enrollments> <stakes> <turnouts> <bringins> <leads> <shifts>
<musters>`: counts of accepted commands only; rejected commands count
nowhere. Listings and roster inspections are not booked.
